use crate::args::{build_args, Account, LaunchContext};
use crate::assets::{install_assets, AssetsLayout, RESOURCES_URL};
use crate::download::{http_client, Download, Downloader};
use crate::java::{component_for, ensure_runtime, RUNTIME_INDEX_URL};
use crate::libraries;
use crate::manifest::{fetch_manifest, MANIFEST_URL};
use crate::paths::{validate_id, Paths};
use crate::rules::Env;
use crate::version::fetch_version;
use crate::{Error, Result};
use std::path::PathBuf;

/// Remote endpoints; overridable for tests and mirrors.
#[derive(Clone, Debug)]
pub struct Endpoints {
    pub manifest: String,
    pub resources: String,
    pub runtime_index: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            manifest: MANIFEST_URL.into(),
            resources: RESOURCES_URL.into(),
            runtime_index: RUNTIME_INDEX_URL.into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LaunchOptions {
    pub version: String,
    /// Instance folder name; defaults to the version id.
    pub instance: Option<String>,
    pub account: Account,
    /// Use this java executable instead of a managed runtime.
    pub java: Option<PathBuf>,
    pub memory_mb: u32,
}

#[derive(Clone, Debug)]
pub struct Prepared {
    pub java: PathBuf,
    pub args: Vec<String>,
    pub game_dir: PathBuf,
    pub version_id: String,
}

/// Downloads everything a version needs and builds the command line.
pub async fn prepare(paths: &Paths, opts: &LaunchOptions, ep: &Endpoints) -> Result<Prepared> {
    let dl = Downloader::new(http_client());
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
    let game_dir = paths.instance(opts.instance.as_deref().unwrap_or(&v.id));
    tokio::fs::create_dir_all(&game_dir).await?;

    let libs = libraries::resolve(&v.libraries, &env, paths)?;
    let mut downloads = libs.downloads.clone();
    let mut classpath = libs.classpath.clone();
    if let Some(client) = v.downloads.as_ref().and_then(|d| d.client.as_ref()) {
        let jar = paths.client_jar(&v.id);
        downloads.push(Download {
            url: client.url.clone(),
            path: jar.clone(),
            sha1: Some(client.sha1.clone()),
        });
        classpath.push(jar);
    }
    let log_config = match v.logging.as_ref().and_then(|l| l.client.as_ref()) {
        Some(c) => {
            validate_id(&c.file.id)?;
            let p = paths.log_config(&c.file.id);
            downloads.push(Download {
                url: c.file.url.clone(),
                path: p.clone(),
                sha1: Some(c.file.sha1.clone()),
            });
            Some(p)
        }
        None => None,
    };
    dl.fetch_all(downloads).await?;

    let assets = match &v.asset_index {
        Some(index) => install_assets(&dl, paths, index, &game_dir, &ep.resources).await?,
        None => AssetsLayout {
            index_id: "legacy".into(),
            assets_root: paths.assets(),
            game_assets: paths.assets(),
        },
    };

    let natives_dir = game_dir.join(".natives").join(&v.id);
    if natives_dir.exists() {
        tokio::fs::remove_dir_all(&natives_dir).await?;
    }
    libraries::extract_natives(&libs.natives, &natives_dir)?;

    let java = match &opts.java {
        Some(j) => j.clone(),
        None => ensure_runtime(&dl, paths, &env, &component_for(&v), &ep.runtime_index).await?,
    };

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
        args: build_args(&v, &ctx),
        game_dir,
        version_id: v.id.clone(),
    })
}

/// Starts the game and waits for it to exit. Output goes to the current console.
pub async fn run(p: &Prepared) -> Result<i32> {
    let status = tokio::process::Command::new(&p.java)
        .args(&p.args)
        .current_dir(&p.game_dir)
        .status()
        .await?;
    Ok(status.code().unwrap_or(-1))
}
