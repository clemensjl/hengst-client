//! Opening pages in the system browser. The web view names a `UrlTarget`; the address is fixed
//! here, so no URL from JavaScript ever reaches the opener.

use crate::dto::error::{ErrorCode, ErrorDto, UrlTarget};
use crate::dto::instances::OpenTarget;
use crate::error::{error_dto, redact, strip_paths};
use hengst_core::instances;
use hengst_core::paths::Paths;
use std::collections::BTreeMap;
use std::fmt::Display;
use std::path::{Path, PathBuf};

pub fn resolve_url(target: &UrlTarget) -> &'static str {
    match target {
        UrlTarget::Repository => "https://github.com/clemensjl/hengst-client",
        UrlTarget::SignInHelp => "https://github.com/clemensjl/hengst-client#readme",
        UrlTarget::XboxSignup => "https://www.xbox.com/",
        UrlTarget::MinecraftProfile => "https://www.minecraft.net/msaprofile/mygames/editprofile",
        UrlTarget::DeviceLogin => "https://www.microsoft.com/link",
        // The "Build" section of the README, which says how to set the Microsoft client id.
        UrlTarget::BuildHelp => "https://github.com/clemensjl/hengst-client#build",
    }
}

/// Opens the fixed address of `target` through `opener`; a refusal becomes `OpenFailed`.
pub fn open_target<E: Display>(
    target: &UrlTarget,
    opener: impl FnOnce(&'static str) -> Result<(), E>,
) -> Result<(), ErrorDto> {
    opener(resolve_url(target)).map_err(|e| ErrorDto {
        code: ErrorCode::OpenFailed,
        params: BTreeMap::new(),
        action: None,
        detail: strip_paths(&redact(&e.to_string())),
    })
}

/// The folder a target names: the data folder (made if it does not exist yet) or the folder of an
/// instance that exists, through the core's gate: the id must be a folder name, the folder must be
/// a real directory (not a link) with an `instance.json`. Unreadable and newer-format instances
/// have one and can be opened. A path from the web view never gets here.
pub fn resolve_location(paths: &Paths, target: &OpenTarget) -> hengst_core::Result<PathBuf> {
    match target {
        OpenTarget::DataDir => {
            std::fs::create_dir_all(paths.root())?;
            Ok(paths.root().to_path_buf())
        }
        OpenTarget::Instance { id } => instances::instance_dir(paths, id),
    }
}

/// Opens `dir` through `opener` (`explore_folder` in the app). The path is checked to be a
/// directory right before. That check can be outrun (the folder swapped for a file a moment
/// later), so the real guard is the opener: it must refuse anything that is not a folder.
pub fn open_folder<E: Display>(
    dir: &Path,
    opener: impl FnOnce(&Path) -> Result<(), E>,
) -> Result<(), ErrorDto> {
    let failed = |detail: String| ErrorDto {
        code: ErrorCode::OpenFailed,
        params: BTreeMap::new(),
        action: None,
        detail: strip_paths(&redact(&detail)),
    };
    if !std::fs::metadata(dir).is_ok_and(|m| m.is_dir()) {
        return Err(failed("the path is not a folder".to_string()));
    }
    let absolute = std::path::absolute(dir).map_err(|e| error_dto(&e.into()))?;
    opener(&absolute).map_err(|e| failed(e.to_string()))
}

/// Shows a folder in Explorer: `ShellExecuteW` with the verb "explore", which exists for folders
/// only. For a file the call fails (measured: result 31, `SE_ERR_NOASSOC`, also for a `.bat`
/// file, with or without a trailing backslash) and nothing runs, so a folder swapped for a file
/// between the check and this call cannot be executed. The verb "open", which the opener plugin
/// uses, would run it (measured), which is why the plugin is not used for folders.
#[cfg(windows)]
pub fn explore_folder(dir: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::System::Com::{
        CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED,
    };
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    // The shell wants COM on the calling thread (this runs on a pooled blocking thread that has
    // none). A thread that is already initialized differently (a negative result) is left as it is
    // and not uninitialized; otherwise the call is balanced below.
    let com = unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) };
    let verb: Vec<u16> = "explore".encode_utf16().chain(Some(0)).collect();
    let file: Vec<u16> = dir.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: both strings are NUL-terminated and outlive the call; the other arguments are null.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    } as isize;
    if com >= 0 {
        unsafe { CoUninitialize() };
    }
    // A value above 32 is success; below that it is an error code.
    if result > 32 {
        Ok(())
    } else {
        Err(format!(
            "Explorer could not show the folder (code {result})"
        ))
    }
}

#[cfg(not(windows))]
pub fn explore_folder(_dir: &Path) -> Result<(), String> {
    Err("opening folders is implemented for Windows only".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::error::{ErrorCode, UrlTarget};

    const ALL: [UrlTarget; 6] = [
        UrlTarget::Repository,
        UrlTarget::SignInHelp,
        UrlTarget::XboxSignup,
        UrlTarget::MinecraftProfile,
        UrlTarget::DeviceLogin,
        UrlTarget::BuildHelp,
    ];

    #[test]
    fn url_targets_are_fixed_https_urls() {
        for t in ALL {
            assert!(resolve_url(&t).starts_with("https://"), "{t:?}");
            assert!(!resolve_url(&t).contains('?'), "{t:?}");
        }
        assert_eq!(
            resolve_url(&UrlTarget::DeviceLogin),
            "https://www.microsoft.com/link"
        );
    }

    #[test]
    fn every_target_resolves_to_its_address() {
        let want = [
            (
                UrlTarget::Repository,
                "https://github.com/clemensjl/hengst-client",
            ),
            (
                UrlTarget::SignInHelp,
                "https://github.com/clemensjl/hengst-client#readme",
            ),
            (UrlTarget::XboxSignup, "https://www.xbox.com/"),
            (
                UrlTarget::MinecraftProfile,
                "https://www.minecraft.net/msaprofile/mygames/editprofile",
            ),
            (UrlTarget::DeviceLogin, "https://www.microsoft.com/link"),
            (
                UrlTarget::BuildHelp,
                "https://github.com/clemensjl/hengst-client#build",
            ),
        ];
        assert_eq!(want.len(), ALL.len());
        for (target, url) in want {
            assert_eq!(resolve_url(&target), url, "{target:?}");
        }
    }

    #[test]
    fn opening_passes_exactly_the_resolved_url_to_the_opener() {
        for t in ALL {
            let mut seen = vec![];
            open_target(&t, |url| {
                seen.push(url.to_string());
                Ok::<(), String>(())
            })
            .unwrap();
            assert_eq!(seen, [resolve_url(&t)], "{t:?}");
        }
    }

    #[test]
    fn a_failing_opener_is_a_typed_error_without_a_panic() {
        let err = open_target(&UrlTarget::BuildHelp, |_| {
            Err::<(), _>("no default browser: https://x/y?token=SECRET")
        })
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::OpenFailed);
        assert!(err.action.is_none());
        assert!(err.detail.contains("no default browser"));
        assert!(!err.detail.contains("SECRET"), "{}", err.detail);
    }

    #[test]
    fn failing_openers_carry_no_local_path_in_the_detail() {
        let msg = "cannot start \\\\?\\C:\\Users\\someone\\browser.exe: denied";
        let url = open_target(&UrlTarget::BuildHelp, |_| Err::<(), _>(msg)).unwrap_err();
        let dir = tempfile::tempdir().unwrap();
        let folder = open_folder(dir.path(), |_| {
            Err::<(), _>("no shell for \\\\fileserver\\share\\someone\\x")
        })
        .unwrap_err();
        for dto in [url, folder] {
            assert!(
                !dto.detail.contains("someone") && !dto.detail.contains("fileserver"),
                "{}",
                dto.detail
            );
        }
    }

    #[test]
    fn only_a_known_target_name_deserializes() {
        for (name, target) in [
            ("signInHelp", UrlTarget::SignInHelp),
            ("buildHelp", UrlTarget::BuildHelp),
            ("deviceLogin", UrlTarget::DeviceLogin),
        ] {
            let parsed: UrlTarget = serde_json::from_str(&format!("\"{name}\"")).unwrap();
            assert_eq!(parsed, target);
        }
        for bad in [
            "\"https://evil.example/\"",
            "\"SignInHelp\"",
            "\"\"",
            "{\"url\":\"https://evil.example/\"}",
        ] {
            assert!(serde_json::from_str::<UrlTarget>(bad).is_err(), "{bad}");
        }
    }

    fn home() -> (tempfile::TempDir, Paths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path().join("home"));
        (dir, paths)
    }

    fn instance(paths: &Paths, id: &str, json: &str) {
        std::fs::create_dir_all(paths.instance(id)).unwrap();
        std::fs::write(paths.instance(id).join("instance.json"), json).unwrap();
    }

    fn by_id(id: &str) -> OpenTarget {
        OpenTarget::Instance { id: id.into() }
    }

    #[test]
    fn open_location_refuses_unknown_and_unsafe_ids() {
        use hengst_core::Error;
        let (_d, paths) = home();
        instance(&paths, "9.9", r#"{"format":1}"#);
        // A CLI folder without instance.json is not an instance.
        std::fs::create_dir_all(paths.instance("9.9-fabric").join("saves")).unwrap();
        for id in ["..\\x", "../x", "C:\\Windows", "a\\b", ""] {
            let e = resolve_location(&paths, &by_id(id)).unwrap_err();
            assert!(matches!(e, Error::UnsafePath(_)), "{id:?}: {e:?}");
        }
        for id in ["fehlt", "9.9-fabric"] {
            let e = resolve_location(&paths, &by_id(id)).unwrap_err();
            assert!(matches!(e, Error::InstanceNotFound { .. }), "{id:?}: {e:?}");
        }
        assert_eq!(
            resolve_location(&paths, &by_id("9.9")).unwrap(),
            paths.instance("9.9")
        );
    }

    #[test]
    fn open_location_opens_unreadable_and_newer_instances_and_makes_the_data_folder() {
        let (_d, paths) = home();
        instance(&paths, "bad", "{");
        instance(&paths, "newer", r#"{"format":2}"#);
        assert!(resolve_location(&paths, &by_id("bad")).is_ok());
        assert!(resolve_location(&paths, &by_id("newer")).is_ok());
        assert!(!paths.root().join("settings.json").exists());
        let root = resolve_location(&paths, &OpenTarget::DataDir).unwrap();
        assert_eq!(root, paths.root());
        assert!(root.is_dir());
    }

    #[cfg(windows)]
    #[test]
    fn open_location_refuses_a_linked_instance() {
        let (d, paths) = home();
        let outside = d.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("instance.json"), r#"{"format":1}"#).unwrap();
        std::fs::create_dir_all(paths.root().join("instances")).unwrap();
        let out = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(paths.instance("linked"))
            .arg(&outside)
            .output()
            .unwrap();
        assert!(out.status.success());
        let e = resolve_location(&paths, &by_id("linked")).unwrap_err();
        assert!(
            matches!(e, hengst_core::Error::InstanceLinked { .. }),
            "{e:?}"
        );
    }

    #[test]
    fn a_folder_is_handed_to_the_opener_as_an_absolute_path_and_only_if_it_is_a_folder() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("inst");
        std::fs::create_dir_all(&folder).unwrap();
        let mut seen = vec![];
        open_folder(&folder, |p| {
            seen.push(p.to_path_buf());
            Ok::<(), String>(())
        })
        .unwrap();
        assert_eq!(seen, [folder]);
        // A file, or nothing at all, is refused before the opener is called.
        let file = dir.path().join("a-file.txt");
        std::fs::write(&file, b"x").unwrap();
        for bad in [file, dir.path().join("missing")] {
            let mut called = false;
            let dto = open_folder(&bad, |_| {
                called = true;
                Ok::<(), String>(())
            })
            .unwrap_err();
            assert_eq!(dto.code, ErrorCode::OpenFailed);
            assert!(!called, "{}", bad.display());
        }
    }

    #[cfg(windows)]
    #[test]
    fn explorer_is_never_asked_to_run_a_file_even_if_the_folder_was_swapped_for_one() {
        // The state after the swap: the path now names a batch file. `explore` refuses it and
        // nothing runs; the verb "open" would run it (measured, see `explore_folder`).
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("executed.marker");
        let script = dir.path().join("swapped.bat");
        std::fs::write(
            &script,
            format!("@echo off\r\necho x > \"{}\"\r\n", marker.display()),
        )
        .unwrap();
        for path in [
            script.clone(),
            PathBuf::from(format!("{}\\", script.display())),
        ] {
            assert!(explore_folder(&path).is_err(), "{}", path.display());
        }
        std::thread::sleep(std::time::Duration::from_millis(1500));
        assert!(!marker.exists(), "the file was run");
    }

    #[test]
    fn a_failing_folder_opener_is_a_typed_error() {
        let dir = tempfile::tempdir().unwrap();
        let dto = open_folder(dir.path(), |_| {
            Err::<(), _>("no shell: https://x/y?token=SECRET")
        })
        .unwrap_err();
        assert_eq!(dto.code, ErrorCode::OpenFailed);
        assert!(!dto.detail.contains("SECRET"), "{}", dto.detail);
    }

    #[test]
    fn only_a_closed_target_deserializes() {
        for (json, target) in [
            (r#"{"kind":"dataDir"}"#, OpenTarget::DataDir),
            (r#"{"kind":"instance","id":"9.9"}"#, by_id("9.9")),
        ] {
            assert_eq!(serde_json::from_str::<OpenTarget>(json).unwrap(), target);
        }
        for bad in [
            r#"{"kind":"path","path":"C:\\Windows"}"#,
            r#""C:\\Windows""#,
            r#"{"kind":"instance"}"#,
            r#"{"kind":"DataDir"}"#,
        ] {
            assert!(serde_json::from_str::<OpenTarget>(bad).is_err(), "{bad}");
        }
    }
}
