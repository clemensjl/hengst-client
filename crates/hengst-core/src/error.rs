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
    #[error("unknown Minecraft version: {0}")]
    UnknownVersion(String),
    #[error("no Java runtime '{component}' available for {platform}")]
    NoRuntime { component: String, platform: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
}

pub type Result<T> = std::result::Result<T, Error>;
