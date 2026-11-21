//! Meta clients for the Fabric family: Fabric, Quilt and Legacy Fabric.

use serde::Deserialize;

use super::{
    is_unreachable, no_versions, sort_newest_first, LoaderEndpoints, LoaderKind, LoaderVersion,
};
use crate::download::get_bytes;
use crate::paths::{validate_id, Paths};
use crate::version::VersionJson;
use crate::{Error, Result};

/// Base URL of the meta service for a Fabric-family loader. Forge and NeoForge are not served by
/// a Fabric-style meta service.
pub fn meta_base<'a>(kind: LoaderKind, game: &str, ep: &'a LoaderEndpoints) -> Result<&'a str> {
    match kind {
        LoaderKind::Fabric => Ok(&ep.fabric),
        LoaderKind::Quilt => Ok(&ep.quilt),
        LoaderKind::LegacyFabric => Ok(&ep.legacy_fabric),
        LoaderKind::NeoForge | LoaderKind::Forge => Err(Error::LoaderUnsupported {
            loader: kind.to_string(),
            game: game.to_string(),
            reason: "not served by a Fabric-style meta service".into(),
        }),
    }
}

#[derive(Deserialize)]
struct Entry {
    loader: Loader,
}

#[derive(Deserialize)]
struct Loader {
    version: String,
    maven: String,
    stable: Option<bool>,
}

/// Version ids may contain inner spaces (`1.14 Pre-Release 1`); nothing else needs escaping once
/// `validate_id` has passed.
fn encode(segment: &str) -> String {
    segment.replace(' ', "%20")
}

fn is_missing(e: &Error) -> bool {
    matches!(
        e,
        Error::Status {
            status: 400 | 404,
            ..
        }
    )
}

/// Loader versions for a game version, newest first.
pub async fn list_versions(
    client: &reqwest::Client,
    ep: &LoaderEndpoints,
    kind: LoaderKind,
    game: &str,
) -> Result<Vec<LoaderVersion>> {
    let base = meta_base(kind, game, ep)?;
    validate_id(game)?;
    let url = format!("{base}/versions/loader/{}", encode(game));
    let none = || no_versions(kind, game);
    let bytes = match get_bytes(client, &url).await {
        Ok(b) => b,
        Err(e) if is_missing(&e) => return Err(none()),
        Err(e) => return Err(e),
    };
    let entries: Vec<Entry> = serde_json::from_slice(&bytes)?;
    let mut versions: Vec<LoaderVersion> = entries
        .into_iter()
        .map(|e| LoaderVersion {
            stable: match kind {
                LoaderKind::Quilt => !e.loader.version.contains('-'),
                _ => e.loader.stable.unwrap_or(false),
            },
            id: e.loader.version,
            maven: e.loader.maven,
        })
        .collect();
    if versions.is_empty() {
        return Err(none());
    }
    sort_newest_first(&mut versions);
    Ok(versions)
}

/// Parses a profile and checks the remote-controlled fields before anything is stored or used.
fn parse_profile(bytes: &[u8], game: &str) -> Result<VersionJson> {
    let profile: VersionJson = serde_json::from_slice(bytes)?;
    validate_id(&profile.id)?;
    if profile.inherits_from.as_deref() != Some(game) {
        return Err(Error::ParentMismatch {
            expected: game.to_string(),
            found: profile.inherits_from.unwrap_or_else(|| "nothing".into()),
        });
    }
    Ok(profile)
}

/// Fetches the launcher profile of one loader build and stores the raw bytes. When the service
/// is unreachable, the stored copy is used.
pub async fn fetch_profile(
    client: &reqwest::Client,
    paths: &Paths,
    ep: &LoaderEndpoints,
    kind: LoaderKind,
    game: &str,
    version: &str,
) -> Result<VersionJson> {
    let base = meta_base(kind, game, ep)?;
    validate_id(game)?;
    validate_id(version)?;
    let url = format!(
        "{base}/versions/loader/{}/{}/profile/json",
        encode(game),
        encode(version)
    );
    let file = paths
        .loader_dir(kind.slug())
        .join(format!("{game}-{version}.json"));
    match get_bytes(client, &url).await {
        Ok(bytes) => {
            let profile = parse_profile(&bytes, game)?;
            if let Some(dir) = file.parent() {
                tokio::fs::create_dir_all(dir).await?;
            }
            tokio::fs::write(&file, &bytes).await?;
            Ok(profile)
        }
        Err(e) if is_missing(&e) => Err(Error::UnknownLoaderVersion {
            loader: kind.to_string(),
            game: game.to_string(),
            version: version.to_string(),
        }),
        Err(e) if is_unreachable(&e) && file.is_file() => {
            parse_profile(&tokio::fs::read(&file).await?, game)
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const FABRIC_LIST: &str =
        include_str!("../../tests/fixtures/loaders/fabric-loaders-1.21.11.json");
    const FABRIC_PROFILE: &str =
        include_str!("../../tests/fixtures/loaders/fabric-profile-1.21.11-0.17.3.json");
    const QUILT_LIST: &str =
        include_str!("../../tests/fixtures/loaders/quilt-loaders-1.21.11.json");
    const QUILT_PROFILE: &str =
        include_str!("../../tests/fixtures/loaders/quilt-profile-1.21.11-0.29.2.json");
    const LEGACY_LIST: &str =
        include_str!("../../tests/fixtures/loaders/legacy-fabric-loaders-1.8.9.json");
    const LEGACY_PROFILE: &str =
        include_str!("../../tests/fixtures/loaders/legacy-fabric-profile-1.8.9-0.20.1.json");

    fn ep(server: &MockServer) -> LoaderEndpoints {
        let u = server.uri();
        LoaderEndpoints {
            fabric: format!("{u}/fabric/v2"),
            quilt: format!("{u}/quilt/v3"),
            legacy_fabric: format!("{u}/legacy/v2"),
            ..LoaderEndpoints::default()
        }
    }

    async fn serve(server: &MockServer, p: &str, status: u16, body: &str) {
        Mock::given(method("GET"))
            .and(path(p))
            .respond_with(ResponseTemplate::new(status).set_body_string(body))
            .mount(server)
            .await;
    }

    fn ids(l: &[LoaderVersion]) -> Vec<&str> {
        l.iter().map(|v| v.id.as_str()).collect()
    }

    #[tokio::test]
    async fn fabric_list_is_newest_first_with_stable_flag() {
        let server = MockServer::start().await;
        serve(
            &server,
            "/fabric/v2/versions/loader/1.21.11",
            200,
            FABRIC_LIST,
        )
        .await;
        let client = reqwest::Client::new();
        let l = list_versions(&client, &ep(&server), LoaderKind::Fabric, "1.21.11")
            .await
            .unwrap();
        assert_eq!(ids(&l), ["0.20.1", "0.20.0", "0.19.5"]);
        assert_eq!(
            l.iter().map(|v| v.stable).collect::<Vec<_>>(),
            [true, false, false]
        );
        assert_eq!(l[0].maven, "net.fabricmc:fabric-loader:0.20.1");
    }

    #[tokio::test]
    async fn quilt_list_is_sorted_and_prereleases_are_unstable() {
        let server = MockServer::start().await;
        serve(
            &server,
            "/quilt/v3/versions/loader/1.21.11",
            200,
            QUILT_LIST,
        )
        .await;
        let client = reqwest::Client::new();
        let l = list_versions(&client, &ep(&server), LoaderKind::Quilt, "1.21.11")
            .await
            .unwrap();
        assert_eq!(
            ids(&l),
            [
                "0.31.0-beta.6",
                "0.30.2",
                "0.30.2-beta.1",
                "0.29.2",
                "0.24.0",
                "0.20.0-beta.9"
            ]
        );
        assert_eq!(
            crate::loaders::pick_version(&l, None, LoaderKind::Quilt, "1.21.11")
                .unwrap()
                .id,
            "0.30.2"
        );
    }

    #[tokio::test]
    async fn legacy_fabric_list_uses_its_own_service() {
        let server = MockServer::start().await;
        serve(
            &server,
            "/legacy/v2/versions/loader/1.8.9",
            200,
            LEGACY_LIST,
        )
        .await;
        let client = reqwest::Client::new();
        let l = list_versions(&client, &ep(&server), LoaderKind::LegacyFabric, "1.8.9")
            .await
            .unwrap();
        assert_eq!(ids(&l), ["0.20.1", "0.20.0"]);
        assert_eq!(
            l.iter().map(|v| v.stable).collect::<Vec<_>>(),
            [true, false]
        );
    }

    #[tokio::test]
    async fn unknown_game_version_means_no_loader_versions() {
        let server = MockServer::start().await;
        serve(&server, "/fabric/v2/versions/loader/9.9.9", 400, "[]").await;
        serve(&server, "/quilt/v3/versions/loader/9.9.9", 404, "").await;
        serve(&server, "/legacy/v2/versions/loader/9.9.9", 200, "[]").await;
        let client = reqwest::Client::new();
        for kind in [
            LoaderKind::Fabric,
            LoaderKind::Quilt,
            LoaderKind::LegacyFabric,
        ] {
            assert!(
                matches!(
                    list_versions(&client, &ep(&server), kind, "9.9.9").await,
                    Err(Error::NoLoaderVersions { .. })
                ),
                "{kind}"
            );
        }
    }

    #[tokio::test]
    async fn forge_family_is_unsupported_not_a_panic() {
        let ep = LoaderEndpoints::default();
        let client = reqwest::Client::new();
        for kind in [LoaderKind::Forge, LoaderKind::NeoForge] {
            assert!(matches!(
                list_versions(&client, &ep, kind, "1.21.11").await,
                Err(Error::LoaderUnsupported { ref reason, .. }) if reason == "not served by a Fabric-style meta service"
            ));
        }
    }

    #[tokio::test]
    async fn fetches_and_caches_profiles_for_all_three() {
        let server = MockServer::start().await;
        serve(
            &server,
            "/fabric/v2/versions/loader/1.21.11/0.17.3/profile/json",
            200,
            FABRIC_PROFILE,
        )
        .await;
        serve(
            &server,
            "/quilt/v3/versions/loader/1.21.11/0.29.2/profile/json",
            200,
            QUILT_PROFILE,
        )
        .await;
        serve(
            &server,
            "/legacy/v2/versions/loader/1.8.9/0.20.1/profile/json",
            200,
            LEGACY_PROFILE,
        )
        .await;
        let client = reqwest::Client::new();
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let ep = ep(&server);
        let f = fetch_profile(
            &client,
            &paths,
            &ep,
            LoaderKind::Fabric,
            "1.21.11",
            "0.17.3",
        )
        .await
        .unwrap();
        assert_eq!(f.id, "fabric-loader-0.17.3-1.21.11");
        assert_eq!(f.inherits_from.as_deref(), Some("1.21.11"));
        assert_eq!(f.libraries.len(), 8);
        assert_eq!(
            f.libraries[0].sha1.as_deref(),
            Some("c29635c8a7afa03d74b33c1884df8abb2b3f3dcc")
        );
        assert!(f
            .libraries
            .iter()
            .find(|l| l.name == "net.fabricmc:fabric-loader:0.17.3")
            .unwrap()
            .sha1
            .is_none());
        let q = fetch_profile(&client, &paths, &ep, LoaderKind::Quilt, "1.21.11", "0.29.2")
            .await
            .unwrap();
        assert_eq!(
            q.main_class,
            "org.quiltmc.loader.impl.launch.knot.KnotClient"
        );
        assert!(q.libraries.iter().all(|l| l.sha1.is_none()));
        let lf = fetch_profile(
            &client,
            &paths,
            &ep,
            LoaderKind::LegacyFabric,
            "1.8.9",
            "0.20.1",
        )
        .await
        .unwrap();
        assert!(lf.arguments.is_none());
        assert!(lf.libraries.iter().any(|l| l.natives.is_some()));
        assert!(paths
            .loader_dir("legacy-fabric")
            .join("1.8.9-0.20.1.json")
            .is_file());
        assert_eq!(
            std::fs::read_to_string(paths.loader_dir("fabric").join("1.21.11-0.17.3.json"))
                .unwrap(),
            FABRIC_PROFILE
        );
    }

    #[tokio::test]
    async fn falls_back_to_cached_profile_when_offline() {
        let server = MockServer::start().await;
        let p = "/fabric/v2/versions/loader/1.21.11/0.17.3/profile/json";
        Mock::given(method("GET"))
            .and(path(p))
            .respond_with(ResponseTemplate::new(200).set_body_string(FABRIC_PROFILE))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        serve(&server, p, 503, "").await;
        let client = reqwest::Client::new();
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let ep = ep(&server);
        let first = fetch_profile(
            &client,
            &paths,
            &ep,
            LoaderKind::Fabric,
            "1.21.11",
            "0.17.3",
        )
        .await
        .unwrap();
        let second = fetch_profile(
            &client,
            &paths,
            &ep,
            LoaderKind::Fabric,
            "1.21.11",
            "0.17.3",
        )
        .await
        .unwrap();
        assert_eq!(first.id, second.id);

        // Without a stored copy the outage is an error.
        let other = tempfile::tempdir().unwrap();
        let err = fetch_profile(
            &client,
            &Paths::new(other.path()),
            &ep,
            LoaderKind::Fabric,
            "1.21.11",
            "0.17.3",
        )
        .await;
        assert!(matches!(err, Err(Error::Status { status: 503, .. })));
    }

    #[tokio::test]
    async fn unknown_loader_version_for_profile() {
        let server = MockServer::start().await;
        serve(
            &server,
            "/fabric/v2/versions/loader/1.21.11/0.0.1/profile/json",
            400,
            "no loader version found for 0.0.1",
        )
        .await;
        let client = reqwest::Client::new();
        let dir = tempfile::tempdir().unwrap();
        let r = fetch_profile(
            &client,
            &Paths::new(dir.path()),
            &ep(&server),
            LoaderKind::Fabric,
            "1.21.11",
            "0.0.1",
        )
        .await;
        assert!(matches!(r, Err(Error::UnknownLoaderVersion { .. })));
    }

    #[tokio::test]
    async fn rejects_profiles_with_wrong_parent_or_unsafe_id() {
        let server = MockServer::start().await;
        // The profile is for 1.21.11, but 1.21.10 is requested.
        serve(
            &server,
            "/fabric/v2/versions/loader/1.21.10/0.17.3/profile/json",
            200,
            FABRIC_PROFILE,
        )
        .await;
        let evil = FABRIC_PROFILE.replace("fabric-loader-0.17.3-1.21.11", "../../evil");
        serve(
            &server,
            "/fabric/v2/versions/loader/1.21.11/0.17.3/profile/json",
            200,
            &evil,
        )
        .await;
        let client = reqwest::Client::new();
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let ep = ep(&server);
        let r = fetch_profile(
            &client,
            &paths,
            &ep,
            LoaderKind::Fabric,
            "1.21.10",
            "0.17.3",
        )
        .await;
        assert!(matches!(r, Err(Error::ParentMismatch { .. })));
        let r = fetch_profile(
            &client,
            &paths,
            &ep,
            LoaderKind::Fabric,
            "1.21.11",
            "0.17.3",
        )
        .await;
        assert!(matches!(r, Err(Error::UnsafePath(_))));
        assert!(!paths.loader_dir("fabric").exists());
    }
}
