use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

/// What went wrong, as a stable code the web view translates. Later tasks add the codes of their
/// own commands (browser login, instances, busy states).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    Internal,
    /// A value the user or the web view sent is not acceptable; `params.field` names it.
    InvalidInput,
    Network,
    Offline,
    ServerError,
    ChecksumMismatch,
    UnsafeData,
    UnknownVersion,
    NoJavaRuntime,
    OfflineAccountBlocked,
    NoLoaderVersions,
    UnknownLoaderVersion,
    LoaderUnsupported,
    InstallerInvalid,
    InstallFailed,
    InstallStepFailed,
    NativesInUse,
    LockTimeout,
    CredentialStore,
    /// No Microsoft client id in this build. It cannot happen in a build of this repository (the id is
    /// built in and an empty override falls back to it); the state exists for forks that remove
    /// the id. DESIGN.md 11.6 describes the "build without client id" block.
    MissingClientId,
    AppMisconfigured,
    AppNotApproved,
    LoginDeclined,
    LoginExpired,
    DeviceCodeExpired,
    BrowserLoginTimedOut,
    /// The core could not listen on the loopback address; the sign-in dialog offers the code instead.
    LoopbackUnavailable,
    /// `params.code` is the sanitized `error` of the authorization response.
    BrowserLoginFailed,
    NoXboxAccount,
    XboxRegionBlocked,
    XboxAdultVerification,
    XboxChildAccount,
    XboxRefused,
    GameNotOwned,
    NoProfile,
    AccountNotFound,
    NoAccount,
    Cancelled,
    FileSystem,
    /// The system refused to open a link in the browser (`open_url`).
    OpenFailed,
    /// No instance with this id (also: an id that is not a folder name at all); `params.id`.
    InstanceNotFound,
    /// Every folder name for this version is taken; `params.id` is the first one.
    InstanceExists,
    /// Saved by a newer build; `params.id` and `params.format`.
    InstanceFormatTooNew,
    InvalidInstanceName,
    /// The instance's file is damaged or cannot be read; `params.id`.
    InstanceUnreadable,
    /// The instance's folder is a link (junction or symlink), which the app does not follow;
    /// `params.id`.
    InstanceLinked,
    /// A failed move to the Recycle Bin took part of the instance with it; `params.id`.
    RecycleBinPartial,
    /// The Recycle Bin did not take the folder (too large for it, a drive without one, the
    /// Windows setting "Don't move files to the Recycle Bin", the folder in use). Nothing was
    /// deleted; `params.id` and `params.reason` (`tooLarge`, `binOff`, `noBin`, `inUse`, `other`, `unchecked`).
    RecycleBinFailed,
    /// The folder is gone, but Windows did not report putting it into the Recycle Bin and the app
    /// did not find it there: it may have been removed permanently. `params.id`.
    RecycleBinUnconfirmed,
    /// The Recycle Bin call did not report a result (it timed out or stopped unexpectedly). The
    /// folder may be gone, partly moved or untouched; nothing is claimed. `params.id`.
    RecycleBinUnknown,
    /// An earlier Recycle Bin call has not ended; this call started nothing. `params.id`.
    RecycleBinPending,
    /// The instance is being prepared or runs; it cannot be deleted now. `params.id`.
    InstanceRunning,
    /// Another create or delete of an instance is in progress, or (`play`) a game is already being
    /// prepared or running: one game at a time.
    Busy,
}

/// A page the web view may ask the app to open. The addresses are fixed in Rust (`open::resolve_url`);
/// JavaScript never supplies a URL.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum UrlTarget {
    Repository,
    SignInHelp,
    XboxSignup,
    MinecraftProfile,
    DeviceLogin,
    BuildHelp,
}

/// The one thing the user can do about an error, besides trying again on their own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ErrorAction {
    Retry,
    SignIn,
    OpenUrl { target: UrlTarget },
    OpenSettings,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ErrorDto {
    pub code: ErrorCode,
    pub params: BTreeMap<String, String>,
    pub action: Option<ErrorAction>,
    /// English core text for the "Details" disclosure. No tokens, authorization headers or URL
    /// query parts; local paths may appear.
    pub detail: String,
}
