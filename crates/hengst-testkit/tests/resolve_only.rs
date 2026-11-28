//! `launch::resolve` (behind `hengst launch --resolve-only`) against the Mojang and Fabric mocks:
//! it names the game version and the loader build and fetches nothing but metadata.

use hengst_core::launch::resolve;
use hengst_core::loaders::{LoaderKind, LoaderRequest};
use hengst_core::paths::Paths;
use hengst_core::Error;
use hengst_testkit::{endpoints, mount_mojang};
use wiremock::MockServer;

async fn mock() -> MockServer {
    let server = MockServer::builder().start().await;
    mount_mojang(&server).await;
    server
}

fn fabric(version: Option<&str>) -> LoaderRequest {
    LoaderRequest {
        kind: LoaderKind::Fabric,
        version: version.map(str::to_string),
    }
}

/// Every request path the mock received, in order.
async fn requested(server: &MockServer) -> Vec<String> {
    server
        .received_requests()
        .await
        .unwrap_or_default()
        .iter()
        .map(|r| r.url.path().to_string())
        .collect()
}

#[tokio::test]
async fn latest_resolves_to_the_newest_release_without_a_loader() {
    let server = mock().await;
    let dir = tempfile::tempdir().unwrap();
    let r = resolve(
        &Paths::new(dir.path()),
        "latest",
        None,
        &endpoints(&server.uri()),
    )
    .await
    .unwrap();
    assert_eq!(r.game, "9.9");
    assert!(r.loader.is_none());
    assert!(!r.loader_list_from_cache);
}

#[tokio::test]
async fn the_default_loader_build_is_the_newest_stable_one() {
    let server = mock().await;
    let dir = tempfile::tempdir().unwrap();
    let r = resolve(
        &Paths::new(dir.path()),
        "9.9",
        Some(&fabric(None)),
        &endpoints(&server.uri()),
    )
    .await
    .unwrap();
    assert_eq!(r.game, "9.9");
    let loader = r.loader.unwrap();
    assert_eq!(loader.kind, LoaderKind::Fabric);
    assert_eq!(loader.version.id, "0.99.0");
    assert!(loader.version.stable);
}

#[tokio::test]
async fn a_named_loader_build_is_checked_against_the_list() {
    let server = mock().await;
    let dir = tempfile::tempdir().unwrap();
    let (paths, ep) = (Paths::new(dir.path()), endpoints(&server.uri()));
    let r = resolve(&paths, "9.9", Some(&fabric(Some("0.100.0-beta.1"))), &ep)
        .await
        .unwrap();
    let loader = r.loader.unwrap();
    assert_eq!(loader.version.id, "0.100.0-beta.1");
    assert!(!loader.version.stable);

    let err = resolve(&paths, "9.9", Some(&fabric(Some("0.1.0"))), &ep)
        .await
        .unwrap_err();
    assert!(matches!(err, Error::UnknownLoaderVersion { .. }), "{err}");
}

#[tokio::test]
async fn unknown_versions_and_empty_loader_lists_are_errors() {
    let server = mock().await;
    let dir = tempfile::tempdir().unwrap();
    let (paths, ep) = (Paths::new(dir.path()), endpoints(&server.uri()));
    let err = resolve(&paths, "8.8", None, &ep).await.unwrap_err();
    assert!(matches!(err, Error::UnknownVersion(_)), "{err}");
    let err = resolve(&paths, "b9.9", Some(&fabric(None)), &ep)
        .await
        .unwrap_err();
    assert!(matches!(err, Error::NoLoaderVersions { .. }), "{err}");
}

#[tokio::test]
async fn only_metadata_is_fetched_and_nothing_is_installed() {
    let server = mock().await;
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    resolve(
        &paths,
        "9.9",
        Some(&fabric(None)),
        &endpoints(&server.uri()),
    )
    .await
    .unwrap();
    assert_eq!(
        requested(&server).await,
        ["/mojang/manifest.json", "/fabric/versions/loader/9.9"]
    );
    assert!(!paths.client_jar("9.9").exists());
    assert!(!paths.libraries().exists());
    assert!(!paths.assets().exists());
    assert!(!paths.instance("9.9-fabric").exists());
    assert!(!paths.tmp().exists(), "no run lock or temp file");
}
