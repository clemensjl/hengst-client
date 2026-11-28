use crate::args::{build_args, Account, AccountKind, LaunchContext};
use crate::assets::{install_assets, AssetsLayout, RESOURCES_URL};
use crate::download::{fill_missing_sha1, http_client, sha1_file, Download, Downloader};
use crate::game::Redactor;
use crate::java::{component_for, ensure_runtime, RUNTIME_INDEX_URL};
use crate::libraries;
use crate::loaders::forge;
use crate::loaders::forge::processors::{ProcessRunner, SystemRunner};
use crate::loaders::merge::merge_version;
use crate::loaders::{
    prepare_loader, resolve_loader, LoaderEndpoints, LoaderRequest, ResolvedLoader,
};
use crate::manifest::{fetch_manifest, MANIFEST_URL};
use crate::paths::{validate_id, Paths};
use crate::progress::{self, ProgressSink, Transfer};
use crate::rules::Env;
use crate::version::fetch_version;
use crate::{Error, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Remote endpoints; overridable for tests and mirrors.
#[derive(Clone, Debug)]
pub struct Endpoints {
    pub manifest: String,
    pub resources: String,
    pub runtime_index: String,
    pub loaders: LoaderEndpoints,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            manifest: MANIFEST_URL.into(),
            resources: RESOURCES_URL.into(),
            runtime_index: RUNTIME_INDEX_URL.into(),
            loaders: LoaderEndpoints::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LaunchOptions {
    pub version: String,
    /// Instance folder name; defaults to the version id, or `<version>-<loader>` with a loader.
    pub instance: Option<String>,
    pub account: Account,
    /// Use this java executable instead of a managed runtime.
    pub java: Option<PathBuf>,
    pub memory_mb: u32,
    pub loader: Option<LoaderRequest>,
}

#[derive(Clone)]
pub struct Prepared {
    pub java: PathBuf,
    pub args: Vec<String>,
    pub game_dir: PathBuf,
    pub version_id: String,
    pub loader: Option<ResolvedLoader>,
    /// The loader build was picked from the saved version list (its service was unreachable).
    pub loader_list_from_cache: bool,
    pub(crate) account: Account,
    /// The instance folder name, validated: names the game's launcher log files.
    pub(crate) instance: String,
    /// `<root>/logs/game` of the data folder the game was prepared in. Private, so nothing outside
    /// the core decides where game logs are written (or deleted).
    pub(crate) game_log_dir: PathBuf,
    /// The instance's run lock (`runlock`), which the game holds while it runs.
    pub(crate) run_lock: PathBuf,
}

impl Prepared {
    /// Arguments safe to print: the secrets `Redactor` knows (a Microsoft access token, also
    /// URL- or JSON-encoded) are replaced everywhere.
    pub fn redacted_args(&self) -> Vec<String> {
        let r = Redactor::for_prepared(self);
        self.args.iter().map(|a| r.redact(a)).collect()
    }
}

impl std::fmt::Debug for Prepared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Prepared")
            .field("java", &self.java)
            .field("args", &self.redacted_args())
            .field("game_dir", &self.game_dir)
            .field("version_id", &self.version_id)
            .field("loader", &self.loader)
            .field("loader_list_from_cache", &self.loader_list_from_cache)
            .field("account", &self.account)
            .field("instance", &self.instance)
            .finish()
    }
}

/// Downloads everything a version needs and builds the command line. For Forge and NeoForge this
/// also runs the installer's processors when the installation is not complete.
pub async fn prepare(paths: &Paths, opts: &LaunchOptions, ep: &Endpoints) -> Result<Prepared> {
    prepare_with_runner(paths, opts, ep, Arc::new(SystemRunner)).await
}

/// `prepare` with the process runner the installer processors are started through.
pub async fn prepare_with_runner(
    paths: &Paths,
    opts: &LaunchOptions,
    ep: &Endpoints,
    runner: Arc<dyn ProcessRunner>,
) -> Result<Prepared> {
    prepare_with_progress(paths, opts, ep, runner, progress::silent()).await
}

/// `prepare_with_runner` that reports what it is doing to `progress` (see `Progress`).
pub async fn prepare_with_progress(
    paths: &Paths,
    opts: &LaunchOptions,
    ep: &Endpoints,
    runner: Arc<dyn ProcessRunner>,
    progress: ProgressSink,
) -> Result<Prepared> {
    let dl = Downloader::new(http_client())
        .with_sha1_cache(paths.sha1_cache())
        .with_progress(progress.clone());
    let env = Env::current();

    let manifest = fetch_manifest(dl.client(), paths, &ep.manifest).await?;
    let entry = manifest
        .find(&opts.version)
        .ok_or_else(|| Error::UnknownVersion(opts.version.clone()))?
        .clone();
    validate_id(&entry.id)?;
    let v = fetch_version(&dl, paths, &entry).await?;

    validate_id(&v.id)?;
    if let Some(name) = &opts.instance {
        validate_id(name)?;
    }
    let default_instance = match &opts.loader {
        Some(req) => format!("{}-{}", v.id, req.kind.slug()),
        None => v.id.clone(),
    };
    let instance = opts.instance.as_deref().unwrap_or(&default_instance);
    validate_id(instance)?;
    // A game of this instance runs (its run lock is held): refused before any loader, library,
    // jar or asset step. The lock itself is taken when the game starts (`run`, `spawn_game`).
    #[cfg(windows)]
    if crate::runlock::is_running(paths, instance)? {
        return Err(Error::InstanceRunning {
            id: instance.to_string(),
        });
    }

    // A loader problem must surface before any library or jar is downloaded.
    let setup = match &opts.loader {
        Some(req) => Some(prepare_loader(&dl, paths, &ep.loaders, req, &v.id, &env).await?),
        None => None,
    };
    let merged = match &setup {
        Some(s) => merge_version(&v, &s.child)?,
        None => v.clone(),
    };
    validate_id(&merged.id)?;

    let game_dir = paths.instance(instance);
    tokio::fs::create_dir_all(&game_dir).await?;

    let libs = libraries::resolve(&merged.libraries, &env, paths)?;
    let mut downloads = libs.downloads.clone();
    if let Some(s) = &setup {
        downloads.extend(s.extra_downloads.iter().cloned());
    }
    let mut classpath = libs.classpath.clone();
    // The vanilla jar is stored under the vanilla id and, with a loader, copied under the
    // loader's version id as the official launcher does.
    let vanilla_jar = paths.client_jar(&v.id);
    let merged_jar = paths.client_jar(&merged.id);
    let mut client_sha1 = None;
    if let Some(client) = v.downloads.as_ref().and_then(|d| d.client.as_ref()) {
        downloads.push(Download {
            url: client.url.clone(),
            path: vanilla_jar.clone(),
            sha1: Some(client.sha1.clone()),
            sha1_from_cache: false,
        });
        client_sha1 = Some(client.sha1.clone());
        classpath.push(merged_jar.clone());
    }
    let log_config = match merged.logging.as_ref().and_then(|l| l.client.as_ref()) {
        Some(c) => {
            validate_id(&c.file.id)?;
            let p = paths.log_config(&c.file.id);
            downloads.push(Download {
                url: c.file.url.clone(),
                path: p.clone(),
                sha1: Some(c.file.sha1.clone()),
                sha1_from_cache: false,
            });
            Some(p)
        }
        None => None,
    };
    // Maven libraries from loader profiles often carry no hash; nothing is fetched unverified.
    fill_missing_sha1(&dl, &mut downloads).await?;
    dl.fetch_all_reporting(downloads, Transfer::GameFiles)
        .await?;
    if let Some(sha1) = client_sha1.as_ref().filter(|_| merged_jar != vanilla_jar) {
        link_client_jar(&vanilla_jar, &merged_jar, sha1).await?;
    }

    let java = match &opts.java {
        Some(j) => j.clone(),
        None => ensure_runtime(&dl, paths, &env, &component_for(&v), &ep.runtime_index).await?,
    };

    if let Some(fi) = setup.as_ref().and_then(|s| s.forge.clone()) {
        let sha1 = client_sha1.clone().ok_or_else(|| {
            Error::Installer(format!("{} has no client jar to install onto", v.id))
        })?;
        let (merged, paths, java, jar) = (
            merged.clone(),
            paths.clone(),
            java.clone(),
            vanilla_jar.clone(),
        );
        let runner = runner.clone();
        let progress = progress.clone();
        tokio::task::spawn_blocking(move || {
            forge::finish_install(
                &fi,
                &merged,
                &paths,
                &java,
                &jar,
                &sha1,
                runner.as_ref(),
                &*progress,
            )
        })
        .await
        .map_err(|e| std::io::Error::other(format!("install task failed: {e}")))??;
    }

    let assets = match &merged.asset_index {
        Some(index) => install_assets(&dl, paths, index, &game_dir, &ep.resources).await?,
        None => AssetsLayout {
            index_id: "legacy".into(),
            assets_root: paths.assets(),
            game_assets: paths.assets(),
        },
    };

    let natives_dir = game_dir.join(".natives").join(&merged.id);
    remove_natives(&natives_dir).await?;
    libraries::extract_natives(&libs.natives, &natives_dir)?;

    let ctx = LaunchContext {
        account: opts.account.clone(),
        game_dir: game_dir.clone(),
        natives_dir,
        libraries_dir: paths.libraries(),
        classpath,
        assets,
        log_config,
        env,
        memory_mb: opts.memory_mb,
    };
    Ok(Prepared {
        java,
        args: build_args(&merged, &ctx)?,
        game_dir,
        version_id: merged.id.clone(),
        loader_list_from_cache: setup.as_ref().is_some_and(|s| s.list_from_cache),
        loader: setup.map(|s| s.resolved),
        account: opts.account.clone(),
        instance: instance.to_string(),
        game_log_dir: paths.logs().join("game"),
        run_lock: crate::runlock::run_lock_path(paths, instance)?,
    })
}

/// What a launch would start, decided from metadata alone.
#[derive(Clone, Debug)]
pub struct Resolution {
    /// The game version id (`latest` and `latest-snapshot` resolved).
    pub game: String,
    pub loader: Option<ResolvedLoader>,
    /// The loader build was picked from the saved version list (its service was unreachable).
    pub loader_list_from_cache: bool,
}

/// Resolves the game version and, with `loader`, the loader build, fetching only the version
/// manifest and the loader's version list (both saved in the data folder like `prepare` does).
/// Downloads no library, jar, asset or installer, needs no account, no Java and no run lock. A
/// named loader build must be on the list.
pub async fn resolve(
    paths: &Paths,
    version: &str,
    loader: Option<&LoaderRequest>,
    ep: &Endpoints,
) -> Result<Resolution> {
    let dl = Downloader::new(http_client());
    let manifest = fetch_manifest(dl.client(), paths, &ep.manifest).await?;
    let game = manifest
        .find(version)
        .ok_or_else(|| Error::UnknownVersion(version.to_string()))?
        .id
        .clone();
    validate_id(&game)?;
    let (loader, loader_list_from_cache) = match loader {
        Some(req) => {
            let (resolved, cached) = resolve_loader(&dl, paths, &ep.loaders, req, &game).await?;
            (Some(resolved), cached)
        }
        None => (None, false),
    };
    Ok(Resolution {
        game,
        loader,
        loader_list_from_cache,
    })
}

/// Copies the vanilla client jar to the loader version's path unless a good copy is there.
async fn link_client_jar(from: &std::path::Path, to: &std::path::Path, sha1: &str) -> Result<()> {
    let (from, to, sha1) = (from.to_owned(), to.to_owned(), sha1.to_owned());
    tokio::task::spawn_blocking(move || -> Result<()> {
        if to.is_file() && sha1_file(&to).is_ok_and(|h| h.eq_ignore_ascii_case(&sha1)) {
            return Ok(());
        }
        if let Some(dir) = to.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::copy(&from, &to)?;
        Ok(())
    })
    .await
    .map_err(|e| std::io::Error::other(format!("copy task failed: {e}")))?
}

/// True in builds with the `dev-offline` cargo feature.
pub const DEV_OFFLINE: bool = cfg!(feature = "dev-offline");

/// Offline accounts may only start the game when `dev_offline` is set.
pub fn ensure_launch_allowed(kind: AccountKind, dev_offline: bool) -> Result<()> {
    if kind == AccountKind::Offline && !dev_offline {
        return Err(Error::OfflineLaunchDisabled);
    }
    Ok(())
}

/// Starts the game and waits for it to exit. Output goes to the current console. Like
/// `spawn_game` it takes the instance's run lock first (`InstanceRunning` while a game of the
/// instance runs) and gives it to the game: this process holds it from before the start until the
/// hand-over right after, so there is no moment without it, and afterwards it stays with the game
/// even if the CLI is killed. A hand-over that fails ends the game again, unless
/// the game has already exited: then its exit code is returned as before.
pub async fn run(p: &Prepared) -> Result<i32> {
    ensure_launch_allowed(p.account.kind(), DEV_OFFLINE)?;
    #[cfg(windows)]
    let lock = {
        let (path, id) = (p.run_lock.clone(), p.instance.clone());
        tokio::task::spawn_blocking(move || crate::runlock::RunLock::acquire(&path))
            .await
            .map_err(|e| std::io::Error::other(format!("run lock task failed: {e}")))??
            .ok_or(Error::InstanceRunning { id })?
    };
    let mut child = tokio::process::Command::new(&p.java)
        .args(&p.args)
        .current_dir(&p.game_dir)
        .spawn()?;
    #[cfg(windows)]
    if let Some(raw) = child.raw_handle() {
        // SAFETY: `raw` is the child's process handle, owned by `child`, which outlives this use.
        let process = unsafe { std::os::windows::io::BorrowedHandle::borrow_raw(raw) };
        if let Err(e) = lock.hand_to(&process) {
            // A game that has already ended cannot take the lock and needs none: its exit code
            // is the answer, not the failed hand-over.
            if !matches!(child.try_wait(), Ok(Some(_))) {
                let _ = child.kill().await;
                return Err(e.into());
            }
        }
    }
    let status = child.wait().await?;
    Ok(status.code().unwrap_or(-1))
}

/// Clears the natives directory of a version. A running game keeps its libraries loaded, which
/// shows up as a denied or sharing-violation error: that is reported as "already running".
async fn remove_natives(dir: &Path) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    tokio::fs::remove_dir_all(dir).await.map_err(|e| {
        // 5 = access denied, 32 = sharing violation.
        if e.kind() == std::io::ErrorKind::PermissionDenied
            || matches!(e.raw_os_error(), Some(5 | 32))
        {
            Error::NativesInUse {
                dir: dir.to_path_buf(),
            }
        } else {
            e.into()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secret::Secret;

    fn prepared(args: &[&str], account: Account) -> Prepared {
        Prepared {
            java: PathBuf::from("java"),
            args: args.iter().map(|s| s.to_string()).collect(),
            game_dir: PathBuf::from("game"),
            version_id: "1.21".into(),
            loader: None,
            loader_list_from_cache: false,
            account,
            instance: "1.21".into(),
            game_log_dir: PathBuf::from("logs/game"),
            run_lock: PathBuf::from("tmp/run-1.21.lock"),
        }
    }

    #[test]
    fn redacts_microsoft_token_everywhere() {
        let account = Account::microsoft(
            "Steve".into(),
            "a".repeat(32),
            Secret::new("tok-123"),
            "1".into(),
        );
        let p = prepared(
            &[
                "-Xmx2048M",
                "--accessToken",
                "tok-123",
                "token:tok-123:uuid",
            ],
            account,
        );
        assert_eq!(
            p.redacted_args(),
            [
                "-Xmx2048M",
                "--accessToken",
                "<redacted>",
                "token:<redacted>:uuid"
            ]
        );
        assert!(!format!("{p:?}").contains("tok-123"));
    }

    #[test]
    fn offline_args_stay_untouched() {
        let p = prepared(
            &["-Xmx2048M", "-Xms512M", "--accessToken", "0"],
            Account::offline("Dev"),
        );
        assert_eq!(p.redacted_args(), p.args);
    }

    #[test]
    fn offline_needs_dev_offline() {
        assert!(matches!(
            ensure_launch_allowed(AccountKind::Offline, false),
            Err(Error::OfflineLaunchDisabled)
        ));
        ensure_launch_allowed(AccountKind::Offline, true).unwrap();
        ensure_launch_allowed(AccountKind::Microsoft, false).unwrap();
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn natives_in_use_by_a_running_game_are_reported_as_such() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let natives = dir.path().join(".natives").join("1.8.9");
        std::fs::create_dir_all(&natives).unwrap();
        let dll = natives.join("lwjgl.dll");
        std::fs::write(&dll, b"x").unwrap();
        // Open without sharing, like a loaded library that cannot be deleted.
        let held = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&dll)
            .unwrap();
        let err = remove_natives(&natives).await.unwrap_err();
        let m = err.to_string();
        assert!(matches!(&err, Error::NativesInUse { .. }), "{m}");
        assert!(m.contains("running already") && m.contains("1.8.9"), "{m}");
        drop(held);
        remove_natives(&natives).await.unwrap();
        assert!(!natives.exists());
        // Nothing to remove is fine.
        remove_natives(&natives).await.unwrap();
    }

    #[cfg(not(feature = "dev-offline"))]
    #[tokio::test]
    async fn run_refuses_offline_account_in_release_builds() {
        let mut p = prepared(&[], Account::offline("Dev"));
        p.java = PathBuf::from("C:/does-not-exist/java.exe");
        // Not Io: the lock applies before any process is spawned.
        assert!(matches!(run(&p).await, Err(Error::OfflineLaunchDisabled)));
    }
}
