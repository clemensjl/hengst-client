use hengst_core::auth::flow::{launch_account, needs_refresh, LoginSession};
use hengst_core::auth::store::{AccountStore, MemorySecretStore, SecretKind};
use hengst_core::auth::{AuthEndpoints, AuthError, MicrosoftAuth, MsaTokens, Profile};
use hengst_core::secret::Secret;
use hengst_core::Error;
use serde_json::json;
use wiremock::matchers::{body_string_contains, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const UUID: &str = "069A79F444E94726A5BEFCA90E38AAF5";

fn auth(server: &MockServer) -> MicrosoftAuth {
    let base = server.uri();
    MicrosoftAuth::new(
        "test-id".into(),
        AuthEndpoints {
            msa: format!("{base}/msa"),
            xbox_user: format!("{base}/xuser"),
            xsts: format!("{base}/xsts"),
            minecraft: format!("{base}/mc"),
        },
    )
}

/// Mounts the Xbox and Minecraft services; every one is hit exactly once.
async fn mount_chain(server: &MockServer, entitlements: serde_json::Value, xid: Option<&str>) {
    let mut xui = json!({"uhs": "uhs1"});
    if let Some(xid) = xid {
        xui["xid"] = json!(xid);
    }
    Mock::given(method("POST"))
        .and(path("/xuser/user/authenticate"))
        .and(body_string_contains("d=at-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "Token": "xbl-token",
            "DisplayClaims": {"xui": [{"uhs": "uhs1"}]}
        })))
        .expect(1)
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/xsts/xsts/authorize"))
        .and(body_string_contains("xbl-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "Token": "xsts-token",
            "DisplayClaims": {"xui": [xui]}
        })))
        .expect(1)
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mc/authentication/login_with_xbox"))
        .and(body_string_contains("XBL3.0 x=uhs1;xsts-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "mc-token", "expires_in": 86400
        })))
        .expect(1)
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/mc/entitlements/mcstore"))
        .and(header("authorization", "Bearer mc-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(entitlements))
        .expect(1)
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/mc/minecraft/profile"))
        .and(header("authorization", "Bearer mc-token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id": UUID, "name": "Notch"})),
        )
        .expect(1)
        .mount(server)
        .await;
}

fn token_ok(refresh: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "access_token": "at-1", "refresh_token": refresh, "expires_in": 3600
    }))
}

/// MSA tokens as the refresh endpoint of `server` returns them.
async fn msa_tokens(server: &MockServer) -> MsaTokens {
    Mock::given(method("POST"))
        .and(path("/msa/token"))
        .respond_with(token_ok("rt-1"))
        .mount(server)
        .await;
    auth(server)
        .refresh_msa(&Secret::new("seed"))
        .await
        .unwrap()
}

#[tokio::test]
async fn device_login_runs_the_whole_chain() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/msa/devicecode"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "device_code": "dc-1", "user_code": "ABCD-EFGH",
            "verification_uri": "https://microsoft.com/link",
            "expires_in": 900, "interval": 0
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/msa/token"))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_json(json!({"error": "authorization_pending", "error_description": "x"})),
        )
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/msa/token"))
        .respond_with(token_ok("rt-1"))
        .with_priority(2)
        .mount(&server)
        .await;
    mount_chain(
        &server,
        json!({"items": [{"name": "game_minecraft"}]}),
        Some("2535400000000001"),
    )
    .await;

    let auth = auth(&server);
    let code = auth.start_device_login().await.unwrap();
    let tokens = auth.poll_device_login(&code).await.unwrap();
    let s = auth.complete_login(tokens, 1_000).await.unwrap();
    assert_eq!(s.profile.name, "Notch");
    assert_eq!(s.profile.id, UUID.to_lowercase());
    assert_eq!(s.xuid, "2535400000000001");
    assert_eq!(s.minecraft_expires_at, 1_000 + 86_400);
    assert_eq!(s.refresh_token.expose(), "rt-1");
    let dbg = format!("{s:?}");
    assert!(!dbg.contains("mc-token"));
    assert!(!dbg.contains("rt-1"));
}

#[tokio::test]
async fn game_pass_profile_without_entitlement_is_accepted() {
    let server = MockServer::start().await;
    mount_chain(&server, json!({"items": []}), Some("2535400000000002")).await;
    let tokens = msa_tokens(&server).await;
    let s = auth(&server).complete_login(tokens, 5).await.unwrap();
    assert_eq!(s.profile.name, "Notch");
    assert_eq!(s.xuid, "2535400000000002");
}

#[tokio::test]
async fn missing_xid_falls_back_to_zero() {
    let server = MockServer::start().await;
    mount_chain(&server, json!({"items": []}), None).await;
    let tokens = msa_tokens(&server).await;
    let s = auth(&server).complete_login(tokens, 0).await.unwrap();
    assert_eq!(s.xuid, "0");
}

#[tokio::test]
async fn refresh_session_returns_rotated_refresh_token() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/msa/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains("refresh_token=rt-1"))
        .respond_with(token_ok("rt-2"))
        .expect(1)
        .mount(&server)
        .await;
    mount_chain(&server, json!({"items": []}), None).await;
    let s = auth(&server)
        .refresh_session(&Secret::new("rt-1"), 7)
        .await
        .unwrap();
    assert_eq!(s.refresh_token.expose(), "rt-2");
    assert_eq!(s.minecraft_expires_at, 7 + 86_400);
}

const NOW: u64 = 1_000_000;

/// A store holding one saved login (`Steve`, or `Notch` to match the mock chain).
fn seeded(name: &str, expires_at: u64) -> (tempfile::TempDir, AccountStore<MemorySecretStore>) {
    let dir = tempfile::tempdir().unwrap();
    let store = AccountStore::new(
        dir.path().join("accounts.json"),
        MemorySecretStore::default(),
    );
    store
        .save_session(&LoginSession {
            profile: Profile {
                id: UUID.to_lowercase(),
                name: name.into(),
            },
            xuid: "2535".into(),
            minecraft_token: Secret::new("mc-old"),
            minecraft_expires_at: expires_at,
            refresh_token: Secret::new("rt-1"),
        })
        .unwrap();
    (dir, store)
}

/// Endpoints on a port nobody listens on.
fn closed_port_auth() -> MicrosoftAuth {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let base = format!("http://127.0.0.1:{port}");
    MicrosoftAuth::new(
        "test-id".into(),
        AuthEndpoints {
            msa: format!("{base}/msa"),
            xbox_user: format!("{base}/xuser"),
            xsts: format!("{base}/xsts"),
            minecraft: format!("{base}/mc"),
        },
    )
}

#[test]
fn refresh_is_due_inside_the_one_hour_margin() {
    let (_d, store) = seeded("Steve", NOW + 3_600);
    let acc = store.find(None).unwrap();
    assert!(needs_refresh(&acc, NOW));
    assert!(!needs_refresh(&acc, NOW - 1));
}

#[tokio::test]
async fn fresh_token_launches_without_network() {
    let (_d, store) = seeded("Steve", NOW + 7_200);
    let a = launch_account(&store, None, None, NOW).await.unwrap();
    assert_eq!(a.user_type(), "msa");
    assert_eq!(a.name(), "Steve");
    assert_eq!(a.uuid(), UUID.to_lowercase());
}

#[tokio::test]
async fn near_expiry_without_client_id_is_reported() {
    let (_d, store) = seeded("Steve", NOW + 600);
    let e = launch_account(&store, None, None, NOW).await.unwrap_err();
    assert!(matches!(e, Error::Auth(AuthError::MissingClientId)));
}

#[tokio::test]
async fn near_expiry_refreshes_and_stores_rotated_token() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/msa/token"))
        .and(body_string_contains("refresh_token=rt-1"))
        .respond_with(token_ok("rt-2"))
        .expect(1)
        .mount(&server)
        .await;
    mount_chain(&server, json!({"items": []}), None).await;
    let (_d, store) = seeded("Notch", NOW + 600);

    let a = launch_account(&store, Some(&auth(&server)), None, NOW)
        .await
        .unwrap();
    assert_eq!(a.name(), "Notch");
    let acc = store.find(None).unwrap();
    assert_eq!(acc.minecraft_expires_at, NOW + 86_400);
    assert_eq!(
        store.secret(&acc, SecretKind::Refresh).unwrap().expose(),
        "rt-2"
    );
    assert_eq!(
        store.secret(&acc, SecretKind::Minecraft).unwrap().expose(),
        "mc-token"
    );

    // The rotated token is the one the next refresh uses.
    let second = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/msa/token"))
        .and(body_string_contains("refresh_token=rt-2"))
        .respond_with(token_ok("rt-3"))
        .expect(1)
        .mount(&second)
        .await;
    mount_chain(&second, json!({"items": []}), None).await;
    launch_account(&store, Some(&auth(&second)), None, NOW + 86_400 - 600)
        .await
        .unwrap();
    let acc = store.find(None).unwrap();
    assert_eq!(
        store.secret(&acc, SecretKind::Refresh).unwrap().expose(),
        "rt-3"
    );
}

#[tokio::test]
async fn rejected_refresh_asks_for_login_again() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/msa/token"))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_json(json!({"error": "invalid_grant", "error_description": "x"})),
        )
        .mount(&server)
        .await;
    let (_d, store) = seeded("Steve", NOW + 600);
    let e = launch_account(&store, Some(&auth(&server)), None, NOW)
        .await
        .unwrap_err();
    assert!(matches!(e, Error::Auth(AuthError::LoginExpired { ref name }) if name == "Steve"));
    assert_eq!(store.load().unwrap().accounts.len(), 1);
}

#[tokio::test]
async fn offline_network_near_expiry_uses_still_valid_token() {
    let (_d, store) = seeded("Steve", NOW + 1_800);
    let a = launch_account(&store, Some(&closed_port_auth()), None, NOW)
        .await
        .unwrap();
    assert_eq!(a.name(), "Steve");

    let (_d, store) = seeded("Steve", NOW - 1);
    let e = launch_account(&store, Some(&closed_port_auth()), None, NOW)
        .await
        .unwrap_err();
    assert!(matches!(e, Error::Http { .. }));
}

#[tokio::test]
async fn server_error_near_expiry_uses_still_valid_token() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/msa/token"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    let (_d, store) = seeded("Steve", NOW + 1_800);
    let a = launch_account(&store, Some(&auth(&server)), None, NOW)
        .await
        .unwrap();
    assert_eq!(a.name(), "Steve");
}

#[tokio::test]
async fn rate_limited_refresh_near_expiry_uses_still_valid_token() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/msa/token"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;
    let (_d, store) = seeded("Steve", NOW + 1_800);
    let a = launch_account(&store, Some(&auth(&server)), None, NOW)
        .await
        .unwrap();
    assert_eq!(a.name(), "Steve");
}

#[tokio::test]
async fn auth_error_near_expiry_blocks_the_launch() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/msa/token"))
        .respond_with(token_ok("rt-2"))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/xuser/user/authenticate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "Token": "xbl-token",
            "DisplayClaims": {"xui": [{"uhs": "uhs1"}]}
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/xsts/xsts/authorize"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "Token": "xsts-token",
            "DisplayClaims": {"xui": [{"uhs": "uhs1"}]}
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mc/authentication/login_with_xbox"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
    let (_d, store) = seeded("Steve", NOW + 1_800);
    let e = launch_account(&store, Some(&auth(&server)), None, NOW)
        .await
        .unwrap_err();
    assert!(matches!(e, Error::Auth(AuthError::AppNotApproved)));
}
