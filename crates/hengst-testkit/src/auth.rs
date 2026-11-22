//! Mock of the sign-in chain: Microsoft identity platform (authorization code with PKCE, device
//! code, refresh), Xbox Live, XSTS and the Minecraft services.
//!
//! The mocks follow the protocols closely enough to catch client mistakes. Wrong requests get the
//! error shapes the core's own tests already model; where the real services' answer is not known,
//! the mock answers with a plain 400 (malformed request) or 401 (unknown credential).
//!
//! All tokens are fake and recognizable: they start with `test-`.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hengst_core::auth::AuthEndpoints;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};
use url::Url;
use wiremock::matchers::path;
use wiremock::{Match, Mock, MockServer, Request, Respond, ResponseTemplate};

/// Path of the Microsoft identity platform below the mock's base.
const MSA: &str = "/consumers/oauth2/v2.0";
const XBOX_USER_PATH: &str = "/user/authenticate";
const XSTS_PATH: &str = "/xsts/authorize";
const MC_LOGIN_PATH: &str = "/authentication/login_with_xbox";
const ENTITLEMENTS_PATH: &str = "/entitlements/mcstore";
const PROFILE_PATH: &str = "/minecraft/profile";

const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
const XSTS_RELYING_PARTY: &str = "rp://api.minecraftservices.com/";

/// The user hash (`uhs`) every mock Xbox token carries.
pub const UHS: &str = "test-uhs";

/// `issued_secrets` leaves out values shorter than this.
const MIN_SECRET_LEN: usize = 8;

/// Lifetime of the Minecraft token the mock hands out, as the real service does.
const MINECRAFT_EXPIRES_IN: u64 = 86_400;

/// A fake player. The two fixed accounts [`PLAYER_A`] and [`PLAYER_B`] are the whole documented
/// set of identities the mocks use; nothing here is a real account, token or UUID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MockAccount {
    pub name: &'static str,
    /// 32 lowercase hex digits, unique per account (the mock derives its chain tokens from it).
    pub uuid: &'static str,
    pub minecraft_token: &'static str,
    /// The refresh token a first sign-in returns; also accepted for refreshing from the start.
    pub refresh_token: &'static str,
}

pub const PLAYER_A: MockAccount = MockAccount {
    name: "E2EPlayer",
    uuid: "0123456789abcdef0123456789abcdef",
    minecraft_token: "test-mc-token-a",
    refresh_token: "test-refresh-a",
};

pub const PLAYER_B: MockAccount = MockAccount {
    name: "SecondPlayer",
    uuid: "fedcba9876543210fedcba9876543210",
    minecraft_token: "test-mc-token-b",
    refresh_token: "test-refresh-b",
};

impl MockAccount {
    /// Microsoft access token of this account.
    pub fn msa_access_token(&self) -> String {
        format!("test-msa-access-{}", self.uuid)
    }

    /// Xbox Live user token.
    pub fn xbox_user_token(&self) -> String {
        format!("test-xbl-{}", self.uuid)
    }

    /// XSTS token for the Minecraft services.
    pub fn xsts_token(&self) -> String {
        format!("test-xsts-{}", self.uuid)
    }

    /// Xbox user id. Obviously fake text, not the digits of a real id: the core keeps it as an
    /// opaque string.
    pub fn xuid(&self) -> String {
        format!("test-xuid-{}", &self.uuid[..8])
    }
}

/// What the mocked services do. Switchable at runtime with [`AuthHandle::set_scenario`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthScenario {
    /// Everything works.
    Ok,
    /// Minecraft login answers 403: the Azure app is not approved by Mojang yet.
    NotApproved,
    /// XSTS answers `XErr 2148916233`: the account has no Xbox profile.
    NoXbox,
    /// The user declines: `error=access_denied` at the authorize step, `authorization_declined`
    /// when polling the device code.
    Declined,
    /// The account does not own the game: no entitlements, no profile.
    NoGame,
    /// The account owns the game but has no player profile yet (404).
    NoProfile,
    /// Every refresh grant answers `invalid_grant`.
    RefreshExpired,
    /// Every refresh token works once; using it again answers `invalid_grant`.
    RefreshRotates,
    /// The sign-in services close the connection without an answer. The Mojang and Fabric
    /// mocks are not affected.
    NetworkFailure,
    /// Polling a device code answers `authorization_pending` this many times, then succeeds.
    DevicePending(u32),
    /// The first poll of a device code answers `slow_down`, the next succeeds.
    DeviceSlowDown,
    /// Polling a device code answers `expired_token`.
    DeviceExpired,
    /// Everything works, but every answer of the sign-in services waits this long. The Mojang
    /// and Fabric mocks (`mount_mojang`) are not delayed.
    Slow(Duration),
}

impl AuthScenario {
    /// The names `hengst-mock` reads from `HENGST_MOCK_SCENARIO`: `ok`, `not-approved`,
    /// `no-xbox`, `declined`, `no-game`, `no-profile`, `refresh-expired`, `refresh-rotates`,
    /// `network-failure`, `device-pending:<polls>`, `device-slow-down`, `device-expired` and
    /// `slow:<milliseconds>`.
    pub fn from_name(name: &str) -> Option<AuthScenario> {
        Some(match name {
            "ok" => AuthScenario::Ok,
            "not-approved" => AuthScenario::NotApproved,
            "no-xbox" => AuthScenario::NoXbox,
            "declined" => AuthScenario::Declined,
            "no-game" => AuthScenario::NoGame,
            "no-profile" => AuthScenario::NoProfile,
            "refresh-expired" => AuthScenario::RefreshExpired,
            "refresh-rotates" => AuthScenario::RefreshRotates,
            "network-failure" => AuthScenario::NetworkFailure,
            "device-slow-down" => AuthScenario::DeviceSlowDown,
            "device-expired" => AuthScenario::DeviceExpired,
            other => {
                if let Some(polls) = other.strip_prefix("device-pending:") {
                    AuthScenario::DevicePending(polls.parse().ok()?)
                } else {
                    let ms = other.strip_prefix("slow:")?.parse().ok()?;
                    AuthScenario::Slow(Duration::from_millis(ms))
                }
            }
        })
    }
}

/// A secret value in a recorded request. `Debug` never shows it.
#[derive(Clone, PartialEq, Eq)]
pub struct Redacted(String);

impl Redacted {
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Redacted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Redacted(<redacted>)")
    }
}

/// What the mock keeps of a request, for assertions. Bodies and queries are not kept.
#[derive(Clone, Debug)]
pub struct RecordedRequest {
    pub method: String,
    pub path: String,
    /// `grant_type` of a token request.
    pub grant_type: Option<String>,
    /// Token of the `Authorization: Bearer` header.
    pub bearer: Option<Redacted>,
}

struct CodeGrant {
    account: usize,
    client_id: String,
    redirect_uri: String,
    challenge: String,
    issued: Instant,
    /// Spent by any redemption attempt, successful or not.
    used: bool,
}

struct DeviceGrant {
    account: usize,
    client_id: String,
    issued: Instant,
    polls: u32,
    used: bool,
}

struct State {
    scenario: AuthScenario,
    expected_client_id: Option<String>,
    accounts: Vec<MockAccount>,
    code_lifetime: Duration,
    /// Lifetime of device codes; independent of `code_lifetime`.
    device_lifetime: Duration,
    next_code: usize,
    codes: HashMap<String, CodeGrant>,
    next_device: usize,
    devices: HashMap<String, DeviceGrant>,
    /// Sign-ins completed per account (index into `accounts`).
    sign_ins: HashMap<usize, usize>,
    /// Every refresh token handed out (and the seeds), with its account.
    refresh: HashMap<String, usize>,
    revoked: HashSet<String>,
    rotations: usize,
    /// States and code verifiers the clients sent: secrets the mock only learns from requests.
    seen: BTreeSet<String>,
    log: Vec<RecordedRequest>,
}

struct Shared {
    state: Mutex<State>,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Control and inspection of a mounted sign-in mock. Clones share one state.
#[derive(Clone)]
pub struct AuthHandle {
    shared: Arc<Shared>,
}

impl AuthHandle {
    /// How long authorization codes stay valid (default two minutes). Device codes are not
    /// affected: see [`AuthHandle::set_device_lifetime`].
    pub fn set_code_lifetime(&self, lifetime: Duration) {
        self.shared.lock().code_lifetime = lifetime;
    }

    /// How long device codes stay valid (default 15 minutes), also the `expires_in` the device
    /// code endpoint reports.
    pub fn set_device_lifetime(&self, lifetime: Duration) {
        self.shared.lock().device_lifetime = lifetime;
    }

    /// Every secret value the mock has issued or seen so far, sorted: the players' Minecraft and
    /// refresh tokens, the derived Microsoft, Xbox and XSTS tokens, rotated and later sign-in
    /// refresh tokens, authorization codes, device codes, and the `state` and `code_verifier`
    /// values clients sent. Values shorter than 8 characters are left out. Meant for assertions that none of them leaks into an output; the
    /// values are fake.
    pub fn issued_secrets(&self) -> Vec<String> {
        let st = self.shared.lock();
        let mut all = st.seen.clone();
        for a in &st.accounts {
            all.extend([
                a.minecraft_token.to_string(),
                a.refresh_token.to_string(),
                a.msa_access_token(),
                a.xbox_user_token(),
                a.xsts_token(),
            ]);
        }
        all.extend(st.refresh.keys().cloned());
        all.extend(st.codes.keys().cloned());
        all.extend(st.devices.keys().cloned());
        // A short value (a client's short `state`) would match unrelated text in an assertion.
        all.into_iter()
            .filter(|s| s.len() >= MIN_SECRET_LEN)
            .collect()
    }

    /// Changes the scenario for all following requests.
    pub fn set_scenario(&self, scenario: AuthScenario) {
        self.shared.lock().scenario = scenario;
    }

    /// From now on the authorize, token and device endpoints refuse every other client id
    /// (by default any non-empty id is accepted, and the token step must repeat the one the
    /// authorize step saw).
    pub fn require_client_id(&self, client_id: &str) {
        self.shared.lock().expected_client_id = Some(client_id.to_string());
    }

    /// The requests the handlers saw, oldest first. Connections dropped by
    /// [`AuthScenario::NetworkFailure`] are not recorded.
    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.shared.lock().log.clone()
    }
}

/// Which handler answers a path.
#[derive(Clone, Copy)]
enum Kind {
    Authorize,
    Token,
    DeviceCode,
    XboxUser,
    Xsts,
    MinecraftLogin,
    Entitlements,
    Profile,
}

impl Kind {
    const ALL: [Kind; 8] = [
        Kind::Authorize,
        Kind::Token,
        Kind::DeviceCode,
        Kind::XboxUser,
        Kind::Xsts,
        Kind::MinecraftLogin,
        Kind::Entitlements,
        Kind::Profile,
    ];

    fn path(self) -> String {
        match self {
            Kind::Authorize => format!("{MSA}/authorize"),
            Kind::Token => format!("{MSA}/token"),
            Kind::DeviceCode => format!("{MSA}/devicecode"),
            Kind::XboxUser => XBOX_USER_PATH.into(),
            Kind::Xsts => XSTS_PATH.into(),
            Kind::MinecraftLogin => MC_LOGIN_PATH.into(),
            Kind::Entitlements => ENTITLEMENTS_PATH.into(),
            Kind::Profile => PROFILE_PATH.into(),
        }
    }

    fn method(self) -> &'static str {
        match self {
            Kind::Authorize | Kind::Entitlements | Kind::Profile => "GET",
            _ => "POST",
        }
    }
}

struct Endpoint {
    shared: Arc<Shared>,
    kind: Kind,
}

impl Respond for Endpoint {
    fn respond(&self, req: &Request) -> ResponseTemplate {
        let mut st = self.shared.lock();
        let form = form_of(req);
        st.log.push(RecordedRequest {
            method: req.method.to_string(),
            path: req.url.path().to_string(),
            grant_type: if matches!(self.kind, Kind::Token) {
                form.get("grant_type").cloned()
            } else {
                None
            },
            bearer: bearer_of(req).map(|t| Redacted(t.to_string())),
        });
        let response = if req.method.as_str() != self.kind.method() {
            ResponseTemplate::new(405).insert_header("allow", self.kind.method())
        } else {
            match self.kind {
                Kind::Authorize => authorize(&mut st, req),
                Kind::Token => token(&mut st, req, &form),
                Kind::DeviceCode => device_code(&mut st, &form),
                Kind::XboxUser => xbox_user(&st, req),
                Kind::Xsts => xsts(&st, req),
                Kind::MinecraftLogin => minecraft_login(&st, req),
                Kind::Entitlements => entitlements(&st, req),
                Kind::Profile => profile(&st, req),
            }
        };
        match st.scenario {
            AuthScenario::Slow(delay) => response.set_delay(delay),
            _ => response,
        }
    }
}

/// Matches the sign-in paths while the scenario is [`AuthScenario::NetworkFailure`].
struct NetworkDown {
    shared: Arc<Shared>,
    paths: Vec<String>,
}

impl Match for NetworkDown {
    fn matches(&self, req: &Request) -> bool {
        self.shared.lock().scenario == AuthScenario::NetworkFailure
            && self.paths.iter().any(|p| p == req.url.path())
    }
}

/// Mounts the whole sign-in chain on `server` for `accounts`. Authorization codes and device
/// codes go to `accounts[0]`, `accounts[1]`, ... in the order they are issued, wrapping around.
/// The returned handle switches scenarios and shows the requests seen.
///
/// Endpoints (below the base of [`auth_endpoints`]): `/consumers/oauth2/v2.0/{authorize,token,
/// devicecode}`, `/user/authenticate`, `/xsts/authorize`, `/authentication/login_with_xbox`,
/// `/entitlements/mcstore`, `/minecraft/profile`.
pub async fn mount_auth(
    server: &MockServer,
    scenario: AuthScenario,
    accounts: &[MockAccount],
) -> AuthHandle {
    assert!(
        !accounts.is_empty(),
        "mount_auth needs at least one account"
    );
    let refresh = accounts
        .iter()
        .enumerate()
        .map(|(i, a)| (a.refresh_token.to_string(), i))
        .collect();
    let shared = Arc::new(Shared {
        state: Mutex::new(State {
            scenario,
            expected_client_id: None,
            accounts: accounts.to_vec(),
            code_lifetime: Duration::from_secs(120),
            device_lifetime: Duration::from_secs(900),
            next_code: 1,
            codes: HashMap::new(),
            next_device: 1,
            devices: HashMap::new(),
            sign_ins: HashMap::new(),
            refresh,
            revoked: HashSet::new(),
            rotations: 0,
            seen: BTreeSet::new(),
            log: Vec::new(),
        }),
    });
    let paths: Vec<String> = Kind::ALL.iter().map(|k| k.path()).collect();
    Mock::given(NetworkDown {
        shared: shared.clone(),
        paths,
    })
    .respond_with_err(|_: &Request| {
        std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "test mock: network failure",
        )
    })
    .with_priority(1)
    .mount(server)
    .await;
    for kind in Kind::ALL {
        Mock::given(path(kind.path()))
            .respond_with(Endpoint {
                shared: shared.clone(),
                kind,
            })
            .mount(server)
            .await;
    }
    AuthHandle { shared }
}

/// The endpoints of the sign-in chain on a mock at `base` (`http://127.0.0.1:<port>`).
pub fn auth_endpoints(base: &str) -> AuthEndpoints {
    let base = base.trim_end_matches('/');
    AuthEndpoints {
        msa: format!("{base}{MSA}"),
        xbox_user: base.to_string(),
        xsts: base.to_string(),
        minecraft: base.to_string(),
    }
}

// ---- request helpers ----

fn form_of(req: &Request) -> HashMap<String, String> {
    url::form_urlencoded::parse(&req.body)
        .into_owned()
        .collect()
}

fn header<'a>(req: &'a Request, name: &str) -> Option<&'a str> {
    req.headers.get(name).and_then(|v| v.to_str().ok())
}

fn bearer_of(req: &Request) -> Option<&str> {
    header(req, "authorization")?.strip_prefix("Bearer ")
}

fn json_body(req: &Request) -> Option<Value> {
    serde_json::from_slice(&req.body).ok()
}

fn oauth_error(status: u16, code: &str, what: &str) -> ResponseTemplate {
    ResponseTemplate::new(status).set_body_json(json!({
        "error": code,
        "error_description": format!("test mock: {what}"),
    }))
}

fn bad_request(what: &str) -> ResponseTemplate {
    ResponseTemplate::new(400).set_body_json(json!({"error": format!("test mock: {what}")}))
}

fn unauthorized(what: &str) -> ResponseTemplate {
    ResponseTemplate::new(401).set_body_json(json!({
        "errorType": "UNAUTHORIZED",
        "error": "UNAUTHORIZED",
        "errorMessage": format!("test mock: {what}"),
    }))
}

fn page(status: u16, text: &str) -> ResponseTemplate {
    ResponseTemplate::new(status)
        .insert_header("content-type", "text/html; charset=utf-8")
        .set_body_string(format!(
            "<!doctype html><title>test mock</title><p>{text}</p>"
        ))
}

fn client_id_ok(st: &State, given: &str) -> bool {
    !given.is_empty()
        && st
            .expected_client_id
            .as_deref()
            .is_none_or(|expected| expected == given)
}

fn has_scope(scope: &str) -> bool {
    let words: Vec<&str> = scope.split(' ').collect();
    words.contains(&"XboxLive.signin") && words.contains(&"offline_access")
}

fn account_for(st: &State, idx: usize) -> MockAccount {
    st.accounts[idx % st.accounts.len()]
}

fn tokens_response(st: &State, account: usize, refresh_token: &str) -> ResponseTemplate {
    let a = account_for(st, account);
    ResponseTemplate::new(200).set_body_json(json!({
        "token_type": "Bearer",
        "scope": "XboxLive.signin",
        "expires_in": 3600,
        "access_token": a.msa_access_token(),
        "refresh_token": refresh_token,
    }))
}

// ---- Microsoft identity platform ----

/// The loopback redirect URIs the Azure registration allows: `http://localhost:<port>`, no path.
fn valid_redirect(uri: &str) -> Option<Url> {
    let url = Url::parse(uri).ok()?;
    let ok = url.scheme() == "http"
        && matches!(url.host_str(), Some("localhost" | "127.0.0.1"))
        && url.port().is_some()
        && url.path() == "/"
        && url.query().is_none()
        && url.fragment().is_none()
        && url.username().is_empty()
        && url.password().is_none();
    ok.then_some(url)
}

fn redirect_to(mut back: Url, params: &[(&str, &str)]) -> ResponseTemplate {
    back.query_pairs_mut().extend_pairs(params);
    ResponseTemplate::new(302).insert_header("location", back.as_str())
}

fn authorize(st: &mut State, req: &Request) -> ResponseTemplate {
    let q: HashMap<String, String> = req.url.query_pairs().into_owned().collect();
    let get = |k: &str| q.get(k).map(String::as_str).unwrap_or("");
    let Some(back) = valid_redirect(get("redirect_uri")) else {
        return page(400, "invalid redirect_uri");
    };
    if !client_id_ok(st, get("client_id")) {
        return page(400, "unknown client_id");
    }
    let state = get("state");
    if !state.is_empty() {
        st.seen.insert(state.to_string());
    }
    let challenge = get("code_challenge");
    let mistake = if get("response_type") != "code" {
        Some("response_type must be code")
    } else if get("code_challenge_method") != "S256" {
        Some("code_challenge_method must be S256")
    } else if challenge.len() != 43
        || !challenge
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        Some("code_challenge must be 43 base64url characters")
    } else if !has_scope(get("scope")) {
        Some("scope must contain XboxLive.signin and offline_access")
    } else if state.is_empty() {
        Some("state is required")
    } else {
        None
    };
    if let Some(what) = mistake {
        let mut params = vec![("error", "invalid_request"), ("error_description", what)];
        if !state.is_empty() {
            params.push(("state", state));
        }
        return redirect_to(back, &params);
    }
    if st.scenario == AuthScenario::Declined {
        return redirect_to(
            back,
            &[
                ("error", "access_denied"),
                ("error_description", "test mock: the user declined"),
                ("state", state),
            ],
        );
    }
    let n = st.next_code;
    st.next_code += 1;
    let code = format!("test-code-{n}");
    st.codes.insert(
        code.clone(),
        CodeGrant {
            account: (n - 1) % st.accounts.len(),
            client_id: get("client_id").to_string(),
            redirect_uri: get("redirect_uri").to_string(),
            challenge: challenge.to_string(),
            issued: Instant::now(),
            used: false,
        },
    );
    redirect_to(back, &[("code", &code), ("state", state)])
}

fn token(st: &mut State, req: &Request, form: &HashMap<String, String>) -> ResponseTemplate {
    let is_form = header(req, "content-type")
        .is_some_and(|t| t.starts_with("application/x-www-form-urlencoded"));
    if !is_form {
        return oauth_error(400, "invalid_request", "the body must be a form");
    }
    let get = |k: &str| form.get(k).map(String::as_str).unwrap_or("");
    let Some(grant) = form.get("grant_type") else {
        return oauth_error(400, "invalid_request", "grant_type is missing");
    };
    if get("client_id").is_empty() {
        return oauth_error(400, "invalid_request", "client_id is missing");
    }
    if !client_id_ok(st, get("client_id")) {
        return oauth_error(400, "invalid_client", "unknown client_id");
    }
    match grant.as_str() {
        "authorization_code" => code_grant(st, form),
        "refresh_token" => refresh_grant(st, form),
        DEVICE_GRANT => device_grant(st, form),
        _ => oauth_error(400, "unsupported_grant_type", "grant_type not supported"),
    }
}

/// RFC 7636 section 4.1: 43 to 128 characters of `[A-Za-z0-9-._~]`.
fn valid_verifier(v: &str) -> bool {
    (43..=128).contains(&v.len())
        && v.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~'))
}

/// The refresh token a sign-in of `account` returns. The first sign-in gets the account's
/// documented constant (unless a rotation already used it up); every later one gets a token of
/// its own, so a client that stored the result of an earlier sign-in is never handed a token
/// that was already used or revoked.
fn issue_sign_in_token(st: &mut State, account: usize) -> String {
    let base = account_for(st, account).refresh_token;
    let count = st.sign_ins.entry(account).or_insert(0);
    *count += 1;
    let n = *count;
    let token = if n == 1 && !st.revoked.contains(base) {
        base.to_string()
    } else {
        format!("{base}-signin-{n}")
    };
    st.refresh.insert(token.clone(), account);
    token
}

fn code_grant(st: &mut State, form: &HashMap<String, String>) -> ResponseTemplate {
    let get = |k: &str| form.get(k).map(String::as_str).unwrap_or("");
    let Some(code) = form.get("code").filter(|c| !c.is_empty()) else {
        return oauth_error(400, "invalid_request", "code is missing");
    };
    let verifier = get("code_verifier");
    if !verifier.is_empty() {
        st.seen.insert(verifier.to_string());
    }
    let invalid = |what| oauth_error(400, "invalid_grant", what);
    let lifetime = st.code_lifetime;
    let Some(grant) = st.codes.get_mut(code) else {
        return invalid("unknown code");
    };
    if grant.used {
        return invalid("the code was already redeemed");
    }
    // From here on the code is spent, whatever the outcome (RFC 6749 section 10.5).
    grant.used = true;
    if grant.issued.elapsed() > lifetime {
        return invalid("the code expired");
    }
    if grant.client_id != get("client_id") {
        return invalid("client_id differs from the authorize request");
    }
    if grant.redirect_uri != get("redirect_uri") {
        return invalid("redirect_uri differs from the authorize request");
    }
    if !valid_verifier(verifier) {
        return invalid("code_verifier must be 43 to 128 characters of [A-Za-z0-9-._~]");
    }
    if URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())) != grant.challenge {
        return invalid("code_verifier does not match the code_challenge");
    }
    let account = grant.account;
    let refresh = issue_sign_in_token(st, account);
    tokens_response(st, account, &refresh)
}

fn refresh_grant(st: &mut State, form: &HashMap<String, String>) -> ResponseTemplate {
    let Some(old) = form.get("refresh_token").filter(|t| !t.is_empty()) else {
        return oauth_error(400, "invalid_request", "refresh_token is missing");
    };
    if st.scenario == AuthScenario::RefreshExpired {
        return oauth_error(400, "invalid_grant", "the refresh token expired");
    }
    let Some(&account) = st.refresh.get(old) else {
        return oauth_error(400, "invalid_grant", "unknown refresh token");
    };
    if st.revoked.contains(old) {
        return oauth_error(400, "invalid_grant", "the refresh token was already used");
    }
    if st.scenario == AuthScenario::RefreshRotates {
        st.revoked.insert(old.clone());
    }
    st.rotations += 1;
    let rotated = format!(
        "{}-rotated-{}",
        account_for(st, account).refresh_token,
        st.rotations
    );
    st.refresh.insert(rotated.clone(), account);
    tokens_response(st, account, &rotated)
}

fn device_code(st: &mut State, form: &HashMap<String, String>) -> ResponseTemplate {
    let get = |k: &str| form.get(k).map(String::as_str).unwrap_or("");
    if get("client_id").is_empty() {
        return oauth_error(400, "invalid_request", "client_id is missing");
    }
    if !client_id_ok(st, get("client_id")) {
        return oauth_error(400, "invalid_client", "unknown client_id");
    }
    if !has_scope(get("scope")) {
        return oauth_error(
            400,
            "invalid_request",
            "scope must contain XboxLive.signin and offline_access",
        );
    }
    let n = st.next_device;
    st.next_device += 1;
    let device_code = format!("test-device-{n}");
    st.devices.insert(
        device_code.clone(),
        DeviceGrant {
            account: (n - 1) % st.accounts.len(),
            client_id: get("client_id").to_string(),
            issued: Instant::now(),
            polls: 0,
            used: false,
        },
    );
    ResponseTemplate::new(200).set_body_json(json!({
        "device_code": device_code,
        "user_code": format!("TEST-{n:04}"),
        // The reserved .invalid domain never resolves: nothing can open a real page by accident.
        "verification_uri": "https://example.invalid/link",
        "expires_in": st.device_lifetime.as_secs(),
        "interval": 0,
    }))
}

fn device_grant(st: &mut State, form: &HashMap<String, String>) -> ResponseTemplate {
    let get = |k: &str| form.get(k).map(String::as_str).unwrap_or("");
    let lifetime = st.device_lifetime;
    let scenario = st.scenario;
    let code = get("device_code");
    let Some(grant) = st.devices.get_mut(code) else {
        return oauth_error(400, "invalid_grant", "unknown device_code");
    };
    if grant.client_id != get("client_id") || grant.used {
        return oauth_error(
            400,
            "invalid_grant",
            "the device_code belongs to another client or was already redeemed",
        );
    }
    if grant.issued.elapsed() > lifetime {
        return oauth_error(400, "expired_token", "the device_code expired");
    }
    grant.polls += 1;
    let polls = grant.polls;
    let account = grant.account;
    match scenario {
        AuthScenario::Declined => {
            return oauth_error(400, "authorization_declined", "the user declined")
        }
        AuthScenario::DeviceExpired => {
            return oauth_error(400, "expired_token", "the device_code expired")
        }
        AuthScenario::DevicePending(n) if polls <= n => {
            return oauth_error(
                400,
                "authorization_pending",
                "the user has not finished yet",
            )
        }
        AuthScenario::DeviceSlowDown if polls == 1 => {
            return oauth_error(400, "slow_down", "polling too fast")
        }
        _ => {}
    }
    if let Some(grant) = st.devices.get_mut(code) {
        grant.used = true;
    }
    let refresh = issue_sign_in_token(st, account);
    tokens_response(st, account, &refresh)
}

// ---- Xbox Live and XSTS ----

fn xbox_user(st: &State, req: &Request) -> ResponseTemplate {
    if header(req, "x-xbl-contract-version") != Some("1") {
        return bad_request("x-xbl-contract-version: 1 is missing");
    }
    let Some(body) = json_body(req) else {
        return bad_request("the body must be JSON");
    };
    let text = |pointer: &str| body.pointer(pointer).and_then(Value::as_str).unwrap_or("");
    if text("/Properties/AuthMethod") != "RPS"
        || text("/Properties/SiteName") != "user.auth.xboxlive.com"
        || text("/RelyingParty") != "http://auth.xboxlive.com"
        || text("/TokenType") != "JWT"
    {
        return bad_request("unexpected Properties, RelyingParty or TokenType");
    }
    let Some(ticket) = text("/Properties/RpsTicket").strip_prefix("d=") else {
        return bad_request("RpsTicket must be d=<access token>");
    };
    match st.accounts.iter().find(|a| a.msa_access_token() == ticket) {
        Some(a) => ResponseTemplate::new(200).set_body_json(json!({
            "Token": a.xbox_user_token(),
            "DisplayClaims": {"xui": [{"uhs": UHS}]},
        })),
        None => ResponseTemplate::new(401),
    }
}

fn xsts(st: &State, req: &Request) -> ResponseTemplate {
    let Some(body) = json_body(req) else {
        return bad_request("the body must be JSON");
    };
    let text = |pointer: &str| body.pointer(pointer).and_then(Value::as_str).unwrap_or("");
    let users = body
        .pointer("/Properties/UserTokens")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if text("/Properties/SandboxId") != "RETAIL"
        || text("/RelyingParty") != XSTS_RELYING_PARTY
        || text("/TokenType") != "JWT"
    {
        return bad_request("unexpected SandboxId, RelyingParty or TokenType");
    }
    let [user] = users else {
        return bad_request("UserTokens must hold exactly one token");
    };
    let Some(account) = st
        .accounts
        .iter()
        .find(|a| Some(a.xbox_user_token().as_str()) == user.as_str())
    else {
        return bad_request("unknown user token");
    };
    if st.scenario == AuthScenario::NoXbox {
        return ResponseTemplate::new(401).set_body_json(json!({
            "Identity": "0",
            "XErr": 2148916233u64,
            "Message": "",
            "Redirect": "https://xbox-signup.invalid/CreateAccount",
        }));
    }
    ResponseTemplate::new(200).set_body_json(json!({
        "Token": account.xsts_token(),
        "DisplayClaims": {"xui": [{"uhs": UHS, "xid": account.xuid()}]},
    }))
}

// ---- Minecraft services ----

fn minecraft_login(st: &State, req: &Request) -> ResponseTemplate {
    let identity = json_body(req)
        .as_ref()
        .and_then(|b| b.get("identityToken"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let Some((uhs, xsts)) = identity
        .as_deref()
        .and_then(|t| t.strip_prefix("XBL3.0 x="))
        .and_then(|t| t.split_once(';'))
    else {
        return bad_request("identityToken must be XBL3.0 x=<uhs>;<token>");
    };
    if st.scenario == AuthScenario::NotApproved {
        return ResponseTemplate::new(403).set_body_json(json!({
            "path": MC_LOGIN_PATH,
            "errorMessage": "test mock: Invalid app registration",
        }));
    }
    let account = st.accounts.iter().find(|a| a.xsts_token() == xsts);
    match account {
        Some(a) if uhs == UHS => ResponseTemplate::new(200).set_body_json(json!({
            "username": a.uuid,
            "roles": [],
            "access_token": a.minecraft_token,
            "token_type": "Bearer",
            "expires_in": MINECRAFT_EXPIRES_IN,
        })),
        _ => unauthorized("unknown identity token"),
    }
}

fn bearer_account(st: &State, req: &Request) -> Option<MockAccount> {
    let token = bearer_of(req)?;
    st.accounts
        .iter()
        .find(|a| a.minecraft_token == token)
        .copied()
}

fn entitlements(st: &State, req: &Request) -> ResponseTemplate {
    if bearer_account(st, req).is_none() {
        return unauthorized("missing or unknown bearer token");
    }
    let items = if st.scenario == AuthScenario::NoGame {
        json!([])
    } else {
        json!([{"name": "game_minecraft"}, {"name": "product_minecraft"}])
    };
    ResponseTemplate::new(200).set_body_json(json!({"items": items}))
}

fn profile(st: &State, req: &Request) -> ResponseTemplate {
    let Some(account) = bearer_account(st, req) else {
        return unauthorized("missing or unknown bearer token");
    };
    if matches!(st.scenario, AuthScenario::NoGame | AuthScenario::NoProfile) {
        return ResponseTemplate::new(404).set_body_json(json!({
            "path": PROFILE_PATH,
            "errorType": "NOT_FOUND",
            "error": "NOT_FOUND",
        }));
    }
    ResponseTemplate::new(200).set_body_json(json!({
        "id": account.uuid,
        "name": account.name,
        "skins": [],
        "capes": [],
    }))
}
