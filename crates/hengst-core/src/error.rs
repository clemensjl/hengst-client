use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("request to {url} failed: {source}")]
    Http {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("request to {url} returned HTTP {status}")]
    Status { url: String, status: u16 },
    #[error("checksum mismatch for {path}: expected {expected}, got {actual}")]
    Checksum {
        path: PathBuf,
        expected: String,
        actual: String,
    },
    #[error("unsafe path from metadata: {0}")]
    UnsafePath(String),
    #[error("unknown Minecraft version: {0}")]
    UnknownVersion(String),
    #[error("no Java runtime '{component}' available for {platform}")]
    NoRuntime { component: String, platform: String },
    #[error("offline accounts can only start the game in development builds (cargo feature dev-offline)")]
    OfflineLaunchDisabled,
    #[error("no {loader} versions available for Minecraft {game}")]
    NoLoaderVersions { loader: String, game: String },
    #[error("{loader} {version} is not available for Minecraft {game} (see `hengst loaders {loader} {game}`)")]
    UnknownLoaderVersion {
        loader: String,
        game: String,
        version: String,
    },
    #[error("{loader} for Minecraft {game} is not supported ({reason})")]
    LoaderUnsupported {
        loader: String,
        game: String,
        reason: String,
    },
    #[error("{path} is missing or does not match its checksum and cannot be downloaded from {url}: the launcher is offline or the server is unreachable")]
    Offline { path: PathBuf, url: String },
    #[error("no checksum available for {url}; refusing to download it unverified")]
    NoChecksum { url: String },
    #[error("loader profile inherits from {found}, expected {expected}")]
    ParentMismatch { expected: String, found: String },
    #[error("invalid loader installer: {0}")]
    Installer(String),
    #[error("loader installer step {jar} failed with exit code {code}, see {log}")]
    Processor {
        jar: String,
        code: i32,
        log: PathBuf,
    },
    #[error("Windows Credential Manager error: {0}")]
    CredentialStore(String),
    #[error(transparent)]
    Auth(#[from] crate::auth::AuthError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
}

pub type Result<T> = std::result::Result<T, Error>;
