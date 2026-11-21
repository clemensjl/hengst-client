//! Serializable types the web view sees. Core types never cross the IPC boundary.

pub mod app;
pub mod error;
pub mod settings;
pub mod versions;

use ts_rs::{Config, ExportError, TS};

/// Every type the TypeScript side gets, with the function that renders it (and its
/// dependencies). `tests/bindings.rs` renders all of them and compares with the checked-in files.
pub type Export = fn(&Config) -> Result<(), ExportError>;

pub const ALL_TYPES: &[Export] = &[
    app::AppInfo::export_all,
    versions::VersionFilter::export_all,
    versions::GameVersionKind::export_all,
    versions::GameVersionDto::export_all,
    versions::GameVersionsDto::export_all,
    error::ErrorCode::export_all,
    error::ErrorAction::export_all,
    error::UrlTarget::export_all,
    error::ErrorDto::export_all,
    crate::events::DataScope::export_all,
    crate::events::NoticeKind::export_all,
    crate::events::AppEvent::export_all,
    crate::settings::Language::export_all,
    crate::settings::Theme::export_all,
    crate::settings::SettingsNotice::export_all,
    settings::SettingsDto::export_all,
    settings::SettingsPatch::export_all,
];
