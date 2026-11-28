//! The Java setting: which paths may be used as the game's `java.exe`, and the check that runs a
//! chosen one with `-version`.
//!
//! The path is executed when the game starts, so every use checks it again, in this order:
//!
//! 1. Text only, before any file-system call: no control or bidi characters, a drive letter
//!    followed by a separator (`C:\...`), no second `:` (no alternate data stream). UNC
//!    (`\\server\share`, `//server/share`), verbatim and device forms (`\\?\`, `\\.\`, `\??\`),
//!    relative, drive-relative (`C:java.exe`) and root-relative (`\Java\java.exe`) paths are refused,
//!    so a path that names a server never makes Windows connect to it (and send the user's
//!    credentials). The file name must be `java.exe` or `javaw.exe` (any case).
//! 2. The drive: its DOS device target (an object-manager lookup, no I/O on the drive) must not be
//!    a network redirector (mapped drives, WebDAV, RDP drives) and a `subst` drive is followed to
//!    its own drive; then `GetDriveTypeW` must say fixed, removable or RAM disk. Network, optical,
//!    unknown and missing drives are refused.
//! 3. The folders, one by one from the drive root without following links: a link (symlink or
//!    junction) is read as text, its target goes through steps 1 and 2 again before anything under
//!    it is touched.
//! 4. The canonical path (`std::fs::canonicalize`, verbatim prefix removed) is checked again with
//!    steps 1 and 2 and must be an existing regular file. The canonical path is what is saved and
//!    what is started.
//!
//! The check `java.exe -version` (`probe_version`) runs only for a file the user has just chosen
//! in the native dialog, never for a stored path and never for a path from the web view.

use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use ts_rs::TS;

/// Why a Java path cannot be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum JavaProblem {
    /// Not a plain path on a local drive (UNC, network drive, device or relative path, or a link
    /// leading to one).
    Location,
    /// Not a file named `java.exe` or `javaw.exe` (also: a folder of that name).
    Name,
    /// The file does not exist (any more).
    Missing,
    /// `-version` failed: did not start, timed out, printed too much or no version, exited with
    /// an error.
    NotStarted,
}

impl JavaProblem {
    /// The `params.reason` of the error.
    pub fn key(self) -> &'static str {
        match self {
            JavaProblem::Location => "location",
            JavaProblem::Name => "name",
            JavaProblem::Missing => "missing",
            JavaProblem::NotStarted => "notStarted",
        }
    }
}

/// Step 1: the checks on the text alone. Never touches the file system.
pub fn check_text(path: &str) -> Result<(), JavaProblem> {
    if path.chars().any(|c| c.is_control() || is_bidi_control(c)) {
        return Err(JavaProblem::Location);
    }
    let b = path.as_bytes();
    let drive_absolute = b.len() >= 3
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
        && (b[2] == b'\\' || b[2] == b'/');
    if !drive_absolute || path[2..].contains(':') {
        return Err(JavaProblem::Location);
    }
    let name = path.rsplit(['\\', '/']).next().unwrap_or("");
    if name.eq_ignore_ascii_case("java.exe") || name.eq_ignore_ascii_case("javaw.exe") {
        Ok(())
    } else {
        Err(JavaProblem::Name)
    }
}

/// U+200E, U+200F, U+061C and the embedding, override and isolate controls.
fn is_bidi_control(c: char) -> bool {
    matches!(
        c,
        '\u{200E}' | '\u{200F}' | '\u{061C}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'
    )
}

/// `\\?\C:\x` and `\??\C:\x` become `C:\x`; every other form stays as it is (and is refused).
fn strip_verbatim(text: &str) -> &str {
    for prefix in [r"\\?\", r"\??\"] {
        if let Some(rest) = text.strip_prefix(prefix) {
            let b = rest.as_bytes();
            if b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\' {
                return rest;
            }
        }
    }
    text
}

/// Step 1 without the name check (for folders and link targets), then step 2.
fn check_location(path: &str) -> Result<(), JavaProblem> {
    match check_text(path) {
        Ok(()) | Err(JavaProblem::Name) => {}
        Err(e) => return Err(e),
    }
    if drive_is_local(path.as_bytes()[0] as char, 0) {
        Ok(())
    } else {
        Err(JavaProblem::Location)
    }
}

/// Any failure to read a component that passed the location checks: it is not there (or not
/// readable, which for the user is the same).
fn missing(_: std::io::Error) -> JavaProblem {
    JavaProblem::Missing
}

/// How many links one path may pass through.
const MAX_LINKS: u32 = 16;

/// Step 3: walks `path` (absolute) component by component without following links; each link's
/// target is checked as text and by its drive before it is walked in turn.
fn resolve(path: &Path, depth: u32) -> Result<PathBuf, JavaProblem> {
    check_location(path.to_str().ok_or(JavaProblem::Location)?)?;
    let mut current = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => current.push(component),
            Component::CurDir => {}
            Component::ParentDir => {
                current.pop();
            }
            Component::Normal(name) => {
                current.push(name);
                let meta = std::fs::symlink_metadata(&current).map_err(missing)?;
                // Symlinks and junctions (name-surrogate reparse points).
                if meta.file_type().is_symlink() {
                    if depth >= MAX_LINKS {
                        return Err(JavaProblem::Location);
                    }
                    let target = std::fs::read_link(&current).map_err(|_| JavaProblem::Location)?;
                    let target = link_target(&current, &target)?;
                    current = resolve(&target, depth + 1)?;
                }
            }
        }
    }
    Ok(current)
}

/// Step 2 for drive `letter`.
#[cfg(windows)]
fn drive_is_local(letter: char, depth: u32) -> bool {
    use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, QueryDosDeviceW};
    use windows_sys::Win32::System::WindowsProgramming::{
        DRIVE_FIXED, DRIVE_RAMDISK, DRIVE_REMOVABLE,
    };
    if depth > 4 {
        return false;
    }
    let letter = letter.to_ascii_uppercase();
    let name: Vec<u16> = format!("{letter}:").encode_utf16().chain(Some(0)).collect();
    let mut buf = vec![0u16; 1024];
    // SAFETY: `name` is NUL-terminated; `buf` is writable for `buf.len()` u16 units.
    let n = unsafe { QueryDosDeviceW(name.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) };
    if n == 0 {
        return false;
    }
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    match classify_device(&String::from_utf16_lossy(&buf[..end])) {
        Device::Network => return false,
        Device::Subst(other) => return drive_is_local(other, depth + 1),
        Device::Local => {}
    }
    let root: Vec<u16> = format!("{letter}:\\")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    // SAFETY: `root` is a NUL-terminated wide string.
    let kind = unsafe { GetDriveTypeW(root.as_ptr()) };
    matches!(kind, DRIVE_FIXED | DRIVE_REMOVABLE | DRIVE_RAMDISK)
}

#[cfg(not(windows))]
fn drive_is_local(_: char, _: u32) -> bool {
    false
}

/// Reads up to `max` bytes of a pipe; past that it sets `overflow` and stops reading.
fn capture(
    mut pipe: impl std::io::Read + Send + 'static,
    max: usize,
    overflow: Arc<AtomicBool>,
) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut kept = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            match pipe.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if kept.len() + n > max {
                        overflow.store(true, Ordering::SeqCst);
                        break;
                    }
                    kept.extend_from_slice(&chunk[..n]);
                }
            }
        }
        kept
    })
}

/// What a drive letter's DOS device target says about the drive.
#[derive(Debug, PartialEq, Eq)]
pub enum Device {
    /// A volume of this machine (`\Device\HarddiskVolume3`, a RAM disk, ...).
    Local,
    /// A network redirector or a UNC target.
    Network,
    /// A `subst` drive onto another drive letter.
    Subst(char),
}

/// Classifies the target `QueryDosDeviceW` returns for `X:`.
pub fn classify_device(target: &str) -> Device {
    if let Some(rest) = target.strip_prefix(r"\??\") {
        let b = rest.as_bytes();
        if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
            return Device::Subst(b[0].to_ascii_uppercase() as char);
        }
        return Device::Network;
    }
    let lower = target.to_ascii_lowercase();
    // Network redirectors name their mappings `\;X:<session>\server\share`.
    if !lower.starts_with(r"\device\")
        || lower.starts_with(r"\device\mup")
        || lower.contains(r"\;")
        || lower.contains("redirector")
    {
        return Device::Network;
    }
    Device::Local
}

/// The target of a link at `link`, made absolute, verbatim prefix removed, and checked as text
/// (step 1 without the name, which only the last component has).
pub fn link_target(link: &Path, target: &Path) -> Result<PathBuf, JavaProblem> {
    let text = strip_verbatim(target.to_str().ok_or(JavaProblem::Location)?);
    let relative =
        !text.starts_with(['\\', '/']) && !text.as_bytes().get(1).is_some_and(|&c| c == b':');
    let joined = if relative {
        link.parent().ok_or(JavaProblem::Location)?.join(text)
    } else {
        PathBuf::from(text)
    };
    let joined = joined.to_str().ok_or(JavaProblem::Location)?;
    match check_text(joined) {
        Ok(()) | Err(JavaProblem::Name) => {}
        Err(e) => return Err(e),
    }
    // Lexical only (GetFullPathNameW, no file-system access): resolves `..` as Windows will.
    std::path::absolute(joined).map_err(|_| JavaProblem::Location)
}

/// Steps 1 to 4. `Ok` is the canonical path to save and to start.
pub fn validate_java(path: &Path) -> Result<PathBuf, JavaProblem> {
    let text = path.to_str().ok_or(JavaProblem::Location)?;
    check_text(text)?;
    // Lexical: `C:\x\..\java.exe` is checked as what Windows will open.
    let absolute = std::path::absolute(text).map_err(|_| JavaProblem::Location)?;
    check_text(absolute.to_str().ok_or(JavaProblem::Location)?)?;
    let resolved = resolve(&absolute, 0)?;
    let canonical = std::fs::canonicalize(&resolved).map_err(missing)?;
    let canonical = PathBuf::from(strip_verbatim(
        canonical.to_str().ok_or(JavaProblem::Location)?,
    ));
    check_location(canonical.to_str().ok_or(JavaProblem::Location)?)?;
    check_text(canonical.to_str().ok_or(JavaProblem::Location)?)?;
    let meta = std::fs::metadata(&canonical).map_err(missing)?;
    if !meta.is_file() {
        return Err(JavaProblem::Name);
    }
    Ok(canonical)
}

/// Limits of `probe_version`.
#[derive(Clone, Copy, Debug)]
pub struct ProbeLimits {
    pub timeout: Duration,
    /// Bytes per stream; more ends the check.
    pub max_output: usize,
}

/// The limits the app uses: a JVM prints its version within a second, a few seconds allow for a
/// cold disk or a virus scan.
pub const PROBE_LIMITS: ProbeLimits = ProbeLimits {
    timeout: Duration::from_secs(8),
    max_output: 64 * 1024,
};

/// Why `-version` failed.
#[derive(Debug, PartialEq, Eq)]
pub enum ProbeError {
    Spawn,
    TimedOut,
    TooMuchOutput,
    Exit(i32),
    NoVersion,
}

impl std::fmt::Display for ProbeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProbeError::Spawn => write!(f, "the Java check could not start the program"),
            ProbeError::TimedOut => write!(f, "the Java check did not end in time and was stopped"),
            ProbeError::TooMuchOutput => {
                write!(f, "the Java check printed too much and was stopped")
            }
            ProbeError::Exit(code) => write!(f, "the Java check exited with code {code}"),
            ProbeError::NoVersion => write!(f, "the Java check printed no Java version"),
        }
    }
}

/// Runs `<exe> -version` in a kill-on-close job without a console window, with the folder of
/// `exe` as the working directory and the app's environment unchanged, and returns the version it
/// prints. Killed after `limits.timeout` or once a stream exceeds `limits.max_output`.
pub fn probe_version(exe: &Path, limits: ProbeLimits) -> Result<String, ProbeError> {
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("-version")
        .current_dir(exe.parent().ok_or(ProbeError::Spawn)?)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let job = hengst_core::process::Job::new_kill_on_close().map_err(|_| ProbeError::Spawn)?;
    let mut child = job.spawn(&mut cmd).map_err(|_| ProbeError::Spawn)?;
    let overflow = Arc::new(AtomicBool::new(false));
    let readers = [
        child
            .stdout
            .take()
            .map(|p| capture(p, limits.max_output, overflow.clone())),
        child
            .stderr
            .take()
            .map(|p| capture(p, limits.max_output, overflow.clone())),
    ];
    let deadline = Instant::now() + limits.timeout;
    let outcome = loop {
        if overflow.load(Ordering::SeqCst) {
            break Err(ProbeError::TooMuchOutput);
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status.code().unwrap_or(-1)),
            Ok(None) => {}
            Err(_) => break Err(ProbeError::Spawn),
        }
        if Instant::now() >= deadline {
            break Err(ProbeError::TimedOut);
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    if outcome.is_err() {
        job.terminate(1);
        let _ = child.kill();
        let _ = child.wait();
    }
    // Ends anything the program left running; only then do the pipes close and the readers end.
    drop(job);
    let mut text = String::new();
    for reader in readers.into_iter().flatten() {
        text.push_str(&String::from_utf8_lossy(&reader.join().unwrap_or_default()));
        text.push('\n');
    }
    if overflow.load(Ordering::SeqCst) {
        return Err(ProbeError::TooMuchOutput);
    }
    match outcome? {
        0 => parse_version(&text).ok_or(ProbeError::NoVersion),
        code => Err(ProbeError::Exit(code)),
    }
}

/// The version in `java -version` output: the quoted text of the first line with `version "`,
/// if it is short and made of digits, letters and `._+-` only. The output is untrusted.
pub fn parse_version(output: &str) -> Option<String> {
    const MARK: &str = "version \"";
    let line = output.lines().find(|l| l.contains(MARK))?;
    let rest = &line[line.find(MARK)? + MARK.len()..];
    let version = &rest[..rest.find('"')?];
    let ok = (1..=40).contains(&version.len())
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._+-".contains(c));
    ok.then(|| version.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn the_text_check_refuses_every_non_local_form() {
        for path in [
            r"\\fileserver\share\java.exe",
            "//fileserver/share/java.exe",
            r"\\?\C:\Java\bin\java.exe",
            r"\\?\UNC\fileserver\share\java.exe",
            r"\\.\C:\Java\bin\java.exe",
            r"\??\C:\Java\bin\java.exe",
            "java.exe",
            r"bin\java.exe",
            r"C:java.exe",
            r"\Java\bin\java.exe",
            "/Java/bin/java.exe",
            r"C:\Java\bin\java.exe:stream",
            "C:\\Java\\bin\\ja\u{202E}va.exe",
            "C:\\Java\\bin\\java.exe\n",
            "C:\\Java\0\\java.exe",
            "",
        ] {
            assert_eq!(check_text(path), Err(JavaProblem::Location), "{path:?}");
        }
    }

    #[test]
    fn the_text_check_wants_java_or_javaw_by_name() {
        for path in [
            r"C:\Windows\notepad.exe",
            r"C:\Java\bin\java",
            r"C:\Java\bin\java.exe.",
            r"C:\Java\bin\java.exe ",
            r"C:\Java\bin\JAVA~1.EXE",
            r"C:\Java\bin\java.cmd",
            r"C:\Java\bin\",
        ] {
            assert_eq!(check_text(path), Err(JavaProblem::Name), "{path:?}");
        }
        for path in [
            r"C:\Program Files\Java\bin\java.exe",
            r"d:\jdk\bin\JAVAW.EXE",
            "C:/jdk/bin/Java.Exe",
        ] {
            assert_eq!(check_text(path), Ok(()), "{path:?}");
        }
    }

    #[test]
    fn a_unc_path_is_refused_before_any_connection() {
        // 192.0.2.0/24 is TEST-NET-1: nothing answers there, so an attempt to connect would take
        // many seconds. The refusal comes from the text alone.
        let start = Instant::now();
        for path in [
            r"\\192.0.2.1\share\java.exe",
            r"\\?\UNC\192.0.2.1\share\java.exe",
        ] {
            assert_eq!(
                validate_java(Path::new(path)),
                Err(JavaProblem::Location),
                "{path}"
            );
        }
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn device_targets_of_network_drives_are_recognized() {
        assert_eq!(classify_device(r"\Device\HarddiskVolume3"), Device::Local);
        assert_eq!(classify_device(r"\Device\Ramdisk0"), Device::Local);
        for target in [
            r"\Device\LanmanRedirector\;Z:000000000001a2b3\fileserver\share",
            r"\Device\Mup\;LanmanRedirector\;Z:0000\fileserver\share",
            r"\Device\WebDavRedirector\;W:0000\host\DavWWWRoot",
            r"\Device\RdpDr\;X:1\tsclient\C",
            r"\??\UNC\fileserver\share",
            r"\??\\fileserver\share",
            "",
        ] {
            assert_eq!(classify_device(target), Device::Network, "{target}");
        }
        assert_eq!(classify_device(r"\??\C:\Tools\jdk"), Device::Subst('C'));
        assert_eq!(classify_device(r"\??\d:\x"), Device::Subst('D'));
    }

    #[test]
    fn link_targets_are_checked_as_text() {
        let link = Path::new(r"C:\Tools\jdk");
        assert_eq!(
            link_target(link, Path::new(r"\\?\D:\jdk-21")),
            Ok(PathBuf::from(r"D:\jdk-21"))
        );
        assert_eq!(
            link_target(link, Path::new(r"\??\D:\jdk-21")),
            Ok(PathBuf::from(r"D:\jdk-21"))
        );
        assert_eq!(
            link_target(link, Path::new(r"..\jdk-21")),
            Ok(PathBuf::from(r"C:\jdk-21"))
        );
        for target in [
            r"\\?\UNC\fileserver\share\jdk",
            r"\\fileserver\share\jdk",
            r"\\?\Volume{01234567-89ab-cdef-0123-456789abcdef}\jdk",
        ] {
            assert_eq!(
                link_target(link, Path::new(target)),
                Err(JavaProblem::Location),
                "{target}"
            );
        }
    }

    fn java_in(dir: &Path, name: &str) -> PathBuf {
        let java = dir.join(name);
        std::fs::copy(hengst_testkit::fake_java(), &java).unwrap();
        java
    }

    #[test]
    fn a_local_java_file_is_accepted_and_canonical() {
        let dir = tempfile::tempdir().unwrap();
        let java = java_in(dir.path(), "java.exe");
        let javaw = java_in(dir.path(), "JAVAW.EXE");
        for path in [&java, &javaw] {
            let ok = validate_java(path).unwrap();
            let text = ok.to_str().unwrap();
            assert!(!text.starts_with(r"\\"), "{text}");
            assert!(ok.is_file());
            assert_eq!(
                std::fs::canonicalize(&ok).unwrap(),
                std::fs::canonicalize(path).unwrap()
            );
        }
    }

    #[test]
    fn other_files_folders_and_missing_files_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let other = java_in(dir.path(), "notepad.exe");
        let folder = dir.path().join("java.exe");
        std::fs::create_dir(&folder).unwrap();
        assert_eq!(validate_java(&other), Err(JavaProblem::Name));
        assert_eq!(validate_java(&folder), Err(JavaProblem::Name));
        assert_eq!(
            validate_java(&dir.path().join("gone").join("java.exe")),
            Err(JavaProblem::Missing)
        );
    }

    #[test]
    fn a_junction_to_a_local_folder_is_followed() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("jdk-21").join("bin");
        std::fs::create_dir_all(&real).unwrap();
        let java = java_in(&real, "java.exe");
        let junction = dir.path().join("current");
        let status = std::process::Command::new("cmd")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(&junction)
            .arg(dir.path().join("jdk-21"))
            .stdout(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "mklink /J needs no privilege");
        let ok = validate_java(&junction.join("bin").join("java.exe")).unwrap();
        assert_eq!(
            std::fs::canonicalize(&ok).unwrap(),
            std::fs::canonicalize(&java).unwrap()
        );
    }

    /// A copy of fake-java as `java.exe` with a script in its folder (the probe's working
    /// directory).
    fn scripted(script: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let java = java_in(dir.path(), "java.exe");
        std::fs::write(dir.path().join("fake-java.txt"), script).unwrap();
        (dir, java)
    }

    const QUICK: ProbeLimits = ProbeLimits {
        timeout: Duration::from_secs(3),
        max_output: 64 * 1024,
    };

    #[test]
    fn the_probe_reads_the_version_from_stderr() {
        let (_dir, java) = scripted(
            "err openjdk version \"21.0.5\" 2024-10-15\nerr OpenJDK Runtime Environment\n",
        );
        assert_eq!(probe_version(&java, QUICK), Ok("21.0.5".to_string()));
        let (_dir, java) = scripted("err java version \"1.8.0_421\"\n");
        assert_eq!(probe_version(&java, QUICK), Ok("1.8.0_421".to_string()));
    }

    #[test]
    fn a_hanging_java_is_killed_after_the_time_limit() {
        let (_dir, java) = scripted("sleep 60000\n");
        let start = Instant::now();
        assert_eq!(probe_version(&java, QUICK), Err(ProbeError::TimedOut));
        assert!(start.elapsed() < Duration::from_secs(20));
    }

    #[test]
    fn garbage_output_is_no_version() {
        let (_dir, java) = scripted("out hello\nerr <b>version</b>\n");
        assert_eq!(probe_version(&java, QUICK), Err(ProbeError::NoVersion));
        let (_dir, java) = scripted("err openjdk version \"21<script>\"\n");
        assert_eq!(probe_version(&java, QUICK), Err(ProbeError::NoVersion));
    }

    #[test]
    fn a_chatty_java_is_stopped_at_the_output_limit() {
        let line = format!("out {}\n", "x".repeat(1000));
        let (_dir, java) = scripted(&format!("{}sleep 60000\n", line.repeat(2000)));
        let start = Instant::now();
        assert_eq!(probe_version(&java, QUICK), Err(ProbeError::TooMuchOutput));
        assert!(start.elapsed() < Duration::from_secs(20));
    }

    #[test]
    fn a_failing_exit_is_reported_even_with_a_version() {
        let (_dir, java) = scripted("err openjdk version \"21.0.5\"\nexit 3\n");
        assert_eq!(probe_version(&java, QUICK), Err(ProbeError::Exit(3)));
    }

    #[test]
    fn a_file_that_is_no_program_does_not_start() {
        let dir = tempfile::tempdir().unwrap();
        let java = dir.path().join("java.exe");
        std::fs::write(&java, b"not a program").unwrap();
        assert_eq!(probe_version(&java, QUICK), Err(ProbeError::Spawn));
    }

    #[test]
    fn version_parsing_is_strict() {
        assert_eq!(
            parse_version(
                "Picked up JAVA_TOOL_OPTIONS: -Xss2m\nopenjdk version \"25\" 2025-09-16\n"
            ),
            Some("25".into())
        );
        assert_eq!(
            parse_version("openjdk version \"17.0.12+7-LTS\""),
            Some("17.0.12+7-LTS".into())
        );
        assert_eq!(parse_version("openjdk version \"\""), None);
        assert_eq!(parse_version("openjdk version \"21"), None);
        assert_eq!(
            parse_version(&format!("openjdk version \"{}\"", "1".repeat(41))),
            None
        );
        assert_eq!(parse_version("openjdk version \"21\u{202E}\""), None);
        assert_eq!(parse_version(""), None);
    }
}
