use crate::download::{Download, Downloader};
use crate::paths::Paths;
use crate::version::AssetIndexRef;
use crate::Result;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub const RESOURCES_URL: &str = "https://resources.download.minecraft.net";

#[derive(Deserialize, Debug)]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
    #[serde(default, rename = "virtual")]
    pub is_virtual: bool,
    #[serde(default)]
    pub map_to_resources: bool,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

/// Where the game should look for assets (`${assets_root}` and `${game_assets}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetsLayout {
    pub index_id: String,
    pub assets_root: PathBuf,
    pub game_assets: PathBuf,
}

pub async fn install_assets(
    dl: &Downloader,
    paths: &Paths,
    index: &AssetIndexRef,
    game_dir: &Path,
    base_url: &str,
) -> Result<AssetsLayout> {
    let index_path = paths.asset_index(&index.id);
    dl.fetch_one(&Download {
        url: index.url.clone(),
        path: index_path.clone(),
        sha1: Some(index.sha1.clone()),
    })
    .await?;
    let parsed: AssetIndex = serde_json::from_slice(&tokio::fs::read(&index_path).await?)?;

    let base = base_url.trim_end_matches('/');
    let mut seen = HashSet::new();
    let downloads: Vec<Download> = parsed
        .objects
        .values()
        .filter(|o| o.hash.len() > 2 && seen.insert(o.hash.clone()))
        .map(|o| Download {
            url: format!("{base}/{}/{}", &o.hash[..2], o.hash),
            path: paths.asset_object(&o.hash),
            sha1: Some(o.hash.clone()),
        })
        .collect();
    dl.fetch_all(downloads).await?;

    let assets_root = paths.assets();
    let game_assets = if parsed.map_to_resources {
        let dest = game_dir.join("resources");
        copy_named(&parsed, paths, &dest)?;
        dest
    } else if parsed.is_virtual {
        let dest = assets_root.join("virtual").join(&index.id);
        copy_named(&parsed, paths, &dest)?;
        dest
    } else {
        assets_root.clone()
    };
    Ok(AssetsLayout {
        index_id: index.id.clone(),
        assets_root,
        game_assets,
    })
}

/// Old versions read assets by name, so copy hashed objects to readable paths.
fn copy_named(index: &AssetIndex, paths: &Paths, dest: &Path) -> Result<()> {
    for (name, obj) in &index.objects {
        if name.contains("..") {
            continue;
        }
        let target = dest.join(name);
        if target.is_file() {
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(paths.asset_object(&obj.hash), &target)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::download::{http_client, sha1_hex};
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn serve(index: serde_json::Value) -> (MockServer, AssetIndexRef) {
        let server = MockServer::start().await;
        let body = serde_json::to_vec(&index).unwrap();
        let hash = sha1_hex(b"sound");
        Mock::given(method("GET"))
            .and(path("/index.json"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body.clone()))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("/{}/{}", &hash[..2], hash)))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"sound".to_vec()))
            .expect(1)
            .mount(&server)
            .await;
        let r = AssetIndexRef {
            id: "legacy".into(),
            sha1: sha1_hex(&body),
            size: body.len() as u64,
            url: format!("{}/index.json", server.uri()),
        };
        (server, r)
    }

    fn objects() -> serde_json::Value {
        let hash = sha1_hex(b"sound");
        json!({"a/b.ogg": {"hash": hash, "size": 5}, "copy/b.ogg": {"hash": hash, "size": 5}})
    }

    #[tokio::test]
    async fn modern_layout_downloads_objects_once() {
        let (server, r) = serve(json!({"objects": objects()})).await;
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path().join("home"));
        let game = dir.path().join("game");
        let layout = install_assets(
            &Downloader::new(http_client()),
            &paths,
            &r,
            &game,
            &server.uri(),
        )
        .await
        .unwrap();
        assert_eq!(layout.assets_root, paths.assets());
        assert_eq!(layout.game_assets, paths.assets());
        assert!(paths.asset_object(&sha1_hex(b"sound")).is_file());
    }

    #[tokio::test]
    async fn virtual_layout_copies_named_files() {
        let (server, r) = serve(json!({"virtual": true, "objects": objects()})).await;
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path().join("home"));
        let layout = install_assets(
            &Downloader::new(http_client()),
            &paths,
            &r,
            &dir.path().join("game"),
            &server.uri(),
        )
        .await
        .unwrap();
        assert_eq!(
            layout.game_assets,
            paths.assets().join("virtual").join("legacy")
        );
        assert_eq!(
            std::fs::read(layout.game_assets.join("a/b.ogg")).unwrap(),
            b"sound"
        );
    }

    #[tokio::test]
    async fn pre_1_6_layout_maps_to_resources() {
        let (server, r) = serve(json!({"map_to_resources": true, "objects": objects()})).await;
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("game");
        let layout = install_assets(
            &Downloader::new(http_client()),
            &Paths::new(dir.path().join("home")),
            &r,
            &game,
            &server.uri(),
        )
        .await
        .unwrap();
        assert_eq!(layout.game_assets, game.join("resources"));
        assert!(game.join("resources/copy/b.ogg").is_file());
    }
}
