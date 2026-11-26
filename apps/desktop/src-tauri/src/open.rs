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
        // A file, not a folder: `open_location` hands it to `resolve_log_path` and `open_file`.
        OpenTarget::LogFile { path } => Err(hengst_core::Error::UnsafePath(path.clone())),
        OpenTarget::GameLog { .. } => Err(hengst_core::Error::UnsafePath("game log".into())),
    }
}

/// The log file a path from the web view names, or why it is not opened. Only the two forms the
/// core reports are accepted, relative to the data folder: `logs/<name>.log` and
/// `instances/<id>/logs/<name>.log` (with an id the core's `instance_dir` accepts). Each part must
/// be a plain name (no `..`, no drive, no `:` so no alternate data stream, no device name like
/// `CON`); this is decided on the text alone, so a UNC or `\\?\` path never reaches the file
/// system. Then the path is resolved with `canonicalize`, which follows links and junctions: the
/// result must lie in the canonical logs folder it named, still end in `.log` (any case) and be
/// a file. A link that leads out of the logs folder is refused, because its target lies outside.
pub fn resolve_log_path(paths: &Paths, path: &str) -> hengst_core::Result<PathBuf> {
    let unsafe_path = || hengst_core::Error::UnsafePath(path.to_string());
    let parts: Vec<&str> = path.split(['/', '\\']).collect();
    let plain = |part: &&str| {
        hengst_core::paths::validate_id(part).is_ok()
            && !part.ends_with('.')
            && !is_device_name(part)
    };
    if !parts.iter().all(plain) {
        return Err(unsafe_path());
    }
    let is_log = |name: &Path| {
        name.extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("log"))
    };
    let root = std::fs::canonicalize(paths.root())?;
    let (base, name) = match parts.as_slice() {
        ["logs", rest @ ..] if !rest.is_empty() => (root.join("logs"), rest),
        ["instances", id, "logs", rest @ ..] if !rest.is_empty() => {
            instances::instance_dir(paths, id)?;
            (root.join("instances").join(id).join("logs"), rest)
        }
        _ => return Err(unsafe_path()),
    };
    let lexical = name.iter().fold(base.clone(), |p, part| p.join(part));
    if !is_log(&lexical) {
        return Err(unsafe_path());
    }
    let file = std::fs::canonicalize(&lexical)?;
    if !file.starts_with(&base) || !is_log(&file) || !std::fs::metadata(&file)?.is_file() {
        return Err(unsafe_path());
    }
    plain_log_path(&file)
}

/// The checked canonical path in the spelling the shell and `.log` handlers expect:
/// `canonicalize` gives `\\?\C:\...` (or `\\?\UNC\...`), which `ShellExecuteW` and many handlers
/// refuse. A verbatim form without a plain spelling (`\\?\Volume{..}`) is refused.
fn plain_log_path(file: &Path) -> hengst_core::Result<PathBuf> {
    crate::recycle::strip_verbatim(file)
        .ok_or_else(|| hengst_core::Error::UnsafePath(file.display().to_string()))
}

/// The names Windows reserves for devices, with or without an extension (`CON`, `nul.log`,
/// `COM1.txt`, `AUX .log`).
fn is_device_name(part: &str) -> bool {
    let stem = part
        .split('.')
        .next()
        .unwrap_or(part)
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    let numbered = |prefix: &str| {
        stem.strip_prefix(prefix)
            .is_some_and(|n| n.len() == 1 && n.as_bytes()[0].is_ascii_digit())
    };
    matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || numbered("COM")
        || numbered("LPT")
}

/// The launcher log file of game run `run_id`, checked as `resolve_log_path` checks a path from
/// the web view (the path the core reported, relative to the data folder). Not held, or no file:
/// `OpenFailed`, without a path in the detail.
pub fn resolve_game_log(
    paths: &Paths,
    log: &crate::log::LiveLog,
    run_id: crate::dto::play::RunId,
) -> Result<PathBuf, ErrorDto> {
    let failed = |detail: &str| ErrorDto {
        code: ErrorCode::OpenFailed,
        params: BTreeMap::new(),
        action: None,
        detail: detail.to_string(),
    };
    let file = log
        .log_file(run_id)
        .ok_or_else(|| failed("this run has no launcher log file"))?;
    let path = crate::play::relative(paths.root(), &file)
        .ok_or_else(|| failed("the launcher log file is outside the data folder"))?;
    resolve_log_path(paths, &path).map_err(|e| log_error(&e))
}

/// A log that could not be opened, refused or missing: `OpenFailed`, with the detail of
/// `error_dto` (redacted, local paths cut to their last component).
pub fn log_error(e: &hengst_core::Error) -> ErrorDto {
    ErrorDto {
        code: ErrorCode::OpenFailed,
        action: None,
        params: BTreeMap::new(),
        ..error_dto(e)
    }
}

/// Whether Windows has a program for `.log` files (`log_handler`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogHandler {
    Present,
    /// No association, or only the app picker.
    Absent,
    /// The query failed.
    Unknown,
}

/// Shows a log, if it is a file right now: in the program Windows has for `.log` files (`open`,
/// `shell_open_file` in the app), else selected in its folder in Explorer (`reveal`,
/// `reveal_in_explorer`). Never "open" without a program: Windows would show its app picker. As
/// for folders, the file check can be outrun; what is shown is still a path `resolve_log_path`
/// let through, with the `.log` extension.
pub fn open_log_file<E: Display>(
    file: &Path,
    handler: LogHandler,
    open: impl FnOnce(&Path) -> Result<(), E>,
    reveal: impl FnOnce(&Path) -> Result<(), E>,
) -> Result<(), ErrorDto> {
    let failed = |detail: String| ErrorDto {
        code: ErrorCode::OpenFailed,
        params: BTreeMap::new(),
        action: None,
        detail: strip_paths(&redact(&detail)),
    };
    if !std::fs::metadata(file).is_ok_and(|m| m.is_file()) {
        return Err(failed("the path is not a file".to_string()));
    }
    let shown = match handler {
        LogHandler::Present => open(file),
        LogHandler::Absent | LogHandler::Unknown => reveal(file),
    };
    shown.map_err(|e| failed(e.to_string()))
}

/// The program `.log` files open with, judged from its executable: the app picker
/// (`OpenWith.exe`, `PickerHost.exe`) or nothing is no program.
fn handler_from_executable(exe: Option<&str>) -> LogHandler {
    let name = exe
        .map(Path::new)
        .and_then(Path::file_name)
        .map(|n| n.to_string_lossy().to_ascii_lowercase());
    match name.as_deref() {
        None | Some("openwith.exe") | Some("pickerhost.exe") => LogHandler::Absent,
        Some(_) => LogHandler::Present,
    }
}

/// Asks Windows which program opens `.log` files (`AssocQueryStringW`, `ASSOCSTR_EXECUTABLE`,
/// verb "open"). `ASSOCF_INIT_IGNOREUNKNOWN`: an extension without an association must not be
/// answered with the "Unknown" class, whose program is the app picker.
#[cfg(windows)]
pub fn log_handler() -> LogHandler {
    use windows_sys::Win32::Foundation::ERROR_NO_ASSOCIATION;
    use windows_sys::Win32::UI::Shell::{
        AssocQueryStringW, ASSOCF_INIT_IGNOREUNKNOWN, ASSOCSTR_EXECUTABLE,
    };
    let ext: Vec<u16> = ".log".encode_utf16().chain(Some(0)).collect();
    let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
    let mut buf = vec![0u16; 1024];
    let mut len = buf.len() as u32;
    // SAFETY: NUL-terminated inputs, an output buffer of `len` characters that outlives the call.
    let hr = unsafe {
        AssocQueryStringW(
            ASSOCF_INIT_IGNOREUNKNOWN,
            ASSOCSTR_EXECUTABLE,
            ext.as_ptr(),
            verb.as_ptr(),
            buf.as_mut_ptr(),
            &mut len,
        )
    };
    // HRESULT_FROM_WIN32(ERROR_NO_ASSOCIATION).
    let no_association = (0x8007_0000u32 | ERROR_NO_ASSOCIATION) as i32;
    if hr == no_association {
        return LogHandler::Absent;
    }
    if hr != 0 {
        return LogHandler::Unknown;
    }
    let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    handler_from_executable(Some(&String::from_utf16_lossy(&buf[..end])))
}

#[cfg(not(windows))]
pub fn log_handler() -> LogHandler {
    LogHandler::Unknown
}

/// Selects `file` in its folder in Explorer (`SHOpenFolderAndSelectItems`): nothing is run, the
/// user sees the log and can open it with a program of their choice.
#[cfg(windows)]
pub fn reveal_in_explorer(file: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::System::Com::{
        CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED,
    };
    use windows_sys::Win32::UI::Shell::{ILCreateFromPathW, ILFree, SHOpenFolderAndSelectItems};
    let com = unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) };
    let name: Vec<u16> = file.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: a NUL-terminated path; the returned list is freed below and not used after.
    let pidl = unsafe { ILCreateFromPathW(name.as_ptr()) };
    let hr = if pidl.is_null() {
        -1
    } else {
        // SAFETY: `pidl` is a valid absolute item list; no child items are passed.
        let hr = unsafe { SHOpenFolderAndSelectItems(pidl, 0, std::ptr::null(), 0) };
        unsafe { ILFree(pidl) };
        hr
    };
    if com >= 0 {
        unsafe { CoUninitialize() };
    }
    if hr == 0 {
        Ok(())
    } else {
        Err(format!("Explorer could not show the log (code {hr:#x})"))
    }
}

#[cfg(not(windows))]
pub fn reveal_in_explorer(_file: &Path) -> Result<(), String> {
    Err("showing files is implemented for Windows only".to_string())
}

/// Opens a file in the program Windows associates with its extension: `ShellExecuteW` with the
/// verb "open". Only `open_location` calls it, with a `.log` path from `resolve_log_path`.
#[cfg(windows)]
pub fn shell_open_file(file: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::System::Com::{
        CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED,
    };
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    // COM as in `explore_folder`.
    let com = unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) };
    let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
    let name: Vec<u16> = file.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: both strings are NUL-terminated and outlive the call; the other arguments are null.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            name.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    } as isize;
    if com >= 0 {
        unsafe { CoUninitialize() };
    }
    if result > 32 {
        Ok(())
    } else {
        Err(format!("Windows could not open the log (code {result})"))
    }
}

#[cfg(not(windows))]
pub fn shell_open_file(_file: &Path) -> Result<(), String> {
    Err("opening files is implemented for Windows only".to_string())
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

    /// A data folder with a log in `logs` and one in the logs of the instance `9.9`.
    fn logs_home() -> (tempfile::TempDir, Paths) {
        let (d, paths) = home();
        std::fs::create_dir_all(paths.logs()).unwrap();
        std::fs::write(paths.logs().join("loader-install-forge-9.9.log"), "x").unwrap();
        instance(&paths, "9.9", r#"{"format":1}"#);
        std::fs::create_dir_all(paths.instance("9.9").join("logs")).unwrap();
        std::fs::write(paths.instance("9.9").join("logs").join("latest.log"), "x").unwrap();
        (d, paths)
    }

    fn refused(paths: &Paths, path: &str) {
        assert!(
            resolve_log_path(paths, path).is_err(),
            "{path:?} was accepted"
        );
    }

    #[test]
    fn a_checked_log_path_is_handed_on_in_its_plain_spelling() {
        let plain = |p: &str| plain_log_path(Path::new(p)).ok();
        assert_eq!(
            plain("\\\\?\\C:\\data\\logs\\x.log"),
            Some(PathBuf::from("C:\\data\\logs\\x.log"))
        );
        assert_eq!(
            plain("\\\\?\\UNC\\server\\share\\logs\\x.log"),
            Some(PathBuf::from("\\\\server\\share\\logs\\x.log"))
        );
        assert_eq!(
            plain("C:\\data\\logs\\x.log"),
            Some(PathBuf::from("C:\\data\\logs\\x.log"))
        );
        // Verbatim forms without a plain spelling are refused.
        for p in [
            "\\\\?\\Volume{12345678-1234-1234-1234-123456789abc}\\logs\\x.log",
            "\\\\?\\GLOBALROOT\\Device\\HarddiskVolume1\\logs\\x.log",
        ] {
            assert_eq!(plain(p), None, "{p}");
        }
    }

    #[test]
    fn log_paths_the_core_reports_resolve_to_the_file() {
        let (_d, paths) = logs_home();
        // The plain spelling: the opener and the .log handler get no `\\?\` path.
        let root =
            crate::recycle::strip_verbatim(&std::fs::canonicalize(paths.root()).unwrap()).unwrap();
        for path in [
            "logs/loader-install-forge-9.9.log",
            "instances/9.9/logs/latest.log",
        ] {
            let resolved = resolve_log_path(&paths, path).unwrap();
            assert!(
                !resolved.to_string_lossy().starts_with("\\\\?\\"),
                "{}",
                resolved.display()
            );
        }
        assert_eq!(
            resolve_log_path(&paths, "logs/loader-install-forge-9.9.log").unwrap(),
            root.join("logs").join("loader-install-forge-9.9.log")
        );
        assert_eq!(
            resolve_log_path(&paths, "instances/9.9/logs/latest.log").unwrap(),
            root.join("instances")
                .join("9.9")
                .join("logs")
                .join("latest.log")
        );
        // Backslashes as Windows writes them.
        assert!(resolve_log_path(&paths, "logs\\loader-install-forge-9.9.log").is_ok());
    }

    #[test]
    fn log_path_outside_the_logs_directories_is_refused() {
        let (d, paths) = logs_home();
        std::fs::write(paths.root().join("stray.log"), "x").unwrap();
        std::fs::write(paths.instance("9.9").join("stray.log"), "x").unwrap();
        std::fs::create_dir_all(paths.root().join("versions")).unwrap();
        std::fs::write(paths.root().join("versions").join("v.log"), "x").unwrap();
        let outside = d.path().join("outside.log");
        std::fs::write(&outside, "x").unwrap();
        for path in [
            "stray.log",
            "instances/9.9/stray.log",
            "versions/v.log",
            "instances/logs/x.log",
        ] {
            refused(&paths, path);
        }
        // Absolute paths, also ones that name a log inside the data folder: the core reports
        // relative paths only.
        let inside = paths.logs().join("loader-install-forge-9.9.log");
        for abs in [&outside, &inside] {
            refused(&paths, &abs.display().to_string());
        }
        refused(&paths, "\\logs\\loader-install-forge-9.9.log");
        refused(&paths, "/logs/loader-install-forge-9.9.log");
        refused(&paths, "C:logs\\loader-install-forge-9.9.log");
        // UNC, verbatim and device paths are refused before anything touches the file system.
        for path in [
            "\\\\server\\share\\logs\\x.log",
            "//server/share/logs/x.log",
            "\\\\?\\C:\\Windows\\x.log",
            "\\\\?\\UNC\\server\\share\\x.log",
            "\\\\.\\pipe\\x.log",
        ] {
            refused(&paths, path);
        }
        // Not the logs of an instance: an id that is not one, or no instance at all.
        std::fs::create_dir_all(paths.instance("loose").join("logs")).unwrap();
        std::fs::write(paths.instance("loose").join("logs").join("x.log"), "x").unwrap();
        for path in [
            "instances/loose/logs/x.log",
            "instances/CON/logs/x.log",
            "instances/a:b/logs/x.log",
            "instances//logs/x.log",
        ] {
            refused(&paths, path);
        }
    }

    #[test]
    fn log_path_needs_the_log_extension() {
        let (_d, paths) = logs_home();
        for name in ["x.txt", "x.log.exe", "x", "x.log.", "x.log ", "x.bat"] {
            let _ = std::fs::write(paths.logs().join(name), "x");
            refused(&paths, &format!("logs/{name}"));
        }
        std::fs::write(paths.logs().join("UPPER.LOG"), "x").unwrap();
        std::fs::write(paths.logs().join("mixed.Log"), "x").unwrap();
        assert!(resolve_log_path(&paths, "logs/UPPER.LOG").is_ok());
        assert!(resolve_log_path(&paths, "logs/mixed.Log").is_ok());
        // A folder named like a log, and a log that is not there.
        std::fs::create_dir_all(paths.logs().join("folder.log")).unwrap();
        refused(&paths, "logs/folder.log");
        refused(&paths, "logs/missing.log");
        refused(&paths, "");
        refused(&paths, "logs");
    }

    #[test]
    fn log_path_traversal_is_refused() {
        let (d, paths) = logs_home();
        std::fs::write(paths.root().join("settings.log"), "x").unwrap();
        std::fs::write(d.path().join("outside.log"), "x").unwrap();
        for path in [
            "logs/../settings.log",
            "logs\\..\\settings.log",
            "logs/../../outside.log",
            "logs/./loader-install-forge-9.9.log",
            "./logs/loader-install-forge-9.9.log",
            "instances/9.9/logs/../../../logs/loader-install-forge-9.9.log",
            "instances/../logs/loader-install-forge-9.9.log",
            "logs/%2e%2e/settings.log",
            "logs//loader-install-forge-9.9.log",
        ] {
            refused(&paths, path);
        }
    }

    #[test]
    fn log_path_streams_and_device_names_are_refused() {
        let (_d, paths) = logs_home();
        for path in [
            "logs/loader-install-forge-9.9.log:secret",
            "logs/loader-install-forge-9.9.log::$DATA",
            "logs/x:y.log",
            "logs/CON.log",
            "logs/nul.log",
            "logs/com1.log",
            "logs/LPT9.log",
            "logs/AUX .log",
            "logs/prn",
            "instances/9.9/logs/CONIN$.log",
        ] {
            refused(&paths, path);
        }
    }

    /// Runs `open_log_file` with recording openers: which one got which path.
    fn open_with(
        file: &Path,
        handler: LogHandler,
    ) -> (Result<(), ErrorDto>, Vec<(&'static str, PathBuf)>) {
        let seen = std::cell::RefCell::new(vec![]);
        let result = open_log_file(
            file,
            handler,
            |p| {
                seen.borrow_mut().push(("open", p.to_path_buf()));
                Ok::<(), String>(())
            },
            |p| {
                seen.borrow_mut().push(("reveal", p.to_path_buf()));
                Ok::<(), String>(())
            },
        );
        (result, seen.into_inner())
    }

    #[test]
    fn a_log_opens_in_its_program_when_windows_has_one_for_log_files() {
        let (_d, paths) = logs_home();
        let file = resolve_log_path(&paths, "logs/loader-install-forge-9.9.log").unwrap();
        let (result, seen) = open_with(&file, LogHandler::Present);
        result.unwrap();
        assert_eq!(seen, [("open", file)]);
    }

    #[test]
    fn without_a_program_for_log_files_the_log_is_selected_in_explorer() {
        let (_d, paths) = logs_home();
        let file = resolve_log_path(&paths, "logs/loader-install-forge-9.9.log").unwrap();
        // No association, and a query that failed: never "open" (that would be the app picker).
        for handler in [LogHandler::Absent, LogHandler::Unknown] {
            let (result, seen) = open_with(&file, handler);
            result.unwrap();
            assert_eq!(seen, [("reveal", file.clone())], "{handler:?}");
        }
    }

    #[test]
    fn a_log_that_is_not_a_file_reaches_no_opener() {
        let (_d, paths) = logs_home();
        for bad in [paths.logs(), paths.logs().join("missing.log")] {
            for handler in [LogHandler::Present, LogHandler::Absent] {
                let (result, seen) = open_with(&bad, handler);
                assert_eq!(result.unwrap_err().code, ErrorCode::OpenFailed);
                assert!(seen.is_empty(), "{}", bad.display());
            }
        }
    }

    #[test]
    fn the_app_picker_does_not_count_as_a_program_for_log_files() {
        for exe in [
            "C:\\Windows\\System32\\OpenWith.exe",
            "C:\\Windows\\System32\\PickerHost.exe",
            "c:\\windows\\system32\\openwith.EXE",
        ] {
            assert_eq!(
                handler_from_executable(Some(exe)),
                LogHandler::Absent,
                "{exe}"
            );
        }
        assert_eq!(
            handler_from_executable(Some("C:\\Program Files\\Notepad++\\notepad++.exe")),
            LogHandler::Present
        );
        assert_eq!(handler_from_executable(Some("")), LogHandler::Absent);
        assert_eq!(handler_from_executable(None), LogHandler::Absent);
    }

    /// Run by hand only, on the agent desktop (it opens a real window): a data folder
    /// `%TEMP%\hengst-manual-log` with `logs\x.log` is made and its log shown as the app does,
    /// with this machine's association (or none). Remove the folder afterwards.
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn manual_open_a_seeded_log_as_the_app_does() {
        let paths = Paths::new(std::env::temp_dir().join("hengst-manual-log"));
        std::fs::create_dir_all(paths.logs()).unwrap();
        std::fs::write(paths.logs().join("x.log"), "a log line\r\n").unwrap();
        let file = resolve_log_path(&paths, "logs/x.log").unwrap();
        let handler = log_handler();
        eprintln!("handler: {handler:?}, file: {}", file.display());
        open_log_file(&file, handler, shell_open_file, reveal_in_explorer).unwrap();
        // Explorer and the handlers start asynchronously; give them time before the process ends.
        std::thread::sleep(std::time::Duration::from_secs(8));
    }

    #[test]
    fn a_refused_log_is_open_failed_without_a_local_path() {
        let (d, paths) = logs_home();
        let outside = d.path().join("someone").join("x.log");
        let e = resolve_log_path(&paths, &outside.display().to_string()).unwrap_err();
        let dto = log_error(&e);
        assert_eq!(dto.code, ErrorCode::OpenFailed);
        assert!(dto.action.is_none());
        assert!(!dto.detail.contains("someone"), "{}", dto.detail);
        let missing = log_error(&resolve_log_path(&paths, "logs/missing.log").unwrap_err());
        assert_eq!(missing.code, ErrorCode::OpenFailed);
    }

    /// Creates a junction at `link` pointing at `target`; `false` if Windows refused (the test
    /// then skips: no elevation is attempted).
    #[cfg(windows)]
    fn junction(link: &Path, target: &Path) -> bool {
        let out = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .unwrap();
        if !out.status.success() {
            eprintln!(
                "SKIPPED: could not create a junction (mklink /J): {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        out.status.success()
    }

    #[cfg(windows)]
    #[test]
    fn log_path_through_a_junction_out_of_logs_is_refused() {
        let (d, paths) = logs_home();
        let outside = d.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("x.log"), "x").unwrap();
        // A junction inside logs, the logs folder of an instance as a junction, and the logs
        // folder itself as one.
        let inner = paths.logs().join("out");
        let instance_logs = paths.instance("9.9").join("logs");
        std::fs::remove_dir_all(&instance_logs).unwrap();
        let cases = [
            (inner.clone(), "logs/out/x.log"),
            (instance_logs.clone(), "instances/9.9/logs/x.log"),
        ];
        for (link, path) in &cases {
            if !junction(link, &outside) {
                return;
            }
            refused(&paths, path);
        }
        for (link, _) in &cases {
            std::fs::remove_dir(link).unwrap();
        }
        std::fs::remove_dir_all(paths.logs()).unwrap();
        if !junction(&paths.logs(), &outside) {
            return;
        }
        refused(&paths, "logs/x.log");
        std::fs::remove_dir(paths.logs()).unwrap();
        assert!(outside.join("x.log").exists(), "the target stays untouched");
    }

    #[test]
    fn only_a_closed_target_deserializes() {
        for (json, target) in [
            (r#"{"kind":"dataDir"}"#, OpenTarget::DataDir),
            (r#"{"kind":"instance","id":"9.9"}"#, by_id("9.9")),
            (
                r#"{"kind":"logFile","path":"logs/x.log"}"#,
                OpenTarget::LogFile {
                    path: "logs/x.log".into(),
                },
            ),
            (
                r#"{"kind":"gameLog","runId":3}"#,
                OpenTarget::GameLog {
                    run_id: crate::dto::play::RunId(3),
                },
            ),
        ] {
            assert_eq!(serde_json::from_str::<OpenTarget>(json).unwrap(), target);
        }
        for bad in [
            r#"{"kind":"path","path":"C:\\Windows"}"#,
            r#""C:\\Windows""#,
            r#"{"kind":"instance"}"#,
            r#"{"kind":"DataDir"}"#,
            r#"{"kind":"gameLog","path":"logs/game/x.log"}"#,
            r#"{"kind":"gameLog","runId":"logs/game/x.log"}"#,
        ] {
            assert!(serde_json::from_str::<OpenTarget>(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_game_log_is_found_by_its_run_id_and_checked_like_any_log() {
        use crate::dto::play::RunId;
        use crate::log::LiveLog;
        let (d, paths) = home();
        let dir = paths.root().join("logs").join("game");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("9.9-20261126-101010.log");
        std::fs::write(&file, "[10:10:10] [main/INFO]: hello\n").unwrap();
        let outside = d.path().join("elsewhere.log");
        std::fs::write(&outside, "x").unwrap();
        let log = LiveLog::new();
        log.start_run(RunId(1), "9.9", 1, Some(&file));
        log.start_run(RunId(2), "9.9", 2, None);
        let found = resolve_game_log(&paths, &log, RunId(1)).unwrap();
        assert!(!found.to_string_lossy().starts_with("\\\\?\\"), "{found:?}");
        assert!(
            found.ends_with("logs/game/9.9-20261126-101010.log"),
            "{found:?}"
        );
        assert_eq!(
            std::fs::read_to_string(&found).unwrap(),
            std::fs::read_to_string(&file).unwrap()
        );
        // Not held, held without a file, a file that is gone, a file outside the data folder.
        let failures = [
            resolve_game_log(&paths, &log, RunId(9)).unwrap_err(),
            resolve_game_log(&paths, &log, RunId(2)).unwrap_err(),
            {
                std::fs::remove_file(&file).unwrap();
                resolve_game_log(&paths, &log, RunId(1)).unwrap_err()
            },
            {
                log.start_run(RunId(3), "9.9", 3, Some(&outside));
                resolve_game_log(&paths, &log, RunId(3)).unwrap_err()
            },
        ];
        let temp = d.path().file_name().unwrap().to_string_lossy().to_string();
        for e in failures {
            assert_eq!(e.code, ErrorCode::OpenFailed, "{e:?}");
            assert!(e.action.is_none());
            assert!(
                !e.detail.contains(&temp) && !e.detail.contains("AppData"),
                "{}",
                e.detail
            );
        }
    }
}
