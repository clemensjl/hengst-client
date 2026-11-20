use hengst_core::auth::msa::MsaTokens;
use hengst_core::auth::{AuthEndpoints, MicrosoftAuth};
use hengst_core::secret::Secret;
use serde_json::json;
use wiremock::matchers::{body_string_contains, method, path};
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
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "Token": "xbl-token",
            "DisplayClaims": {"xui": [{"uhs": "uhs1"}]}
        })))
        .expect(1)
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/xsts/xsts/authorize"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "Token": "xsts-token",
            "DisplayClaims": {"xui": [xui]}
        })))
        .expect(1)
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mc/authentication/login_with_xbox"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "mc-token", "expires_in": 86400
        })))
        .expect(1)
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/mc/entitlements/mcstore"))
        .respond_with(ResponseTemplate::new(200).set_body_json(entitlements))
        .expect(1)
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/mc/minecraft/profile"))
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
    let a = s.to_account();
    assert_eq!(a.user_type(), "msa");
    assert_eq!(a.name(), "Notch");
    assert_eq!(a.uuid(), UUID.to_lowercase());
    let dbg = format!("{s:?}");
    assert!(!dbg.contains("mc-token"));
    assert!(!dbg.contains("rt-1"));
}

#[tokio::test]
async fn game_pass_profile_without_entitlement_is_accepted() {
    let server = MockServer::start().await;
    mount_chain(&server, json!({"items": []}), None).await;
    let tokens = msa_tokens(&server).await;
    let s = auth(&server).complete_login(tokens, 5).await.unwrap();
    assert_eq!(s.profile.name, "Notch");
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
