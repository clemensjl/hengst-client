use clap::{Parser, Subcommand, ValueEnum};
use hengst_core::args::Account;
use hengst_core::auth::flow::launch_account;
#[cfg(windows)]
use hengst_core::auth::store::KeyringSecretStore;
#[cfg(not(windows))]
use hengst_core::auth::store::MemorySecretStore;
use hengst_core::auth::store::{load_accounts_file, AccountStore};
use hengst_core::auth::{client_id, unix_now, AuthEndpoints, MicrosoftAuth};
use hengst_core::download::http_client;
use hengst_core::launch::{
    ensure_launch_allowed, prepare, run, Endpoints, LaunchOptions, DEV_OFFLINE,
};
use hengst_core::loaders::{
    list_versions, pick_version, LoaderEndpoints, LoaderKind, LoaderRequest, LoaderVersion,
    ResolvedLoader,
};
use hengst_core::manifest::{fetch_manifest, VersionKind, MANIFEST_URL};
use hengst_core::paths::Paths;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Parser)]
#[command(
    name = "hengst",
    version,
    about = "Hengst Client: open-source Minecraft launcher"
)]
struct Cli {
    /// Data directory (default: %APPDATA%\HengstClient or HENGST_HOME)
    #[arg(long, global = true)]
    home: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// List available game versions
    Versions {
        #[arg(long, value_enum, default_value_t = KindFilter::Release)]
        kind: KindFilter,
        #[arg(long, default_value_t = 30)]
        limit: usize,
    },
    /// Download a version and start it
    Launch {
        /// Version id, or `latest` / `latest-snapshot`
        version: String,
        #[arg(long)]
        instance: Option<String>,
        /// Saved account to play with (default: the default account)
        #[arg(long, conflicts_with = "offline")]
        account: Option<String>,
        /// Offline development account with this name (needs a dev-offline build to start)
        #[arg(long, value_name = "NAME")]
        offline: Option<String>,
        #[arg(long)]
        java: Option<PathBuf>,
        #[arg(long, default_value_t = 2048)]
        memory: u32,
        /// Mod loader: fabric, quilt, legacy-fabric, neoforge or forge (case-insensitive)
        #[arg(long, value_parser = LoaderKind::from_str)]
        loader: Option<LoaderKind>,
        /// Loader version (default: the newest stable one)
        #[arg(long, requires = "loader")]
        loader_version: Option<String>,
        /// Download everything and print the command line instead of starting
        #[arg(long)]
        dry_run: bool,
    },
    /// List loader versions for a game version, newest first
    Loaders {
        /// Loader: fabric, quilt, legacy-fabric, neoforge or forge (case-insensitive)
        #[arg(value_parser = LoaderKind::from_str)]
        loader: LoaderKind,
        /// Game version id, or `latest` / `latest-snapshot`
        version: String,
        /// Maximum number of versions to list
        #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u64).range(1..))]
        limit: u64,
    },
    /// Sign in with a Microsoft account and save it
    Login,
    /// List saved accounts, or change the default
    Accounts {
        #[command(subcommand)]
        action: Option<AccountsCmd>,
    },
    /// Remove a saved account (only deletes the locally stored tokens)
    Logout {
        /// Account name or UUID
        name: String,
    },
}

#[derive(Subcommand)]
enum AccountsCmd {
    /// Make an account the default
    Use {
        /// Account name or UUID
        name: String,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum KindFilter {
    Release,
    Snapshot,
    OldBeta,
    OldAlpha,
    All,
}

impl KindFilter {
    fn matches(self, kind: VersionKind) -> bool {
        match self {
            KindFilter::All => true,
            KindFilter::Release => kind == VersionKind::Release,
            KindFilter::Snapshot => kind == VersionKind::Snapshot,
            KindFilter::OldBeta => kind == VersionKind::OldBeta,
            KindFilter::OldAlpha => kind == VersionKind::OldAlpha,
        }
    }
}

/// Whether the unstable-fallback note can apply at all (no explicit version, unstable pick).
fn note_applies(requested: Option<&str>, resolved: &ResolvedLoader) -> bool {
    requested.is_none() && !resolved.version.stable
}

/// Warns when the default pick had to fall back to a non-stable loader build.
fn unstable_note(requested: Option<&str>, resolved: &ResolvedLoader, game: &str) -> Option<String> {
    if !note_applies(requested, resolved) {
        return None;
    }
    Some(format!(
        "note: no stable {} release for Minecraft {game}, using {}",
        resolved.kind, resolved.version.id
    ))
}

/// One line per loader version (at most `limit`), the default marked.
fn loader_lines(versions: &[LoaderVersion], limit: usize, default_id: &str) -> Vec<String> {
    versions
        .iter()
        .take(limit)
        .map(|v| {
            let stable = if v.stable { "stable" } else { "unstable" };
            let marker = if v.id == default_id { " (default)" } else { "" };
            format!("{:<24} {stable}{marker}", v.id)
        })
        .collect()
}

/// Maps `latest` / `latest-snapshot` to a game version id; other ids pass through.
async fn resolve_game(paths: &Paths, version: &str) -> anyhow::Result<String> {
    let manifest = fetch_manifest(&http_client(), paths, MANIFEST_URL).await?;
    Ok(manifest
        .find(version)
        .map(|v| v.id.clone())
        .unwrap_or_else(|| version.to_string()))
}

#[cfg(windows)]
fn open_store(paths: &Paths) -> anyhow::Result<AccountStore<KeyringSecretStore>> {
    Ok(AccountStore::new(
        paths.accounts_file(),
        KeyringSecretStore::new()?,
    ))
}

#[cfg(not(windows))]
fn open_store(_paths: &Paths) -> anyhow::Result<AccountStore<MemorySecretStore>> {
    anyhow::bail!("account storage is only implemented on Windows")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let paths = Paths::new(cli.home.unwrap_or_else(Paths::default_root));
    match cli.cmd {
        Cmd::Versions { kind, limit } => {
            let manifest = fetch_manifest(&http_client(), &paths, MANIFEST_URL).await?;
            for v in manifest
                .versions
                .iter()
                .filter(|v| kind.matches(v.kind))
                .take(limit)
            {
                println!("{:<28} {:?}", v.id, v.kind);
            }
        }
        Cmd::Launch {
            version,
            instance,
            account,
            offline,
            java,
            memory,
            loader,
            loader_version,
            dry_run,
        } => {
            let account = match offline {
                Some(name) => Account::offline(&name),
                None => {
                    let store = open_store(&paths)?;
                    let auth = client_id()
                        .ok()
                        .map(|id| MicrosoftAuth::new(id, AuthEndpoints::default()));
                    launch_account(&store, auth.as_ref(), account.as_deref(), unix_now()).await?
                }
            };
            if !dry_run {
                ensure_launch_allowed(account.kind(), DEV_OFFLINE)?;
            }
            let opts = LaunchOptions {
                version,
                instance,
                account,
                java,
                memory_mb: memory,
                loader: loader.map(|kind| LoaderRequest {
                    kind,
                    version: loader_version.clone(),
                }),
            };
            let prepared = prepare(&paths, &opts, &Endpoints::default()).await?;
            if let Some(resolved) = prepared
                .loader
                .as_ref()
                .filter(|_| prepared.loader_list_from_cache)
            {
                eprintln!(
                    "note: the {} version service is unreachable; picked {} from the last saved list",
                    resolved.kind, resolved.version.id
                );
            }
            if let Some(resolved) = prepared
                .loader
                .as_ref()
                .filter(|r| note_applies(loader_version.as_deref(), r))
            {
                // The note is cosmetic: a failed lookup skips it silently.
                if let Ok(game) = resolve_game(&paths, &opts.version).await {
                    if let Some(note) = unstable_note(loader_version.as_deref(), resolved, &game) {
                        eprintln!("{note}");
                    }
                }
            }
            if dry_run {
                println!("{}", prepared.java.display());
                for arg in &prepared.redacted_args() {
                    println!("  {arg}");
                }
                return Ok(());
            }
            let code = run(&prepared).await?;
            std::process::exit(code);
        }
        Cmd::Loaders {
            loader,
            version,
            limit,
        } => {
            let game = resolve_game(&paths, &version).await?;
            let versions =
                list_versions(&http_client(), &LoaderEndpoints::default(), loader, &game).await?;
            let default = pick_version(&versions, None, loader, &game)?;
            for line in loader_lines(&versions, limit as usize, &default.id) {
                println!("{line}");
            }
        }
        Cmd::Login => {
            let auth = MicrosoftAuth::new(client_id()?, AuthEndpoints::default());
            let store = open_store(&paths)?;
            let first = store.load()?.accounts.is_empty();
            let code = auth.start_device_login().await?;
            println!(
                "To sign in, open {} and enter the code {}",
                code.verification_uri, code.user_code
            );
            println!("Waiting for sign-in...");
            let tokens = auth.poll_device_login(&code).await?;
            let session = auth.complete_login(tokens, unix_now()).await?;
            let saved = store.save_session(&session)?;
            let suffix = if first { " (default account)" } else { "" };
            println!("Signed in as {}{suffix}", saved.name);
        }
        Cmd::Accounts { action: None } => {
            let file = load_accounts_file(&paths.accounts_file())?;
            if file.accounts.is_empty() {
                println!("No accounts. Run \"hengst login\" to add one.");
            }
            for a in &file.accounts {
                let marker = if file.default.as_deref() == Some(a.uuid.as_str()) {
                    '*'
                } else {
                    ' '
                };
                println!("{marker} {:<16} {}", a.name, a.uuid);
            }
        }
        Cmd::Accounts {
            action: Some(AccountsCmd::Use { name }),
        } => {
            let a = open_store(&paths)?.set_default(&name)?;
            println!("{} is now the default account", a.name);
        }
        Cmd::Logout { name } => {
            let a = open_store(&paths)?.remove(&name)?;
            println!("Removed {}", a.name);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn launch_account_and_offline_conflict() {
        assert!(Cli::try_parse_from([
            "hengst",
            "launch",
            "1.8.9",
            "--account",
            "a",
            "--offline",
            "b"
        ])
        .is_err());
        assert!(Cli::try_parse_from(["hengst", "accounts", "use", "Steve"]).is_ok());
        assert!(Cli::try_parse_from(["hengst", "logout", "Steve"]).is_ok());
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_loader_options() {
        let cli = Cli::try_parse_from([
            "hengst",
            "launch",
            "1.20.1",
            "--loader",
            "forge",
            "--loader-version",
            "47.4.28",
        ])
        .unwrap();
        match cli.cmd {
            Cmd::Launch {
                version,
                loader,
                loader_version,
                ..
            } => {
                assert_eq!(version, "1.20.1");
                assert_eq!(loader, Some(LoaderKind::Forge));
                assert_eq!(loader_version.as_deref(), Some("47.4.28"));
            }
            _ => panic!("expected launch"),
        }
    }

    #[test]
    fn loader_defaults_to_none() {
        let cli = Cli::try_parse_from(["hengst", "launch", "1.20.1"]).unwrap();
        match cli.cmd {
            Cmd::Launch {
                loader,
                loader_version,
                ..
            } => {
                assert_eq!(loader, None);
                assert_eq!(loader_version, None);
            }
            _ => panic!("expected launch"),
        }
    }

    #[test]
    fn loader_version_needs_loader() {
        assert!(
            Cli::try_parse_from(["hengst", "launch", "1.20.1", "--loader-version", "1"]).is_err()
        );
    }

    #[test]
    fn rejects_unknown_loader() {
        let err = Cli::try_parse_from(["hengst", "launch", "1.8.9", "--loader", "ornithe"])
            .err()
            .expect("unknown loader must be rejected")
            .to_string();
        assert!(
            err.contains("unknown loader 'ornithe' (expected fabric, quilt, legacy-fabric, neoforge or forge)"),
            "{err}"
        );
    }

    #[test]
    fn parses_loaders_command() {
        let cli =
            Cli::try_parse_from(["hengst", "loaders", "quilt", "1.21.11", "--limit", "5"]).unwrap();
        match cli.cmd {
            Cmd::Loaders {
                loader,
                version,
                limit,
            } => {
                assert_eq!(loader, LoaderKind::Quilt);
                assert_eq!(version, "1.21.11");
                assert_eq!(limit, 5u64);
            }
            _ => panic!("expected loaders"),
        }
        let cli = Cli::try_parse_from(["hengst", "loaders", "fabric", "1.21.11"]).unwrap();
        match cli.cmd {
            Cmd::Loaders { limit, .. } => assert_eq!(limit, 20u64),
            _ => panic!("expected loaders"),
        }
    }

    fn resolved(id: &str, stable: bool) -> ResolvedLoader {
        ResolvedLoader {
            kind: LoaderKind::Quilt,
            version: LoaderVersion {
                id: id.into(),
                maven: id.into(),
                stable,
            },
        }
    }

    #[test]
    fn notes_only_unrequested_unstable_picks() {
        assert_eq!(
            unstable_note(None, &resolved("0.31.0-beta.6", false), "1.21.11").as_deref(),
            Some("note: no stable quilt release for Minecraft 1.21.11, using 0.31.0-beta.6")
        );
        assert_eq!(
            unstable_note(None, &resolved("0.30.2", true), "1.21.11"),
            None
        );
        assert_eq!(
            unstable_note(
                Some("0.31.0-beta.6"),
                &resolved("0.31.0-beta.6", false),
                "1.21.11"
            ),
            None
        );
    }

    #[test]
    fn note_lookup_only_for_unrequested_unstable_picks() {
        assert!(note_applies(None, &resolved("0.31.0-beta.6", false)));
        assert!(!note_applies(None, &resolved("0.30.2", true)));
        assert!(!note_applies(Some("1"), &resolved("1", false)));
    }

    #[test]
    fn rejects_zero_limit() {
        assert!(
            Cli::try_parse_from(["hengst", "loaders", "fabric", "1.21.11", "--limit", "0"])
                .is_err()
        );
    }

    #[test]
    fn formats_loader_lines() {
        let v = |id: &str, stable| LoaderVersion {
            id: id.into(),
            maven: id.into(),
            stable,
        };
        let list = [v("0.31.0-beta.6", false), v("0.30.2", true)];
        let lines = loader_lines(&list, 20, "0.30.2");
        assert_eq!(lines[0], format!("{:<24} unstable", "0.31.0-beta.6"));
        assert_eq!(lines[1], format!("{:<24} stable (default)", "0.30.2"));
        assert_eq!(loader_lines(&list, 1, "0.30.2").len(), 1);
    }
}
