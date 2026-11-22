use crate::dto::error::ErrorDto;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Id of the offline development account (`dev-offline` builds only).
pub const DEV_ACCOUNT_ID: &str = "offline-dev";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum AccountKindDto {
    Microsoft,
    OfflineDev,
}

/// What the account chip and menu show. No token, no Microsoft identifier beyond the player UUID.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AccountDto {
    /// The player UUID of a Microsoft account (32 hex digits), `"offline-dev"` for the
    /// development account. Also the argument of `account_select` and `account_remove`.
    pub id: String,
    pub name: String,
    pub kind: AccountKindDto,
    pub selected: bool,
    /// The saved login stopped working: the user has to sign in again.
    pub sign_in_required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AccountsDto {
    pub accounts: Vec<AccountDto>,
    /// `false` when this build has no Microsoft client id.
    pub can_sign_in: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum LoginMethod {
    Browser,
    DeviceCode,
}

/// The (translated) texts of the page the browser shows after the redirect. The core escapes them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LoginPagesDto {
    pub success_title: String,
    pub success_body: String,
    pub failure_title: String,
    pub failure_body: String,
}

/// Names one running operation. Unique within one run of the app, never reused. A newtype struct
/// is a plain number in JSON.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(type = "number")]
pub struct OperationId(pub u64);

/// The progress of one sign-in, on the channel of that sign-in only. `Started` is always first;
/// exactly one of `SignedIn`, `Failed` and `Cancelled` ends the sequence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "event", content = "data", rename_all = "camelCase")]
pub enum LoginEvent {
    Started {
        op: OperationId,
    },
    /// The system browser was asked to open the sign-in page (the address stays in Rust).
    BrowserOpened,
    /// What the user has to do on the device-code page. The device code itself stays in Rust.
    #[serde(rename_all = "camelCase")]
    DeviceCode {
        user_code: String,
        verification_uri: String,
        /// Unix seconds.
        #[ts(type = "number")]
        expires_at: u64,
    },
    /// The Microsoft step is done; the Xbox and Minecraft steps follow.
    Completing,
    SignedIn {
        account: AccountDto,
    },
    Failed {
        error: ErrorDto,
    },
    Cancelled,
}
