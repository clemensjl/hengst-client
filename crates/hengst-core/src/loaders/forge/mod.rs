//! Version lists and installers for Forge and NeoForge.

pub mod installer;
pub mod processors;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use self::installer::{
    check_game, extract_embedded, read_installer, InstallerContents, InstallerKind,
};
use self::processors::{
    install_state_valid, plan, run_processors, write_marker, InstallEnv, ProcessRunner,
};
use super::{sort_newest_first, LoaderEndpoints, LoaderKind, LoaderVersion};
use crate::download::{fill_missing_sha1, get_bytes, sha1_file, Download, Downloader};
use crate::libraries::maven_path;
use crate::paths::{join_relative, validate_id, Paths};
use crate::version::VersionJson;
use crate::{Error, Result};

/// Forge has installers from 1.7.10 upward. IDs that are not of the form `1.X[.Y]` (snapshots,
/// year-based versions) are not refused here.
pub fn forge_supports(game: &str) -> bool {
    let parts: Vec<Option<u64>> = game.split('.').map(|p| p.parse().ok()).collect();
    match parts.as_slice() {
        [Some(1), Some(x)] => (1, *x, 0) >= (1, 7, 10),
        [Some(1), Some(x), Some(y)] => (1, *x, *y) >= (1, 7, 10),
        _ => true,
    }
}

/// The game version a NeoForge build belongs to, or `None` for snapshots and the pre-release
/// experiments that are not tied to a release.
pub fn neoforge_game_version(version: &str) -> Option<String> {
    if version.contains('+') || version.starts_with("0.") {
        return None;
    }
    let base = version.split('-').next()?;
    let parts: Vec<u64> = base
        .split('.')
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    match parts.as_slice() {
        [a, b, c, _] if *a >= 26 => Some(if *c == 0 {
            format!("{a}.{b}")
        } else {
            format!("{a}.{b}.{c}")
        }),
        [a, b, _] if *a < 26 => Some(if *b == 0 {
            format!("1.{a}")
        } else {
            format!("1.{a}.{b}")
        }),
        _ => None,
    }
}

fn check_forge_supports(game: &str) -> Result<()> {
    if !forge_supports(game) {
        return Err(Error::LoaderUnsupported {
            loader: "forge".into(),
            game: game.to_string(),
            reason: "Forge support starts at 1.7.10".into(),
        });
    }
    Ok(())
}

#[derive(Deserialize)]
struct Promotions {
    promos: HashMap<String, String>,
}

#[derive(Deserialize)]
struct MavenVersions {
    versions: Vec<String>,
}

/// Forge builds for a game version, newest first. A build is stable when it is the recommended one.
pub async fn list_forge(
    client: &reqwest::Client,
    ep: &LoaderEndpoints,
    game: &str,
) -> Result<Vec<LoaderVersion>> {
    check_forge_supports(game)?;
    validate_id(game)?;
    let base = &ep.forge_files;
    let metadata: HashMap<String, Vec<String>> =
        serde_json::from_slice(&get_bytes(client, &format!("{base}/maven-metadata.json")).await?)?;
    let promos: Promotions =
        serde_json::from_slice(&get_bytes(client, &format!("{base}/promotions_slim.json")).await?)?;
    let recommended = promos.promos.get(&format!("{game}-recommended"));
    let prefix = format!("{game}-");
    let suffix = format!("-{game}");
    let mut versions: Vec<LoaderVersion> = metadata
        .get(game)
        .into_iter()
        .flatten()
        .filter_map(|maven| {
            let rest = maven.strip_prefix(&prefix)?;
            let id = rest.strip_suffix(&suffix).unwrap_or(rest);
            if validate_id(maven).is_err() || validate_id(id).is_err() {
                return None;
            }
            Some(LoaderVersion {
                id: id.to_string(),
                maven: maven.clone(),
                stable: recommended.is_some_and(|r| r == id),
            })
        })
        .collect();
    sort_newest_first(&mut versions);
    Ok(versions)
}

/// NeoForge builds for a game version, newest first. A build is stable when its version has no
/// pre-release marker. Snapshot builds are not listed.
pub async fn list_neoforge(
    client: &reqwest::Client,
    ep: &LoaderEndpoints,
    game: &str,
) -> Result<Vec<LoaderVersion>> {
    validate_id(game)?;
    // 1.20.1 predates the neoforge artifact and lives under the old `forge` one.
    let old = game == "1.20.1";
    let artifact = if old { "forge" } else { "neoforge" };
    let url = format!(
        "{}/api/maven/versions/releases/net/neoforged/{artifact}",
        ep.neoforge_maven
    );
    let listed: MavenVersions = serde_json::from_slice(&get_bytes(client, &url).await?)?;
    let prefix = "1.20.1-";
    let mut versions: Vec<LoaderVersion> = listed
        .versions
        .into_iter()
        .filter_map(|maven| {
            let id = if old {
                maven.strip_prefix(prefix)?.to_string()
            } else if neoforge_game_version(&maven).as_deref() == Some(game) {
                maven.clone()
            } else {
                return None;
            };
            if validate_id(&maven).is_err() || validate_id(&id).is_err() {
                return None;
            }
            Some(LoaderVersion {
                stable: !id.contains('-'),
                id,
                maven,
            })
        })
        .collect();
    sort_newest_first(&mut versions);
    Ok(versions)
}

/// Maven coordinate of the installer jar. Only Forge and NeoForge have installers.
fn installer_coords(kind: LoaderKind, game: &str, maven: &str) -> Result<String> {
    match kind {
        LoaderKind::Forge => Ok(format!("net.minecraftforge:forge:{maven}:installer")),
        LoaderKind::NeoForge if game == "1.20.1" => {
            Ok(format!("net.neoforged:forge:{maven}:installer"))
        }
        LoaderKind::NeoForge => Ok(format!("net.neoforged:neoforge:{maven}:installer")),
        other => Err(Error::LoaderUnsupported {
            loader: other.to_string(),
            game: game.to_string(),
            reason: "it has no installer".into(),
        }),
    }
}

/// Download URL of the installer for a Forge or NeoForge build (`maven` is the full Maven version).
/// Any other loader kind is `Error::LoaderUnsupported`.
pub fn installer_url(
    kind: LoaderKind,
    game: &str,
    maven: &str,
    ep: &LoaderEndpoints,
) -> Result<String> {
    let rel = maven_path(&installer_coords(kind, game, maven)?).expect("installer coordinates");
    Ok(match kind {
        LoaderKind::Forge => format!("{}/{rel}", ep.forge_maven),
        _ => format!("{}/releases/{rel}", ep.neoforge_maven),
    })
}

/// Downloads the installer into the shared libraries directory (at its Maven path), verified
/// against the `.sha1` the Maven repository publishes next to it. A local copy is hashed again
/// and reused only when it matches. Returns the file and the SHA-1 it was verified against.
pub async fn fetch_installer(
    dl: &Downloader,
    paths: &Paths,
    kind: LoaderKind,
    game: &str,
    maven: &str,
    ep: &LoaderEndpoints,
) -> Result<(PathBuf, String)> {
    validate_id(game)?;
    validate_id(maven)?;
    let rel = maven_path(&installer_coords(kind, game, maven)?).expect("installer coordinates");
    let mut item = [Download {
        url: installer_url(kind, game, maven, ep)?,
        path: join_relative(&paths.libraries(), &rel)?,
        sha1: None,
        sha1_from_cache: false,
    }];
    let dl = dl.clone().with_sha1_cache(paths.sha1_cache());
    fill_missing_sha1(&dl, &mut item).await?;
    let (_, sha1) = dl.fetch_verified(&item[0]).await?;
    let [item] = item;
    let sha1 = sha1.expect("fill_missing_sha1 set a hash");
    Ok((item.path, sha1))
}

/// The builds a typed version can mean, without asking for the version list. Forge ids are short
/// (`47.4.28`) or full (`1.20.1-47.4.28`); old Forge builds carry the game version a second time
/// at the end (`1.8.9-11.15.1.2318-1.8.9`). NeoForge for 1.20.1 is `1.20.1-<id>`.
fn named_candidates(kind: LoaderKind, game: &str, requested: &str) -> Vec<LoaderVersion> {
    let v = |id: &str, maven: String| LoaderVersion {
        id: id.to_string(),
        maven,
        stable: false,
    };
    let prefix = format!("{game}-");
    match kind {
        LoaderKind::Forge => match requested.strip_prefix(&prefix) {
            Some(rest) => {
                let id = rest.strip_suffix(&format!("-{game}")).unwrap_or(rest);
                vec![v(id, requested.to_string())]
            }
            None => vec![
                v(requested, format!("{game}-{requested}")),
                v(requested, format!("{game}-{requested}-{game}")),
            ],
        },
        LoaderKind::NeoForge if game == "1.20.1" => {
            let id = requested.strip_prefix(&prefix).unwrap_or(requested);
            vec![v(id, format!("{prefix}{id}"))]
        }
        _ => vec![v(requested, requested.to_string())],
    }
}

/// Fetches the installer of a build the user named, without the version list. A candidate whose
/// installer is already on disk is tried first, so an installed build needs no network. A build
/// without an installer is `Error::UnknownLoaderVersion`. The gates of the version list apply
/// first: Forge before 1.7.10 is unsupported, and a NeoForge build must belong to `game` (no
/// snapshot builds).
pub async fn fetch_named(
    dl: &Downloader,
    paths: &Paths,
    ep: &LoaderEndpoints,
    kind: LoaderKind,
    game: &str,
    requested: &str,
) -> Result<(LoaderVersion, PathBuf, String)> {
    validate_id(game)?;
    validate_id(requested)?;
    let unknown = || Error::UnknownLoaderVersion {
        loader: kind.to_string(),
        game: game.to_string(),
        version: requested.to_string(),
    };
    match kind {
        LoaderKind::Forge => check_forge_supports(game)?,
        LoaderKind::NeoForge
            if game != "1.20.1" && neoforge_game_version(requested).as_deref() != Some(game) =>
        {
            return Err(unknown())
        }
        _ => {}
    }
    let mut candidates = named_candidates(kind, game, requested);
    for c in &candidates {
        validate_id(&c.maven)?;
    }
    let on_disk = |c: &LoaderVersion| {
        installer_coords(kind, game, &c.maven)
            .ok()
            .and_then(|coord| maven_path(&coord))
            .and_then(|rel| join_relative(&paths.libraries(), &rel).ok())
            .is_some_and(|p| p.is_file())
    };
    candidates.sort_by_key(|c| !on_disk(c));
    for c in candidates {
        match fetch_installer(dl, paths, kind, game, &c.maven, ep).await {
            Ok((installer, sha1)) => return Ok((c, installer, sha1)),
            Err(Error::NoChecksum { .. } | Error::Status { status: 404, .. }) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(unknown())
}

/// A Forge or NeoForge build whose installer was fetched, verified and read.
#[derive(Clone, Debug)]
pub struct ForgeInstall {
    pub kind: LoaderKind,
    pub game: String,
    /// Full Maven version of the requested build: picked from the validated version list, or
    /// mapped from the build the user named (`fetch_named`); never a value read from the installer.
    pub maven: String,
    pub installer: PathBuf,
    /// SHA-1 the installer was verified against (the Maven repository's).
    pub installer_sha1: String,
    /// For a legacy installer, `version` already points at the extracted universal jar.
    pub contents: InstallerContents,
}

/// The launcher root as an absolute path that is not verbatim (`std::fs::canonicalize` would add
/// `\\?\`, which no processor argument could match), and the libraries directory exactly
/// `<root>/libraries`, as NeoForge passes `{ROOT}/libraries/`.
fn absolute_root(paths: &Paths) -> Result<(PathBuf, PathBuf)> {
    let root = std::path::absolute(paths.root())?;
    let libraries = root.join("libraries");
    Ok((root, libraries))
}

/// Processors write files shared by all builds of one game version (split and remapped client
/// jars, mappings), so there is one install lock per loader kind and game version, not per build.
/// The lock lives outside the work directory.
fn install_lock_path(paths: &Paths, kind: LoaderKind, game: &str) -> Result<PathBuf> {
    validate_id(game)?;
    Ok(paths
        .tmp()
        .join(format!("loader-{}-{game}.lock", kind.slug())))
}

/// How long `lock_file` waits before saying so, and at most.
#[derive(Clone, Copy)]
struct LockTiming {
    notice_after: std::time::Duration,
    limit: std::time::Duration,
}

impl LockTiming {
    const DEFAULT: LockTiming = LockTiming {
        notice_after: std::time::Duration::from_secs(3),
        limit: std::time::Duration::from_secs(30 * 60),
    };
}

/// An exclusive lock on `path`, held until the file is dropped; the OS releases it if the process
/// dies. While another launch holds it, waits: after `notice_after` it tells the user once through
/// `notice`, after `limit` it gives up with an error.
fn lock_file(
    path: &Path,
    label: &str,
    timing: LockTiming,
    notice: &dyn Fn(&str),
) -> Result<std::fs::File> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)?;
    let start = std::time::Instant::now();
    let mut told = false;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(std::fs::TryLockError::WouldBlock) => {}
            Err(std::fs::TryLockError::Error(e)) => return Err(e.into()),
        }
        let waited = start.elapsed();
        if waited >= timing.limit {
            return Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!(
                    "another install of {label} is still running after {} s (lock {}); try again later",
                    timing.limit.as_secs(),
                    path.display()
                ),
            )));
        }
        if !told && waited >= timing.notice_after {
            notice(&format!(
                "another install of {label} is running; waiting for it to finish"
            ));
            told = true;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// Every path a processor gets lies under the launcher directory, and the planner refuses `;` and
/// `,` in path arguments. Such a directory is refused here, before anything is fetched.
pub fn check_home(paths: &Paths, kind: LoaderKind, game: &str) -> Result<()> {
    let root = std::path::absolute(paths.root())?;
    let text = root.to_string_lossy();
    if let Some(c) = text.chars().find(|c| matches!(c, ';' | ',')) {
        return Err(Error::LoaderUnsupported {
            loader: kind.to_string(),
            game: game.to_string(),
            reason: format!(
                "the launcher directory {text} contains '{c}', which installer processors cannot take in a path; use a directory without ';' or ','"
            ),
        });
    }
    Ok(())
}

/// The installer must still be the file that was verified: it is re-hashed right before it is
/// read again.
fn check_installer(installer: &Path, sha1: &str) -> Result<()> {
    let actual = sha1_file(installer)?;
    if !actual.eq_ignore_ascii_case(sha1) {
        return Err(Error::Checksum {
            path: installer.to_path_buf(),
            expected: sha1.to_string(),
            actual,
        });
    }
    Ok(())
}

/// Reads a verified installer and checks it is for `game`. A legacy installer's universal jar is
/// extracted right away, because the version info it rewrites decides which libraries are
/// downloaded. That extraction needs no lock: it replaces the jar only when it differs, through a
/// temp file and a rename.
pub fn open_installer(
    paths: &Paths,
    kind: LoaderKind,
    game: &str,
    maven: &str,
    installer: PathBuf,
    installer_sha1: String,
) -> Result<ForgeInstall> {
    validate_id(game)?;
    validate_id(maven)?;
    check_home(paths, kind, game)?;
    check_installer(&installer, &installer_sha1)?;
    let mut contents = read_installer(&installer)?;
    check_game(&contents, game)?;
    if matches!(contents.kind, InstallerKind::Legacy { .. }) {
        let (_, libraries) = absolute_root(paths)?;
        extract_embedded(&installer, &mut contents, &libraries, kind, maven)?;
    }
    Ok(ForgeInstall {
        kind,
        game: game.to_string(),
        maven: maven.to_string(),
        installer,
        installer_sha1,
        contents,
    })
}

/// Every installer-provided artifact (empty `url`) of the merged version must be present with the
/// SHA-1 the installer declares; these never pass through the downloader.
fn verify_provided(merged: &VersionJson, libraries: &Path) -> Result<()> {
    let provided = merged
        .libraries
        .iter()
        .filter_map(|l| l.downloads.as_ref()?.artifact.as_ref())
        .filter(|a| a.url.is_empty());
    for a in provided {
        let file = join_relative(libraries, &a.path)?;
        if !sha1_file(&file).is_ok_and(|h| h.eq_ignore_ascii_case(&a.sha1)) {
            return Err(Error::Installer(format!(
                "installer-provided {} is missing or does not match its declared SHA-1",
                file.display()
            )));
        }
    }
    Ok(())
}

/// Where a finished processor run is recorded.
pub fn marker_path(paths: &Paths, kind: LoaderKind, id: &str) -> PathBuf {
    paths
        .loader_dir(kind.slug())
        .join(format!("{id}.installed"))
}

/// Log of the processor run for a loader version.
pub fn install_log(paths: &Paths, id: &str) -> PathBuf {
    paths.logs().join(format!("loader-install-{id}.log"))
}

/// Brings the installation of `fi` (merged into `merged`) up to date on every launch, under an
/// exclusive lock per loader kind and game version: restores and verifies the installer-provided artifacts, verifies
/// the vanilla jar the processors read, plans the processors and runs them unless the recorded
/// install state is still valid, then records it. Blocking; call it off the async runtime.
/// `java` is the runtime the game itself uses. Profiles without client processors (legacy,
/// spec 0) have no processor stage and no marker: the provided-artifact check is all.
pub fn finish_install(
    fi: &ForgeInstall,
    merged: &VersionJson,
    paths: &Paths,
    java: &Path,
    vanilla_jar: &Path,
    vanilla_sha1: &str,
    runner: &dyn ProcessRunner,
) -> Result<()> {
    check_home(paths, fi.kind, &fi.game)?;
    let (root, libraries) = absolute_root(paths)?;
    let id = merged.id.as_str();
    validate_id(id)?;
    let _lock = lock_file(
        &install_lock_path(paths, fi.kind, &fi.game)?,
        &format!("{} for Minecraft {}", fi.kind, fi.game),
        LockTiming::DEFAULT,
        &|m| runner.notice(m),
    )?;
    // Read again below (extraction, data files): it must still be the verified file.
    check_installer(&fi.installer, &fi.installer_sha1)?;

    // A foreign file may have been put at a provided path since the last launch: restore it from
    // the verified installer, then check what goes on the classpath.
    let mut contents = fi.contents.clone();
    extract_embedded(&fi.installer, &mut contents, &libraries, fi.kind, &fi.maven)?;
    verify_provided(merged, &libraries)?;
    let InstallerKind::Modern(profile) = &contents.kind else {
        return Ok(());
    };

    // NeoForge declares no output hashes: a wrong input jar would be patched and recorded.
    let actual = sha1_file(vanilla_jar)?;
    if !actual.eq_ignore_ascii_case(vanilla_sha1) {
        return Err(Error::Checksum {
            path: vanilla_jar.to_path_buf(),
            expected: vanilla_sha1.to_string(),
            actual,
        });
    }

    let work = std::path::absolute(paths.tmp())?.join(format!("loader-{id}"));
    match std::fs::remove_dir_all(&work) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
        _ => {}
    }
    std::fs::create_dir_all(&work)?;
    let env = InstallEnv {
        root,
        libraries: libraries.clone(),
        minecraft_jar: std::path::absolute(vanilla_jar)?,
        game: fi.game.clone(),
        installer: std::path::absolute(&fi.installer)?,
        work: work.clone(),
    };
    // A relative path to java would be resolved against the processors' working directory.
    let java = if java.components().count() > 1 {
        std::path::absolute(java)?
    } else {
        java.to_path_buf()
    };
    let result = (|| {
        let plan = plan(profile, &env)?;
        if plan.commands.is_empty() {
            return Ok(());
        }
        let marker = marker_path(paths, fi.kind, id);
        if install_state_valid(&plan, merged, &libraries, &marker, &fi.installer_sha1)? {
            return Ok(());
        }
        match std::fs::remove_file(&marker) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
            _ => {}
        }
        run_processors(
            runner,
            &java,
            &plan.commands,
            &work,
            &install_log(paths, id),
        )?;
        write_marker(&plan, &marker, &fi.installer_sha1)
    })();
    if result.is_ok() {
        let _ = std::fs::remove_dir_all(&work);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loaders::{list_versions, pick_version, LoaderKind};
    use wiremock::matchers::{any, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const FORGE_METADATA: &str =
        include_str!("../../../tests/fixtures/loaders/forge-maven-metadata.json");
    const FORGE_PROMOS: &str =
        include_str!("../../../tests/fixtures/loaders/forge-promotions.json");
    const NEO_VERSIONS: &str =
        include_str!("../../../tests/fixtures/loaders/neoforge-versions.json");
    const NEO_FORGE_VERSIONS: &str =
        include_str!("../../../tests/fixtures/loaders/neoforge-forge-versions.json");

    const FORGE_META_PATH: &str = "/forge/maven-metadata.json";
    const FORGE_PROMOS_PATH: &str = "/forge/promotions_slim.json";
    const NEO_PATH: &str = "/neo/api/maven/versions/releases/net/neoforged/neoforge";
    const NEO_FORGE_PATH: &str = "/neo/api/maven/versions/releases/net/neoforged/forge";

    fn ep(server: &MockServer) -> LoaderEndpoints {
        let u = server.uri();
        LoaderEndpoints {
            forge_files: format!("{u}/forge"),
            neoforge_maven: format!("{u}/neo"),
            ..LoaderEndpoints::default()
        }
    }

    async fn serve(server: &MockServer, p: &str, body: &str) {
        Mock::given(method("GET"))
            .and(path(p))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(server)
            .await;
    }

    async fn forge_server() -> MockServer {
        let server = MockServer::start().await;
        serve(&server, FORGE_META_PATH, FORGE_METADATA).await;
        serve(&server, FORGE_PROMOS_PATH, FORGE_PROMOS).await;
        server
    }

    async fn neo_server() -> MockServer {
        let server = MockServer::start().await;
        serve(&server, NEO_PATH, NEO_VERSIONS).await;
        serve(&server, NEO_FORGE_PATH, NEO_FORGE_VERSIONS).await;
        server
    }

    fn ids(l: &[LoaderVersion]) -> Vec<&str> {
        l.iter().map(|v| v.id.as_str()).collect()
    }

    #[test]
    fn forge_cutoff_is_1_7_10() {
        for (g, ok) in [
            ("1.6.4", false),
            ("1.7.2", false),
            ("1.5.2", false),
            ("1.7", false),
            ("1.7.10", true),
            ("1.8", true),
            ("1.12.2", true),
            ("1.20.1", true),
            ("26.3", true),
            ("1.21-pre1", true),
        ] {
            assert_eq!(forge_supports(g), ok, "{g}");
        }
    }

    #[test]
    fn maps_neoforge_versions_to_game_versions() {
        for (v, g) in [
            ("20.2.88", Some("1.20.2")),
            ("20.2.3-beta", Some("1.20.2")),
            ("20.4.237", Some("1.20.4")),
            ("21.0.167", Some("1.21")),
            ("21.1.267", Some("1.21.1")),
            ("21.11.48", Some("1.21.11")),
            ("26.3.0.115", Some("26.3")),
            ("26.1.0.1-beta", Some("26.1")),
            ("26.1.1.0-beta", Some("26.1.1")),
            ("26.1.2.53-beta", Some("26.1.2")),
            ("26.1.0.0-alpha.1+snapshot-1", None),
            ("0.25w14craftmine.3-beta", None),
            ("21.1", None),
            ("x.y.z", None),
            ("", None),
        ] {
            assert_eq!(neoforge_game_version(v).as_deref(), g, "{v}");
        }
    }

    #[tokio::test]
    async fn forge_list_uses_recommended_promo_and_short_ids() {
        let server = forge_server().await;
        let client = reqwest::Client::new();
        let l = list_forge(&client, &ep(&server), "1.7.10").await.unwrap();
        assert_eq!(l[0].id, "10.13.4.1614");
        assert_eq!(l[0].maven, "1.7.10-10.13.4.1614-1.7.10");
        assert!(l[0].stable);
        assert!(l[1..].iter().all(|v| !v.stable));
        // Old entries have no game version suffix, and some carry a different suffix.
        assert!(l
            .iter()
            .any(|v| v.maven == "1.7.10-10.13.0.1150" && v.id == "10.13.0.1150"));
        assert!(l.iter().any(|v| v.id == "10.13.1.1210-new"));
        let l = list_forge(&client, &ep(&server), "1.20.1").await.unwrap();
        assert_eq!(l[0].id, "47.4.29");
        assert!(!l[0].stable);
        assert_eq!(
            pick_version(&l, None, LoaderKind::Forge, "1.20.1")
                .unwrap()
                .id,
            "47.4.28"
        );
        let l = list_forge(&client, &ep(&server), "1.12.2").await.unwrap();
        assert_eq!(l[0].id, "14.23.5.2864");
        assert_eq!(
            pick_version(&l, None, LoaderKind::Forge, "1.12.2")
                .unwrap()
                .id,
            "14.23.5.2859"
        );
    }

    #[tokio::test]
    async fn forge_without_recommended_build_falls_back_to_newest_unstable() {
        let server = forge_server().await;
        let client = reqwest::Client::new();
        let l = list_forge(&client, &ep(&server), "1.8.8").await.unwrap();
        assert_eq!(l[0].id, "11.15.0.1655");
        assert_eq!(l[0].maven, "1.8.8-11.15.0.1655");
        assert!(l.iter().all(|v| !v.stable));
        assert_eq!(
            pick_version(&l, None, LoaderKind::Forge, "1.8.8")
                .unwrap()
                .id,
            "11.15.0.1655"
        );
    }

    #[tokio::test]
    async fn forge_before_1_7_10_is_refused_without_network() {
        let server = MockServer::start().await;
        Mock::given(any())
            .respond_with(ResponseTemplate::new(404))
            .expect(0)
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let err = list_forge(&client, &ep(&server), "1.6.4")
            .await
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "forge for Minecraft 1.6.4 is not supported (Forge support starts at 1.7.10)"
        );
        assert!(matches!(
            list_versions(&client, &ep(&server), LoaderKind::Forge, "1.6.4").await,
            Err(Error::LoaderUnsupported { .. })
        ));
    }

    #[tokio::test]
    async fn neoforge_lists_per_game_version() {
        let server = neo_server().await;
        let client = reqwest::Client::new();
        let ep = ep(&server);
        let l = list_neoforge(&client, &ep, "26.3").await.unwrap();
        assert_eq!(ids(&l), ["26.3.0.115", "26.3.0.114", "26.3.0.75-beta"]);
        assert_eq!(
            l.iter().map(|v| v.stable).collect::<Vec<_>>(),
            [true, true, false]
        );
        let l = list_neoforge(&client, &ep, "1.21.1").await.unwrap();
        assert_eq!(l[0].id, "21.1.267");
        assert_eq!(l[0].maven, "21.1.267");
        let l = list_neoforge(&client, &ep, "1.21").await.unwrap();
        assert_eq!(ids(&l), ["21.0.167", "21.0.166"]);
        let l = list_neoforge(&client, &ep, "1.20.2").await.unwrap();
        assert_eq!(ids(&l), ["20.2.88", "20.2.3-beta"]);
        assert!(!l[1].stable);
        let l = list_neoforge(&client, &ep, "1.20.1").await.unwrap();
        assert_eq!(ids(&l), ["47.1.106", "47.1.5"]);
        assert_eq!(l[0].maven, "1.20.1-47.1.106");
        assert!(l.iter().all(|v| v.stable));
        let l = list_neoforge(&client, &ep, "26.1.1").await.unwrap();
        assert_eq!(ids(&l), ["26.1.1.1-beta", "26.1.1.0-beta"]);
        assert!(l.iter().all(|v| !v.stable));
        // Snapshot builds are not listed under any game version.
        let l = list_neoforge(&client, &ep, "26.1").await.unwrap();
        assert_eq!(ids(&l), ["26.1.0.1-beta"]);
        assert!(matches!(
            list_versions(&client, &ep, LoaderKind::NeoForge, "1.16.5").await,
            Err(Error::NoLoaderVersions { .. })
        ));
    }

    #[tokio::test]
    async fn dispatch_reaches_forge_neoforge_and_fabric_family() {
        let server = MockServer::start().await;
        serve(&server, FORGE_META_PATH, FORGE_METADATA).await;
        serve(&server, FORGE_PROMOS_PATH, FORGE_PROMOS).await;
        serve(&server, NEO_PATH, NEO_VERSIONS).await;
        serve(
            &server,
            "/fabric/versions/loader/1.21.11",
            r#"[{"loader":{"version":"0.20.1","maven":"net.fabricmc:fabric-loader:0.20.1","stable":true}}]"#,
        )
        .await;
        let ep = LoaderEndpoints {
            fabric: format!("{}/fabric", server.uri()),
            ..ep(&server)
        };
        let client = reqwest::Client::new();
        let f = list_versions(&client, &ep, LoaderKind::Forge, "1.7.10")
            .await
            .unwrap();
        assert_eq!(f[0].id, "10.13.4.1614");
        let n = list_versions(&client, &ep, LoaderKind::NeoForge, "1.21.1")
            .await
            .unwrap();
        assert_eq!(n[0].id, "21.1.267");
        let fab = list_versions(&client, &ep, LoaderKind::Fabric, "1.21.11")
            .await
            .unwrap();
        assert_eq!(fab[0].id, "0.20.1");
        // A game version Forge has no builds for.
        assert!(matches!(
            list_versions(&client, &ep, LoaderKind::Forge, "9.9.9").await,
            Err(Error::NoLoaderVersions { .. })
        ));
    }

    #[tokio::test]
    async fn hostile_entries_are_skipped() {
        let server = MockServer::start().await;
        let meta = r#"{"1.20.1":["1.20.1-47.4.28","1.20.1-../x","1.20.1-a/b","1.20.1-47.4.1-1.20.1/../y","1.20.1-"]}"#;
        serve(&server, FORGE_META_PATH, meta).await;
        serve(&server, FORGE_PROMOS_PATH, FORGE_PROMOS).await;
        serve(
            &server,
            NEO_PATH,
            r#"{"versions":["21.1.5","21.1.6-a/b","21.1.7/../x","21.1.8-..\\x"]}"#,
        )
        .await;
        serve(
            &server,
            NEO_FORGE_PATH,
            r#"{"versions":["1.20.1-47.1.5","1.20.1-../x","1.20.1-a/b","1.20.1-"]}"#,
        )
        .await;
        let client = reqwest::Client::new();
        let ep = ep(&server);
        let l = list_forge(&client, &ep, "1.20.1").await.unwrap();
        assert_eq!(ids(&l), ["47.4.28"]);
        let l = list_neoforge(&client, &ep, "1.21.1").await.unwrap();
        assert_eq!(ids(&l), ["21.1.5"]);
        let l = list_neoforge(&client, &ep, "1.20.1").await.unwrap();
        assert_eq!(ids(&l), ["47.1.5"]);
    }

    #[tokio::test]
    async fn unreachable_service_is_a_transport_error() {
        // A port that was just free and has no listener.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let u = format!("http://127.0.0.1:{port}");
        let ep = LoaderEndpoints {
            forge_files: u.clone(),
            neoforge_maven: u,
            ..LoaderEndpoints::default()
        };
        let client = reqwest::Client::new();
        assert!(matches!(
            list_forge(&client, &ep, "1.20.1").await,
            Err(Error::Http { .. })
        ));
        assert!(matches!(
            list_neoforge(&client, &ep, "1.21.1").await,
            Err(Error::Http { .. })
        ));
    }

    #[test]
    fn named_builds_map_to_maven_versions_without_a_list() {
        let mavens = |kind, game, req| -> Vec<(String, String)> {
            named_candidates(kind, game, req)
                .into_iter()
                .map(|v| (v.id, v.maven))
                .collect()
        };
        let p = |id: &str, m: &str| (id.to_string(), m.to_string());
        assert_eq!(
            mavens(LoaderKind::Forge, "1.20.1", "47.4.28"),
            [
                p("47.4.28", "1.20.1-47.4.28"),
                p("47.4.28", "1.20.1-47.4.28-1.20.1")
            ]
        );
        assert_eq!(
            mavens(LoaderKind::Forge, "1.8.9", "1.8.9-11.15.1.2318-1.8.9"),
            [p("11.15.1.2318", "1.8.9-11.15.1.2318-1.8.9")]
        );
        assert_eq!(
            mavens(LoaderKind::NeoForge, "1.20.1", "47.1.106"),
            [p("47.1.106", "1.20.1-47.1.106")]
        );
        assert_eq!(
            mavens(LoaderKind::NeoForge, "1.20.1", "1.20.1-47.1.106"),
            [p("47.1.106", "1.20.1-47.1.106")]
        );
        assert_eq!(
            mavens(LoaderKind::NeoForge, "26.3", "26.3.0.115"),
            [p("26.3.0.115", "26.3.0.115")]
        );
    }

    #[tokio::test]
    async fn named_build_with_an_unsafe_id_is_refused_without_network() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let dl = Downloader::new(reqwest::Client::new());
        let ep = LoaderEndpoints::default();
        for bad in ["../x", "a/b", "a:b"] {
            assert!(matches!(
                fetch_named(&dl, &paths, &ep, LoaderKind::Forge, "1.20.1", bad).await,
                Err(Error::UnsafePath(_))
            ));
        }
    }

    #[tokio::test]
    async fn named_build_gets_the_gates_of_the_version_list_without_network() {
        let server = MockServer::start().await;
        Mock::given(any())
            .respond_with(ResponseTemplate::new(404))
            .expect(0)
            .mount(&server)
            .await;
        let ep = LoaderEndpoints {
            forge_maven: format!("{}/fm", server.uri()),
            ..ep(&server)
        };
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let dl = Downloader::new(reqwest::Client::new());
        let err = fetch_named(&dl, &paths, &ep, LoaderKind::Forge, "1.6.4", "9.11.1.1345")
            .await
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "forge for Minecraft 1.6.4 is not supported (Forge support starts at 1.7.10)"
        );
        // A NeoForge build of another game version, and a snapshot build.
        for (game, build) in [
            ("1.21", "21.1.267"),
            ("26.1", "26.1.0.0-alpha.1+snapshot-1"),
        ] {
            let err = fetch_named(&dl, &paths, &ep, LoaderKind::NeoForge, game, build)
                .await
                .unwrap_err();
            assert!(
                matches!(&err, Error::UnknownLoaderVersion { version, .. } if version == build),
                "{err}"
            );
        }
    }

    #[test]
    fn installer_urls() {
        let ep = LoaderEndpoints::default();
        assert_eq!(
            installer_url(LoaderKind::Forge, "1.7.10", "1.7.10-10.13.4.1614-1.7.10", &ep).unwrap(),
            "https://maven.minecraftforge.net/net/minecraftforge/forge/1.7.10-10.13.4.1614-1.7.10/forge-1.7.10-10.13.4.1614-1.7.10-installer.jar"
        );
        assert_eq!(
            installer_url(LoaderKind::NeoForge, "1.20.1", "1.20.1-47.1.106", &ep).unwrap(),
            "https://maven.neoforged.net/releases/net/neoforged/forge/1.20.1-47.1.106/forge-1.20.1-47.1.106-installer.jar"
        );
        assert_eq!(
            installer_url(LoaderKind::NeoForge, "26.3", "26.3.0.115", &ep).unwrap(),
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/26.3.0.115/neoforge-26.3.0.115-installer.jar"
        );
    }

    #[test]
    fn installer_url_for_other_loaders_is_unsupported() {
        let ep = LoaderEndpoints::default();
        assert!(matches!(
            installer_url(LoaderKind::Fabric, "1.21.1", "0.17.3", &ep),
            Err(Error::LoaderUnsupported { .. })
        ));
    }

    const NEO_INSTALLER: &str =
        "/neo/releases/net/neoforged/neoforge/21.1.172/neoforge-21.1.172-installer.jar";

    async fn installer_server(sidecar: Option<String>) -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(NEO_INSTALLER))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"installer".to_vec()))
            .mount(&server)
            .await;
        if let Some(body) = sidecar {
            Mock::given(method("GET"))
                .and(path(format!("{NEO_INSTALLER}.sha1")))
                .respond_with(ResponseTemplate::new(200).set_body_string(body))
                .mount(&server)
                .await;
        }
        server
    }

    #[tokio::test]
    async fn fetches_installer_verified_by_sidecar() {
        let server = installer_server(Some(crate::download::sha1_hex(b"installer"))).await;
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let dl = Downloader::new(reqwest::Client::new());
        let ep = ep(&server);
        let fetch =
            || fetch_installer(&dl, &paths, LoaderKind::NeoForge, "1.21.1", "21.1.172", &ep);
        let (file, sha1) = fetch().await.unwrap();
        assert_eq!(
            file,
            paths
                .libraries()
                .join("net/neoforged/neoforge/21.1.172/neoforge-21.1.172-installer.jar")
        );
        assert_eq!(sha1, crate::download::sha1_hex(b"installer"));
        assert_eq!(std::fs::read(&file).unwrap(), b"installer");
        // A local copy is hashed again on every run and replaced when it no longer matches.
        std::fs::write(&file, b"tampered").unwrap();
        let (file, again) = fetch().await.unwrap();
        assert_eq!(again, sha1);
        assert_eq!(std::fs::read(&file).unwrap(), b"installer");
    }

    #[tokio::test]
    async fn installer_with_wrong_sidecar_is_refused() {
        let server = installer_server(Some(crate::download::sha1_hex(b"something else"))).await;
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let dl = Downloader::new(reqwest::Client::new());
        let err = fetch_installer(
            &dl,
            &paths,
            LoaderKind::NeoForge,
            "1.21.1",
            "21.1.172",
            &ep(&server),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, Error::Checksum { .. }), "{err}");
        assert!(!paths
            .libraries()
            .join("net/neoforged/neoforge/21.1.172/neoforge-21.1.172-installer.jar")
            .exists());
    }

    #[tokio::test]
    async fn installer_without_sidecar_is_refused() {
        let server = installer_server(None).await;
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let dl = Downloader::new(reqwest::Client::new());
        let err = fetch_installer(
            &dl,
            &paths,
            LoaderKind::NeoForge,
            "1.21.1",
            "21.1.172",
            &ep(&server),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, Error::NoChecksum { .. }), "{err}");
    }

    #[tokio::test]
    async fn installer_version_must_be_a_plain_id() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let dl = Downloader::new(reqwest::Client::new());
        let ep = LoaderEndpoints::default();
        for bad in ["../x", "1.0/../../x", "a:b", r"a\b"] {
            assert!(
                matches!(
                    fetch_installer(&dl, &paths, LoaderKind::Forge, "1.12.2", bad, &ep).await,
                    Err(Error::UnsafePath(_))
                ),
                "{bad}"
            );
        }
    }

    // --- finish_install ----------------------------------------------------------------------

    use crate::download::sha1_hex;
    use processors::ProcessRunner;
    use std::io::Write;
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    const MAVEN: &str = "1.20.1-1.0";
    const ID: &str = "1.20.1-forge-1.0";
    const PROVIDED: &str = "net/minecraftforge/forge/1.20.1-1.0/forge-1.20.1-1.0.jar";

    fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(&mut buf);
        for (n, data) in entries {
            zip.start_file(*n, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
        buf.into_inner()
    }

    struct Setup {
        _tmp: tempfile::TempDir,
        paths: Paths,
        fi: ForgeInstall,
        vanilla: PathBuf,
    }

    impl Setup {
        fn finish(&self, runner: &dyn ProcessRunner) -> Result<()> {
            self.finish_with(runner, Path::new("java.exe"))
        }
        fn finish_with(&self, runner: &dyn ProcessRunner, java: &Path) -> Result<()> {
            finish_install(
                &self.fi,
                &self.fi.contents.version,
                &self.paths,
                java,
                &self.vanilla,
                &sha1_hex(b"vanilla"),
                runner,
            )
        }
        fn marker(&self) -> PathBuf {
            marker_path(&self.paths, LoaderKind::Forge, ID)
        }
        fn patched(&self) -> PathBuf {
            self.paths
                .libraries()
                .join("net/minecraftforge/forge/1.20.1-1.0/forge-1.20.1-1.0-client.jar")
        }
        fn provided(&self) -> PathBuf {
            self.paths.libraries().join(PROVIDED)
        }
    }

    /// A modern installer with one client processor that writes PATCHED, an installer-provided
    /// jar, the processor jar staged in the libraries and the vanilla jar in place.
    fn setup() -> Setup {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().join("home"));
        let tool = zip_bytes(&[(
            "META-INF/MANIFEST.MF",
            b"Manifest-Version: 1.0\r\nMain-Class: test.Patcher\r\n\r\n",
        )]);
        let tool_rel = "net/minecraftforge/tool/1/tool-1.jar";
        let tool_path = paths.libraries().join(tool_rel);
        std::fs::create_dir_all(tool_path.parent().unwrap()).unwrap();
        std::fs::write(&tool_path, &tool).unwrap();
        let profile = serde_json::json!({
            "spec": 1, "minecraft": "1.20.1", "json": "/version.json",
            "data": {
                "PATCHED": {"client": "[net.minecraftforge:forge:1.20.1-1.0:client]", "server": "'x'"},
                "BINPATCH": {"client": "/data/client.lzma", "server": "'x'"}
            },
            "processors": [{"jar": "net.minecraftforge:tool:1",
                "args": ["--input", "{MINECRAFT_JAR}", "--output", "{PATCHED}", "--apply", "{BINPATCH}"]}],
            "libraries": [{"name": "net.minecraftforge:tool:1", "downloads": {"artifact": {
                "path": tool_rel, "url": "https://example.invalid/tool-1.jar",
                "sha1": sha1_hex(&tool), "size": tool.len()}}}]
        });
        let version = serde_json::json!({
            "id": ID, "inheritsFrom": "1.20.1", "mainClass": "m",
            "libraries": [{"name": "net.minecraftforge:forge:1.20.1-1.0", "downloads": {"artifact": {
                "path": PROVIDED, "url": "", "sha1": sha1_hex(b"provided"), "size": 8}}}]
        });
        let installer = tmp.path().join("installer.jar");
        std::fs::write(
            &installer,
            zip_bytes(&[
                ("install_profile.json", profile.to_string().as_bytes()),
                ("version.json", version.to_string().as_bytes()),
                (&format!("maven/{PROVIDED}"), b"provided"),
                ("data/client.lzma", b"lzma"),
            ]),
        )
        .unwrap();
        let vanilla = paths.client_jar("1.20.1");
        std::fs::create_dir_all(vanilla.parent().unwrap()).unwrap();
        std::fs::write(&vanilla, b"vanilla").unwrap();
        let sha = sha1_hex(&std::fs::read(&installer).unwrap());
        let fi =
            open_installer(&paths, LoaderKind::Forge, "1.20.1", MAVEN, installer, sha).unwrap();
        Setup {
            _tmp: tmp,
            paths,
            fi,
            vanilla,
        }
    }

    /// Writes the value after `--output` like the real patcher would, counts runs and how many run
    /// at once, and records the working directory.
    #[derive(Default)]
    struct Patcher {
        runs: AtomicUsize,
        active: AtomicUsize,
        max_active: AtomicUsize,
        delay_ms: u64,
        cwd: std::sync::Mutex<Option<PathBuf>>,
    }

    impl ProcessRunner for Patcher {
        fn run(&self, cmd: &mut Command, _: Duration) -> std::io::Result<Option<i32>> {
            let now = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_active.fetch_max(now, Ordering::SeqCst);
            self.runs.fetch_add(1, Ordering::SeqCst);
            let cwd = cmd.get_current_dir().map(Path::to_path_buf).unwrap();
            assert!(!cwd.join("stale.txt").exists(), "work was not emptied");
            *self.cwd.lock().unwrap() = Some(cwd);
            std::thread::sleep(std::time::Duration::from_millis(self.delay_ms));
            let args: Vec<String> = cmd
                .get_args()
                .map(|a| a.to_string_lossy().into_owned())
                .collect();
            let out = &args[args.iter().position(|a| a == "--output").unwrap() + 1];
            std::fs::create_dir_all(Path::new(out).parent().unwrap())?;
            std::fs::write(out, b"patched")?;
            self.active.fetch_sub(1, Ordering::SeqCst);
            Ok(Some(0))
        }
    }

    /// Fails the test if any process would be started.
    struct NoStart;

    impl ProcessRunner for NoStart {
        fn run(&self, cmd: &mut Command, _: Duration) -> std::io::Result<Option<i32>> {
            panic!("no process may start: {:?}", cmd.get_program());
        }
    }

    #[test]
    fn processor_error_message_names_step_code_and_log() {
        let e = Error::Processor {
            jar: "net.minecraftforge:binarypatcher:1.1.1".into(),
            code: 1,
            log: PathBuf::from("C:/h/logs/x.log"),
        };
        assert_eq!(
            e.to_string(),
            "loader installer step net.minecraftforge:binarypatcher:1.1.1 failed with exit code 1, see C:/h/logs/x.log"
        );
    }

    #[test]
    fn installs_once_then_valid_install_skips_processors() {
        let s = setup();
        let patcher = Patcher::default();
        s.finish(&patcher).unwrap();
        assert_eq!(patcher.runs.load(Ordering::SeqCst), 1);
        assert_eq!(std::fs::read(s.patched()).unwrap(), b"patched");
        assert!(s.marker().is_file());
        // Valid state: no child process at all, not even with a java that does not exist.
        s.finish(&NoStart).unwrap();
        s.finish_with(
            &processors::SystemRunner,
            &s.paths.root().join("no-java.exe"),
        )
        .unwrap();
    }

    #[test]
    fn failed_processor_leaves_no_marker() {
        let s = setup();
        // A real child process that exits non-zero (where.exe rejects `-cp`).
        let helper = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32")
            .join("where.exe");
        let err = s
            .finish_with(&processors::SystemRunner, &helper)
            .unwrap_err();
        assert!(
            matches!(&err, Error::Processor { jar, code, log } if jar.starts_with("processor 0")
                && *code != 0 && *log == install_log(&s.paths, ID)),
            "{err}"
        );
        assert!(!s.marker().exists());
        // After a good install, a later failed re-run removes the old marker too.
        s.finish(&Patcher::default()).unwrap();
        assert!(s.marker().is_file());
        std::fs::write(s.patched(), b"tampered").unwrap();
        assert!(s.finish_with(&processors::SystemRunner, &helper).is_err());
        assert!(!s.marker().exists());
    }

    #[test]
    fn tampered_patched_jar_reruns_the_processors() {
        let s = setup();
        let patcher = Patcher::default();
        s.finish(&patcher).unwrap();
        std::fs::write(s.patched(), b"tampered").unwrap();
        s.finish(&patcher).unwrap();
        assert_eq!(patcher.runs.load(Ordering::SeqCst), 2);
        assert_eq!(std::fs::read(s.patched()).unwrap(), b"patched");
    }

    #[test]
    fn work_directory_is_fresh_outside_libraries_and_meta_and_removed_after() {
        let s = setup();
        let root = std::path::absolute(s.paths.root()).unwrap();
        let work = root.join("tmp").join(format!("loader-{ID}"));
        std::fs::create_dir_all(&work).unwrap();
        std::fs::write(work.join("stale.txt"), b"old run").unwrap();
        let patcher = Patcher::default();
        s.finish(&patcher).unwrap();
        let cwd = patcher.cwd.lock().unwrap().clone().unwrap();
        assert_eq!(cwd, work);
        assert!(!cwd.starts_with(root.join("libraries")) && !cwd.starts_with(root.join("meta")));
        assert!(!work.exists());
        // One lock for every Forge build of this game version, outside the work directory.
        assert!(root.join("tmp").join("loader-forge-1.20.1.lock").is_file());
    }

    #[test]
    fn provided_jar_is_restored_even_when_processors_are_skipped() {
        let s = setup();
        s.finish(&Patcher::default()).unwrap();
        std::fs::write(s.provided(), b"foreign bytes at the shared path").unwrap();
        s.finish(&NoStart).unwrap();
        assert_eq!(std::fs::read(s.provided()).unwrap(), b"provided");
        std::fs::remove_file(s.provided()).unwrap();
        s.finish(&NoStart).unwrap();
        assert_eq!(std::fs::read(s.provided()).unwrap(), b"provided");
    }

    #[test]
    fn wrong_vanilla_jar_is_refused_before_any_processor() {
        let s = setup();
        std::fs::write(&s.vanilla, b"not the vanilla jar").unwrap();
        let err = s.finish(&NoStart).unwrap_err();
        assert!(matches!(err, Error::Checksum { .. }), "{err}");
        assert!(!s.marker().exists());
    }

    #[test]
    fn concurrent_installs_of_one_version_run_the_processors_once() {
        let s = Arc::new(setup());
        let patcher = Arc::new(Patcher {
            delay_ms: 300,
            ..Patcher::default()
        });
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let (s, p) = (s.clone(), patcher.clone());
                std::thread::spawn(move || s.finish(p.as_ref()))
            })
            .collect();
        for h in handles {
            h.join().unwrap().unwrap();
        }
        assert_eq!(patcher.max_active.load(Ordering::SeqCst), 1);
        assert_eq!(patcher.runs.load(Ordering::SeqCst), 1);
        assert!(s.marker().is_file());
    }

    #[test]
    fn installer_changed_after_the_downloads_is_refused_inside_the_lock() {
        let s = setup();
        std::fs::write(&s.fi.installer, b"swapped after verification").unwrap();
        let err = s.finish(&NoStart).unwrap_err();
        assert!(matches!(err, Error::Checksum { .. }), "{err}");
    }

    #[test]
    fn install_lock_waits_with_one_notice_and_gives_up_at_the_limit() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("tmp").join("x.lock");
        let holder = lock_file(
            &path,
            "forge for Minecraft 1.20.1",
            LockTiming::DEFAULT,
            &|_| {},
        )
        .unwrap();
        let notices = std::sync::Mutex::new(Vec::new());
        let timing = LockTiming {
            notice_after: std::time::Duration::from_millis(100),
            limit: std::time::Duration::from_millis(600),
        };
        let err = lock_file(&path, "forge for Minecraft 1.20.1", timing, &|m| {
            notices.lock().unwrap().push(m.to_string())
        })
        .unwrap_err();
        let m = err.to_string();
        assert!(
            m.contains("another install of forge for Minecraft 1.20.1 is still running"),
            "{m}"
        );
        assert_eq!(
            *notices.lock().unwrap(),
            ["another install of forge for Minecraft 1.20.1 is running; waiting for it to finish"]
        );
        // Released while the second one waits: it gets the lock.
        let release = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            drop(holder);
        });
        let timing = LockTiming {
            notice_after: std::time::Duration::from_millis(100),
            limit: std::time::Duration::from_secs(10),
        };
        lock_file(&path, "forge for Minecraft 1.20.1", timing, &|_| {}).unwrap();
        release.join().unwrap();
    }

    #[test]
    fn launcher_home_with_list_separators_is_refused_up_front() {
        for (dir, c) in [("a;b", ';'), ("a,b", ',')] {
            let paths = Paths::new(Path::new("C:/games").join(dir).join("home"));
            let err = check_home(&paths, LoaderKind::NeoForge, "1.21.1").unwrap_err();
            let m = err.to_string();
            assert!(
                matches!(err, Error::LoaderUnsupported { .. })
                    && m.contains(&format!("'{c}'"))
                    && m.contains(dir),
                "{m}"
            );
        }
        check_home(&Paths::new("C:/games/home"), LoaderKind::Forge, "1.20.1").unwrap();
    }

    #[tokio::test]
    async fn prepare_loader_checks_the_home_before_any_request() {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let u = format!("http://127.0.0.1:{port}");
        let ep = LoaderEndpoints {
            forge_files: u.clone(),
            forge_maven: u,
            ..LoaderEndpoints::default()
        };
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().join("a;b"));
        let dl = Downloader::new(reqwest::Client::new());
        let req = crate::loaders::LoaderRequest {
            kind: LoaderKind::Forge,
            version: None,
        };
        let err = crate::loaders::prepare_loader(
            &dl,
            &paths,
            &ep,
            &req,
            "1.20.1",
            &crate::rules::Env::windows_x64(),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, Error::LoaderUnsupported { .. }), "{err}");
    }
}
