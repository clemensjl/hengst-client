//! Serializable types the web view sees. Core types never cross the IPC boundary.

pub mod accounts;
pub mod app;
pub mod error;
pub mod instances;
pub mod play;
pub mod settings;
pub mod versions;

use ts_rs::{Config, ExportError, TS};

/// Every type the TypeScript side gets, with the function that renders it (and its
/// dependencies). `tests/bindings.rs` renders all of them and compares with the checked-in files.
pub type Export = fn(&Config) -> Result<(), ExportError>;

pub const ALL_TYPES: &[Export] = &[
    app::AppInfo::export_all,
    accounts::AccountKindDto::export_all,
    accounts::AccountDto::export_all,
    accounts::AccountsDto::export_all,
    accounts::LoginMethod::export_all,
    accounts::OperationId::export_all,
    accounts::LoginEvent::export_all,
    instances::LoaderKindDto::export_all,
    instances::LoaderInfoDto::export_all,
    instances::LoaderVersionDto::export_all,
    instances::LoaderVersionsDto::export_all,
    instances::InstanceLoaderDto::export_all,
    instances::InstanceStatus::export_all,
    instances::InstanceDto::export_all,
    instances::InstancesDto::export_all,
    instances::LoaderRequestDto::export_all,
    instances::CreateInstanceRequest::export_all,
    instances::OpenTarget::export_all,
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
    play::RunId::export_all,
    play::TransferDto::export_all,
    play::PlayEvent::export_all,
    play::LastExit::export_all,
    play::GameStateDto::export_all,
    play::StopResult::export_all,
];
