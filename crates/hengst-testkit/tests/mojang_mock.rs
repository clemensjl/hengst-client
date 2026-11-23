//! The Mojang fixtures are complete enough for the core to prepare a version from them.
#![cfg(windows)]

use hengst_core::args::Account;
use hengst_core::launch::{prepare, LaunchOptions};
use hengst_core::paths::Paths;
use hengst_testkit::{endpoints, mount_mojang};
use wiremock::MockServer;

#[tokio::test]
async fn the_core_prepares_every_listed_version_from_the_mock() {
    let server = MockServer::builder().start().await;
    mount_mojang(&server).await;
    let ep = endpoints(&server.uri());
    for id in ["9.9", "9.9-snap", "b9.9", "a9.9"] {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let opts = LaunchOptions {
            version: id.into(),
            instance: None,
            account: Account::offline("Dev"),
            java: None,
            memory_mb: 1024,
            loader: None,
        };
        let p = prepare(&paths, &opts, &ep).await.unwrap();
        assert_eq!(p.version_id, id);
        assert!(paths.client_jar(id).is_file(), "{id}");
    }
}
