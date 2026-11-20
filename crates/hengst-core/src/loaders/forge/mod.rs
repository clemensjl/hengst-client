//! Version lists and installers for Forge and NeoForge.

pub mod installer;
pub mod processors;

use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;

use super::{sort_newest_first, LoaderEndpoints, LoaderKind, LoaderVersion};
use crate::download::{fill_missing_sha1, get_bytes, Download, Downloader};
use crate::libraries::maven_path;
use crate::paths::{join_relative, validate_id, Paths};
use crate::{Error, Result};

/// Forge has installers from 1.7.10 upward. IDs that are not of the form `1.X[.Y]` (snapshots,
/// year-based versions) are not refused here.
pub fn forge_supports(game: &str) -> bool {
    let parts: Vec<Option<u64>> = game.split('.').map(|p| p.parse().ok()).collect();
    match parts.as_slice() {
        [Some(1), Some(x)] => (1, *x, 0) >= (1, 7, 10),
        [Some(1), Some(x), Some(y)] => (1, *x, *y) >= (1, 7, 10),
        _ => true,
    }
}

/// The game version a NeoForge build belongs to, or `None` for snapshots and the pre-release
/// experiments that are not tied to a release.
pub fn neoforge_game_version(version: &str) -> Option<String> {
    if version.contains('+') || version.starts_with("0.") {
        return None;
    }
    let base = version.split('-').next()?;
    let parts: Vec<u64> = base
        .split('.')
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    match parts.as_slice() {
        [a, b, c, _] if *a >= 26 => Some(if *c == 0 {
            format!("{a}.{b}")
        } else {
            format!("{a}.{b}.{c}")
        }),
        [a, b, _] if *a < 26 => Some(if *b == 0 {
            format!("1.{a}")
        } else {
            format!("1.{a}.{b}")
        }),
        _ => None,
    }
}

#[derive(Deserialize)]
struct Promotions {
    promos: HashMap<String, String>,
}

#[derive(Deserialize)]
struct MavenVersions {
    versions: Vec<String>,
}

/// Forge builds for a game version, newest first. A build is stable when it is the recommended one.
pub async fn list_forge(
    client: &reqwest::Client,
    ep: &LoaderEndpoints,
    game: &str,
) -> Result<Vec<LoaderVersion>> {
    if !forge_supports(game) {
        return Err(Error::LoaderUnsupported {
            loader: "forge".into(),
            game: game.to_string(),
            reason: "Forge support starts at 1.7.10".into(),
        });
    }
    validate_id(game)?;
    let base = &ep.forge_files;
    let metadata: HashMap<String, Vec<String>> =
        serde_json::from_slice(&get_bytes(client, &format!("{base}/maven-metadata.json")).await?)?;
    let promos: Promotions =
        serde_json::from_slice(&get_bytes(client, &format!("{base}/promotions_slim.json")).await?)?;
    let recommended = promos.promos.get(&format!("{game}-recommended"));
    let prefix = format!("{game}-");
    let suffix = format!("-{game}");
    let mut versions: Vec<LoaderVersion> = metadata
        .get(game)
        .into_iter()
        .flatten()
        .filter_map(|maven| {
            let rest = maven.strip_prefix(&prefix)?;
            let id = rest.strip_suffix(&suffix).unwrap_or(rest);
            if validate_id(maven).is_err() || validate_id(id).is_err() {
                return None;
            }
            Some(LoaderVersion {
                id: id.to_string(),
                maven: maven.clone(),
                stable: recommended.is_some_and(|r| r == id),
            })
        })
        .collect();
    sort_newest_first(&mut versions);
    Ok(versions)
}

/// NeoForge builds for a game version, newest first. A build is stable when its version has no
/// pre-release marker. Snapshot builds are not listed.
pub async fn list_neoforge(
    client: &reqwest::Client,
    ep: &LoaderEndpoints,
    game: &str,
) -> Result<Vec<LoaderVersion>> {
    validate_id(game)?;
    // 1.20.1 predates the neoforge artifact and lives under the old `forge` one.
    let old = game == "1.20.1";
    let artifact = if old { "forge" } else { "neoforge" };
    let url = format!(
        "{}/api/maven/versions/releases/net/neoforged/{artifact}",
        ep.neoforge_maven
    );
    let listed: MavenVersions = serde_json::from_slice(&get_bytes(client, &url).await?)?;
    let prefix = "1.20.1-";
    let mut versions: Vec<LoaderVersion> = listed
        .versions
        .into_iter()
        .filter_map(|maven| {
            let id = if old {
                maven.strip_prefix(prefix)?.to_string()
            } else if neoforge_game_version(&maven).as_deref() == Some(game) {
                maven.clone()
            } else {
                return None;
            };
            if validate_id(&maven).is_err() || validate_id(&id).is_err() {
                return None;
            }
            Some(LoaderVersion {
                stable: !id.contains('-'),
                id,
                maven,
            })
        })
        .collect();
    sort_newest_first(&mut versions);
    Ok(versions)
}

/// Maven coordinate of the installer jar. Only Forge and NeoForge have installers.
fn installer_coords(kind: LoaderKind, game: &str, maven: &str) -> Result<String> {
    match kind {
        LoaderKind::Forge => Ok(format!("net.minecraftforge:forge:{maven}:installer")),
        LoaderKind::NeoForge if game == "1.20.1" => {
            Ok(format!("net.neoforged:forge:{maven}:installer"))
        }
        LoaderKind::NeoForge => Ok(format!("net.neoforged:neoforge:{maven}:installer")),
        other => Err(Error::LoaderUnsupported {
            loader: other.to_string(),
            game: game.to_string(),
            reason: "it has no installer".into(),
        }),
    }
}

/// Download URL of the installer for a Forge or NeoForge build (`maven` is the full Maven version).
/// Any other loader kind is `Error::LoaderUnsupported`.
pub fn installer_url(
    kind: LoaderKind,
    game: &str,
    maven: &str,
    ep: &LoaderEndpoints,
) -> Result<String> {
    let rel = maven_path(&installer_coords(kind, game, maven)?).expect("installer coordinates");
    Ok(match kind {
        LoaderKind::Forge => format!("{}/{rel}", ep.forge_maven),
        _ => format!("{}/releases/{rel}", ep.neoforge_maven),
    })
}

/// Downloads the installer into the shared libraries directory (at its Maven path), verified
/// against the `.sha1` the Maven repository publishes next to it. Reuses a valid local copy.
pub async fn fetch_installer(
    dl: &Downloader,
    paths: &Paths,
    kind: LoaderKind,
    game: &str,
    maven: &str,
    ep: &LoaderEndpoints,
) -> Result<PathBuf> {
    validate_id(game)?;
    validate_id(maven)?;
    let rel = maven_path(&installer_coords(kind, game, maven)?).expect("installer coordinates");
    let mut item = [Download {
        url: installer_url(kind, game, maven, ep)?,
        path: join_relative(&paths.libraries(), &rel)?,
        sha1: None,
    }];
    fill_missing_sha1(dl, &mut item).await?;
    dl.fetch_one(&item[0]).await?;
    let [item] = item;
    Ok(item.path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loaders::{list_versions, pick_version, LoaderKind};
    use wiremock::matchers::{any, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const FORGE_METADATA: &str =
        include_str!("../../../tests/fixtures/loaders/forge-maven-metadata.json");
    const FORGE_PROMOS: &str =
        include_str!("../../../tests/fixtures/loaders/forge-promotions.json");
    const NEO_VERSIONS: &str =
        include_str!("../../../tests/fixtures/loaders/neoforge-versions.json");
    const NEO_FORGE_VERSIONS: &str =
        include_str!("../../../tests/fixtures/loaders/neoforge-forge-versions.json");

    const FORGE_META_PATH: &str = "/forge/maven-metadata.json";
    const FORGE_PROMOS_PATH: &str = "/forge/promotions_slim.json";
    const NEO_PATH: &str = "/neo/api/maven/versions/releases/net/neoforged/neoforge";
    const NEO_FORGE_PATH: &str = "/neo/api/maven/versions/releases/net/neoforged/forge";

    fn ep(server: &MockServer) -> LoaderEndpoints {
        let u = server.uri();
        LoaderEndpoints {
            forge_files: format!("{u}/forge"),
            neoforge_maven: format!("{u}/neo"),
            ..LoaderEndpoints::default()
        }
    }

    async fn serve(server: &MockServer, p: &str, body: &str) {
        Mock::given(method("GET"))
            .and(path(p))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(server)
            .await;
    }

    async fn forge_server() -> MockServer {
        let server = MockServer::start().await;
        serve(&server, FORGE_META_PATH, FORGE_METADATA).await;
        serve(&server, FORGE_PROMOS_PATH, FORGE_PROMOS).await;
        server
    }

    async fn neo_server() -> MockServer {
        let server = MockServer::start().await;
        serve(&server, NEO_PATH, NEO_VERSIONS).await;
        serve(&server, NEO_FORGE_PATH, NEO_FORGE_VERSIONS).await;
        server
    }

    fn ids(l: &[LoaderVersion]) -> Vec<&str> {
        l.iter().map(|v| v.id.as_str()).collect()
    }

    #[test]
    fn forge_cutoff_is_1_7_10() {
        for (g, ok) in [
            ("1.6.4", false),
            ("1.7.2", false),
            ("1.5.2", false),
            ("1.7", false),
            ("1.7.10", true),
            ("1.8", true),
            ("1.12.2", true),
            ("1.20.1", true),
            ("26.3", true),
            ("1.21-pre1", true),
        ] {
            assert_eq!(forge_supports(g), ok, "{g}");
        }
    }

    #[test]
    fn maps_neoforge_versions_to_game_versions() {
        for (v, g) in [
            ("20.2.88", Some("1.20.2")),
            ("20.2.3-beta", Some("1.20.2")),
            ("20.4.237", Some("1.20.4")),
            ("21.0.167", Some("1.21")),
            ("21.1.267", Some("1.21.1")),
            ("21.11.48", Some("1.21.11")),
            ("26.3.0.115", Some("26.3")),
            ("26.1.0.1-beta", Some("26.1")),
            ("26.1.1.0-beta", Some("26.1.1")),
            ("26.1.2.53-beta", Some("26.1.2")),
            ("26.1.0.0-alpha.1+snapshot-1", None),
            ("0.25w14craftmine.3-beta", None),
            ("21.1", None),
            ("x.y.z", None),
            ("", None),
        ] {
            assert_eq!(neoforge_game_version(v).as_deref(), g, "{v}");
        }
    }

    #[tokio::test]
    async fn forge_list_uses_recommended_promo_and_short_ids() {
        let server = forge_server().await;
        let client = reqwest::Client::new();
        let l = list_forge(&client, &ep(&server), "1.7.10").await.unwrap();
        assert_eq!(l[0].id, "10.13.4.1614");
        assert_eq!(l[0].maven, "1.7.10-10.13.4.1614-1.7.10");
        assert!(l[0].stable);
        assert!(l[1..].iter().all(|v| !v.stable));
        // Old entries have no game version suffix, and some carry a different suffix.
        assert!(l
            .iter()
            .any(|v| v.maven == "1.7.10-10.13.0.1150" && v.id == "10.13.0.1150"));
        assert!(l.iter().any(|v| v.id == "10.13.1.1210-new"));
        let l = list_forge(&client, &ep(&server), "1.20.1").await.unwrap();
        assert_eq!(l[0].id, "47.4.29");
        assert!(!l[0].stable);
        assert_eq!(
            pick_version(&l, None, LoaderKind::Forge, "1.20.1")
                .unwrap()
                .id,
            "47.4.28"
        );
        let l = list_forge(&client, &ep(&server), "1.12.2").await.unwrap();
        assert_eq!(l[0].id, "14.23.5.2864");
        assert_eq!(
            pick_version(&l, None, LoaderKind::Forge, "1.12.2")
                .unwrap()
                .id,
            "14.23.5.2859"
        );
    }

    #[tokio::test]
    async fn forge_without_recommended_build_falls_back_to_newest_unstable() {
        let server = forge_server().await;
        let client = reqwest::Client::new();
        let l = list_forge(&client, &ep(&server), "1.8.8").await.unwrap();
        assert_eq!(l[0].id, "11.15.0.1655");
        assert_eq!(l[0].maven, "1.8.8-11.15.0.1655");
        assert!(l.iter().all(|v| !v.stable));
        assert_eq!(
            pick_version(&l, None, LoaderKind::Forge, "1.8.8")
                .unwrap()
                .id,
            "11.15.0.1655"
        );
    }

    #[tokio::test]
    async fn forge_before_1_7_10_is_refused_without_network() {
        let server = MockServer::start().await;
        Mock::given(any())
            .respond_with(ResponseTemplate::new(404))
            .expect(0)
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let err = list_forge(&client, &ep(&server), "1.6.4")
            .await
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "forge for Minecraft 1.6.4 is not supported (Forge support starts at 1.7.10)"
        );
        assert!(matches!(
            list_versions(&client, &ep(&server), LoaderKind::Forge, "1.6.4").await,
            Err(Error::LoaderUnsupported { .. })
        ));
    }

    #[tokio::test]
    async fn neoforge_lists_per_game_version() {
        let server = neo_server().await;
        let client = reqwest::Client::new();
        let ep = ep(&server);
        let l = list_neoforge(&client, &ep, "26.3").await.unwrap();
        assert_eq!(ids(&l), ["26.3.0.115", "26.3.0.114", "26.3.0.75-beta"]);
        assert_eq!(
            l.iter().map(|v| v.stable).collect::<Vec<_>>(),
            [true, true, false]
        );
        let l = list_neoforge(&client, &ep, "1.21.1").await.unwrap();
        assert_eq!(l[0].id, "21.1.267");
        assert_eq!(l[0].maven, "21.1.267");
        let l = list_neoforge(&client, &ep, "1.21").await.unwrap();
        assert_eq!(ids(&l), ["21.0.167", "21.0.166"]);
        let l = list_neoforge(&client, &ep, "1.20.2").await.unwrap();
        assert_eq!(ids(&l), ["20.2.88", "20.2.3-beta"]);
        assert!(!l[1].stable);
        let l = list_neoforge(&client, &ep, "1.20.1").await.unwrap();
        assert_eq!(ids(&l), ["47.1.106", "47.1.5"]);
        assert_eq!(l[0].maven, "1.20.1-47.1.106");
        assert!(l.iter().all(|v| v.stable));
        let l = list_neoforge(&client, &ep, "26.1.1").await.unwrap();
        assert_eq!(ids(&l), ["26.1.1.1-beta", "26.1.1.0-beta"]);
        assert!(l.iter().all(|v| !v.stable));
        // Snapshot builds are not listed under any game version.
        let l = list_neoforge(&client, &ep, "26.1").await.unwrap();
        assert_eq!(ids(&l), ["26.1.0.1-beta"]);
        assert!(matches!(
            list_versions(&client, &ep, LoaderKind::NeoForge, "1.16.5").await,
            Err(Error::NoLoaderVersions { .. })
        ));
    }

    #[tokio::test]
    async fn dispatch_reaches_forge_neoforge_and_fabric_family() {
        let server = MockServer::start().await;
        serve(&server, FORGE_META_PATH, FORGE_METADATA).await;
        serve(&server, FORGE_PROMOS_PATH, FORGE_PROMOS).await;
        serve(&server, NEO_PATH, NEO_VERSIONS).await;
        serve(
            &server,
            "/fabric/versions/loader/1.21.11",
            r#"[{"loader":{"version":"0.20.1","maven":"net.fabricmc:fabric-loader:0.20.1","stable":true}}]"#,
        )
        .await;
        let ep = LoaderEndpoints {
            fabric: format!("{}/fabric", server.uri()),
            ..ep(&server)
        };
        let client = reqwest::Client::new();
        let f = list_versions(&client, &ep, LoaderKind::Forge, "1.7.10")
            .await
            .unwrap();
        assert_eq!(f[0].id, "10.13.4.1614");
        let n = list_versions(&client, &ep, LoaderKind::NeoForge, "1.21.1")
            .await
            .unwrap();
        assert_eq!(n[0].id, "21.1.267");
        let fab = list_versions(&client, &ep, LoaderKind::Fabric, "1.21.11")
            .await
            .unwrap();
        assert_eq!(fab[0].id, "0.20.1");
        // A game version Forge has no builds for.
        assert!(matches!(
            list_versions(&client, &ep, LoaderKind::Forge, "9.9.9").await,
            Err(Error::NoLoaderVersions { .. })
        ));
    }

    #[tokio::test]
    async fn hostile_entries_are_skipped() {
        let server = MockServer::start().await;
        let meta = r#"{"1.20.1":["1.20.1-47.4.28","1.20.1-../x","1.20.1-a/b","1.20.1-47.4.1-1.20.1/../y","1.20.1-"]}"#;
        serve(&server, FORGE_META_PATH, meta).await;
        serve(&server, FORGE_PROMOS_PATH, FORGE_PROMOS).await;
        serve(
            &server,
            NEO_PATH,
            r#"{"versions":["21.1.5","21.1.6-a/b","21.1.7/../x","21.1.8-..\\x"]}"#,
        )
        .await;
        serve(
            &server,
            NEO_FORGE_PATH,
            r#"{"versions":["1.20.1-47.1.5","1.20.1-../x","1.20.1-a/b","1.20.1-"]}"#,
        )
        .await;
        let client = reqwest::Client::new();
        let ep = ep(&server);
        let l = list_forge(&client, &ep, "1.20.1").await.unwrap();
        assert_eq!(ids(&l), ["47.4.28"]);
        let l = list_neoforge(&client, &ep, "1.21.1").await.unwrap();
        assert_eq!(ids(&l), ["21.1.5"]);
        let l = list_neoforge(&client, &ep, "1.20.1").await.unwrap();
        assert_eq!(ids(&l), ["47.1.5"]);
    }

    #[tokio::test]
    async fn unreachable_service_is_a_transport_error() {
        // A port that was just free and has no listener.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let u = format!("http://127.0.0.1:{port}");
        let ep = LoaderEndpoints {
            forge_files: u.clone(),
            neoforge_maven: u,
            ..LoaderEndpoints::default()
        };
        let client = reqwest::Client::new();
        assert!(matches!(
            list_forge(&client, &ep, "1.20.1").await,
            Err(Error::Http { .. })
        ));
        assert!(matches!(
            list_neoforge(&client, &ep, "1.21.1").await,
            Err(Error::Http { .. })
        ));
    }

    #[test]
    fn installer_urls() {
        let ep = LoaderEndpoints::default();
        assert_eq!(
            installer_url(LoaderKind::Forge, "1.7.10", "1.7.10-10.13.4.1614-1.7.10", &ep).unwrap(),
            "https://maven.minecraftforge.net/net/minecraftforge/forge/1.7.10-10.13.4.1614-1.7.10/forge-1.7.10-10.13.4.1614-1.7.10-installer.jar"
        );
        assert_eq!(
            installer_url(LoaderKind::NeoForge, "1.20.1", "1.20.1-47.1.106", &ep).unwrap(),
            "https://maven.neoforged.net/releases/net/neoforged/forge/1.20.1-47.1.106/forge-1.20.1-47.1.106-installer.jar"
        );
        assert_eq!(
            installer_url(LoaderKind::NeoForge, "26.3", "26.3.0.115", &ep).unwrap(),
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/26.3.0.115/neoforge-26.3.0.115-installer.jar"
        );
    }

    #[test]
    fn installer_url_for_other_loaders_is_unsupported() {
        let ep = LoaderEndpoints::default();
        assert!(matches!(
            installer_url(LoaderKind::Fabric, "1.21.1", "0.17.3", &ep),
            Err(Error::LoaderUnsupported { .. })
        ));
    }

    const NEO_INSTALLER: &str =
        "/neo/releases/net/neoforged/neoforge/21.1.172/neoforge-21.1.172-installer.jar";

    async fn installer_server(sidecar: Option<String>) -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(NEO_INSTALLER))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"installer".to_vec()))
            .mount(&server)
            .await;
        if let Some(body) = sidecar {
            Mock::given(method("GET"))
                .and(path(format!("{NEO_INSTALLER}.sha1")))
                .respond_with(ResponseTemplate::new(200).set_body_string(body))
                .mount(&server)
                .await;
        }
        server
    }

    #[tokio::test]
    async fn fetches_installer_verified_by_sidecar() {
        let server = installer_server(Some(crate::download::sha1_hex(b"installer"))).await;
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let dl = Downloader::new(reqwest::Client::new());
        let file = fetch_installer(
            &dl,
            &paths,
            LoaderKind::NeoForge,
            "1.21.1",
            "21.1.172",
            &ep(&server),
        )
        .await
        .unwrap();
        assert_eq!(
            file,
            paths
                .libraries()
                .join("net/neoforged/neoforge/21.1.172/neoforge-21.1.172-installer.jar")
        );
        assert_eq!(std::fs::read(&file).unwrap(), b"installer");
    }

    #[tokio::test]
    async fn installer_with_wrong_sidecar_is_refused() {
        let server = installer_server(Some(crate::download::sha1_hex(b"something else"))).await;
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let dl = Downloader::new(reqwest::Client::new());
        let err = fetch_installer(
            &dl,
            &paths,
            LoaderKind::NeoForge,
            "1.21.1",
            "21.1.172",
            &ep(&server),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, Error::Checksum { .. }), "{err}");
        assert!(!paths
            .libraries()
            .join("net/neoforged/neoforge/21.1.172/neoforge-21.1.172-installer.jar")
            .exists());
    }

    #[tokio::test]
    async fn installer_without_sidecar_is_refused() {
        let server = installer_server(None).await;
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let dl = Downloader::new(reqwest::Client::new());
        let err = fetch_installer(
            &dl,
            &paths,
            LoaderKind::NeoForge,
            "1.21.1",
            "21.1.172",
            &ep(&server),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, Error::NoChecksum { .. }), "{err}");
    }

    #[tokio::test]
    async fn installer_version_must_be_a_plain_id() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let dl = Downloader::new(reqwest::Client::new());
        let ep = LoaderEndpoints::default();
        for bad in ["../x", "1.0/../../x", "a:b", r"a\b"] {
            assert!(
                matches!(
                    fetch_installer(&dl, &paths, LoaderKind::Forge, "1.12.2", bad, &ep).await,
                    Err(Error::UnsafePath(_))
                ),
                "{bad}"
            );
        }
    }
}
