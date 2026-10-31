use anyhow::bail;
use clap::{Parser, Subcommand, ValueEnum};
use hengst_core::args::Account;
use hengst_core::download::http_client;
use hengst_core::launch::{prepare, run, Endpoints, LaunchOptions};
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
        /// Offline development name (Microsoft login arrives in phase 2)
        #[arg(long, default_value = "Dev")]
        username: String,
        #[arg(long)]
        java: Option<PathBuf>,
        #[arg(long, default_value_t = 2048)]
        memory: u32,
        /// Download everything and print the command line instead of starting
        #[arg(long)]
        dry_run: bool,
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
            username,
            java,
            memory,
            dry_run,
        } => {
            let opts = LaunchOptions {
                version,
                instance,
                account: Account::offline(&username),
                java,
                memory_mb: memory,
            };
            let prepared = prepare(&paths, &opts, &Endpoints::default()).await?;
            if dry_run {
                println!("{}", prepared.java.display());
                for arg in &prepared.args {
                    println!("  {arg}");
                }
                return Ok(());
            }
            if !cfg!(feature = "dev-offline") {
                bail!("starting the game requires Microsoft login (phase 2); build with --features dev-offline for local testing");
            }
            let code = run(&prepared).await?;
            std::process::exit(code);
        }
    }
    Ok(())
}
