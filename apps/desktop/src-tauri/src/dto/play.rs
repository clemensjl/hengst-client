use crate::dto::accounts::OperationId;
use crate::dto::error::ErrorDto;
use crate::dto::instances::LoaderKindDto;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Names one game run. Unique within one run of the app, never reused; the live log (Task 18)
/// tags its lines with it. A newtype struct is a plain number in JSON.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(type = "number")]
pub struct RunId(pub u64);

/// What a batch of downloads is for (the core's `Transfer`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum TransferDto {
    /// Libraries, the client jar and, with a loader, the loader's libraries.
    GameFiles,
    Assets,
    /// The managed Java runtime.
    Java,
}

/// The progress of one game start, on the channel of that start only. `Preparing` is always
/// first; exactly one of `Started`, `Failed` and `Cancelled` ends the sequence. The middle events
/// are the core's `Progress`, one to one. What happens after the start (running, stopping, the
/// exit) is the global `AppEvent::GameState`, so it reaches the window after a reload too.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "event", content = "data", rename_all = "camelCase")]
pub enum PlayEvent {
    Preparing {
        op: OperationId,
    },
    /// A loader build is being picked.
    ResolvingLoader {
        kind: LoaderKindDto,
        game: String,
    },
    /// The Forge or NeoForge installer is being fetched.
    DownloadingInstaller {
        kind: LoaderKindDto,
    },
    /// `count` missing files are being fetched.
    Downloading {
        what: TransferDto,
        count: u32,
    },
    Downloaded {
        what: TransferDto,
    },
    /// The installer's steps are about to run (first start of a Forge or NeoForge build).
    InstallStarted {
        kind: LoaderKindDto,
        version: String,
        game: String,
    },
    /// Where the output of the installer steps goes: relative to the data folder, with `/`
    /// (`logs/loader-install-forge-<id>.log`), never the absolute path, which names the Windows
    /// user. Opening it is Rust's job (Task 16 resolves it under the data folder's log folders).
    InstallLog {
        path: String,
    },
    /// Step `index` of `total` (1-based) starts; `name` is the step's artifact id.
    InstallStep {
        index: u32,
        total: u32,
        name: String,
    },
    VerifyingInstall,
    #[serde(rename_all = "camelCase")]
    InstallFinished {
        #[ts(type = "number")]
        elapsed_ms: u64,
    },
    /// The recorded install is still valid; the installer steps are skipped.
    InstallReused,
    /// A fact in plain English (a lock wait, say), paths shortened like error details.
    Notice {
        message: String,
    },
    /// The game process runs; from here on `AppEvent::GameState` tells the rest.
    #[serde(rename_all = "camelCase")]
    Started {
        run_id: RunId,
    },
    Failed {
        error: ErrorDto,
    },
    Cancelled,
}

/// How the last game run of this app run ended.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LastExit {
    pub instance: String,
    pub run_id: RunId,
    /// The java process's exit code; -1 when it is not known.
    pub code: i32,
    /// The app asked the game to stop (`game_stop` reached a window, or `game_force_stop`): the
    /// exit is never "unexpected", whatever the code.
    pub stop_requested: bool,
}

/// The one game of the app. `Idle` keeps the last exit until the next start begins.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "state", content = "data", rename_all = "camelCase")]
pub enum GameStateDto {
    #[serde(rename_all = "camelCase")]
    Idle {
        last_exit: Option<LastExit>,
    },
    Preparing {
        instance: String,
    },
    #[serde(rename_all = "camelCase")]
    Running {
        instance: String,
        run_id: RunId,
        /// Unix milliseconds of the start.
        #[ts(type = "number")]
        since: u64,
    },
    /// A stop was sent and the game has not exited yet. Still names the run: the instance stays
    /// "running" (not deletable) until the exit.
    #[serde(rename_all = "camelCase")]
    Stopping {
        instance: String,
        run_id: RunId,
    },
}

/// The answer of `game_stop`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum StopResult {
    /// The close request went to at least one window of the game; the state is `Stopping`.
    Asked,
    /// The game runs but has no window to ask (still loading, or hung before its window): nothing
    /// was sent and the state stays `Running`. Only `game_force_stop` ends it now.
    NoWindow,
    /// No game runs (also while one is still being prepared: `play_cancel` ends that).
    NotRunning,
}
