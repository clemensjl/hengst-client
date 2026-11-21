//! Progress events a launch preparation reports to its caller. The core never prints: the CLI
//! turns each event into a line, the desktop app will show them in its window.
//!
//! Events carry no secret and no account data. The only absolute path is the install log.

use crate::loaders::LoaderKind;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// What a batch of downloads is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transfer {
    /// Libraries, the client jar and, with a loader, the loader's libraries.
    GameFiles,
    Assets,
    /// The managed Java runtime.
    Java,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Progress {
    /// A loader build is being picked (version list, profile).
    ResolvingLoader {
        kind: LoaderKind,
        game: String,
    },
    /// The Forge or NeoForge installer is not on disk yet and is being fetched.
    DownloadingInstaller {
        kind: LoaderKind,
    },
    /// `count` files are missing and are being fetched. Not sent when nothing is missing.
    Downloading {
        what: Transfer,
        count: usize,
    },
    /// The batch announced by `Downloading` is complete.
    Downloaded {
        what: Transfer,
    },
    /// The installer's steps are about to run (first start of a Forge or NeoForge build).
    InstallStarted {
        kind: LoaderKind,
        version: String,
        game: String,
    },
    /// Where the output of the installer steps goes.
    InstallLog {
        path: PathBuf,
    },
    /// Step `index` of `total` (1-based) starts; `name` is the step's artifact id.
    InstallStep {
        index: usize,
        total: usize,
        name: String,
    },
    /// The produced files are being checked and the install state recorded.
    VerifyingInstall,
    InstallFinished {
        elapsed: Duration,
    },
    /// The recorded install is still valid; the installer steps are skipped.
    InstallReused,
    /// A fact the user should know, in plain English (for example a lock wait).
    Notice {
        message: String,
    },
}

/// Receives events. Called from the preparing task and from a blocking worker thread.
pub type ProgressSink = Arc<dyn Fn(Progress) + Send + Sync>;

/// A sink that drops every event.
pub fn silent() -> ProgressSink {
    Arc::new(|_| {})
}
