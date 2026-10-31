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

    pub fn instance(&self, name: &str) -> PathBuf {
        self.root.join("instances").join(name)
    }
}

/// Joins a relative path taken from remote metadata onto `base`, refusing anything that could
/// leave it: only plain components, no drive letters, roots, `..`, `:` or backslashes.
pub fn join_relative(base: &Path, rel: &str) -> Result<PathBuf> {
    let unsafe_path = || Error::UnsafePath(rel.to_string());
    if rel.is_empty() || rel.contains([':', '\\']) {
        return Err(unsafe_path());
    }
    let mut out = base.to_path_buf();
    for component in Path::new(rel).components() {
        match component {
            Component::Normal(part) => out.push(part),
            _ => return Err(unsafe_path()),
        }
    }
    Ok(out)
}

/// Version, asset index, runtime and instance ids end up in file names: ASCII alphanumerics and
/// `. _ - +` only (e.g. `1.21.11`, `26.4-snapshot-6`, `3.3.3+5`).
pub fn validate_id(id: &str) -> Result<()> {
    let ok = !id.is_empty()
        && id.len() <= 128
        && id != "."
        && id != ".."
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'+'));
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
    fn validate_id_covers_real_ids_and_rejects_the_rest() {
        for ok in [
            "1.21.11",
            "26.4-snapshot-6",
            "rd-132211",
            "b1.7.3",
            "client-1.12.xml",
            "java-runtime-epsilon",
            "3.3.3+5",
        ] {
            validate_id(ok).unwrap();
        }
        let long = "a".repeat(129);
        for bad in ["", ".", "..", "a/b", "a\\b", "a:b", "a b", "\u{e4}", &long] {
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
    }
}
