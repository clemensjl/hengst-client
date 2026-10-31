use std::path::{Path, PathBuf};

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

#[cfg(test)]
mod tests {
    use super::*;

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
