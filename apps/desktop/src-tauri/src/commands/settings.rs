use crate::accounts::internal;
use crate::dto::error::{ErrorAction, ErrorCode, ErrorDto};
use crate::dto::settings::{JavaChoice, JavaPhase, Patch, SettingsDto, SettingsPatch};
use crate::error::error_dto;
use crate::events::{AppEvent, DataScope, EventHub};
use crate::java::{probe_version, validate_java, JavaProblem, ProbeLimits, PROBE_LIMITS};
use crate::settings::{Settings, SettingsHandle};
use crate::state::AppState;
use std::collections::BTreeMap;
use std::path::PathBuf;
use tauri::ipc::Channel;
use tauri::{Runtime, State, WebviewWindow};

/// The memory setting's range (DESIGN.md 11.7), checked here and again when a game starts.
pub const MEMORY_MB: std::ops::RangeInclusive<u32> = 512..=65536;

pub fn invalid_input(field: &str) -> ErrorDto {
    ErrorDto {
        code: ErrorCode::InvalidInput,
        params: BTreeMap::from([("field".to_string(), field.to_string())]),
        action: None::<ErrorAction>,
        detail: format!("invalid value for {field}"),
    }
}

fn validate(patch: &SettingsPatch) -> Result<(), ErrorDto> {
    if patch.memory_mb.is_some_and(|m| !MEMORY_MB.contains(&m)) {
        return Err(invalid_input("memoryMb"));
    }
    // A Java path from the web view is refused without looking at it: a UNC path would make even
    // a file check connect to a server (and send the Windows credentials), and the path is run
    // when the game starts. It is chosen through a file dialog in Rust (Phase 4b); clearing it
    // stays possible.
    if let Patch::Set(_) = &patch.java_override {
        return Err(invalid_input("javaOverride"));
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
    get_blocking(&state.settings)
}

/// Blocking: `SettingsDto::new` checks the stored Java path on the file system.
fn get_blocking(settings: &SettingsHandle) -> Result<SettingsDto, ErrorDto> {
    // Notice first, then the values: an update that lands in between is not announced twice.
    let notice = settings.take_notice();
    Ok(SettingsDto::new(settings.get(), notice))
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
    // An update that found the file damaged reset it; the notice goes out with this answer. It is
    // taken before the event, so the reload the event causes does not announce it a second time.
    let notice = settings.take_notice();
    events.emit(AppEvent::DataChanged {
        scope: DataScope::Settings,
    });
    Ok(SettingsDto::new(saved, notice))
}

pub fn settings_update_impl(
    state: &AppState,
    patch: SettingsPatch,
) -> Result<SettingsDto, ErrorDto> {
    update_blocking(&state.settings, &state.events, patch)
}

/// The error of a Java path that cannot be used. `detail` is English and names no path.
pub fn java_error(problem: JavaProblem, detail: &str) -> ErrorDto {
    ErrorDto {
        code: ErrorCode::JavaInvalid,
        params: BTreeMap::from([("reason".to_string(), problem.key().to_string())]),
        action: None,
        detail: detail.to_string(),
    }
}

fn java_detail(problem: JavaProblem) -> &'static str {
    match problem {
        JavaProblem::Location => "the Java path is not a plain path on a local drive",
        JavaProblem::Name => "the Java path is not a file named java.exe or javaw.exe",
        JavaProblem::Missing => "the Java file does not exist",
        JavaProblem::Unavailable => "the drive of the Java path is not available",
        JavaProblem::NotStarted => "the Java check failed",
    }
}

/// A stored Java path, checked before a game starts. The error carries "open settings".
pub fn checked_java(path: &std::path::Path) -> Result<PathBuf, ErrorDto> {
    validate_java(path).map_err(|p| ErrorDto {
        action: Some(ErrorAction::OpenSettings),
        ..java_error(p, java_detail(p))
    })
}

/// What happens after the dialog: `None` (closed) changes nothing; for a file, `on_checking` is
/// called first (the web view shows "Checking" from then on, not while the dialog is open), the
/// file is validated (text
/// before any file-system call), only then run with `-version` within `limits`, and only a file
/// that passes both is saved, as its canonical path. Any refusal keeps the previous value.
/// Blocking (runs a program): call it on a blocking thread.
pub fn java_choose_impl(
    settings: &SettingsHandle,
    events: &EventHub,
    picked: Option<PathBuf>,
    limits: ProbeLimits,
    on_checking: &dyn Fn(),
) -> Result<JavaChoice, ErrorDto> {
    let Some(picked) = picked else {
        return Ok(JavaChoice::Cancelled);
    };
    on_checking();
    let java = validate_java(&picked).map_err(|p| java_error(p, java_detail(p)))?;
    let version = probe_version(&java, limits)
        .map_err(|e| java_error(JavaProblem::NotStarted, &e.to_string()))?;
    let saved = settings
        .update(|s| {
            s.java_override = Some(java);
            Ok(())
        })
        .map_err(|e| error_dto(&e))?;
    let notice = settings.take_notice();
    events.emit(AppEvent::DataChanged {
        scope: DataScope::Settings,
    });
    Ok(JavaChoice::Chosen {
        version,
        settings: SettingsDto::new(saved, notice),
    })
}

/// "Choose Java": the Windows file dialog, opened here and owned by the app window, then
/// `java_choose_impl`. Takes no path: the web view cannot name the file. `channel` gets
/// `checking` once a file was picked (never the path). `busy` while a dialog is open.
#[tauri::command]
pub async fn java_choose<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    channel: Channel<JavaPhase>,
) -> Result<JavaChoice, ErrorDto> {
    use tauri_plugin_dialog::DialogExt;
    let Some(claim) = state.java_dialog.try_claim() else {
        return Err(ErrorDto {
            code: ErrorCode::Busy,
            params: BTreeMap::new(),
            action: None,
            detail: "a Java file dialog is already open".to_string(),
        });
    };
    let (settings, events) = (state.settings.clone(), state.events.clone());
    let dialog = window
        .dialog()
        .file()
        .set_parent(&window)
        .add_filter("java.exe, javaw.exe", &["exe"]);
    tauri::async_runtime::spawn_blocking(move || {
        let _claim = claim;
        let picked = dialog.blocking_pick_file().and_then(|f| f.into_path().ok());
        java_choose_impl(&settings, &events, picked, PROBE_LIMITS, &|| {
            let _ = channel.send(JavaPhase::Checking);
        })
    })
    .await
    .map_err(|_| internal("a background task failed"))?
}

/// Async: the answer checks the stored Java path (file-system calls), which must not run on the
/// main thread.
#[tauri::command]
pub async fn settings_get(state: State<'_, AppState>) -> Result<SettingsDto, ErrorDto> {
    let settings = state.settings.clone();
    tauri::async_runtime::spawn_blocking(move || get_blocking(&settings))
        .await
        .map_err(|_| internal("a background task failed"))?
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
            .map_err(|_| internal("a background task failed"))??;
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
    use crate::java::JavaProblem;
    use crate::settings::{load_settings, Language, SettingsNotice, Theme};
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
        let answer = settings_update_impl(&state, theme_patch(Theme::Dark)).unwrap();
        assert_eq!(answer.notice, Some(SettingsNotice::Reset));
        // The update's own answer carries the notice (a failed reload afterwards cannot lose it) ...
        assert_eq!(
            *log.lock().unwrap(),
            [AppEvent::DataChanged {
                scope: DataScope::Settings
            }]
        );
        // ... and the reload the event causes finds nothing left to announce.
        let after = settings_get_impl(&state).unwrap();
        assert_eq!(after.notice, None);
        assert_eq!(after.theme, Theme::Dark);
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
    fn java_override_cannot_be_set_from_the_web_view() {
        let dir = tempfile::tempdir().unwrap();
        let java = dir.path().join("java.exe");
        std::fs::write(&java, b"").unwrap();
        let state = state_in(dir.path());
        settings_update_impl(&state, theme_patch(Theme::Dark)).unwrap();
        let before = std::fs::read(dir.path().join("settings.json")).unwrap();
        // Even an existing local java.exe is refused; a UNC or verbatim path is never looked at.
        for path in [
            java.display().to_string(),
            r"\\fileserver\share\java.exe".to_string(),
            r"\\?\C:\Program Files\Java\bin\java.exe".to_string(),
            r"\\?\UNC\fileserver\share\java.exe".to_string(),
        ] {
            let e = settings_update_impl(
                &state,
                SettingsPatch {
                    java_override: Patch::Set(path.clone()),
                    ..Default::default()
                },
            )
            .unwrap_err();
            assert_eq!(e.code, ErrorCode::InvalidInput, "{path}");
            assert_eq!(e.params["field"], "javaOverride");
            assert!(!e.detail.contains("fileserver"), "{}", e.detail);
        }
        assert_eq!(
            std::fs::read(dir.path().join("settings.json")).unwrap(),
            before
        );
        assert_eq!(state.settings.get().java_override, None);
        // Clearing stays possible, and so does any patch that keeps the value.
        state
            .settings
            .update(|s| {
                s.java_override = Some(java.clone());
                Ok(())
            })
            .unwrap();
        let kept = settings_update_impl(&state, theme_patch(Theme::Light)).unwrap();
        assert_eq!(
            kept.java_override.map(|j| j.path),
            Some(java.display().to_string())
        );
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

    const QUICK: ProbeLimits = ProbeLimits {
        timeout: std::time::Duration::from_secs(3),
        max_output: 64 * 1024,
    };

    /// fake-java copied as `name` into `<dir>/<folder>/`, with `script` as what it does there.
    fn fake(dir: &std::path::Path, folder: &str, name: &str, script: &str) -> PathBuf {
        let bin = dir.join(folder);
        std::fs::create_dir_all(&bin).unwrap();
        let java = bin.join(name);
        std::fs::copy(hengst_testkit::fake_java(), &java).unwrap();
        std::fs::write(bin.join("fake-java.txt"), script).unwrap();
        java
    }

    const VERSION_21: &str = "err openjdk version \"21.0.5\" 2024-10-15\n";

    fn choose(state: &AppState, picked: Option<PathBuf>) -> Result<JavaChoice, ErrorDto> {
        java_choose_impl(&state.settings, &state.events, picked, QUICK, &|| {})
    }

    #[test]
    fn checking_is_announced_only_after_a_file_was_picked() {
        let dir = tempfile::tempdir().unwrap();
        let state = state_in(dir.path());
        let announced = std::cell::Cell::new(0);
        let count = || announced.set(announced.get() + 1);
        java_choose_impl(&state.settings, &state.events, None, QUICK, &count).unwrap();
        assert_eq!(
            announced.get(),
            0,
            "the dialog was closed: nothing is checked"
        );
        let java = fake(dir.path(), "jdk", "java.exe", VERSION_21);
        java_choose_impl(&state.settings, &state.events, Some(java), QUICK, &count).unwrap();
        assert_eq!(announced.get(), 1);
        let other = dir.path().join("notepad.exe");
        let _ = java_choose_impl(&state.settings, &state.events, Some(other), QUICK, &count);
        assert_eq!(announced.get(), 2, "a refused file was checked too");
    }

    fn refusal(e: &ErrorDto) -> &str {
        assert_eq!(e.code, ErrorCode::JavaInvalid, "{e:?}");
        assert!(e.action.is_none());
        &e.params["reason"]
    }

    #[test]
    fn a_chosen_java_is_checked_and_saved_as_its_canonical_path() {
        let dir = tempfile::tempdir().unwrap();
        let state = state_in(dir.path());
        let log = record(&state);
        let java = fake(dir.path(), "jdk", "java.exe", VERSION_21);
        let JavaChoice::Chosen { version, settings } = choose(&state, Some(java.clone())).unwrap()
        else {
            panic!("not chosen");
        };
        assert_eq!(version, "21.0.5");
        let shown = settings.java_override.unwrap();
        assert_eq!(shown.problem, None);
        assert!(!shown.path.starts_with(r"\\"), "{}", shown.path);
        let stored = state.settings.get().java_override.unwrap();
        assert_eq!(stored, crate::java::validate_java(&java).unwrap());
        assert_eq!(
            load_settings(&state.paths, 1).settings.java_override,
            Some(stored)
        );
        assert_eq!(
            *log.lock().unwrap(),
            [AppEvent::DataChanged {
                scope: DataScope::Settings
            }]
        );
    }

    #[test]
    fn a_closed_dialog_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let state = state_in(dir.path());
        let log = record(&state);
        assert!(matches!(choose(&state, None), Ok(JavaChoice::Cancelled)));
        assert!(log.lock().unwrap().is_empty());
        assert!(!dir.path().join("settings.json").exists());
    }

    #[test]
    fn a_refused_choice_is_never_run_and_keeps_the_previous_value() {
        let dir = tempfile::tempdir().unwrap();
        let state = state_in(dir.path());
        let good = fake(dir.path(), "jdk", "java.exe", VERSION_21);
        choose(&state, Some(good)).unwrap();
        let before = state.settings.get().java_override;
        // A program that would take 5 s if it were run: the refusal comes at once.
        let other = fake(dir.path(), "tools", "notepad.exe", "sleep 5000\n");
        let folder = dir.path().join("java.exe");
        std::fs::create_dir(&folder).unwrap();
        for (picked, reason) in [
            (other, "name"),
            (folder, "name"),
            (dir.path().join("gone").join("java.exe"), "missing"),
            (PathBuf::from(r"\\192.0.2.1\share\java.exe"), "location"),
            (
                PathBuf::from(r"\\?\UNC\192.0.2.1\share\java.exe"),
                "location",
            ),
        ] {
            let start = std::time::Instant::now();
            let e = choose(&state, Some(picked.clone())).unwrap_err();
            assert!(start.elapsed() < std::time::Duration::from_secs(2));
            assert_eq!(refusal(&e), reason, "{}", picked.display());
            assert!(!e.detail.contains("192.0.2.1"), "{}", e.detail);
            assert!(
                !e.detail.contains(&*dir.path().to_string_lossy()),
                "{}",
                e.detail
            );
            assert_eq!(state.settings.get().java_override, before);
        }
    }

    #[test]
    fn a_java_that_fails_its_check_keeps_the_previous_value() {
        let dir = tempfile::tempdir().unwrap();
        let state = state_in(dir.path());
        choose(
            &state,
            Some(fake(dir.path(), "jdk", "java.exe", VERSION_21)),
        )
        .unwrap();
        let before = state.settings.get().java_override;
        for (folder, script) in [
            ("exit3", "err openjdk version \"21\"\nexit 3\n"),
            ("hangs", "sleep 60000\n"),
            ("garbage", "out <img src=x>\n"),
        ] {
            let picked = fake(dir.path(), folder, "javaw.exe", script);
            let e = choose(&state, Some(picked)).unwrap_err();
            assert_eq!(refusal(&e), "notStarted", "{folder}");
            assert!(
                !e.detail.contains(&*dir.path().to_string_lossy()),
                "{}",
                e.detail
            );
            assert!(!e.detail.contains("img"), "{}", e.detail);
            assert_eq!(state.settings.get().java_override, before);
        }
    }

    #[test]
    fn a_stored_java_path_is_checked_for_display_without_being_run() {
        let dir = tempfile::tempdir().unwrap();
        let state = state_in(dir.path());
        let shown = |stored: PathBuf| {
            state
                .settings
                .update(|s| {
                    s.java_override = Some(stored);
                    Ok(())
                })
                .unwrap();
            settings_get_impl(&state).unwrap().java_override.unwrap()
        };
        let start = std::time::Instant::now();
        let unc = shown(PathBuf::from(r"\\192.0.2.1\share\java.exe"));
        assert!(start.elapsed() < std::time::Duration::from_secs(1));
        assert_eq!(unc.problem, Some(JavaProblem::Location));
        assert_eq!(unc.path, r"\\192.0.2.1\share\java.exe");
        assert_eq!(
            shown(dir.path().join("gone").join("java.exe")).problem,
            Some(JavaProblem::Missing)
        );
        assert_eq!(
            shown(PathBuf::from(r"C:\Windows\notepad.exe")).problem,
            Some(JavaProblem::Name)
        );
        // A file that would hang if it were run: showing it is instant.
        let hangs = fake(dir.path(), "jdk", "java.exe", "sleep 60000\n");
        let start = std::time::Instant::now();
        assert_eq!(shown(hangs).problem, None);
        assert!(start.elapsed() < std::time::Duration::from_secs(1));
    }

    #[test]
    fn only_one_java_dialog_at_a_time() {
        let dir = tempfile::tempdir().unwrap();
        let state = state_in(dir.path());
        let first = state.java_dialog.try_claim().unwrap();
        assert!(state.java_dialog.try_claim().is_none());
        drop(first);
        assert!(state.java_dialog.try_claim().is_some());
    }
}
