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
//!    its own drive; then `GetDriveTypeW` must say fixed, removable or RAM disk. Network, optical
//!    and unknown drives are refused as not local; an undefined letter or a drive without a root
//!    (a USB stick that is out) as unavailable.
//! 3. The folders, one by one from the drive root without following links: a link (symlink or
//!    junction) is read as text, its target goes through steps 1 and 2 again before anything under
//!    it is touched. At most 16 links per check, counted over the whole walk.
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
    /// The drive is not there right now (a USB stick that is out, a letter not in use).
    Unavailable,
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
            JavaProblem::Unavailable => "unavailable",
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
    match drive_state(path.as_bytes()[0] as char, &SystemDrives) {
        DriveState::Local => Ok(()),
        DriveState::NotLocal => Err(JavaProblem::Location),
        DriveState::Unavailable => Err(JavaProblem::Unavailable),
    }
}

/// Any failure to read a component that passed the location checks: it is not there (or not
/// readable, which for the user is the same).
fn missing(_: std::io::Error) -> JavaProblem {
    JavaProblem::Missing
}

/// How many links one whole check may pass through, counted over the entire walk (nested and
/// one after another), so a crafted tree of links cannot make the walk expensive.
const MAX_LINKS: u32 = 16;

/// Step 3: walks `path` (absolute) component by component without following links; each link's
/// target is checked as text and by its drive before it is walked in turn. `links` counts every
/// link of the whole check.
fn resolve(path: &Path, links: &mut u32) -> Result<PathBuf, JavaProblem> {
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
                    *links += 1;
                    if *links > MAX_LINKS {
                        return Err(JavaProblem::Location);
                    }
                    let target = std::fs::read_link(&current).map_err(|_| JavaProblem::Location)?;
                    let target = link_target(&current, &target)?;
                    current = resolve(&target, links)?;
                }
            }
        }
    }
    Ok(current)
}

/// What step 2 asks about a drive letter; `SystemDrives` asks Windows, tests answer themselves.
pub trait DriveProbe {
    /// The DOS device target of `X:` (`QueryDosDeviceW`, an object-manager lookup, no I/O on the
    /// drive); `None` when the letter is not defined.
    fn dos_device(&self, letter: char) -> Option<String>;
    /// `GetDriveTypeW` of `X:\`.
    fn drive_type(&self, letter: char) -> u32;
}

/// The answer of step 2.
#[derive(Debug, PartialEq, Eq)]
pub enum DriveState {
    /// A fixed, removable or RAM disk of this machine.
    Local,
    /// A network drive (also behind `subst`), an optical or unknown drive.
    NotLocal,
    /// No such drive right now (a letter that is not defined, no root: a USB stick that is out).
    Unavailable,
}

/// `GetDriveTypeW` results (values from the Windows SDK, `DRIVE_*`).
const DRIVE_NO_ROOT: u32 = 1;
const DRIVE_REMOVABLE_KIND: u32 = 2;
const DRIVE_FIXED_KIND: u32 = 3;
const DRIVE_RAMDISK_KIND: u32 = 6;

/// Step 2: the device target first (a redirector or UNC target is never asked anything more,
/// a `subst` drive is followed to its drive, at most 4 deep), then the drive type.
pub fn drive_state(letter: char, probe: &dyn DriveProbe) -> DriveState {
    let mut letter = letter.to_ascii_uppercase();
    for _ in 0..=4 {
        let Some(target) = probe.dos_device(letter) else {
            return DriveState::Unavailable;
        };
        match classify_device(&target) {
            Device::Network => return DriveState::NotLocal,
            Device::Subst(other) => {
                letter = other;
                continue;
            }
            Device::Local => {}
        }
        return match probe.drive_type(letter) {
            DRIVE_FIXED_KIND | DRIVE_REMOVABLE_KIND | DRIVE_RAMDISK_KIND => DriveState::Local,
            DRIVE_NO_ROOT => DriveState::Unavailable,
            _ => DriveState::NotLocal,
        };
    }
    DriveState::NotLocal
}

/// The real drives of this machine.
pub struct SystemDrives;

#[cfg(windows)]
impl DriveProbe for SystemDrives {
    fn dos_device(&self, letter: char) -> Option<String> {
        use windows_sys::Win32::Storage::FileSystem::QueryDosDeviceW;
        let name: Vec<u16> = format!("{letter}:").encode_utf16().chain(Some(0)).collect();
        let mut buf = vec![0u16; 1024];
        // SAFETY: `name` is NUL-terminated; `buf` is writable for `buf.len()` u16 units.
        let n = unsafe { QueryDosDeviceW(name.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) };
        if n == 0 {
            return None;
        }
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..end]))
    }

    fn drive_type(&self, letter: char) -> u32 {
        use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
        let root: Vec<u16> = format!("{letter}:\\")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        // SAFETY: `root` is a NUL-terminated wide string.
        unsafe { GetDriveTypeW(root.as_ptr()) }
    }
}

#[cfg(not(windows))]
impl DriveProbe for SystemDrives {
    fn dos_device(&self, _: char) -> Option<String> {
        None
    }
    fn drive_type(&self, _: char) -> u32 {
        0
    }
}

/// Reads up to `max` bytes of a pipe; past that it sets `overflow` and stops reading for good
/// (the rest is not drained: the caller sees the flag within one poll and kills the job, which
/// ends the writer and closes the pipe).
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
    let resolved = resolve(&absolute, &mut 0)?;
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
    use windows_sys::Win32::System::WindowsProgramming::{
        DRIVE_CDROM, DRIVE_FIXED, DRIVE_NO_ROOT_DIR, DRIVE_RAMDISK, DRIVE_REMOTE, DRIVE_REMOVABLE,
        DRIVE_UNKNOWN,
    };

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

    /// Drives as a test describes them: letter -> (DOS device target, GetDriveTypeW).
    struct FakeDrives(std::collections::HashMap<char, (Option<&'static str>, u32)>);

    impl DriveProbe for FakeDrives {
        fn dos_device(&self, letter: char) -> Option<String> {
            self.0
                .get(&letter)
                .and_then(|(target, _)| target.map(str::to_string))
        }
        fn drive_type(&self, letter: char) -> u32 {
            self.0
                .get(&letter)
                .map_or(DRIVE_NO_ROOT_DIR, |(_, kind)| *kind)
        }
    }

    #[test]
    fn the_drive_decision_follows_device_targets_and_drive_types() {
        let local = Some(r"\Device\HarddiskVolume3");
        let drives = FakeDrives(
            [
                ('C', (local, DRIVE_FIXED)),
                ('U', (Some(r"\Device\HarddiskVolume9"), DRIVE_REMOVABLE)),
                ('R', (Some(r"\Device\Ramdisk0"), DRIVE_RAMDISK)),
                ('O', (Some(r"\Device\CdRom0"), DRIVE_CDROM)),
                ('K', (Some(r"\Device\HarddiskVolume5"), DRIVE_UNKNOWN)),
                // A redirector target is refused before the drive type is asked (which would
                // say "fixed" here on purpose).
                (
                    'Z',
                    (
                        Some(r"\Device\LanmanRedirector\;Z:0000000000012345\fileserver\share"),
                        DRIVE_FIXED,
                    ),
                ),
                (
                    'W',
                    (
                        Some(r"\Device\WebDavRedirector\;W:0000\host\DavWWWRoot"),
                        DRIVE_REMOTE,
                    ),
                ),
                ('X', (Some(r"\Device\RdpDr\;X:1\tsclient\C"), DRIVE_REMOTE)),
                ('N', (local, DRIVE_REMOTE)),
                // subst drives: onto a local drive, onto a network drive, onto UNC, onto
                // themselves, onto a missing drive.
                ('E', (Some(r"\??\C:\Tools"), DRIVE_FIXED)),
                ('F', (Some(r"\??\Z:\jdk"), DRIVE_FIXED)),
                ('G', (Some(r"\??\UNC\fileserver\share"), DRIVE_FIXED)),
                ('H', (Some(r"\??\H:\loop"), DRIVE_FIXED)),
                ('I', (Some(r"\??\Q:\gone"), DRIVE_FIXED)),
                // Defined, but no medium or root.
                ('P', (local, DRIVE_NO_ROOT_DIR)),
            ]
            .into_iter()
            .collect(),
        );
        for (letter, expected) in [
            ('C', DriveState::Local),
            ('c', DriveState::Local),
            ('U', DriveState::Local),
            ('R', DriveState::Local),
            ('E', DriveState::Local),
            ('O', DriveState::NotLocal),
            ('K', DriveState::NotLocal),
            ('Z', DriveState::NotLocal),
            ('W', DriveState::NotLocal),
            ('X', DriveState::NotLocal),
            ('N', DriveState::NotLocal),
            ('F', DriveState::NotLocal),
            ('G', DriveState::NotLocal),
            ('H', DriveState::NotLocal),
            ('I', DriveState::Unavailable),
            ('P', DriveState::Unavailable),
            ('Q', DriveState::Unavailable),
        ] {
            assert_eq!(drive_state(letter, &drives), expected, "{letter}");
        }
    }

    #[test]
    fn a_drive_that_is_not_there_is_unavailable() {
        use windows_sys::Win32::Storage::FileSystem::GetLogicalDrives;
        // SAFETY: plain call without arguments.
        let present = unsafe { GetLogicalDrives() };
        let free = (b'D'..=b'Z')
            .rev()
            .find(|l| present & (1 << (l - b'A')) == 0)
            .expect("a free drive letter") as char;
        let path = format!(r"{free}:\jdk\bin\java.exe");
        assert_eq!(
            validate_java(Path::new(&path)),
            Err(JavaProblem::Unavailable)
        );
    }

    /// Junctions `<dir>/hop<i>` -> `<dir>/d<i>`, each `d<i>` holding the next hop: a path through
    /// `n` links to `java.exe`.
    fn junction_chain(dir: &Path, n: usize) -> PathBuf {
        let mut path = dir.to_path_buf();
        for i in 0..n {
            let real = dir.join(format!("d{i}"));
            std::fs::create_dir_all(&real).unwrap();
            let status = std::process::Command::new("cmd")
                .args(["/d", "/c", "mklink", "/J"])
                .arg(path.join(format!("hop{i}")))
                .arg(&real)
                .stdout(std::process::Stdio::null())
                .status()
                .unwrap();
            assert!(status.success(), "mklink /J needs no privilege");
            path = path.join(format!("hop{i}"));
        }
        let last = dir.join(format!("d{}", n - 1));
        java_in(&last, "java.exe");
        path.join("java.exe")
    }

    #[test]
    fn the_number_of_links_on_one_path_is_bounded_in_total() {
        let dir = tempfile::tempdir().unwrap();
        // Each hop is at nesting depth one (the walk returns to the top before the next), so only
        // a count over the whole walk stops a long chain.
        let path = junction_chain(dir.path(), MAX_LINKS as usize + 1);
        assert_eq!(validate_java(&path), Err(JavaProblem::Location));
        let dir = tempfile::tempdir().unwrap();
        let path = junction_chain(dir.path(), MAX_LINKS as usize);
        assert!(validate_java(&path).is_ok());
    }

    #[test]
    fn a_symlinked_java_exe_pointing_at_a_unc_path_is_refused_without_a_connection() {
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join("java.exe");
        // Creating the link does not touch the target.
        if let Err(e) = std::os::windows::fs::symlink_file(r"\\192.0.2.1\share\java.exe", &link) {
            eprintln!(
                "SKIPPED: symlink creation is not permitted here (needs developer mode or the \
                 privilege): {e}"
            );
            return;
        }
        let start = Instant::now();
        assert_eq!(validate_java(&link), Err(JavaProblem::Location));
        assert!(start.elapsed() < Duration::from_secs(1));
    }
}
