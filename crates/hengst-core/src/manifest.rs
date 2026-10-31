use crate::download::get_bytes;
use crate::paths::Paths;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};

pub const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct VersionManifest {
    pub latest: Latest,
    pub versions: Vec<VersionEntry>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct VersionEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: VersionKind,
    pub url: String,
    pub sha1: String,
    #[serde(rename = "releaseTime")]
    pub release_time: String,
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VersionKind {
    Release,
    Snapshot,
    OldBeta,
    OldAlpha,
    #[serde(other)]
    Other,
}

impl VersionManifest {
    /// Finds a version by id. Accepts `latest`/`latest-release` and `latest-snapshot`.
    pub fn find(&self, id: &str) -> Option<&VersionEntry> {
        let id = match id {
            "latest" | "latest-release" => self.latest.release.as_str(),
            "latest-snapshot" => self.latest.snapshot.as_str(),
            other => other,
        };
        self.versions.iter().find(|v| v.id == id)
    }
}

/// Downloads the manifest and caches it. Falls back to the cache when the network fails.
pub async fn fetch_manifest(
    client: &reqwest::Client,
    paths: &Paths,
    url: &str,
) -> Result<VersionManifest> {
    let cache = paths.manifest_file();
    match get_bytes(client, url).await {
        Ok(bytes) => {
            let manifest: VersionManifest = serde_json::from_slice(&bytes)?;
            if let Some(parent) = cache.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(&cache, &bytes).await?;
            Ok(manifest)
        }
        Err(e @ (Error::Http { .. } | Error::Status { .. })) => {
            if cache.is_file() {
                Ok(serde_json::from_slice(&tokio::fs::read(&cache).await?)?)
            } else {
                Err(e)
            }
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn sample() -> serde_json::Value {
        json!({
            "latest": {"release": "1.21.11", "snapshot": "26.4-snapshot-1"},
            "versions": [
                {"id": "26.4-snapshot-1", "type": "snapshot", "url": "u1", "sha1": "s1", "time": "t", "releaseTime": "r1", "complianceLevel": 1},
                {"id": "1.21.11", "type": "release", "url": "u2", "sha1": "s2", "time": "t", "releaseTime": "r2", "complianceLevel": 1},
                {"id": "b1.7.3", "type": "old_beta", "url": "u3", "sha1": "s3", "time": "t", "releaseTime": "r3", "complianceLevel": 0},
                {"id": "x", "type": "pending", "url": "u4", "sha1": "s4", "time": "t", "releaseTime": "r4"}
            ]
        })
    }

    #[test]
    fn parses_and_finds_aliases() {
        let m: VersionManifest = serde_json::from_value(sample()).unwrap();
        assert_eq!(m.find("latest").unwrap().id, "1.21.11");
        assert_eq!(m.find("latest-snapshot").unwrap().id, "26.4-snapshot-1");
        assert_eq!(m.find("b1.7.3").unwrap().kind, VersionKind::OldBeta);
        assert_eq!(m.find("x").unwrap().kind, VersionKind::Other);
        assert!(m.find("nope").is_none());
    }

    #[tokio::test]
    async fn caches_and_falls_back_when_offline() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample()))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let client = crate::download::http_client();
        let url = format!("{}/manifest.json", server.uri());
        let online = fetch_manifest(&client, &paths, &url).await.unwrap();
        assert!(paths.manifest_file().is_file());
        let offline = fetch_manifest(&client, &paths, &url).await.unwrap();
        assert_eq!(online.versions.len(), offline.versions.len());
    }
}
