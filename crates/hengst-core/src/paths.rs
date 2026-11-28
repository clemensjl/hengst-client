use crate::{Error, Result};
use std::path::{Component, Path, PathBuf};

/// Layout of the launcher data directory.
#[derive(Clone, Debug)]
pub struct Paths {
    root: PathBuf,
}

impl Paths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// `HENGST_HOME` if set, otherwise `%APPDATA%\HengstClient`.
    pub fn default_root() -> PathBuf {
        if let Some(p) = std::env::var_os("HENGST_HOME") {
            return PathBuf::from(p);
        }
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("HengstClient")
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn accounts_file(&self) -> PathBuf {
        self.root.join("accounts.json")
    }

    pub fn manifest_file(&self) -> PathBuf {
        self.root.join("meta").join("version_manifest_v2.json")
    }

    pub(crate) fn version_json(&self, id: &str) -> PathBuf {
        self.root
            .join("meta")
            .join("versions")
            .join(format!("{id}.json"))
    }

    pub fn client_jar(&self, id: &str) -> PathBuf {
        self.root
            .join("versions")
            .join(id)
            .join(format!("{id}.jar"))
    }

    pub fn libraries(&self) -> PathBuf {
        self.root.join("libraries")
    }

    pub fn assets(&self) -> PathBuf {
        self.root.join("assets")
    }

    pub(crate) fn asset_index(&self, id: &str) -> PathBuf {
        self.assets().join("indexes").join(format!("{id}.json"))
    }

    pub fn asset_object(&self, hash: &str) -> PathBuf {
        self.assets().join("objects").join(&hash[..2]).join(hash)
    }

    pub(crate) fn log_config(&self, id: &str) -> PathBuf {
        self.assets().join("log_configs").join(id)
    }

    pub fn java_runtime(&self, component: &str) -> PathBuf {
        self.root.join("java").join(component)
    }

    pub(crate) fn loader_dir(&self, loader: &str) -> PathBuf {
        self.root.join("meta").join("loaders").join(loader)
    }

    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }

    /// Where SHA-1 values learned from `<url>.sha1` are remembered, keyed by the full artifact URL.
    /// Deliberately outside `libraries/`: a file next to a library is never read as a hash.
    pub(crate) fn sha1_cache(&self) -> PathBuf {
        self.root.join("meta").join("sha1")
    }

    pub fn tmp(&self) -> PathBuf {
        self.root.join("tmp")
    }

    pub fn instance(&self, name: &str) -> PathBuf {
        self.root.join("instances").join(name)
    }
}

/// Joins a relative path taken from remote metadata onto `base`, refusing anything that could
/// leave it: only plain components, no drive letters, roots, `..`, `:` or backslashes, and nothing
/// Windows would reinterpret (components ending in `.` or a space, device names such as `CON`).
/// Empty components (leading, doubled or trailing `/`) and `.` components are refused rather than
/// silently dropped, so the result is exactly the path that was written.
pub(crate) fn join_relative(base: &Path, rel: &str) -> Result<PathBuf> {
    let unsafe_path = || Error::UnsafePath(rel.to_string());
    if rel.is_empty() || rel.contains([':', '\\']) {
        return Err(unsafe_path());
    }
    if rel.split('/').any(|c| c.is_empty() || c == ".") {
        return Err(unsafe_path());
    }
    let mut out = base.to_path_buf();
    for component in Path::new(rel).components() {
        match component {
            Component::Normal(part) => {
                if part.to_str().is_none_or(is_windows_reserved) {
                    return Err(unsafe_path());
                }
                out.push(part)
            }
            _ => return Err(unsafe_path()),
        }
    }
    Ok(out)
}

/// A temp file name next to `dest`, unique to this process and call, so it cannot collide with
/// another download or extraction of the same destination.
pub fn unique_temp(dest: &Path) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut tmp = dest.as_os_str().to_owned();
    tmp.push(format!(".{}.{n}.part", std::process::id()));
    PathBuf::from(tmp)
}

/// Writes `bytes` to `file` through a temp file next to it and a rename, so a reader never sees a
/// partial file. Creates the parent folder; a failed write or rename removes the temp file. No
/// fsync: the callers write caches that are fetched again when they are missing or damaged.
pub(crate) async fn write_atomic(file: &Path, bytes: &[u8]) -> std::io::Result<()> {
    write_atomic_owned(file.to_path_buf(), bytes.to_vec()).await
}

/// `write_atomic` taking ownership (no copy of a large body). Temp file, write and rename run as
/// one blocking task: once started it finishes even when the awaiting task is aborted (a
/// cancelled launch), so an abort never leaves this process's temp file behind, which
/// `remove_stale_temps` would keep (it skips the own pid).
pub(crate) async fn write_atomic_owned(file: PathBuf, bytes: Vec<u8>) -> std::io::Result<()> {
    write_atomic_hooked(file, bytes, || {}).await
}

/// `write_atomic_owned` that calls `at_temp` on the blocking thread once the temp file is written,
/// right before the rename (tests hold the write there).
async fn write_atomic_hooked(
    file: PathBuf,
    bytes: Vec<u8>,
    at_temp: impl FnOnce() + Send + 'static,
) -> std::io::Result<()> {
    tokio::task::spawn_blocking(move || {
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = unique_temp(&file);
        let result = std::fs::write(&tmp, &bytes).and_then(|()| {
            at_temp();
            replace_file(&tmp, &file)
        });
        if result.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        result
    })
    .await
    .map_err(|e| std::io::Error::other(format!("the write task failed: {e}")))?
}

/// The pauses between the attempts of `replace_file`, under a second in all.
const REPLACE_PAUSES: [std::time::Duration; 5] = [
    std::time::Duration::from_millis(25),
    std::time::Duration::from_millis(50),
    std::time::Duration::from_millis(100),
    std::time::Duration::from_millis(200),
    std::time::Duration::from_millis(400),
];

/// Renames `from` over `to` (the last step of every atomic write). On Windows another program that
/// has one of the two files open for a moment (a virus scanner or the search indexer, right after
/// the file was written) makes the rename fail with "access denied", a sharing or a lock
/// violation: those are tried again after each of `REPLACE_PAUSES`, then the error is returned.
/// Blocks the calling thread for at most those pauses.
pub fn replace_file(from: &Path, to: &Path) -> std::io::Result<()> {
    replace_with(&mut || std::fs::rename(from, to), &mut std::thread::sleep)
}

fn replace_with(
    attempt: &mut dyn FnMut() -> std::io::Result<()>,
    pause: &mut dyn FnMut(std::time::Duration),
) -> std::io::Result<()> {
    let mut pauses = REPLACE_PAUSES.iter();
    loop {
        match attempt() {
            Err(e) if held_by_another_program(&e) => match pauses.next() {
                Some(d) => pause(*d),
                None => return Err(e),
            },
            other => return other,
        }
    }
}

/// `ERROR_ACCESS_DENIED`, `ERROR_SHARING_VIOLATION`, `ERROR_LOCK_VIOLATION`.
fn held_by_another_program(e: &std::io::Error) -> bool {
    cfg!(windows) && matches!(e.raw_os_error(), Some(5 | 32 | 33))
}

/// Whether `write_atomic_sync` may replace a file that is already there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Replace {
    Always,
    /// Fails with `AlreadyExists` and leaves the existing file alone.
    Never,
}

/// The sync sibling of `write_atomic` for data that must not be cut off: temp file next to `file`,
/// flushed (`sync_all`), then a rename (`Replace::Always`) or a hard link (`Replace::Never`, which
/// refuses an existing target; on a volume without hard links the file is created directly with
/// create-new semantics, which is no longer all-or-nothing). Never creates parent folders. A failed
/// write removes the temp file.
pub(crate) fn write_atomic_sync(
    file: &Path,
    bytes: &[u8],
    replace: Replace,
) -> std::io::Result<()> {
    use std::io::Write;
    let tmp = unique_temp(file);
    let result = (|| {
        let mut f = std::fs::File::create_new(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        match replace {
            Replace::Always => replace_file(&tmp, file),
            Replace::Never => match std::fs::hard_link(&tmp, file) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(e),
                Err(_) => {
                    let mut f = std::fs::File::create_new(file)?;
                    let written = f.write_all(bytes).and_then(|()| f.sync_all());
                    drop(f);
                    if written.is_err() {
                        let _ = std::fs::remove_file(file);
                    }
                    written
                }
            },
        }
    })();
    // After a rename the temp file is gone; after a hard link or a failure it is not.
    if result.is_err() || replace == Replace::Never {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// How long a temp file of another process must have been untouched before it counts as left over
/// by a crashed run. Real writes finish within seconds.
const STALE_TEMP_AGE: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// Removes temp files `unique_temp` left next to `dest` by a crashed run: exactly
/// `<file name>.<pid>.<n>.part`, from another process, untouched for `STALE_TEMP_AGE`. Only the
/// parent directory is listed (no recursion). Best effort: failures are ignored.
pub(crate) fn remove_stale_temps(dest: &Path) {
    let (Some(dir), Some(name)) = (dest.parent(), dest.file_name().and_then(|n| n.to_str())) else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let own = std::process::id().to_string();
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(rest) = file_name
            .to_str()
            .and_then(|n| n.strip_prefix(name))
            .and_then(|r| r.strip_prefix('.'))
            .and_then(|r| r.strip_suffix(".part"))
        else {
            continue;
        };
        let Some((pid, n)) = rest.split_once('.') else {
            continue;
        };
        if !digits(pid) || !digits(n) || pid == own {
            continue;
        }
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age >= STALE_TEMP_AGE);
        if old {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Components Windows strips or maps to something else: trailing dots and spaces, and device
/// names (with or without an extension, any case).
pub(crate) fn is_windows_reserved(name: &str) -> bool {
    if name.ends_with('.') || name.ends_with(' ') {
        return true;
    }
    // Windows ignores spaces before the extension dot: `AUX .txt` is the device `AUX`.
    let stem = name
        .split('.')
        .next()
        .unwrap_or(name)
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM\u{b9}"
            | "COM\u{b2}"
            | "COM\u{b3}"
            | "LPT\u{b9}"
            | "LPT\u{b2}"
            | "LPT\u{b3}"
    ) || (stem.len() == 4
        && (stem.starts_with("COM") || stem.starts_with("LPT"))
        && matches!(stem.as_bytes()[3], b'1'..=b'9'))
}

/// Version, asset index, runtime and instance ids end up in file names: ASCII alphanumerics and
/// `. _ - +` only (e.g. `1.21.11`, `26.4-snapshot-6`, `3.3.3+5`). Inner spaces are allowed because
/// some Mojang ids have them (`1.14 Pre-Release 1`); leading or trailing ones are not, since
/// Windows strips them.
pub fn validate_id(id: &str) -> Result<()> {
    let ok = !id.is_empty()
        && id.len() <= 128
        && id != "."
        && id != ".."
        && !id.starts_with(' ')
        && !id.ends_with(' ')
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'+' | b' '));
    if ok {
        Ok(())
    } else {
        Err(Error::UnsafePath(id.to_string()))
    }
}

/// A SHA-1 as 40 hex digits.
pub(crate) fn validate_sha1(hash: &str) -> Result<()> {
    if hash.len() == 40 && hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(Error::UnsafePath(hash.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temps(dir: &Path) -> usize {
        std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".part"))
            .count()
    }

    /// A download task aborted (the play operation cancelled) while it writes: the write is
    /// dropped exactly while its temp file exists (held there by the hook), and still ends with
    /// the finished file and without this process's temp file, which `remove_stale_temps` would
    /// keep (own pid).
    #[tokio::test(flavor = "multi_thread")]
    async fn a_write_dropped_while_its_temp_file_exists_still_finishes_and_leaves_no_temp() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("client.jar");
        let (at_temp, temp_written) = std::sync::mpsc::channel();
        let (go, wait_for_go) = std::sync::mpsc::channel::<()>();
        let write = tokio::spawn(write_atomic_hooked(
            file.clone(),
            vec![7u8; 64 * 1024],
            move || {
                at_temp.send(()).unwrap();
                wait_for_go.recv().unwrap();
            },
        ));
        tokio::task::spawn_blocking(move || temp_written.recv().unwrap())
            .await
            .unwrap();
        assert_eq!(temps(dir.path()), 1, "held with its temp file written");
        // The abort, while the blocking part waits with the temp file in place.
        write.abort();
        assert!(write.await.unwrap_err().is_cancelled());
        go.send(()).unwrap();
        // The blocking part goes on by itself; the finished file appears in one rename.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !file.exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "the write never ended"
            );
            tokio::task::yield_now().await;
        }
        assert_eq!(std::fs::read(&file).unwrap().len(), 64 * 1024);
        assert_eq!(temps(dir.path()), 0, "a temp file was left behind");
    }

    #[tokio::test]
    async fn a_write_dropped_before_it_starts_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("client.jar");
        drop(write_atomic(&file, b"x"));
        assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());
    }

    #[cfg(windows)]
    #[test]
    fn a_replace_tries_again_while_another_program_holds_the_file_then_gives_up() {
        let dir = tempfile::tempdir().unwrap();
        let (from, to) = (
            dir.path().join("new.part"),
            dir.path().join("instance.json"),
        );
        std::fs::write(&from, b"new").unwrap();
        // Held twice, then free: two pauses, then the replace.
        let mut tries = 0;
        let mut pauses = Vec::new();
        replace_with(
            &mut || {
                tries += 1;
                if tries <= 2 {
                    Err(std::io::Error::from_raw_os_error(32))
                } else {
                    std::fs::rename(&from, &to)
                }
            },
            &mut |d| pauses.push(d),
        )
        .unwrap();
        assert_eq!(std::fs::read(&to).unwrap(), b"new");
        assert_eq!(pauses, REPLACE_PAUSES[..2]);
        // Held for good: every pause, then the error; the whole wait stays short.
        for code in [5, 32, 33] {
            let mut pauses = Vec::new();
            let err = replace_with(
                &mut || Err(std::io::Error::from_raw_os_error(code)),
                &mut |d| pauses.push(d),
            )
            .unwrap_err();
            assert_eq!(err.raw_os_error(), Some(code));
            assert_eq!(pauses, REPLACE_PAUSES);
        }
        assert!(
            REPLACE_PAUSES.iter().sum::<std::time::Duration>() < std::time::Duration::from_secs(1)
        );
        // Any other error is final at once.
        let mut pauses = Vec::new();
        let err = replace_with(
            &mut || Err(std::io::Error::from(std::io::ErrorKind::NotFound)),
            &mut |d| pauses.push(d),
        )
        .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
        assert!(pauses.is_empty());
    }

    /// The flake this fixes: a program that opens the target without delete sharing (as a virus
    /// scanner may right after a file was written) makes a plain rename over it fail with
    /// "access denied"; `replace_file` waits it out.
    #[cfg(windows)]
    #[test]
    fn a_replace_waits_for_a_reader_without_delete_sharing() {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};
        let dir = tempfile::tempdir().unwrap();
        let (from, to) = (
            dir.path().join("new.part"),
            dir.path().join("instance.json"),
        );
        std::fs::write(&from, b"new").unwrap();
        std::fs::write(&to, b"old").unwrap();
        let open = || {
            std::fs::OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
                .open(&to)
                .unwrap()
        };
        let held = open();
        let err = std::fs::rename(&from, &to).unwrap_err();
        assert_eq!(err.raw_os_error(), Some(5), "{err}");
        let mut held = Some(held);
        let mut pauses = 0;
        replace_with(&mut || std::fs::rename(&from, &to), &mut |_| {
            pauses += 1;
            // The other program closes the file during the first pause.
            held.take();
        })
        .unwrap();
        assert_eq!(pauses, 1);
        assert_eq!(std::fs::read(&to).unwrap(), b"new");
        assert!(!from.exists());
        // Through the real entry point, with a reader that never lets go: the error after the
        // bounded wait.
        std::fs::write(&from, b"newer").unwrap();
        let _held = open();
        let started = std::time::Instant::now();
        assert_eq!(
            replace_file(&from, &to).unwrap_err().raw_os_error(),
            Some(5)
        );
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
        assert_eq!(std::fs::read(&to).unwrap(), b"new");
    }

    #[test]
    fn join_relative_accepts_plain_paths_only() {
        let base = Path::new("C:/h/libs");
        assert_eq!(
            join_relative(base, "a/b.ogg").unwrap(),
            base.join("a").join("b.ogg")
        );
        assert_eq!(
            join_relative(base, "com/mojang/x.jar").unwrap(),
            base.join("com").join("mojang").join("x.jar")
        );
        for bad in ["../x", "a/../x", "\\x", "/x", "C:\\x", "C:x", "a:b", ""] {
            assert!(
                matches!(join_relative(base, bad), Err(Error::UnsafePath(_))),
                "{bad:?} must be rejected"
            );
        }
    }

    #[test]
    fn join_relative_refuses_windows_reinterpreted_names() {
        let base = Path::new("C:/h/libs");
        for bad in [
            "a/.. /x",
            "a/.../x",
            "a/x.",
            "a/x ",
            "a/CON",
            "a/con.txt",
            "a/Nul.tar.gz",
            "AUX",
            "a/COM1",
            "a/lpt9.jar",
            "prn",
            "a/AUX .x",
            "a/CON .txt",
            "a/nul  .tar.gz",
            "a/a.jar:s",
            "//server/share/x",
        ] {
            assert!(
                matches!(join_relative(base, bad), Err(Error::UnsafePath(_))),
                "{bad:?} must be rejected"
            );
        }
        for bad in [
            "a//b",
            "/a",
            "a/",
            "a/./b",
            "./a",
            "a/b/",
            "COM\u{b9}",
            "lpt\u{b3}.txt",
        ] {
            assert!(
                matches!(join_relative(base, bad), Err(Error::UnsafePath(_))),
                "{bad:?} must be rejected"
            );
        }
        for ok in [
            "a/console.jar",
            "a/COM10",
            "a/COM0",
            "a/com.x",
            "a/lpt",
            "a/.hidden",
            "a/b c/d",
        ] {
            join_relative(base, ok).unwrap_or_else(|_| panic!("{ok:?} must be accepted"));
        }
    }

    #[test]
    fn validate_id_covers_real_ids_and_rejects_the_rest() {
        for ok in [
            "1.21.11",
            "26.4-snapshot-6",
            "rd-132211",
            "b1.7.3",
            "client-1.12.xml",
            "java-runtime-epsilon",
            "3.3.3+5",
            "1.14 Pre-Release 1",
            "3D Shareware v1.34",
        ] {
            validate_id(ok).unwrap();
        }
        let long = "a".repeat(129);
        for bad in [
            "", ".", "..", "a/b", "a\\b", "a:b", " 1.8.9", "1.8.9 ", "\u{e4}", &long,
        ] {
            assert!(validate_id(bad).is_err(), "{bad:?} must be rejected");
        }
    }

    #[test]
    fn write_atomic_sync_replaces_or_refuses_and_never_creates_folders() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f.json");
        write_atomic_sync(&file, b"one", Replace::Never).unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"one");
        let err = write_atomic_sync(&file, b"two", Replace::Never).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(&file).unwrap(), b"one");
        write_atomic_sync(&file, b"three", Replace::Always).unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"three");
        let missing = dir.path().join("no").join("f.json");
        assert!(write_atomic_sync(&missing, b"x", Replace::Always).is_err());
        assert!(write_atomic_sync(&missing, b"x", Replace::Never).is_err());
        assert!(!dir.path().join("no").exists());
        let left: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(
            left,
            [std::ffi::OsString::from("f.json")],
            "no temp file is left"
        );
    }

    #[test]
    fn validate_sha1_needs_forty_hex_digits() {
        validate_sha1(&"ab12".repeat(10)).unwrap();
        for bad in ["", "abcdef", &"g".repeat(40), &"a".repeat(41), "../../etc"] {
            assert!(validate_sha1(bad).is_err(), "{bad:?} must be rejected");
        }
    }

    #[test]
    fn layout_is_stable() {
        let p = Paths::new("C:/h");
        assert_eq!(
            p.manifest_file(),
            PathBuf::from("C:/h/meta/version_manifest_v2.json")
        );
        assert_eq!(
            p.version_json("1.8.9"),
            PathBuf::from("C:/h/meta/versions/1.8.9.json")
        );
        assert_eq!(
            p.client_jar("1.8.9"),
            PathBuf::from("C:/h/versions/1.8.9/1.8.9.jar")
        );
        assert_eq!(
            p.asset_object("abcdef"),
            PathBuf::from("C:/h/assets/objects/ab/abcdef")
        );
        assert_eq!(
            p.asset_index("17"),
            PathBuf::from("C:/h/assets/indexes/17.json")
        );
        assert_eq!(
            p.java_runtime("jre-legacy"),
            PathBuf::from("C:/h/java/jre-legacy")
        );
        assert_eq!(p.instance("pvp"), PathBuf::from("C:/h/instances/pvp"));
        assert_eq!(
            p.loader_dir("fabric"),
            PathBuf::from("C:/h/meta/loaders/fabric")
        );
        assert_eq!(p.logs(), PathBuf::from("C:/h/logs"));
        assert_eq!(p.tmp(), PathBuf::from("C:/h/tmp"));
        assert_eq!(p.accounts_file(), PathBuf::from("C:/h/accounts.json"));
    }
}
