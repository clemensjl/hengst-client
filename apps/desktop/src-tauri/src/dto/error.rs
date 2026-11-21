use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

/// Task 4 completes this list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    Internal,
    Network,
}

/// Task 4 completes this list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ErrorAction {
    Retry,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ErrorDto {
    pub code: ErrorCode,
    pub params: BTreeMap<String, String>,
    pub action: Option<ErrorAction>,
    /// English core text for the "Details" disclosure, without URL query parts.
    pub detail: String,
}
