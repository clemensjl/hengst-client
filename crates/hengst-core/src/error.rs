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
    #[error("no {loader} versions available for Minecraft {game}{}", hint_suffix(.hint))]
    NoLoaderVersions {
        loader: String,
        game: String,
        /// A likely reason the launcher can tell without asking anything.
        hint: Option<crate::loaders::LoaderHint>,
    },
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
    #[error("{path} is missing, damaged or cannot be verified, and {url} cannot be reached: the launcher is offline or the server is unreachable ({source})")]
    Offline {
        path: PathBuf,
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error(
        "{url} sends more than {limit} bytes, which is more than the launcher accepts from it"
    )]
    TooLarge { url: String, limit: usize },
    #[error("checksum file {url} does not contain a SHA-1")]
    InvalidChecksumFile { url: String },
    #[error("no checksum available for {url}; refusing to download it unverified")]
    NoChecksum { url: String },
    #[error("loader profile inherits from {found}, expected {expected}")]
    ParentMismatch { expected: String, found: String },
    #[error("{version} uses the launch argument placeholder ${{{key}}}, which the launcher does not know")]
    UnknownPlaceholder { version: String, key: String },
    #[error("invalid loader installer: {0}")]
    Installer(String),
    #[error("cannot replace {dir}: this game version appears to be running already, close it and try again")]
    NativesInUse { dir: PathBuf },
    #[error("loader install failed: {0}")]
    InstallFailed(String),
    #[error("loader installer step {jar} failed with exit code {code}, see {log}")]
    Processor {
        jar: String,
        code: i32,
        log: PathBuf,
    },
    #[error("another install of {label} is still running after {waited_secs} s (lock {}); try again later", .lock.display())]
    LockTimeout {
        label: String,
        waited_secs: u64,
        lock: PathBuf,
    },
    #[error("the operation was cancelled")]
    Cancelled,
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

fn hint_suffix(hint: &Option<crate::loaders::LoaderHint>) -> String {
    hint.map(|h| format!(" ({h})")).unwrap_or_default()
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_timeout_and_cancel_keep_their_texts() {
        let e = Error::LockTimeout {
            label: "forge for Minecraft 1.20.1".into(),
            waited_secs: 600,
            lock: PathBuf::from("C:/h/tmp/x.lock"),
        };
        assert_eq!(
            e.to_string(),
            "another install of forge for Minecraft 1.20.1 is still running after 600 s (lock C:/h/tmp/x.lock); try again later"
        );
        assert_eq!(Error::Cancelled.to_string(), "the operation was cancelled");
    }
}
