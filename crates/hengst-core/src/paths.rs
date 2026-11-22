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

    pub fn version_json(&self, id: &str) -> PathBuf {
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

    pub fn asset_index(&self, id: &str) -> PathBuf {
        self.assets().join("indexes").join(format!("{id}.json"))
    }

    pub fn asset_object(&self, hash: &str) -> PathBuf {
        self.assets().join("objects").join(&hash[..2]).join(hash)
    }

    pub fn log_config(&self, id: &str) -> PathBuf {
        self.assets().join("log_configs").join(id)
    }

    pub fn java_runtime(&self, component: &str) -> PathBuf {
        self.root.join("java").join(component)
    }

    pub fn loader_dir(&self, loader: &str) -> PathBuf {
        self.root.join("meta").join("loaders").join(loader)
    }

    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }

    /// Where SHA-1 values learned from `<url>.sha1` are remembered, keyed by the full artifact URL.
    /// Deliberately outside `libraries/`: a file next to a library is never read as a hash.
    pub fn sha1_cache(&self) -> PathBuf {
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
pub fn join_relative(base: &Path, rel: &str) -> Result<PathBuf> {
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
/// partial file. Creates the parent folder; a failed rename removes the temp file. No fsync: the
/// callers write caches that are fetched again when they are missing or damaged.
pub(crate) async fn write_atomic(file: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = file.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let tmp = unique_temp(file);
    tokio::fs::write(&tmp, bytes).await?;
    if let Err(e) = tokio::fs::rename(&tmp, file).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(e);
    }
    Ok(())
}

/// How long a temp file of another process must have been untouched before it counts as left over
/// by a crashed run. Real writes finish within seconds.
const STALE_TEMP_AGE: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// Removes temp files `unique_temp` left next to `dest` by a crashed run: exactly
/// `<file name>.<pid>.<n>.part`, from another process, untouched for `STALE_TEMP_AGE`. Only the
/// parent directory is listed (no recursion). Best effort: failures are ignored.
pub fn remove_stale_temps(dest: &Path) {
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
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
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
pub fn validate_sha1(hash: &str) -> Result<()> {
    if hash.len() == 40 && hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(Error::UnsafePath(hash.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
