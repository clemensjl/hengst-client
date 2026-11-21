//! `<data root>/settings.json`: the desktop app's own settings file (the CLI does not read it).

use hengst_core::paths::{unique_temp, Paths};
use hengst_core::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use ts_rs::TS;

pub const SETTINGS_FORMAT: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    De,
    En,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// What happened to the settings file at start, reported to the web view once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum SettingsNotice {
    /// The file was damaged or from a newer version, was moved aside and defaults are in use.
    Reset,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub format: u32,
    pub language: Option<Language>,
    pub theme: Theme,
    pub memory_mb: u32,
    pub java_override: Option<PathBuf>,
    pub minimize_on_launch: bool,
    pub selected_instance: Option<String>,
    pub dev_account: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            format: SETTINGS_FORMAT,
            language: None,
            theme: Theme::System,
            memory_mb: 2048,
            java_override: None,
            minimize_on_launch: true,
            selected_instance: None,
            dev_account: false,
        }
    }
}

fn file(paths: &Paths) -> PathBuf {
    paths.root().join("settings.json")
}

/// Missing file: defaults. Unreadable, damaged or of a newer `format`: the file is moved to
/// `settings.json.broken-<now>` (never overwritten, never deleted), defaults are used and the
/// notice says so.
pub fn load_settings(paths: &Paths, now: u64) -> (Settings, Option<SettingsNotice>) {
    let path = file(paths);
    let parsed = match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice::<Settings>(&bytes)
            .ok()
            .filter(|s| s.format <= SETTINGS_FORMAT),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return (Settings::default(), None);
        }
        Err(_) => None,
    };
    if let Some(s) = parsed {
        return (s, None);
    }
    let mut aside = path.as_os_str().to_owned();
    aside.push(format!(".broken-{now}"));
    if let Err(e) = std::fs::rename(&path, &aside) {
        eprintln!("could not move the damaged settings file aside: {e}");
    }
    (Settings::default(), Some(SettingsNotice::Reset))
}

/// Temp file next to the target, then rename over it.
pub fn save_settings(paths: &Paths, s: &Settings) -> Result<()> {
    let target = file(paths);
    std::fs::create_dir_all(paths.root())?;
    let tmp = unique_temp(&target);
    std::fs::write(&tmp, serde_json::to_vec_pretty(s)?)?;
    if let Err(e) = std::fs::rename(&tmp, &target) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

/// The settings in memory. Every writer goes through `update`; there is no second way to write.
#[derive(Clone)]
pub struct SettingsHandle {
    inner: Arc<Mutex<(Paths, Settings)>>,
}

impl SettingsHandle {
    pub fn new(paths: Paths, settings: Settings) -> SettingsHandle {
        SettingsHandle {
            inner: Arc::new(Mutex::new((paths, settings))),
        }
    }

    pub fn get(&self) -> Settings {
        self.lock().1.clone()
    }

    /// Changes a copy, saves it and only then takes it over. An error from `f` or from saving
    /// changes neither the file nor the settings in memory.
    pub fn update(&self, f: impl FnOnce(&mut Settings) -> Result<()>) -> Result<Settings> {
        let mut guard = self.lock();
        let mut copy = guard.1.clone();
        f(&mut copy)?;
        save_settings(&guard.0, &copy)?;
        guard.1 = copy.clone();
        Ok(copy)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, (Paths, Settings)> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }
}

/// The native window theme for a setting: `System` is "follow the system" (`None`).
pub fn window_theme(setting: Theme) -> Option<tauri::Theme> {
    match setting {
        Theme::System => None,
        Theme::Light => Some(tauri::Theme::Light),
        Theme::Dark => Some(tauri::Theme::Dark),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hengst_core::paths::Paths;

    #[test]
    fn missing_settings_give_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let (s, notice) = load_settings(&Paths::new(dir.path()), 1700);
        assert_eq!(s, Settings::default());
        assert_eq!(s.memory_mb, 2048);
        assert_eq!(s.theme, Theme::System);
        assert!(s.minimize_on_launch);
        assert_eq!(s.language, None);
        assert_eq!(notice, None);
    }

    #[test]
    fn settings_round_trip_through_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let s = Settings {
            language: Some(Language::De),
            theme: Theme::Dark,
            memory_mb: 4096,
            java_override: Some(r"C:\java\bin\java.exe".into()),
            minimize_on_launch: false,
            selected_instance: Some("1.20.1".into()),
            dev_account: true,
            ..Settings::default()
        };
        save_settings(&paths, &s).unwrap();
        let text = std::fs::read_to_string(dir.path().join("settings.json")).unwrap();
        assert!(text.contains("\"format\": 1"), "{text}");
        assert!(text.contains("\"memoryMb\""), "{text}");
        assert!(text.contains("\"language\": \"de\""), "{text}");
        assert_eq!(load_settings(&paths, 1).0, s);
        let leftovers: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
        assert_eq!(leftovers.len(), 1, "no temp file stays behind");
    }

    #[test]
    fn broken_settings_are_moved_aside() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("settings.json"), "{").unwrap();
        let (s, notice) = load_settings(&Paths::new(dir.path()), 1700);
        assert_eq!(s, Settings::default());
        assert_eq!(notice, Some(SettingsNotice::Reset));
        let moved = dir.path().join("settings.json.broken-1700");
        assert_eq!(std::fs::read_to_string(moved).unwrap(), "{");
        assert!(!dir.path().join("settings.json").exists());
    }

    #[test]
    fn too_new_settings_are_moved_aside_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let text = r#"{"format":2,"theme":"sepia"}"#;
        std::fs::write(dir.path().join("settings.json"), text).unwrap();
        let (s, notice) = load_settings(&Paths::new(dir.path()), 1700);
        assert_eq!(s, Settings::default());
        assert_eq!(notice, Some(SettingsNotice::Reset));
        let moved = dir.path().join("settings.json.broken-1700");
        assert_eq!(std::fs::read_to_string(moved).unwrap(), text);
        assert!(!dir.path().join("settings.json").exists());
    }

    #[test]
    fn a_file_missing_fields_keeps_the_defaults_for_them() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"format":1,"theme":"dark"}"#,
        )
        .unwrap();
        let (s, notice) = load_settings(&Paths::new(dir.path()), 1);
        assert_eq!(notice, None);
        assert_eq!(s.theme, Theme::Dark);
        assert_eq!(s.memory_mb, 2048);
    }

    #[test]
    fn window_theme_follows_the_setting() {
        assert_eq!(window_theme(Theme::System), None);
        assert_eq!(window_theme(Theme::Light), Some(tauri::Theme::Light));
        assert_eq!(window_theme(Theme::Dark), Some(tauri::Theme::Dark));
    }

    #[test]
    fn a_failed_save_changes_neither_file_nor_memory() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let handle = SettingsHandle::new(paths.clone(), Settings::default());
        // A folder where the file belongs: the rename cannot replace it.
        std::fs::create_dir(dir.path().join("settings.json")).unwrap();
        let result = handle.update(|s| {
            s.theme = Theme::Dark;
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(handle.get().theme, Theme::System);
        assert!(dir.path().join("settings.json").is_dir());
    }

    #[test]
    fn a_failing_closure_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let handle = SettingsHandle::new(Paths::new(dir.path()), Settings::default());
        let result = handle.update(|s| {
            s.theme = Theme::Dark;
            Err(hengst_core::Error::UnknownVersion("x".into()))
        });
        assert!(result.is_err());
        assert_eq!(handle.get(), Settings::default());
        assert!(!dir.path().join("settings.json").exists());
    }
}
