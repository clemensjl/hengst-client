use crate::settings::{Language, Settings, SettingsNotice, Theme};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A change to a value that can also be removed: keep it, set it, or clear it are three different
/// requests, so no nested `Option<Option<_>>` is needed.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "op", content = "value", rename_all = "camelCase")]
pub enum Patch<T> {
    #[default]
    Keep,
    Set(T),
    Clear,
}

/// What the settings view may change. `dev_account`, `selected_instance`, `format` and the data
/// folder are deliberately missing: unknown fields are refused.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(optional_fields)]
pub struct SettingsPatch {
    #[serde(default)]
    pub language: Patch<Language>,
    #[serde(default)]
    pub theme: Option<Theme>,
    #[serde(default)]
    pub memory_mb: Option<u32>,
    #[serde(default)]
    pub java_override: Patch<String>,
    #[serde(default)]
    pub minimize_on_launch: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    pub language: Option<Language>,
    pub theme: Theme,
    pub memory_mb: u32,
    pub java_override: Option<String>,
    pub minimize_on_launch: bool,
    pub selected_instance: Option<String>,
    pub dev_account: bool,
    /// Set in the first answer after a start that had to reset the file, never again.
    pub notice: Option<SettingsNotice>,
}

impl SettingsDto {
    pub fn new(s: Settings, notice: Option<SettingsNotice>) -> SettingsDto {
        SettingsDto {
            language: s.language,
            theme: s.theme,
            memory_mb: s.memory_mb,
            java_override: s.java_override.map(|p| p.display().to_string()),
            minimize_on_launch: s.minimize_on_launch,
            selected_instance: s.selected_instance,
            dev_account: s.dev_account,
            notice,
        }
    }
}
