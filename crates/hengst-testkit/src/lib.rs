//! Mock services for tests of the Hengst Client: the Microsoft, Xbox and Minecraft sign-in
//! chain ([`auth`]) and the Mojang version manifest plus the Fabric meta service ([`mojang`]).
//!
//! Dev only. Nothing that ships may depend on this crate unconditionally: the CLI does not
//! depend on it at all, the desktop app only through its `e2e` feature, which cannot be built in
//! release mode. `tests/release_isolation.rs` checks that on every `cargo test --workspace`.
//!
//! Every mock lives on a [`wiremock::MockServer`], which binds an OS-chosen port on
//! `127.0.0.1`. Start it with `MockServer::builder().start()` (as [`Services`] does): that server
//! stops when its handle is dropped, whereas `MockServer::start()` hands out a pooled server that
//! outlives the handle. Nothing here uses global state, so tests can
//! run in parallel. All values are fake: tokens start with `test-`, hosts that are not the mock
//! use the reserved `.invalid` top-level domain.

pub mod auth;
pub mod mojang;

pub use auth::{
    auth_endpoints, mount_auth, AuthHandle, AuthScenario, MockAccount, RecordedRequest, Redacted,
    PLAYER_A, PLAYER_B,
};
pub use mojang::{endpoints, mount_mojang};

use wiremock::MockServer;

/// Plays the system browser of a browser sign-in: opens `authorize_url` (the core's
/// `BrowserLogin::authorize_url`), follows the mock's redirect to the core's loopback listener
/// (`http://localhost:<port>/?code=..&state=..`, sent with the matching `Host` header, which the
/// listener requires) and returns the page the launcher answered with. Call it concurrently with
/// `finish_browser_login`: the page only arrives after the token exchange.
pub async fn browser_visit(authorize_url: &str) -> Result<String, reqwest::Error> {
    // No proxy: the loopback listener must be reached directly, whatever the machine configures.
    reqwest::Client::builder()
        .no_proxy()
        .build()?
        .get(authorize_url)
        .send()
        .await?
        .text()
        .await
}

/// The `fake-java` binary of this crate, for tests of other crates (only this crate's own tests
/// get `CARGO_BIN_EXE_fake-java`). It is looked for in the profile folder of the running test
/// executable (`<target>/debug`, where `cargo test --workspace` puts it); when it is missing or
/// older than its source it is built there first with `cargo build -j 6 -p hengst-testkit --bin
/// fake-java`, once per test process. Panics if that build fails.
pub fn fake_java() -> std::path::PathBuf {
    use std::path::{Path, PathBuf};
    use std::sync::OnceLock;
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let exe = std::env::current_exe().expect("the test executable's path");
        // `<target>/debug/deps/<test>.exe` (unit and integration tests) or `<target>/debug/<x>.exe`.
        let mut profile = exe.parent().expect("a folder").to_path_buf();
        if profile.file_name().is_some_and(|n| n == "deps") {
            profile.pop();
        }
        let bin = profile.join(format!("fake-java{}", std::env::consts::EXE_SUFFIX));
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bin/fake-java.rs");
        let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
        let fresh = match (modified(&bin), modified(&source)) {
            (Some(b), Some(s)) => b >= s,
            (Some(_), None) => true,
            (None, _) => false,
        };
        if !fresh {
            let target = profile.parent().expect("the target folder");
            let status = std::process::Command::new(
                std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()),
            )
            .args([
                "build",
                "-j",
                "6",
                "-p",
                "hengst-testkit",
                "--bin",
                "fake-java",
            ])
            .arg("--target-dir")
            .arg(target)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .status()
            .expect("cargo could not be started to build fake-java");
            assert!(status.success(), "building fake-java failed");
        }
        assert!(bin.is_file(), "no fake-java at {}", bin.display());
        bin
    })
    .clone()
}

/// One mock server with both the sign-in chain (accounts A and B) and the Mojang services
/// mounted: what the end-to-end binary `hengst-mock` serves.
pub struct Services {
    server: MockServer,
    /// Control and inspection of the sign-in mock.
    pub auth: AuthHandle,
}

impl Services {
    pub async fn start(scenario: AuthScenario) -> Services {
        let server = MockServer::builder().start().await;
        mount_mojang(&server).await;
        let auth = mount_auth(&server, scenario, &[PLAYER_A, PLAYER_B]).await;
        Services { server, auth }
    }

    /// `http://127.0.0.1:<port>`, the base for [`auth_endpoints`] and [`endpoints`].
    pub fn uri(&self) -> String {
        self.server.uri()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hengst_core::auth::browser::BrowserLoginPages;
    use hengst_core::auth::{AuthError, MicrosoftAuth};
    use hengst_core::loaders::{list_versions, pick_version, LoaderKind};
    use hengst_core::manifest::{fetch_manifest, VersionKind};
    use hengst_core::paths::Paths;
    use hengst_core::Error;
    use std::time::Duration;

    fn pages() -> BrowserLoginPages {
        BrowserLoginPages {
            success_title: "Signed in".into(),
            success_body: "You can close this tab.".into(),
            failure_title: "Sign-in failed".into(),
            failure_body: "Go back to the launcher.".into(),
        }
    }

    async fn browser_login(
        auth: &MicrosoftAuth,
    ) -> (hengst_core::auth::flow::LoginSession, String) {
        let login = auth.start_browser_login().await.unwrap();
        let url = login.authorize_url().to_string();
        let visit = tokio::spawn(async move { browser_visit(&url).await.unwrap() });
        let tokens = auth
            .finish_browser_login(login, &pages(), Duration::from_secs(20))
            .await
            .unwrap();
        let session = auth
            .complete_login(tokens, hengst_core::auth::unix_now())
            .await
            .unwrap();
        (session, visit.await.unwrap())
    }

    #[tokio::test]
    async fn device_flow_against_mock_signs_in_player_a() {
        let s = Services::start(AuthScenario::Ok).await;
        let auth = MicrosoftAuth::new("e2e-client".into(), auth_endpoints(&s.uri()));
        let code = auth.start_device_login().await.unwrap();
        assert!(!code.user_code.is_empty());
        let tokens = auth.poll_device_login(&code).await.unwrap();
        let session = auth.complete_login(tokens, 1_000).await.unwrap();
        assert_eq!(session.profile.name, "E2EPlayer");
        assert_eq!(session.profile.id, PLAYER_A.uuid);
        assert_eq!(session.minecraft_token.expose(), PLAYER_A.minecraft_token);
        assert_eq!(session.refresh_token.expose(), PLAYER_A.refresh_token);
        assert_eq!(session.xuid, PLAYER_A.xuid());
    }

    #[tokio::test]
    async fn browser_flow_against_mock_signs_in_a_then_b() {
        let s = Services::start(AuthScenario::Ok).await;
        let auth = MicrosoftAuth::new("e2e-client".into(), auth_endpoints(&s.uri()));
        let (first, page) = browser_login(&auth).await;
        assert_eq!(first.profile.name, "E2EPlayer");
        assert!(page.contains("Signed in"), "{page}");
        let (second, _) = browser_login(&auth).await;
        assert_eq!(second.profile.name, "SecondPlayer");
        assert_eq!(second.profile.id, PLAYER_B.uuid);
    }

    #[tokio::test]
    async fn not_approved_scenario_yields_app_not_approved() {
        let s = Services::start(AuthScenario::NotApproved).await;
        let auth = MicrosoftAuth::new("e2e-client".into(), auth_endpoints(&s.uri()));
        let code = auth.start_device_login().await.unwrap();
        let tokens = auth.poll_device_login(&code).await.unwrap();
        let err = auth.complete_login(tokens, 1).await.unwrap_err();
        assert!(
            matches!(err, Error::Auth(AuthError::AppNotApproved)),
            "{err}"
        );
    }

    #[tokio::test]
    async fn mojang_mock_lists_all_kinds_and_fabric_versions() {
        let s = Services::start(AuthScenario::Ok).await;
        let ep = endpoints(&s.uri());
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let client = reqwest::Client::new();

        let manifest = fetch_manifest(&client, &paths, &ep.manifest).await.unwrap();
        assert_eq!(manifest.versions.len(), 4);
        let kind = |id: &str| manifest.find(id).unwrap().kind;
        assert_eq!(kind("9.9"), VersionKind::Release);
        assert_eq!(kind("9.9-snap"), VersionKind::Snapshot);
        assert_eq!(kind("b9.9"), VersionKind::OldBeta);
        assert_eq!(kind("a9.9"), VersionKind::OldAlpha);
        assert_eq!(manifest.latest.release, "9.9");

        let list = list_versions(&client, &ep.loaders, LoaderKind::Fabric, "9.9")
            .await
            .unwrap();
        let ids: Vec<&str> = list.iter().map(|v| v.id.as_str()).collect();
        assert_eq!(ids, ["0.100.0-beta.1", "0.99.0"]);
        assert!(!list[0].stable && list[1].stable);
        let picked = pick_version(&list, None, LoaderKind::Fabric, "9.9").unwrap();
        assert_eq!(picked.id, "0.99.0");

        let none = list_versions(&client, &ep.loaders, LoaderKind::Fabric, "b9.9")
            .await
            .unwrap_err();
        assert!(matches!(none, Error::NoLoaderVersions { .. }), "{none}");
    }

    #[test]
    fn scenario_names_parse() {
        assert_eq!(AuthScenario::from_name("ok"), Some(AuthScenario::Ok));
        assert_eq!(
            AuthScenario::from_name("not-approved"),
            Some(AuthScenario::NotApproved)
        );
        assert_eq!(
            AuthScenario::from_name("slow:250"),
            Some(AuthScenario::Slow(Duration::from_millis(250)))
        );
        assert_eq!(
            AuthScenario::from_name("device-pending:3"),
            Some(AuthScenario::DevicePending(3))
        );
        assert_eq!(
            AuthScenario::from_name("device-slow-down"),
            Some(AuthScenario::DeviceSlowDown)
        );
        assert_eq!(
            AuthScenario::from_name("device-expired"),
            Some(AuthScenario::DeviceExpired)
        );
        assert_eq!(
            AuthScenario::from_name("browser-stalls:6"),
            Some(AuthScenario::BrowserStalls(6))
        );
        assert_eq!(AuthScenario::from_name("browser-stalls:x"), None);
        assert_eq!(AuthScenario::from_name("device-pending:x"), None);
        assert_eq!(AuthScenario::from_name("slow:abc"), None);
        assert_eq!(AuthScenario::from_name("nope"), None);
    }
}
