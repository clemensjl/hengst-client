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
    ensure_launch_allowed, prepare_with_progress, resolve, run, Endpoints, LaunchOptions,
    Resolution, DEV_OFFLINE,
};
use hengst_core::loaders::{
    list_versions, pick_version, LoaderEndpoints, LoaderKind, LoaderRequest, LoaderVersion,
    ResolvedLoader,
};
use hengst_core::manifest::{fetch_manifest, VersionKind, MANIFEST_URL};
use hengst_core::paths::Paths;
use hengst_core::process::SystemRunner;
use hengst_core::progress::{Progress, Transfer};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

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
        /// Download everything and print the command line instead of starting (Forge and
        /// NeoForge also run the installer's processors)
        #[arg(long)]
        dry_run: bool,
        /// Only resolve the game version and the loader build and print them as one JSON line,
        /// e.g. {"game":"1.21.11","loader":{"kind":"fabric","version":"0.17.3","stable":true}}
        /// ("loader":null without --loader). Fetches only the version manifest and the loader's
        /// version list: no downloads, no account, no Java. Exit code 0; 1 on errors, 2 on invalid arguments.
        #[arg(long, conflicts_with_all = ["account", "offline", "java", "memory", "instance", "dry_run"])]
        resolve_only: bool,
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

/// The stderr notes about a picked loader build: the saved list was used because the service is
/// unreachable, then the unstable fallback (only with the game id; `None` skips it).
fn loader_notes(
    requested: Option<&str>,
    resolved: &ResolvedLoader,
    from_cache: bool,
    game: Option<&str>,
) -> Vec<String> {
    let mut notes = Vec::new();
    if from_cache {
        notes.push(format!(
            "note: the {} version service is unreachable; picked {} from the last saved list",
            resolved.kind, resolved.version.id
        ));
    }
    if let Some(note) = game.and_then(|game| unstable_note(requested, resolved, game)) {
        notes.push(note);
    }
    notes.iter().map(|n| printable(n)).collect()
}

/// Text from a server (or a file) for the terminal: every control character (C0, DEL, C1)
/// becomes U+FFFD, so a version id or an error text cannot move the cursor, change the window
/// title or recolour the terminal.
fn printable(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { '\u{fffd}' } else { c })
        .collect()
}

/// What a failed command prints: the error and its causes, each made `printable` (error texts
/// embed remote ids, e.g. `UnsafePath`).
fn error_text(e: &anyhow::Error) -> String {
    let mut text = format!("Error: {}", printable(&e.to_string()));
    let causes: Vec<String> = e
        .chain()
        .skip(1)
        .map(|c| printable(&c.to_string()))
        .collect();
    if !causes.is_empty() {
        text.push_str("\n\nCaused by:");
        for (i, cause) in causes.iter().enumerate() {
            text.push_str(&format!("\n    {i}: {cause}"));
        }
    }
    text
}

/// One line per loader version (at most `limit`), the default marked.
fn loader_lines(versions: &[LoaderVersion], limit: usize, default_id: &str) -> Vec<String> {
    versions
        .iter()
        .take(limit)
        .map(|v| {
            let stable = if v.stable { "stable" } else { "unstable" };
            let marker = if v.id == default_id { " (default)" } else { "" };
            printable(&format!("{:<24} {stable}{marker}", v.id))
        })
        .collect()
}

/// The stdout line of `launch --resolve-only`: one JSON object, keys in this order.
fn resolution_line(r: &Resolution) -> String {
    let text = |s: &str| serde_json::Value::from(s).to_string();
    let loader = match &r.loader {
        Some(l) => format!(
            r#"{{"kind":{},"version":{},"stable":{}}}"#,
            text(l.kind.slug()),
            text(&l.version.id),
            l.version.stable
        ),
        None => "null".to_string(),
    };
    format!(r#"{{"game":{},"loader":{loader}}}"#, text(&r.game))
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
async fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    match cli_main(cli).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{}", error_text(&e));
            std::process::ExitCode::FAILURE
        }
    }
}

async fn cli_main(cli: Cli) -> anyhow::Result<()> {
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
                println!("{}", printable(&format!("{:<28} {:?}", v.id, v.kind)));
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
            resolve_only,
        } => {
            let request = loader.map(|kind| LoaderRequest {
                kind,
                version: loader_version.clone(),
            });
            if resolve_only {
                let r = resolve(&paths, &version, request.as_ref(), &Endpoints::default()).await?;
                if let Some(resolved) = r.loader.as_ref() {
                    let (requested, cached) = (loader_version.as_deref(), r.loader_list_from_cache);
                    for note in loader_notes(requested, resolved, cached, Some(&r.game)) {
                        eprintln!("{note}");
                    }
                }
                println!("{}", resolution_line(&r));
                return Ok(());
            }
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
                loader: request,
            };
            let sink = Arc::new(|event: Progress| {
                if let Some(line) = progress_line(&event) {
                    eprintln!("{}", printable(&line));
                }
            });
            let prepared = prepare_with_progress(
                &paths,
                &opts,
                &Endpoints::default(),
                Arc::new(SystemRunner),
                sink,
            )
            .await?;
            if let Some(resolved) = prepared.loader.as_ref() {
                let requested = loader_version.as_deref();
                // The unstable note is cosmetic: a failed lookup skips it silently.
                let game = if note_applies(requested, resolved) {
                    resolve_game(&paths, &opts.version).await.ok()
                } else {
                    None
                };
                let cached = prepared.loader_list_from_cache;
                for note in loader_notes(requested, resolved, cached, game.as_deref()) {
                    eprintln!("{note}");
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
                printable(&code.verification_uri),
                printable(&code.user_code)
            );
            println!("Waiting for sign-in...");
            let tokens = auth.poll_device_login(&code).await?;
            let session = auth.complete_login(tokens, unix_now()).await?;
            let saved = store.save_session(&session)?;
            let suffix = if first { " (default account)" } else { "" };
            println!("Signed in as {}{suffix}", printable(&saved.name));
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
                println!(
                    "{}",
                    printable(&format!("{marker} {:<16} {}", a.name, a.uuid))
                );
            }
        }
        Cmd::Accounts {
            action: Some(AccountsCmd::Use { name }),
        } => {
            let a = open_store(&paths)?.set_default(&name)?;
            println!("{} is now the default account", printable(&a.name));
        }
        Cmd::Logout { name } => {
            let a = open_store(&paths)?.remove(&name)?;
            println!("Removed {}", printable(&a.name));
        }
    }
    Ok(())
}

/// 3402 -> "3,402".
fn grouped(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The line printed to stderr for a progress event; `None` for events that stay silent.
fn progress_line(event: &Progress) -> Option<String> {
    Some(match event {
        Progress::ResolvingLoader { kind, game } => {
            format!("Resolving {} version for {game}", kind.display_name())
        }
        Progress::DownloadingInstaller { kind } => {
            format!("Downloading the {} installer", kind.display_name())
        }
        Progress::Downloading { what, count } => {
            let n = grouped(*count);
            match what {
                Transfer::GameFiles => format!("Downloading {n} game files"),
                Transfer::Assets => format!("Downloading {n} assets"),
                Transfer::Java => format!("Preparing the Java runtime ({n} files)"),
            }
        }
        Progress::Downloaded { .. } => return None,
        Progress::InstallStarted {
            kind,
            version,
            game,
        } => format!(
            "Installing {} {version} for {game} (runs the official installer locally)",
            kind.display_name()
        ),
        Progress::InstallLog { path } => format!("Install log: {}", path.display()),
        Progress::InstallStep { index, total, name } => {
            format!("Step {index} of {total}: {name}")
        }
        Progress::VerifyingInstall => "Verifying the install".to_string(),
        Progress::InstallFinished { elapsed } => {
            format!(
                "Install finished in {} s",
                (elapsed.as_millis() + 500) / 1000
            )
        }
        Progress::InstallReused => "Install is up to date, reusing it".to_string(),
        Progress::Notice { message } => message.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn progress_events_become_plain_lines() {
        use std::time::Duration;
        let line = |e: Progress| progress_line(&e);
        assert_eq!(
            line(Progress::ResolvingLoader {
                kind: LoaderKind::Forge,
                game: "1.20.1".into()
            })
            .as_deref(),
            Some("Resolving Forge version for 1.20.1")
        );
        assert_eq!(
            line(Progress::DownloadingInstaller {
                kind: LoaderKind::NeoForge
            })
            .as_deref(),
            Some("Downloading the NeoForge installer")
        );
        assert_eq!(
            line(Progress::Downloading {
                what: Transfer::GameFiles,
                count: 142
            })
            .as_deref(),
            Some("Downloading 142 game files")
        );
        assert_eq!(
            line(Progress::Downloading {
                what: Transfer::Assets,
                count: 3402
            })
            .as_deref(),
            Some("Downloading 3,402 assets")
        );
        assert_eq!(
            line(Progress::Downloading {
                what: Transfer::Java,
                count: 1234567
            })
            .as_deref(),
            Some("Preparing the Java runtime (1,234,567 files)")
        );
        assert_eq!(
            line(Progress::Downloaded {
                what: Transfer::Assets
            }),
            None
        );
        assert_eq!(
            line(Progress::InstallStarted {
                kind: LoaderKind::Forge,
                version: "47.4.28".into(),
                game: "1.20.1".into()
            })
            .as_deref(),
            Some("Installing Forge 47.4.28 for 1.20.1 (runs the official installer locally)")
        );
        assert_eq!(
            line(Progress::InstallLog {
                path: PathBuf::from("logs").join("install.log")
            })
            .as_deref(),
            Some(
                format!(
                    "Install log: {}",
                    PathBuf::from("logs").join("install.log").display()
                )
                .as_str()
            )
        );
        assert_eq!(
            line(Progress::InstallStep {
                index: 3,
                total: 6,
                name: "jarsplitter".into()
            })
            .as_deref(),
            Some("Step 3 of 6: jarsplitter")
        );
        assert_eq!(
            line(Progress::VerifyingInstall).as_deref(),
            Some("Verifying the install")
        );
        assert_eq!(
            line(Progress::InstallFinished {
                elapsed: Duration::from_millis(26_400)
            })
            .as_deref(),
            Some("Install finished in 26 s")
        );
        assert_eq!(
            line(Progress::InstallReused).as_deref(),
            Some("Install is up to date, reusing it")
        );
        assert_eq!(
            line(Progress::Notice {
                message: "another install is running".into()
            })
            .as_deref(),
            Some("another install is running")
        );
    }

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
    fn resolve_only_needs_no_account_and_refuses_launch_options() {
        let cli = Cli::try_parse_from([
            "hengst",
            "launch",
            "latest",
            "--loader",
            "fabric",
            "--resolve-only",
        ])
        .unwrap();
        match cli.cmd {
            Cmd::Launch { resolve_only, .. } => assert!(resolve_only),
            _ => panic!("expected launch"),
        }
        for extra in [
            &["--offline", "Dev"][..],
            &["--account", "Steve"],
            &["--dry-run"],
            &["--java", "java.exe"],
            &["--memory", "1024"],
            &["--instance", "x"],
        ] {
            let mut args = vec!["hengst", "launch", "1.21.11", "--resolve-only"];
            args.extend_from_slice(extra);
            assert!(Cli::try_parse_from(&args).is_err(), "{extra:?}");
        }
    }

    #[test]
    fn a_resolution_is_one_json_line() {
        let vanilla = Resolution {
            game: "1.21.11".into(),
            loader: None,
            loader_list_from_cache: false,
        };
        assert_eq!(
            resolution_line(&vanilla),
            r#"{"game":"1.21.11","loader":null}"#
        );
        let fabric = Resolution {
            game: "1.21.11".into(),
            loader: Some(ResolvedLoader {
                kind: LoaderKind::LegacyFabric,
                version: LoaderVersion {
                    id: "0.17.3".into(),
                    maven: "net.fabricmc:fabric-loader:0.17.3".into(),
                    stable: true,
                },
            }),
            loader_list_from_cache: true,
        };
        assert_eq!(
            resolution_line(&fabric),
            r#"{"game":"1.21.11","loader":{"kind":"legacy-fabric","version":"0.17.3","stable":true}}"#
        );
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
    fn printable_replaces_every_control_character() {
        assert_eq!(printable("1.21.11 Pre-Release 1"), "1.21.11 Pre-Release 1");
        assert_eq!(
            printable("a\u{1b}[31mb\u{7}c\u{7f}d\u{9b}2Je\nf\tg\u{0}"),
            "a\u{fffd}[31mb\u{fffd}c\u{fffd}d\u{fffd}2Je\u{fffd}f\u{fffd}g\u{fffd}"
        );
        // Not control characters: kept.
        assert_eq!(printable("Größe \u{202e}x"), "Größe \u{202e}x");
    }

    #[test]
    fn remote_text_reaches_the_terminal_without_control_characters() {
        let bad = "0.1\u{1b}]0;title\u{7}";
        let clean = |lines: &[String]| lines.iter().all(|l| !l.chars().any(char::is_control));
        let v = LoaderVersion {
            id: bad.into(),
            maven: bad.into(),
            stable: false,
        };
        assert!(clean(&loader_lines(std::slice::from_ref(&v), 5, bad)));
        assert!(clean(&loader_notes(
            None,
            &resolved(bad, false),
            true,
            Some("1\u{1b}c")
        )));
        let err = anyhow::Error::from(hengst_core::Error::UnsafePath(bad.into()));
        let text = error_text(&err.context("resolving the loader"));
        assert!(
            !text.replace('\n', "").chars().any(char::is_control),
            "{text:?}"
        );
        assert!(text.starts_with("Error: resolving the loader"), "{text}");
        assert!(text.contains("0.1\u{fffd}]0;title\u{fffd}"), "{text}");
    }

    #[test]
    fn loader_notes_are_the_cache_note_then_the_unstable_note() {
        let r = resolved("0.31.0-beta.6", false);
        assert_eq!(
            loader_notes(None, &r, true, Some("1.21.11")),
            [
                "note: the quilt version service is unreachable; picked 0.31.0-beta.6 from the last saved list",
                "note: no stable quilt release for Minecraft 1.21.11, using 0.31.0-beta.6",
            ]
        );
        // No game id (its lookup failed or was not needed): only the cache note.
        assert_eq!(loader_notes(None, &r, true, None).len(), 1);
        assert!(loader_notes(None, &resolved("0.30.2", true), false, Some("1.21.11")).is_empty());
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
