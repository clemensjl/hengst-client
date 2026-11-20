use clap::{Parser, Subcommand, ValueEnum};
use hengst_core::args::Account;
use hengst_core::auth::flow::launch_account;
use hengst_core::auth::store::AccountStore;
#[cfg(windows)]
use hengst_core::auth::store::KeyringSecretStore;
#[cfg(not(windows))]
use hengst_core::auth::store::MemorySecretStore;
use hengst_core::auth::{client_id, unix_now, AuthEndpoints, MicrosoftAuth};
use hengst_core::download::http_client;
use hengst_core::launch::{
    ensure_launch_allowed, prepare, run, Endpoints, LaunchOptions, DEV_OFFLINE,
};
use hengst_core::manifest::{fetch_manifest, VersionKind, MANIFEST_URL};
use hengst_core::paths::Paths;
use std::path::PathBuf;

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
        /// Download everything and print the command line instead of starting
        #[arg(long)]
        dry_run: bool,
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
            };
            let prepared = prepare(&paths, &opts, &Endpoints::default()).await?;
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
            let file = open_store(&paths)?.load()?;
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
}
