//! Each mock refuses a wrong request and accepts a right one; each scenario switch does what it
//! says. Wrong requests are sent by hand, scenarios are driven through the core's own client.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hengst_core::auth::{AuthError, MicrosoftAuth};
use hengst_core::secret::Secret;
use hengst_core::Error;
use hengst_testkit::auth::UHS;
use hengst_testkit::{auth_endpoints, AuthScenario, Services, PLAYER_A, PLAYER_B};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::{Duration, Instant};

const MSA: &str = "/consumers/oauth2/v2.0";
const REDIRECT: &str = "http://localhost:54321";
const VERIFIER: &str = "test-verifier-0123456789-0123456789-0123456789-abcd";
const SCOPE: &str = "XboxLive.signin offline_access";
const RELYING_PARTY: &str = "rp://api.minecraftservices.com/";

fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn http() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap()
}

struct Reply {
    status: u16,
    location: Option<String>,
    body: Value,
}

impl Reply {
    fn error(&self) -> &str {
        self.body["error"].as_str().unwrap_or("")
    }

    /// The query of the `Location` header.
    fn redirect_query(&self) -> HashMap<String, String> {
        reqwest::Url::parse(self.location.as_deref().expect("a Location header"))
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect()
    }
}

async fn reply(r: reqwest::Response) -> Reply {
    let status = r.status().as_u16();
    let location = r
        .headers()
        .get("location")
        .map(|v| v.to_str().unwrap().to_string());
    let text = r.text().await.unwrap();
    Reply {
        status,
        location,
        body: serde_json::from_str(&text).unwrap_or(Value::Null),
    }
}

fn auth(s: &Services) -> MicrosoftAuth {
    MicrosoftAuth::new("e2e-client".into(), auth_endpoints(&s.uri()))
}

// ---- authorize ----

fn good_authorize(challenge: &str) -> Vec<(String, String)> {
    [
        ("client_id", "e2e-client"),
        ("response_type", "code"),
        ("redirect_uri", REDIRECT),
        ("response_mode", "query"),
        ("scope", SCOPE),
        ("code_challenge", challenge),
        ("code_challenge_method", "S256"),
        ("state", "test-state"),
    ]
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .to_vec()
}

/// `params` with `key` replaced by `value`, or removed when `value` is `None`.
fn changed(params: &[(String, String)], key: &str, value: Option<&str>) -> Vec<(String, String)> {
    let mut p: Vec<_> = params.iter().filter(|(k, _)| k != key).cloned().collect();
    if let Some(v) = value {
        p.push((key.to_string(), v.to_string()));
    }
    p
}

async fn authorize(s: &Services, params: &[(String, String)]) -> Reply {
    let url =
        reqwest::Url::parse_with_params(&format!("{}{MSA}/authorize", s.uri()), params).unwrap();
    reply(http().get(url).send().await.unwrap()).await
}

/// Runs the authorize step as a good client would and returns the code.
async fn code(s: &Services) -> String {
    let r = authorize(s, &good_authorize(&challenge(VERIFIER))).await;
    assert_eq!(r.status, 302);
    r.redirect_query()["code"].clone()
}

#[tokio::test]
async fn authorize_redirects_back_with_code_and_state() {
    let s = Services::start(AuthScenario::Ok).await;
    let r = authorize(&s, &good_authorize(&challenge(VERIFIER))).await;
    assert_eq!(r.status, 302);
    let loc = r.location.clone().unwrap();
    assert!(loc.starts_with("http://localhost:54321/?"), "{loc}");
    let q = r.redirect_query();
    assert!(q["code"].starts_with("test-code-"));
    assert_eq!(q["state"], "test-state");
}

#[tokio::test]
async fn authorize_refuses_an_unusable_redirect_uri_with_a_page_not_a_redirect() {
    let s = Services::start(AuthScenario::Ok).await;
    let good = good_authorize(&challenge(VERIFIER));
    for bad in [
        "https://localhost:1",
        "http://evil.invalid:1",
        "http://localhost",
        "http://localhost:1/callback",
        "http://localhost:1/?x=1",
    ] {
        let r = authorize(&s, &changed(&good, "redirect_uri", Some(bad))).await;
        assert_eq!(r.status, 400, "{bad}");
        assert!(r.location.is_none(), "{bad}");
    }
    let r = authorize(&s, &changed(&good, "redirect_uri", None)).await;
    assert_eq!(r.status, 400);
    let r = authorize(&s, &changed(&good, "client_id", None)).await;
    assert_eq!(r.status, 400);
}

#[tokio::test]
async fn authorize_answers_other_mistakes_with_invalid_request_to_the_client() {
    let s = Services::start(AuthScenario::Ok).await;
    let good = good_authorize(&challenge(VERIFIER));
    let mistakes = [
        ("response_type", Some("token")),
        ("code_challenge_method", Some("plain")),
        ("code_challenge_method", None),
        ("code_challenge", Some("short")),
        ("code_challenge", None),
        ("scope", Some("openid")),
        ("scope", None),
        ("state", None),
    ];
    for (key, value) in mistakes {
        let r = authorize(&s, &changed(&good, key, value)).await;
        assert_eq!(r.status, 302, "{key}={value:?}");
        let q = r.redirect_query();
        assert_eq!(q["error"], "invalid_request", "{key}={value:?}");
        assert!(!q.contains_key("code"), "{key}={value:?}");
    }
}

#[tokio::test]
async fn the_expected_client_id_can_be_pinned() {
    let s = Services::start(AuthScenario::Ok).await;
    s.auth.require_client_id("e2e-client");
    let good = good_authorize(&challenge(VERIFIER));
    let r = authorize(&s, &changed(&good, "client_id", Some("someone-else"))).await;
    assert_eq!(r.status, 400);
    assert_eq!(authorize(&s, &good).await.status, 302);
    let r = token(
        &s,
        &[
            ("grant_type", "refresh_token"),
            ("client_id", "someone-else"),
            ("refresh_token", PLAYER_A.refresh_token),
        ],
    )
    .await;
    assert_eq!((r.status, r.error()), (400, "invalid_client"));
}

// ---- token ----

async fn token(s: &Services, form: &[(&str, &str)]) -> Reply {
    reply(
        http()
            .post(format!("{}{MSA}/token", s.uri()))
            .form(form)
            .send()
            .await
            .unwrap(),
    )
    .await
}

fn code_grant<'a>(code: &'a str, verifier: &'a str, redirect: &'a str) -> Vec<(&'a str, &'a str)> {
    vec![
        ("grant_type", "authorization_code"),
        ("client_id", "e2e-client"),
        ("code", code),
        ("redirect_uri", redirect),
        ("code_verifier", verifier),
        ("scope", SCOPE),
    ]
}

#[tokio::test]
async fn token_endpoint_checks_the_grant_type() {
    let s = Services::start(AuthScenario::Ok).await;
    let r = token(&s, &[("client_id", "e2e-client")]).await;
    assert_eq!((r.status, r.error()), (400, "invalid_request"));
    let r = token(
        &s,
        &[("grant_type", "password"), ("client_id", "e2e-client")],
    )
    .await;
    assert_eq!((r.status, r.error()), (400, "unsupported_grant_type"));
}

#[tokio::test]
async fn token_endpoint_needs_a_form_body() {
    let s = Services::start(AuthScenario::Ok).await;
    let r = reply(
        http()
            .post(format!("{}{MSA}/token", s.uri()))
            .json(&json!({"grant_type": "refresh_token"}))
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!((r.status, r.error()), (400, "invalid_request"));
}

#[tokio::test]
async fn code_grant_checks_client_pkce_and_redirect_and_the_code_is_single_use() {
    let s = Services::start(AuthScenario::Ok).await;
    let c = code(&s).await;

    // No client id; another client id than the authorize step saw.
    let mut no_client = code_grant(&c, VERIFIER, REDIRECT);
    no_client.retain(|(k, _)| *k != "client_id");
    let r = token(&s, &no_client).await;
    assert_eq!((r.status, r.error()), (400, "invalid_request"));
    let mut other = code_grant(&c, VERIFIER, REDIRECT);
    other.retain(|(k, _)| *k != "client_id");
    other.push(("client_id", "some-other-client"));
    let r = token(&s, &other).await;
    assert_eq!((r.status, r.error()), (400, "invalid_grant"));

    // PKCE: the verifier must hash to the challenge the authorize step saw; none at all fails.
    let wrong = "test-wrong-verifier-0123456789-0123456789-abcdef";
    let r = token(&s, &code_grant(&c, wrong, REDIRECT)).await;
    assert_eq!((r.status, r.error()), (400, "invalid_grant"));
    let mut no_verifier = code_grant(&c, VERIFIER, REDIRECT);
    no_verifier.retain(|(k, _)| *k != "code_verifier");
    let r = token(&s, &no_verifier).await;
    assert_eq!((r.status, r.error()), (400, "invalid_grant"));

    // The redirect URI must equal the one of the authorize step.
    let r = token(&s, &code_grant(&c, VERIFIER, "http://localhost:54322")).await;
    assert_eq!((r.status, r.error()), (400, "invalid_grant"));

    // Unknown code.
    let r = token(&s, &code_grant("test-code-unknown", VERIFIER, REDIRECT)).await;
    assert_eq!((r.status, r.error()), (400, "invalid_grant"));

    // The right request works (the failed ones above did not burn the code), once.
    let ok = token(&s, &code_grant(&c, VERIFIER, REDIRECT)).await;
    assert_eq!(ok.status, 200);
    assert_eq!(ok.body["refresh_token"], PLAYER_A.refresh_token);
    assert_eq!(ok.body["access_token"], PLAYER_A.msa_access_token());
    assert_eq!(ok.body["token_type"], "Bearer");
    let again = token(&s, &code_grant(&c, VERIFIER, REDIRECT)).await;
    assert_eq!((again.status, again.error()), (400, "invalid_grant"));
}

#[tokio::test]
async fn codes_go_to_the_accounts_in_order() {
    let s = Services::start(AuthScenario::Ok).await;
    let a = token(&s, &code_grant(&code(&s).await, VERIFIER, REDIRECT)).await;
    let b = token(&s, &code_grant(&code(&s).await, VERIFIER, REDIRECT)).await;
    assert_eq!(a.body["refresh_token"], PLAYER_A.refresh_token);
    assert_eq!(b.body["refresh_token"], PLAYER_B.refresh_token);
}

#[tokio::test]
async fn refresh_grant_rotates_and_refuses_unknown_tokens() {
    let s = Services::start(AuthScenario::Ok).await;
    let refresh = |t: &'static str| {
        let s = &s;
        async move {
            token(
                s,
                &[
                    ("grant_type", "refresh_token"),
                    ("client_id", "e2e-client"),
                    ("refresh_token", t),
                    ("scope", SCOPE),
                ],
            )
            .await
        }
    };
    let ok = refresh(PLAYER_A.refresh_token).await;
    assert_eq!(ok.status, 200);
    let rotated = ok.body["refresh_token"].as_str().unwrap().to_string();
    assert_ne!(rotated, PLAYER_A.refresh_token);
    assert!(rotated.starts_with("test-"));
    assert_eq!(ok.body["access_token"], PLAYER_A.msa_access_token());
    // Outside the rotating scenario the old token stays good, and the new one belongs to the
    // same account.
    assert_eq!(refresh(PLAYER_A.refresh_token).await.status, 200);
    let next = token(
        &s,
        &[
            ("grant_type", "refresh_token"),
            ("client_id", "e2e-client"),
            ("refresh_token", &rotated),
        ],
    )
    .await;
    assert_eq!(next.status, 200);
    assert_eq!(next.body["access_token"], PLAYER_A.msa_access_token());

    let unknown = refresh("test-refresh-unknown").await;
    assert_eq!((unknown.status, unknown.error()), (400, "invalid_grant"));
    let none = token(
        &s,
        &[("grant_type", "refresh_token"), ("client_id", "e2e-client")],
    )
    .await;
    assert_eq!((none.status, none.error()), (400, "invalid_request"));
}

#[tokio::test]
async fn expired_refresh_scenario_rejects_every_refresh_until_switched_back() {
    let s = Services::start(AuthScenario::Ok).await;
    let a = auth(&s);
    let seed = Secret::new(PLAYER_B.refresh_token);
    assert!(a.refresh_msa(&seed).await.is_ok());
    s.auth.set_scenario(AuthScenario::RefreshExpired);
    let err = a.refresh_msa(&seed).await.unwrap_err();
    assert!(
        matches!(err, Error::Auth(AuthError::LoginExpired { .. })),
        "{err}"
    );
    // Other grants are untouched by it.
    assert!(a.start_device_login().await.is_ok());
    s.auth.set_scenario(AuthScenario::Ok);
    assert!(a.refresh_msa(&seed).await.is_ok());
}

#[tokio::test]
async fn rotating_scenario_accepts_each_refresh_token_once() {
    let s = Services::start(AuthScenario::RefreshRotates).await;
    let a = auth(&s);
    let seed = Secret::new(PLAYER_A.refresh_token);
    let first = a.refresh_msa(&seed).await.unwrap();
    let again = a.refresh_msa(&seed).await.unwrap_err();
    assert!(
        matches!(again, Error::Auth(AuthError::LoginExpired { .. })),
        "{again}"
    );
    // The new token is good once as well.
    assert!(a.refresh_msa(&first.refresh_token).await.is_ok());
    assert!(a.refresh_msa(&first.refresh_token).await.is_err());
}

#[tokio::test]
async fn device_endpoints_check_client_scope_and_code() {
    let s = Services::start(AuthScenario::Ok).await;
    let post_form = |path: &'static str, form: Vec<(&'static str, &'static str)>| {
        let url = format!("{}{MSA}/{path}", s.uri());
        async move { reply(http().post(url).form(&form).send().await.unwrap()).await }
    };
    let r = post_form("devicecode", vec![("scope", SCOPE)]).await;
    assert_eq!((r.status, r.error()), (400, "invalid_request"));
    let r = post_form(
        "devicecode",
        vec![("client_id", "e2e-client"), ("scope", "openid")],
    )
    .await;
    assert_eq!((r.status, r.error()), (400, "invalid_request"));
    let ok = post_form(
        "devicecode",
        vec![("client_id", "e2e-client"), ("scope", SCOPE)],
    )
    .await;
    assert_eq!(ok.status, 200);
    assert!(ok.body["verification_uri"]
        .as_str()
        .unwrap()
        .contains(".invalid"));
    assert_eq!(ok.body["interval"], 0);
    let device_code = ok.body["device_code"].as_str().unwrap().to_string();

    let grant = "urn:ietf:params:oauth:grant-type:device_code";
    let poll = |code: String| {
        let s = &s;
        async move {
            token(
                s,
                &[
                    ("grant_type", grant),
                    ("client_id", "e2e-client"),
                    ("device_code", &code),
                ],
            )
            .await
        }
    };
    let r = poll("test-device-unknown".into()).await;
    assert_eq!((r.status, r.error()), (400, "invalid_grant"));
    let r = poll(device_code).await;
    assert_eq!(r.status, 200);
    assert_eq!(r.body["refresh_token"], PLAYER_A.refresh_token);
}

// ---- Xbox, XSTS, Minecraft ----

async fn post_json(url: String, headers: &[(&str, &str)], body: Value) -> Reply {
    let mut req = http().post(url).json(&body);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    reply(req.send().await.unwrap()).await
}

async fn get_bearer(url: String, bearer: Option<&str>) -> Reply {
    let mut req = http().get(url);
    if let Some(b) = bearer {
        req = req.header("authorization", format!("Bearer {b}"));
    }
    reply(req.send().await.unwrap()).await
}

fn xbox_body(ticket: &str) -> Value {
    json!({
        "Properties": {"AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": ticket},
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT"
    })
}

fn xsts_body(user_token: &str, relying_party: &str) -> Value {
    json!({
        "Properties": {"SandboxId": "RETAIL", "UserTokens": [user_token]},
        "RelyingParty": relying_party,
        "TokenType": "JWT"
    })
}

const XBL_HEADERS: &[(&str, &str)] = &[("x-xbl-contract-version", "1")];

#[tokio::test]
async fn xbox_user_authenticate_checks_body_header_and_ticket() {
    let s = Services::start(AuthScenario::Ok).await;
    let url = format!("{}/user/authenticate", s.uri());
    let ticket = format!("d={}", PLAYER_A.msa_access_token());
    let post = |headers: &'static [(&'static str, &'static str)], body: Value| {
        post_json(url.clone(), headers, body)
    };

    let ok = post(XBL_HEADERS, xbox_body(&ticket)).await;
    assert_eq!(ok.status, 200);
    assert_eq!(ok.body["Token"], PLAYER_A.xbox_user_token());
    assert_eq!(ok.body["DisplayClaims"]["xui"][0]["uhs"], UHS);

    assert_eq!(post(&[], xbox_body(&ticket)).await.status, 400);
    // The ticket needs the `d=` prefix; an unknown ticket is unauthorized.
    assert_eq!(
        post(XBL_HEADERS, xbox_body(&PLAYER_A.msa_access_token()))
            .await
            .status,
        400
    );
    assert_eq!(
        post(XBL_HEADERS, xbox_body("d=test-msa-access-unknown"))
            .await
            .status,
        401
    );
    for (pointer, value) in [
        ("/Properties/SiteName", "elsewhere"),
        ("/Properties/AuthMethod", "password"),
        ("/RelyingParty", RELYING_PARTY),
        ("/TokenType", "JWE"),
    ] {
        let mut body = xbox_body(&ticket);
        *body.pointer_mut(pointer).unwrap() = json!(value);
        assert_eq!(post(XBL_HEADERS, body).await.status, 400, "{pointer}");
    }
}

#[tokio::test]
async fn xsts_authorize_checks_relying_party_sandbox_and_user_token() {
    let s = Services::start(AuthScenario::Ok).await;
    let url = format!("{}/xsts/authorize", s.uri());
    let user = PLAYER_B.xbox_user_token();

    let ok = post_json(url.clone(), &[], xsts_body(&user, RELYING_PARTY)).await;
    assert_eq!(ok.status, 200);
    assert_eq!(ok.body["Token"], PLAYER_B.xsts_token());
    assert_eq!(ok.body["DisplayClaims"]["xui"][0]["uhs"], UHS);
    assert_eq!(ok.body["DisplayClaims"]["xui"][0]["xid"], PLAYER_B.xuid());

    let wrong_rp = xsts_body(&user, "http://auth.xboxlive.com");
    assert_eq!(post_json(url.clone(), &[], wrong_rp).await.status, 400);
    let unknown = xsts_body("test-xbl-unknown", RELYING_PARTY);
    assert_eq!(post_json(url.clone(), &[], unknown).await.status, 400);
    let mut sandbox = xsts_body(&user, RELYING_PARTY);
    sandbox["Properties"]["SandboxId"] = json!("XDKS.1");
    assert_eq!(post_json(url.clone(), &[], sandbox).await.status, 400);
    let mut two = xsts_body(&user, RELYING_PARTY);
    two["Properties"]["UserTokens"] = json!([PLAYER_A.xbox_user_token(), user]);
    assert_eq!(post_json(url, &[], two).await.status, 400);
}

fn identity(uhs: &str, xsts: &str) -> Value {
    json!({"identityToken": format!("XBL3.0 x={uhs};{xsts}")})
}

#[tokio::test]
async fn minecraft_login_checks_the_identity_token_format() {
    let s = Services::start(AuthScenario::Ok).await;
    let login = format!("{}/authentication/login_with_xbox", s.uri());
    let xsts = PLAYER_A.xsts_token();

    let ok = post_json(login.clone(), &[], identity(UHS, &xsts)).await;
    assert_eq!(ok.status, 200);
    assert_eq!(ok.body["access_token"], PLAYER_A.minecraft_token);
    assert_eq!(ok.body["expires_in"], 86_400);

    let bad_shapes = [
        json!({"identityToken": format!("XBL3.0 {UHS};{xsts}")}),
        json!({"identityToken": format!("XBL2.0 x={UHS};{xsts}")}),
        json!({"identityToken": format!("XBL3.0 x={UHS}")}),
        json!({"identityToken": xsts}),
        json!({"token": format!("XBL3.0 x={UHS};{xsts}")}),
        json!({}),
    ];
    for body in bad_shapes {
        assert_eq!(
            post_json(login.clone(), &[], body.clone()).await.status,
            400,
            "{body}"
        );
    }
    let other_uhs = identity("test-other-uhs", &xsts);
    assert_eq!(post_json(login.clone(), &[], other_uhs).await.status, 401);
    let unknown = identity(UHS, "test-xsts-unknown");
    assert_eq!(post_json(login, &[], unknown).await.status, 401);
}

#[tokio::test]
async fn entitlement_and_profile_need_the_bearer_token() {
    let s = Services::start(AuthScenario::Ok).await;
    for path in ["/entitlements/mcstore", "/minecraft/profile"] {
        let url = format!("{}{path}", s.uri());
        assert_eq!(get_bearer(url.clone(), None).await.status, 401, "{path}");
        let unknown = get_bearer(url.clone(), Some("test-mc-token-unknown")).await;
        assert_eq!(unknown.status, 401, "{path}");
        let wrong_scheme = http()
            .get(&url)
            .header(
                "authorization",
                format!("Token {}", PLAYER_B.minecraft_token),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(wrong_scheme.status().as_u16(), 401, "{path}");
        let r = get_bearer(url, Some(PLAYER_B.minecraft_token)).await;
        assert_eq!(r.status, 200, "{path}");
        if path.ends_with("profile") {
            assert_eq!(r.body["name"], "SecondPlayer");
            assert_eq!(r.body["id"], PLAYER_B.uuid);
        } else {
            assert_eq!(r.body["items"][0]["name"], "game_minecraft");
        }
    }
}

#[tokio::test]
async fn wrong_methods_are_refused() {
    let s = Services::start(AuthScenario::Ok).await;
    for (method, path) in [
        ("GET", format!("{MSA}/token")),
        ("POST", format!("{MSA}/authorize")),
        ("GET", "/user/authenticate".to_string()),
        ("GET", "/xsts/authorize".to_string()),
        ("GET", "/authentication/login_with_xbox".to_string()),
        ("POST", "/entitlements/mcstore".to_string()),
        ("POST", "/minecraft/profile".to_string()),
    ] {
        let r = http()
            .request(method.parse().unwrap(), format!("{}{path}", s.uri()))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status().as_u16(), 405, "{method} {path}");
    }
}

// ---- scenarios, driven through the core's client ----

async fn sign_in_error(scenario: AuthScenario) -> Error {
    let s = Services::start(scenario).await;
    let a = auth(&s);
    let code = a.start_device_login().await.unwrap();
    let tokens = a.poll_device_login(&code).await.unwrap();
    a.complete_login(tokens, 1).await.unwrap_err()
}

#[tokio::test]
async fn account_scenarios_map_to_the_core_errors() {
    let e = sign_in_error(AuthScenario::NoXbox).await;
    assert!(matches!(e, Error::Auth(AuthError::NoXboxAccount)), "{e}");
    let e = sign_in_error(AuthScenario::NotApproved).await;
    assert!(matches!(e, Error::Auth(AuthError::AppNotApproved)), "{e}");
    let e = sign_in_error(AuthScenario::NoGame).await;
    assert!(matches!(e, Error::Auth(AuthError::GameNotOwned)), "{e}");
    let e = sign_in_error(AuthScenario::NoProfile).await;
    assert!(matches!(e, Error::Auth(AuthError::NoProfile)), "{e}");
}

#[tokio::test]
async fn declined_scenario_ends_both_flows_as_declined() {
    let s = Services::start(AuthScenario::Declined).await;
    let a = auth(&s);
    let code = a.start_device_login().await.unwrap();
    let e = a.poll_device_login(&code).await.unwrap_err();
    assert!(matches!(e, Error::Auth(AuthError::LoginDeclined)), "{e}");

    let r = authorize(&s, &good_authorize(&challenge(VERIFIER))).await;
    assert_eq!(r.status, 302);
    let q = r.redirect_query();
    assert_eq!(q["error"], "access_denied");
    assert_eq!(q["state"], "test-state");
    assert!(!q.contains_key("code"));
}

#[tokio::test]
async fn network_failure_scenario_drops_connections_and_recovers() {
    let s = Services::start(AuthScenario::NetworkFailure).await;
    let a = auth(&s);
    let e = a.start_device_login().await.unwrap_err();
    assert!(matches!(e, Error::Http { .. }), "{e}");
    // Only the sign-in services are affected.
    let manifest = reqwest::get(format!("{}/mojang/manifest.json", s.uri()))
        .await
        .unwrap();
    assert_eq!(manifest.status(), 200);
    s.auth.set_scenario(AuthScenario::Ok);
    assert!(a.start_device_login().await.is_ok());
}

#[tokio::test]
async fn slow_scenario_delays_every_answer() {
    let s = Services::start(AuthScenario::Slow(Duration::from_millis(400))).await;
    let a = auth(&s);
    let start = Instant::now();
    a.start_device_login().await.unwrap();
    assert!(
        start.elapsed() >= Duration::from_millis(400),
        "{:?}",
        start.elapsed()
    );
}

// ---- recording, isolation ----

#[tokio::test]
async fn recorded_requests_are_inspectable_but_never_print_tokens() {
    let s = Services::start(AuthScenario::Ok).await;
    get_bearer(
        format!("{}/minecraft/profile", s.uri()),
        Some(PLAYER_A.minecraft_token),
    )
    .await;
    token(
        &s,
        &[
            ("grant_type", "refresh_token"),
            ("client_id", "e2e-client"),
            ("refresh_token", PLAYER_A.refresh_token),
        ],
    )
    .await;
    let log = s.auth.requests();
    assert_eq!(log.len(), 2);
    assert_eq!(
        (log[0].method.as_str(), log[0].path.as_str()),
        ("GET", "/minecraft/profile")
    );
    assert_eq!(
        log[0].bearer.as_ref().unwrap().expose(),
        PLAYER_A.minecraft_token
    );
    assert_eq!(log[1].grant_type.as_deref(), Some("refresh_token"));
    let printed = format!("{log:?}");
    assert!(!printed.contains(PLAYER_A.minecraft_token), "{printed}");
    assert!(!printed.contains(PLAYER_A.refresh_token), "{printed}");
}

#[tokio::test]
async fn services_are_independent_and_listen_on_loopback_only() {
    let a = Services::start(AuthScenario::Ok).await;
    let b = Services::start(AuthScenario::NoXbox).await;
    assert_ne!(a.uri(), b.uri());
    for s in [&a, &b] {
        assert!(s.uri().starts_with("http://127.0.0.1:"), "{}", s.uri());
    }
    // Codes and requests are per instance.
    assert_eq!(code(&a).await, "test-code-1");
    assert_eq!(code(&b).await, "test-code-1");
    assert_eq!(a.auth.requests().len(), 1);
}

#[tokio::test]
async fn the_server_is_gone_when_its_handle_is_dropped() {
    let s = Services::start(AuthScenario::Ok).await;
    let uri = s.uri();
    assert!(reqwest::get(format!("{uri}/mojang/manifest.json"))
        .await
        .is_ok());
    drop(s);
    // The server stops on its own thread; give it a moment.
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        if reqwest::get(format!("{uri}/mojang/manifest.json"))
            .await
            .is_err()
        {
            break;
        }
        assert!(
            Instant::now() < end,
            "the server still answers after its drop"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
