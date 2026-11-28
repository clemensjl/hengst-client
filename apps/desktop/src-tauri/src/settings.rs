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
    /// Keys this version does not know (written by a newer or a later version of the app). They
    /// are kept when the file is saved.
    #[serde(flatten)]
    pub unknown: serde_json::Map<String, serde_json::Value>,
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
            unknown: serde_json::Map::new(),
        }
    }
}

fn file(paths: &Paths) -> PathBuf {
    paths.root().join("settings.json")
}

/// The result of reading the settings file at start.
#[derive(Debug)]
pub struct Loaded {
    pub settings: Settings,
    pub notice: Option<SettingsNotice>,
    /// `Some(reason)`: the file must not be written (it could not be read, or a damaged file could
    /// not be moved aside), so the defaults in `settings` are in memory only.
    pub blocked: Option<String>,
}

/// Missing file: defaults. A file that was read and does not parse, or is of a newer `format`, is
/// damaged: it is moved to `settings.json.broken-<now>` (never overwritten, never deleted),
/// defaults are used and the notice says so. A read error other than "not found" (a sharing
/// violation, missing permission) says nothing about the content: the file stays untouched, no
/// notice is given, and `blocked` forbids saving over it. If moving a damaged file aside fails,
/// it is blocked too.
pub fn load_settings(paths: &Paths, now: u64) -> Loaded {
    let path = file(paths);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Loaded {
                settings: Settings::default(),
                notice: None,
                blocked: None,
            };
        }
        Err(e) => {
            return Loaded {
                settings: Settings::default(),
                notice: None,
                blocked: Some(format!("could not read {}: {e}", path.display())),
            };
        }
    };
    if let Some(s) = serde_json::from_slice::<Settings>(&bytes)
        .ok()
        .filter(|s| s.format <= SETTINGS_FORMAT)
    {
        return Loaded {
            settings: s,
            notice: None,
            blocked: None,
        };
    }
    let mut aside = path.as_os_str().to_owned();
    aside.push(format!(".broken-{now}"));
    let blocked = std::fs::rename(&path, &aside)
        .err()
        .map(|e| format!("could not move the damaged settings file aside, it stays as it is: {e}"));
    Loaded {
        settings: Settings::default(),
        notice: Some(SettingsNotice::Reset),
        blocked,
    }
}

/// Temp file next to the target, flushed to disk, then renamed over it.
pub fn save_settings(paths: &Paths, s: &Settings) -> Result<()> {
    use std::io::Write;
    let target = file(paths);
    std::fs::create_dir_all(paths.root())?;
    let tmp = unique_temp(&target);
    let written = (|| -> std::io::Result<()> {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(&serde_json::to_vec_pretty(s).map_err(std::io::Error::other)?)?;
        f.sync_all()
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    if let Err(e) = hengst_core::paths::replace_file(&tmp, &target) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

struct Inner {
    paths: Paths,
    settings: Settings,
    /// See `Loaded::blocked`. While set, `update` reads the file again first.
    blocked: Option<String>,
    /// The "damaged and reset" notice, from the start or from an update that found the file
    /// damaged when it read it again. `take_notice` hands it out once.
    notice: Option<SettingsNotice>,
}

/// The settings in memory. Every writer goes through `update`; there is no second way to write.
///
/// Two locks: `state` guards the values and is only ever held for a copy or a commit, never across
/// file I/O, so `get` (the window's theme handler runs on the event loop) cannot be held up by a
/// save; `save` serializes whole updates (copy, change, save, commit).
#[derive(Clone)]
pub struct SettingsHandle {
    state: Arc<Mutex<Inner>>,
    save: Arc<Mutex<()>>,
}

impl SettingsHandle {
    pub fn new(paths: Paths, settings: Settings) -> SettingsHandle {
        SettingsHandle::from_loaded(
            paths,
            Loaded {
                settings,
                notice: None,
                blocked: None,
            },
        )
    }

    pub fn from_loaded(paths: Paths, loaded: Loaded) -> SettingsHandle {
        SettingsHandle {
            state: Arc::new(Mutex::new(Inner {
                paths,
                settings: loaded.settings,
                blocked: loaded.blocked,
                notice: loaded.notice,
            })),
            save: Arc::new(Mutex::new(())),
        }
    }

    pub fn get(&self) -> Settings {
        self.lock().settings.clone()
    }

    /// The pending "damaged and reset" notice, once.
    pub fn take_notice(&self) -> Option<SettingsNotice> {
        self.lock().notice.take()
    }

    /// Changes a copy, saves it and only then takes it over. An error from `f` or from saving
    /// changes neither the file nor the settings in memory. Blocking (file I/O under the save
    /// lock): call it from a blocking thread, never across an await.
    ///
    /// If the start could not read the file, the defaults in memory are not saved over it: the
    /// file is read again here, and only a successful read (or a missing file) unblocks and
    /// replaces the defaults; otherwise this fails and the file stays as it is. A file found
    /// damaged on that read is moved aside and its notice is kept for `take_notice`.
    pub fn update(&self, f: impl FnOnce(&mut Settings) -> Result<()>) -> Result<Settings> {
        let _saving = self.save.lock().unwrap_or_else(|p| p.into_inner());
        let (paths, blocked, mut copy) = {
            let guard = self.lock();
            (
                guard.paths.clone(),
                guard.blocked.clone(),
                guard.settings.clone(),
            )
        };
        if let Some(reason) = blocked {
            let again = load_settings(&paths, hengst_core::auth::unix_now());
            if again.blocked.is_some() {
                return Err(hengst_core::Error::Io(std::io::Error::other(reason)));
            }
            copy = again.settings.clone();
            let mut guard = self.lock();
            guard.settings = again.settings;
            guard.blocked = None;
            if again.notice.is_some() {
                guard.notice = again.notice;
            }
        }
        f(&mut copy)?;
        save_settings(&paths, &copy)?;
        self.lock().settings = copy.clone();
        Ok(copy)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
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
        let Loaded {
            settings: s,
            notice,
            ..
        } = load_settings(&Paths::new(dir.path()), 1700);
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
        assert_eq!(load_settings(&paths, 1).settings, s);
        let leftovers: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
        assert_eq!(leftovers.len(), 1, "no temp file stays behind");
    }

    #[test]
    fn broken_settings_are_moved_aside() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("settings.json"), "{").unwrap();
        let Loaded {
            settings: s,
            notice,
            ..
        } = load_settings(&Paths::new(dir.path()), 1700);
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
        let Loaded {
            settings: s,
            notice,
            ..
        } = load_settings(&Paths::new(dir.path()), 1700);
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
        let Loaded {
            settings: s,
            notice,
            ..
        } = load_settings(&Paths::new(dir.path()), 1);
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

    #[test]
    fn a_read_error_is_not_a_damaged_file() {
        let dir = tempfile::tempdir().unwrap();
        // A folder where the file belongs: reading it fails with something other than "not found".
        std::fs::create_dir(dir.path().join("settings.json")).unwrap();
        let loaded = load_settings(&Paths::new(dir.path()), 1700);
        assert_eq!(loaded.settings, Settings::default());
        assert_eq!(loaded.notice, None, "no 'damaged' message for a read error");
        assert!(loaded.blocked.is_some());
        assert!(dir.path().join("settings.json").is_dir(), "left untouched");
        assert!(!dir.path().join("settings.json.broken-1700").exists());
    }

    #[test]
    fn a_damaged_file_found_when_reread_during_an_update_keeps_its_reset_notice() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        // Unreadable at start (a folder where the file belongs): no notice yet, saving blocked.
        std::fs::create_dir(dir.path().join("settings.json")).unwrap();
        let handle = SettingsHandle::from_loaded(paths.clone(), load_settings(&paths, 1));
        assert_eq!(handle.take_notice(), None);
        // Then it becomes readable, but damaged: the update moves it aside and starts from the
        // defaults, and the user must still hear about it, once.
        std::fs::remove_dir(dir.path().join("settings.json")).unwrap();
        std::fs::write(dir.path().join("settings.json"), "{").unwrap();
        let saved = handle
            .update(|s| {
                s.memory_mb = 4096;
                Ok(())
            })
            .unwrap();
        assert_eq!(saved.memory_mb, 4096);
        assert_eq!(handle.take_notice(), Some(SettingsNotice::Reset));
        assert_eq!(handle.take_notice(), None, "handed out once");
        let aside: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("settings.json.broken-")
            })
            .collect();
        assert_eq!(aside.len(), 1, "the damaged file is kept aside");
    }

    #[test]
    fn a_pending_start_notice_survives_an_update_and_is_taken_once() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        std::fs::write(dir.path().join("settings.json"), "{").unwrap();
        let handle = SettingsHandle::from_loaded(paths.clone(), load_settings(&paths, 1));
        handle.update(|_| Ok(())).unwrap();
        assert_eq!(handle.take_notice(), Some(SettingsNotice::Reset));
        assert_eq!(handle.take_notice(), None);
    }

    #[test]
    fn reading_the_settings_does_not_wait_for_a_save_in_progress() {
        // The window's theme handler runs on the event loop and calls `get`; a save must not be
        // able to block it. The closure of `update` runs inside the save, so asking from another
        // thread there proves `get` needs no lock that the save holds.
        let dir = tempfile::tempdir().unwrap();
        let handle = SettingsHandle::new(Paths::new(dir.path()), Settings::default());
        let reader = handle.clone();
        handle
            .update(|_| {
                let (tx, rx) = std::sync::mpsc::channel();
                let reader = reader.clone();
                std::thread::spawn(move || {
                    let _ = tx.send(reader.get());
                });
                assert!(
                    rx.recv_timeout(std::time::Duration::from_secs(2)).is_ok(),
                    "get() waited for the save"
                );
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn saves_are_still_serialized() {
        let dir = tempfile::tempdir().unwrap();
        let handle = SettingsHandle::new(Paths::new(dir.path()), Settings::default());
        let start = handle.get().memory_mb;
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let handle = handle.clone();
                std::thread::spawn(move || {
                    handle
                        .update(|s| {
                            s.memory_mb += 1;
                            Ok(())
                        })
                        .unwrap();
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        assert_eq!(handle.get().memory_mb, start + 8, "no update was lost");
        assert_eq!(
            load_settings(&Paths::new(dir.path()), 1).settings.memory_mb,
            start + 8
        );
    }

    #[test]
    fn nothing_is_saved_over_an_unreadable_file_until_a_read_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        std::fs::create_dir(dir.path().join("settings.json")).unwrap();
        let handle = SettingsHandle::from_loaded(paths.clone(), load_settings(&paths, 1));
        let set_memory = |h: &SettingsHandle| {
            h.update(|s| {
                s.memory_mb = 4096;
                Ok(())
            })
        };
        assert!(set_memory(&handle).is_err());
        assert!(dir.path().join("settings.json").is_dir());
        // The file becomes readable (the sharing violation is over): its content is adopted, not
        // replaced by the defaults, and only then is it written.
        std::fs::remove_dir(dir.path().join("settings.json")).unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"format":1,"theme":"dark"}"#,
        )
        .unwrap();
        let saved = set_memory(&handle).unwrap();
        assert_eq!(saved.theme, Theme::Dark);
        assert_eq!(saved.memory_mb, 4096);
        assert_eq!(load_settings(&paths, 2).settings, saved);
    }

    #[test]
    fn a_damaged_file_that_cannot_be_moved_aside_is_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        std::fs::write(dir.path().join("settings.json"), "{").unwrap();
        // Another program has the file open for reading without allowing a rename (Windows share
        // mode FILE_SHARE_READ): it can be read, but not moved aside.
        use std::os::windows::fs::OpenOptionsExt;
        let _held = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(dir.path().join("settings.json"))
            .unwrap();
        let loaded = load_settings(&paths, 1700);
        assert!(loaded.blocked.is_some());
        let handle = SettingsHandle::from_loaded(paths, loaded);
        assert!(handle.update(|_| Ok(())).is_err());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("settings.json")).unwrap(),
            "{"
        );
    }

    #[test]
    fn unknown_keys_survive_a_save() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"format":1,"futureKey":{"a":[1,2]},"theme":"light"}"#,
        )
        .unwrap();
        let loaded = load_settings(&paths, 1);
        assert_eq!(loaded.notice, None);
        let handle = SettingsHandle::from_loaded(paths, loaded);
        handle
            .update(|s| {
                s.theme = Theme::Dark;
                Ok(())
            })
            .unwrap();
        let text = std::fs::read_to_string(dir.path().join("settings.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(json["futureKey"]["a"][1], 2);
        assert_eq!(json["theme"], "dark");
    }
}
