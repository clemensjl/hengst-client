use crate::dto::error::{ErrorAction, ErrorCode, ErrorDto};
use crate::dto::settings::{Patch, SettingsDto, SettingsPatch};
use crate::error::error_dto;
use crate::events::{AppEvent, DataScope, EventHub};
use crate::settings::{Settings, SettingsHandle};
use crate::state::AppState;
use std::collections::BTreeMap;
use std::path::PathBuf;
use tauri::{Runtime, State, WebviewWindow};

const MEMORY_MB: std::ops::RangeInclusive<u32> = 512..=65536;

fn invalid_input(field: &str) -> ErrorDto {
    ErrorDto {
        code: ErrorCode::InvalidInput,
        params: BTreeMap::from([("field".to_string(), field.to_string())]),
        action: None::<ErrorAction>,
        detail: format!("invalid value for {field}"),
    }
}

/// An absolute path to an existing file named `java.exe` or `javaw.exe` (any case).
fn is_java_file(path: &std::path::Path) -> bool {
    let name_ok = path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.eq_ignore_ascii_case("java.exe") || n.eq_ignore_ascii_case("javaw.exe"));
    path.is_absolute() && name_ok && path.is_file()
}

fn validate(patch: &SettingsPatch) -> Result<(), ErrorDto> {
    if patch.memory_mb.is_some_and(|m| !MEMORY_MB.contains(&m)) {
        return Err(invalid_input("memoryMb"));
    }
    if let Patch::Set(path) = &patch.java_override {
        if !is_java_file(std::path::Path::new(path)) {
            return Err(invalid_input("javaOverride"));
        }
    }
    Ok(())
}

fn apply(s: &mut Settings, patch: SettingsPatch) {
    match patch.language {
        Patch::Keep => {}
        Patch::Set(l) => s.language = Some(l),
        Patch::Clear => s.language = None,
    }
    if let Some(theme) = patch.theme {
        s.theme = theme;
    }
    if let Some(memory_mb) = patch.memory_mb {
        s.memory_mb = memory_mb;
    }
    match patch.java_override {
        Patch::Keep => {}
        Patch::Set(p) => s.java_override = Some(PathBuf::from(p)),
        Patch::Clear => s.java_override = None,
    }
    if let Some(minimize) = patch.minimize_on_launch {
        s.minimize_on_launch = minimize;
    }
}

/// The notice of a reset settings file (from the start or found by an update) is handed out once.
pub fn settings_get_impl(state: &AppState) -> Result<SettingsDto, ErrorDto> {
    // Notice first, then the values: an update that lands in between is not announced twice.
    let notice = state.settings.take_notice();
    Ok(SettingsDto::new(state.settings.get(), notice))
}

/// Validates, saves and announces a change. Blocking (file I/O): the command runs it on a
/// blocking thread.
fn update_blocking(
    settings: &SettingsHandle,
    events: &EventHub,
    patch: SettingsPatch,
) -> Result<SettingsDto, ErrorDto> {
    validate(&patch)?;
    let saved = settings
        .update(|s| {
            apply(s, patch);
            Ok(())
        })
        .map_err(|e| error_dto(&e))?;
    events.emit(AppEvent::DataChanged {
        scope: DataScope::Settings,
    });
    Ok(SettingsDto::new(saved, None))
}

pub fn settings_update_impl(
    state: &AppState,
    patch: SettingsPatch,
) -> Result<SettingsDto, ErrorDto> {
    update_blocking(&state.settings, &state.events, patch)
}

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> Result<SettingsDto, ErrorDto> {
    settings_get_impl(&state)
}

/// Async so the file I/O (under `SettingsHandle`'s lock, which is never held across an await)
/// runs on a blocking thread and not on the main thread.
#[tauri::command]
pub async fn settings_update<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    patch: SettingsPatch,
) -> Result<SettingsDto, ErrorDto> {
    let theme_changed = patch.theme.is_some();
    let (settings, events) = (state.settings.clone(), state.events.clone());
    let dto =
        tauri::async_runtime::spawn_blocking(move || update_blocking(&settings, &events, patch))
            .await
            .map_err(|e| {
                error_dto(&hengst_core::Error::Io(std::io::Error::other(
                    e.to_string(),
                )))
            })??;
    if theme_changed {
        // The settings are already saved in the new theme; a failing title bar must not undo that.
        if let Err(e) = crate::startup::apply_window_theme(&window, dto.theme) {
            eprintln!("could not set the window theme: {e}");
        }
    }
    Ok(dto)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::error::ErrorCode;
    use crate::dto::settings::{Patch, SettingsPatch};
    use crate::events::{AppEvent, DataScope};
    use crate::settings::{Language, SettingsNotice, Theme};
    use hengst_core::launch::Endpoints;
    use hengst_core::paths::Paths;
    use std::sync::{Arc, Mutex};

    fn state_in(dir: &std::path::Path) -> AppState {
        AppState::new(Paths::new(dir), Endpoints::default())
    }

    fn theme_patch(theme: Theme) -> SettingsPatch {
        SettingsPatch {
            theme: Some(theme),
            ..Default::default()
        }
    }

    fn record(state: &AppState) -> Arc<Mutex<Vec<AppEvent>>> {
        let log: Arc<Mutex<Vec<AppEvent>>> = Arc::default();
        let sink = log.clone();
        state
            .events
            .subscribe(Arc::new(move |e| sink.lock().unwrap().push(e)));
        log
    }

    #[test]
    fn memory_outside_range_is_invalid_input() {
        let dir = tempfile::tempdir().unwrap();
        let state = state_in(dir.path());
        settings_update_impl(&state, theme_patch(Theme::Dark)).unwrap();
        let before = std::fs::read(dir.path().join("settings.json")).unwrap();
        for bad in [100, 511, 65537] {
            let patch = SettingsPatch {
                memory_mb: Some(bad),
                ..Default::default()
            };
            let e = settings_update_impl(&state, patch).unwrap_err();
            assert_eq!(e.code, ErrorCode::InvalidInput);
            assert_eq!(e.params["field"], "memoryMb");
        }
        assert_eq!(
            std::fs::read(dir.path().join("settings.json")).unwrap(),
            before
        );
        for good in [512, 65536] {
            let patch = SettingsPatch {
                memory_mb: Some(good),
                ..Default::default()
            };
            assert_eq!(settings_update_impl(&state, patch).unwrap().memory_mb, good);
        }
    }

    #[test]
    fn update_emits_settings_changed() {
        let dir = tempfile::tempdir().unwrap();
        let state = state_in(dir.path());
        let log = record(&state);
        settings_update_impl(&state, theme_patch(Theme::Light)).unwrap();
        assert_eq!(
            *log.lock().unwrap(),
            [AppEvent::DataChanged {
                scope: DataScope::Settings
            }]
        );
    }

    #[test]
    fn a_rejected_update_emits_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let state = state_in(dir.path());
        let log = record(&state);
        let patch = SettingsPatch {
            memory_mb: Some(1),
            ..Default::default()
        };
        assert!(settings_update_impl(&state, patch).is_err());
        assert!(log.lock().unwrap().is_empty());
    }

    #[test]
    fn notice_is_delivered_once() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("settings.json"), "{").unwrap();
        let state = state_in(dir.path());
        let first = settings_get_impl(&state).unwrap();
        assert_eq!(first.notice, Some(SettingsNotice::Reset));
        assert_eq!(settings_get_impl(&state).unwrap().notice, None);
        assert_eq!(settings_get_impl(&state).unwrap().notice, None);
        // A reload after an update does not bring it back either.
        settings_update_impl(&state, theme_patch(Theme::Dark)).unwrap();
        assert_eq!(settings_get_impl(&state).unwrap().notice, None);
    }

    #[test]
    fn an_update_that_finds_the_file_damaged_announces_the_reset_through_the_next_get() {
        let dir = tempfile::tempdir().unwrap();
        // Unreadable at start, damaged when the update reads it again.
        std::fs::create_dir(dir.path().join("settings.json")).unwrap();
        let state = state_in(dir.path());
        let log = record(&state);
        assert_eq!(settings_get_impl(&state).unwrap().notice, None);
        std::fs::remove_dir(dir.path().join("settings.json")).unwrap();
        std::fs::write(dir.path().join("settings.json"), "{").unwrap();
        settings_update_impl(&state, theme_patch(Theme::Dark)).unwrap();
        // The event that makes the web view ask again is the one every update emits.
        assert_eq!(
            *log.lock().unwrap(),
            [AppEvent::DataChanged {
                scope: DataScope::Settings
            }]
        );
        let after = settings_get_impl(&state).unwrap();
        assert_eq!(after.notice, Some(SettingsNotice::Reset));
        assert_eq!(after.theme, Theme::Dark);
        assert_eq!(settings_get_impl(&state).unwrap().notice, None);
    }

    #[test]
    fn patch_distinguishes_keep_set_and_clear() {
        let keep: SettingsPatch = serde_json::from_str("{}").unwrap();
        assert_eq!(keep.language, Patch::Keep);
        let set: SettingsPatch =
            serde_json::from_str(r#"{"language":{"op":"set","value":"de"}}"#).unwrap();
        assert_eq!(set.language, Patch::Set(Language::De));
        let clear: SettingsPatch = serde_json::from_str(r#"{"language":{"op":"clear"}}"#).unwrap();
        assert_eq!(clear.language, Patch::Clear);
        assert_eq!(
            serde_json::to_string(&Patch::Set(1)).unwrap(),
            r#"{"op":"set","value":1}"#
        );

        let dir = tempfile::tempdir().unwrap();
        let state = state_in(dir.path());
        let lang = |p: Patch<Language>| SettingsPatch {
            language: p,
            ..Default::default()
        };
        assert_eq!(
            settings_update_impl(&state, lang(Patch::Set(Language::De)))
                .unwrap()
                .language,
            Some(Language::De)
        );
        // Keep leaves "de" in place, even while another field changes.
        assert_eq!(
            settings_update_impl(&state, theme_patch(Theme::Dark))
                .unwrap()
                .language,
            Some(Language::De)
        );
        assert_eq!(
            settings_update_impl(&state, lang(Patch::Clear))
                .unwrap()
                .language,
            None
        );
        assert_eq!(state.settings.get().language, None);
    }

    #[test]
    fn patch_cannot_touch_dev_account_or_selected_instance() {
        assert!(serde_json::from_str::<SettingsPatch>(r#"{"devAccount":true}"#).is_err());
        assert!(serde_json::from_str::<SettingsPatch>(r#"{"selectedInstance":"x"}"#).is_err());
        assert!(serde_json::from_str::<SettingsPatch>(r#"{"format":2}"#).is_err());
    }

    #[test]
    fn java_override_must_be_an_existing_java_file() {
        let dir = tempfile::tempdir().unwrap();
        let java = dir.path().join("java.exe");
        let javaw = dir.path().join("JAVAW.EXE");
        let other = dir.path().join("notepad.exe");
        let folder = dir.path().join("java.exe.d");
        let folder_named_java = dir.path().join("sub").join("java.exe");
        for f in [&java, &javaw, &other] {
            std::fs::write(f, b"").unwrap();
        }
        std::fs::create_dir(&folder).unwrap();
        std::fs::create_dir_all(&folder_named_java).unwrap();
        let state = state_in(dir.path());
        let set = |p: &std::path::Path| {
            settings_update_impl(
                &state,
                SettingsPatch {
                    java_override: Patch::Set(p.display().to_string()),
                    ..Default::default()
                },
            )
        };
        assert!(set(&java).is_ok());
        assert!(set(&javaw).is_ok());
        for bad in [
            other,
            folder,
            folder_named_java,
            dir.path().join("missing").join("java.exe"),
            std::path::PathBuf::from("java.exe"),
        ] {
            let e = set(&bad).unwrap_err();
            assert_eq!(e.code, ErrorCode::InvalidInput, "{}", bad.display());
            assert_eq!(e.params["field"], "javaOverride");
        }
        let cleared = settings_update_impl(
            &state,
            SettingsPatch {
                java_override: Patch::Clear,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(cleared.java_override, None);
    }

    #[test]
    fn a_failed_save_changes_neither_file_nor_memory() {
        let dir = tempfile::tempdir().unwrap();
        let state = state_in(dir.path());
        std::fs::create_dir(dir.path().join("settings.json")).unwrap();
        assert!(settings_update_impl(&state, theme_patch(Theme::Dark)).is_err());
        assert_eq!(state.settings.get().theme, Theme::System);
        assert!(dir.path().join("settings.json").is_dir());
    }
}
