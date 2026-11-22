use super::*;
use hengst_core::auth::minecraft::Profile;
use hengst_core::auth::store::{secret_key, MemorySecretStore, SecretKind};
use hengst_testkit::{
    auth_endpoints, browser_visit, AuthScenario, MockAccount, Services, PLAYER_A, PLAYER_B,
};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// In-memory secrets that can be told to refuse.
#[derive(Default)]
struct TestStore {
    inner: MemorySecretStore,
    refuse_set: AtomicBool,
    deletes: AtomicUsize,
    /// From this delete call on (1 = the first) every delete is refused; 0 = never.
    refuse_delete_from: AtomicUsize,
}

struct Shared(Arc<TestStore>);

impl SecretStore for Shared {
    fn get(&self, key: &str) -> Result<Option<Secret>> {
        self.0.inner.get(key)
    }

    fn set(&self, key: &str, value: &Secret) -> Result<()> {
        if self.0.refuse_set.load(Ordering::SeqCst) {
            return Err(Error::CredentialStore("refused by the test".into()));
        }
        self.0.inner.set(key, value)
    }

    fn delete(&self, key: &str) -> Result<()> {
        let n = self.0.deletes.fetch_add(1, Ordering::SeqCst) + 1;
        let from = self.0.refuse_delete_from.load(Ordering::SeqCst);
        if from != 0 && n >= from {
            return Err(Error::CredentialStore("refused by the test".into()));
        }
        self.0.inner.delete(key)
    }
}

impl TestStore {
    fn has(&self, uuid: &str, kind: SecretKind) -> bool {
        self.inner.get(&secret_key(uuid, kind)).unwrap().is_some()
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Browser {
    /// The kit plays the user: it opens the address and follows the redirect.
    Visits,
    /// Nobody answers.
    Silent,
}

type Events = Arc<Mutex<Vec<LoginEvent>>>;

struct Harness {
    _dir: tempfile::TempDir,
    paths: Paths,
    services: Services,
    accounts: Arc<Accounts>,
    ops: Operations,
    settings: SettingsHandle,
    hub: EventHub,
    store: Arc<TestStore>,
    opened: Arc<Mutex<Vec<String>>>,
    open_refused: Arc<AtomicBool>,
    log: Arc<Mutex<Vec<String>>>,
    app_events: Arc<Mutex<Vec<AppEvent>>>,
}

fn pages() -> BrowserLoginPages {
    BrowserLoginPages {
        success_title: "Done".into(),
        success_body: "You can return to the launcher.".into(),
        failure_title: "Not done".into(),
        failure_body: "Go back to the launcher.".into(),
    }
}

fn is_terminal(e: &LoginEvent) -> bool {
    matches!(
        e,
        LoginEvent::SignedIn { .. } | LoginEvent::Failed { .. } | LoginEvent::Cancelled
    )
}

fn snapshot(events: &Events) -> Vec<LoginEvent> {
    lock(events).clone()
}

async fn until(what: &str, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "timed out: {what}"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn wait_terminal(events: &Events) -> Vec<LoginEvent> {
    until("a terminal event", || {
        lock(events).last().is_some_and(is_terminal)
    })
    .await;
    snapshot(events)
}

async fn wait_for(events: &Events, wanted: fn(&LoginEvent) -> bool) {
    until("an event", || lock(events).iter().any(wanted)).await;
}

fn sink_into(events: &Events) -> EventSink<LoginEvent> {
    let events = events.clone();
    Arc::new(move |e| lock(&events).push(e))
}

impl Harness {
    async fn new(scenario: AuthScenario, browser: Browser) -> Harness {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let services = Services::start(scenario).await;
        let store = Arc::new(TestStore::default());
        let opened: Arc<Mutex<Vec<String>>> = Arc::default();
        let open_refused = Arc::new(AtomicBool::new(false));
        let log: Arc<Mutex<Vec<String>>> = Arc::default();
        let opener: BrowserOpener = {
            let (opened, refused) = (opened.clone(), open_refused.clone());
            Arc::new(move |url| {
                if refused.load(Ordering::SeqCst) {
                    return Err(());
                }
                lock(&opened).push(url.to_string());
                if browser == Browser::Visits {
                    let url = url.to_string();
                    tokio::spawn(async move {
                        let _ = browser_visit(&url).await;
                    });
                }
                Ok(())
            })
        };
        let auth = MicrosoftAuth::new("e2e-client".into(), auth_endpoints(&services.uri()));
        let sink_log = log.clone();
        let accounts = Accounts::new(&paths, Box::new(Shared(store.clone())), Some(auth), opener)
            .with_log(Arc::new(move |line| lock(&sink_log).push(line.to_string())));
        let hub = EventHub::default();
        let app_events: Arc<Mutex<Vec<AppEvent>>> = Arc::default();
        let seen = app_events.clone();
        hub.subscribe(Arc::new(move |e| lock(&seen).push(e)));
        Harness {
            settings: SettingsHandle::new(paths.clone(), Settings::default()),
            _dir: dir,
            paths,
            services,
            accounts: Arc::new(accounts),
            ops: Operations::default(),
            hub,
            store,
            opened,
            open_refused,
            log,
            app_events,
        }
    }

    fn login(&self, method: LoginMethod) -> (OperationId, Events) {
        let events: Events = Arc::default();
        let op = self.accounts.start_login(
            &self.ops,
            &self.hub,
            method,
            pages(),
            self.settings.clone(),
            sink_into(&events),
        );
        (op, events)
    }

    async fn run(&self, method: LoginMethod) -> Vec<LoginEvent> {
        let (_, events) = self.login(method);
        wait_terminal(&events).await
    }

    fn list(&self) -> AccountsDto {
        self.accounts.list(&self.settings.get()).unwrap()
    }

    /// The Microsoft accounts of the list (the development account is not one).
    fn microsoft(&self) -> Vec<AccountDto> {
        self.list()
            .accounts
            .into_iter()
            .filter(|a| a.kind == AccountKindDto::Microsoft)
            .collect()
    }

    fn selected(&self) -> Option<String> {
        self.microsoft()
            .into_iter()
            .find(|a| a.selected)
            .map(|a| a.name)
    }

    /// The port the sign-in page asked the browser to come back to.
    fn port_of(&self, index: usize) -> u16 {
        let url = lock(&self.opened)[index].clone();
        let url = tauri::Url::parse(&url).unwrap();
        let redirect = url
            .query_pairs()
            .find(|(k, _)| k == "redirect_uri")
            .unwrap()
            .1
            .into_owned();
        tauri::Url::parse(&redirect).unwrap().port().unwrap()
    }
}

fn signed_in(events: &[LoginEvent]) -> AccountDto {
    match events.last() {
        Some(LoginEvent::SignedIn { account }) => account.clone(),
        other => panic!("not signed in: {other:?}"),
    }
}

fn failed(events: &[LoginEvent]) -> ErrorDto {
    match events.last() {
        Some(LoginEvent::Failed { error }) => error.clone(),
        other => panic!("did not fail: {other:?}"),
    }
}

async fn port_is_free(port: u16) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        if TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    false
}

fn session(p: MockAccount) -> LoginSession {
    LoginSession {
        profile: Profile {
            id: p.uuid.into(),
            name: p.name.into(),
        },
        xuid: p.xuid(),
        minecraft_token: Secret::new(p.minecraft_token),
        minecraft_expires_at: unix_now() + 86_400,
        refresh_token: Secret::new(p.refresh_token),
    }
}

fn seeded(players: &[MockAccount]) -> (tempfile::TempDir, Paths, Accounts, Arc<TestStore>) {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let store = Arc::new(TestStore::default());
    let accounts = Accounts::new(
        &paths,
        Box::new(Shared(store.clone())),
        None,
        Arc::new(|_| Err(())),
    );
    for p in players {
        accounts.store.save_session(&session(*p)).unwrap();
    }
    (dir, paths, accounts, store)
}

fn settings_json(paths: &Paths) -> serde_json::Value {
    let text = std::fs::read_to_string(paths.root().join("settings.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

// ---- the happy paths ----

#[tokio::test(flavor = "multi_thread")]
async fn browser_login_saves_and_selects_the_account() {
    let h = Harness::new(AuthScenario::Ok, Browser::Visits).await;
    let (op, events) = h.login(LoginMethod::Browser);
    let events = wait_terminal(&events).await;
    assert_eq!(events.len(), 4, "{events:?}");
    assert_eq!(events[0], LoginEvent::Started { op });
    assert_eq!(events[1], LoginEvent::BrowserOpened);
    assert_eq!(events[2], LoginEvent::Completing);
    let account = signed_in(&events);
    assert_eq!(account.name, "E2EPlayer");
    assert_eq!(account.id, PLAYER_A.uuid);
    assert!(account.selected && !account.sign_in_required);
    assert_eq!(h.microsoft(), vec![account]);
    assert!(h.list().can_sign_in);
    // The data changed: the web view is told to read the list and the settings again.
    let app = lock(&h.app_events).clone();
    assert!(app.contains(&AppEvent::DataChanged {
        scope: DataScope::Accounts
    }));
    assert!(h.store.has(PLAYER_A.uuid, SecretKind::Refresh));
    assert!(h.store.has(PLAYER_A.uuid, SecretKind::Minecraft));
}

#[tokio::test(flavor = "multi_thread")]
async fn device_login_reports_the_code_then_signs_in() {
    let h = Harness::new(AuthScenario::Ok, Browser::Silent).await;
    let before = unix_now();
    let (op, events) = h.login(LoginMethod::DeviceCode);
    let events = wait_terminal(&events).await;
    assert_eq!(events.len(), 4, "{events:?}");
    assert_eq!(events[0], LoginEvent::Started { op });
    match &events[1] {
        LoginEvent::DeviceCode {
            user_code,
            verification_uri,
            expires_at,
        } => {
            assert!(!user_code.is_empty() && !verification_uri.is_empty());
            assert!(*expires_at >= before + 890 && *expires_at <= unix_now() + 910);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(events[2], LoginEvent::Completing);
    assert_eq!(signed_in(&events).name, "E2EPlayer");
    assert!(
        lock(&h.opened).is_empty(),
        "the device flow opens no browser"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn started_is_always_the_first_event_and_carries_the_op_returned_by_login_start() {
    for (method, browser) in [
        (LoginMethod::Browser, Browser::Visits),
        (LoginMethod::DeviceCode, Browser::Silent),
    ] {
        let h = Harness::new(AuthScenario::Ok, browser).await;
        let (op, events) = h.login(method);
        // Present before anything else could happen: it is sent before `start_login` returns.
        assert_eq!(snapshot(&events)[0], LoginEvent::Started { op });
        let events = wait_terminal(&events).await;
        assert_eq!(events[0], LoginEvent::Started { op });
        let started = events
            .iter()
            .filter(|e| matches!(e, LoginEvent::Started { .. }))
            .count();
        assert_eq!(started, 1);
    }
    // Ids are never reused within a run.
    let h = Harness::new(AuthScenario::Ok, Browser::Visits).await;
    let (a, ea) = h.login(LoginMethod::Browser);
    wait_terminal(&ea).await;
    let (b, eb) = h.login(LoginMethod::Browser);
    wait_terminal(&eb).await;
    assert_ne!(a, b);
}

#[tokio::test(flavor = "multi_thread")]
async fn second_account_becomes_selected_and_switch_works() {
    let h = Harness::new(AuthScenario::Ok, Browser::Visits).await;
    assert_eq!(
        signed_in(&h.run(LoginMethod::Browser).await).name,
        "E2EPlayer"
    );
    assert_eq!(
        signed_in(&h.run(LoginMethod::Browser).await).name,
        "SecondPlayer"
    );
    let names: Vec<String> = h.microsoft().into_iter().map(|a| a.name).collect();
    assert_eq!(names, ["E2EPlayer", "SecondPlayer"]);
    assert_eq!(h.selected().as_deref(), Some("SecondPlayer"));
    h.accounts.select(PLAYER_A.uuid, &h.settings).unwrap();
    assert_eq!(h.selected().as_deref(), Some("E2EPlayer"));
    let file = load_accounts_file(&h.paths.accounts_file()).unwrap();
    assert_eq!(file.default.as_deref(), Some(PLAYER_A.uuid));
    // Only the exact id selects; unknown ids and names are refused.
    for bad in ["nobody", "E2EPlayer", ""] {
        let e = h.accounts.select(bad, &h.settings).unwrap_err();
        assert!(
            matches!(e, Error::Auth(AuthError::AccountNotFound { .. })),
            "{bad}"
        );
    }
    assert_eq!(h.selected().as_deref(), Some("E2EPlayer"));
}

#[tokio::test(flavor = "multi_thread")]
async fn remove_deletes_tokens_and_moves_selection() {
    let h = Harness::new(AuthScenario::Ok, Browser::Visits).await;
    h.run(LoginMethod::Browser).await;
    h.run(LoginMethod::Browser).await;
    assert_eq!(h.selected().as_deref(), Some("SecondPlayer"));
    h.accounts.remove(PLAYER_B.uuid).unwrap();
    let list = h.microsoft();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "E2EPlayer");
    assert!(list[0].selected);
    assert!(!h.store.has(PLAYER_B.uuid, SecretKind::Refresh));
    assert!(!h.store.has(PLAYER_B.uuid, SecretKind::Minecraft));
    assert!(h.store.has(PLAYER_A.uuid, SecretKind::Refresh));
    // A non-selected account goes without changing the selection.
    h.run(LoginMethod::Browser).await; // the mock's third code: A again
    h.run(LoginMethod::Browser).await; // the fourth: B, selected
    assert_eq!(h.selected().as_deref(), Some("SecondPlayer"));
    h.accounts.select(PLAYER_A.uuid, &h.settings).unwrap();
    h.accounts.remove(PLAYER_B.uuid).unwrap();
    assert_eq!(h.selected().as_deref(), Some("E2EPlayer"));
    // The last one: no account, nothing selected.
    h.accounts.remove(PLAYER_A.uuid).unwrap();
    assert!(h.microsoft().is_empty());
    assert!(!h.store.has(PLAYER_A.uuid, SecretKind::Minecraft));
    let e = h.accounts.remove(PLAYER_A.uuid).unwrap_err();
    assert!(matches!(e, Error::Auth(AuthError::AccountNotFound { .. })));
}

// ---- failures ----

#[tokio::test(flavor = "multi_thread")]
async fn not_approved_login_maps_to_app_not_approved() {
    let h = Harness::new(AuthScenario::NotApproved, Browser::Visits).await;
    let events = h.run(LoginMethod::Browser).await;
    assert_eq!(failed(&events).code, ErrorCode::AppNotApproved);
    assert!(h.microsoft().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn every_failure_scenario_ends_in_one_typed_failure_and_saves_nothing() {
    use LoginMethod::{Browser as B, DeviceCode as D};
    let table = [
        (AuthScenario::NotApproved, B, ErrorCode::AppNotApproved),
        (AuthScenario::NoXbox, B, ErrorCode::NoXboxAccount),
        (AuthScenario::NoGame, B, ErrorCode::GameNotOwned),
        (AuthScenario::NoProfile, B, ErrorCode::NoProfile),
        (AuthScenario::Declined, B, ErrorCode::LoginDeclined),
        (AuthScenario::Declined, D, ErrorCode::LoginDeclined),
        (AuthScenario::NotApproved, D, ErrorCode::AppNotApproved),
        (AuthScenario::NetworkFailure, D, ErrorCode::Network),
        (AuthScenario::DeviceExpired, D, ErrorCode::DeviceCodeExpired),
    ];
    for (scenario, method, code) in table {
        let h = Harness::new(scenario, Browser::Visits).await;
        let (_, events) = h.login(method);
        let seen = wait_terminal(&events).await;
        assert_eq!(failed(&seen).code, code, "{scenario:?} {method:?}");
        let terminals = seen.iter().filter(|e| is_terminal(e)).count();
        assert_eq!(terminals, 1, "{scenario:?} {method:?}: {seen:?}");
        // Nothing follows the terminal event.
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(
            snapshot(&events).len(),
            seen.len(),
            "{scenario:?} {method:?}"
        );
        assert!(h.microsoft().is_empty(), "{scenario:?} {method:?}");
        assert!(!h.store.has(PLAYER_A.uuid, SecretKind::Minecraft));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn scenarios_that_only_delay_or_do_not_concern_sign_in_still_sign_in() {
    // `device-pending:<n>`: the code is polled until it is approved.
    let h = Harness::new(AuthScenario::DevicePending(3), Browser::Silent).await;
    let events = h.run(LoginMethod::DeviceCode).await;
    assert_eq!(signed_in(&events).name, "E2EPlayer");
    // `refresh-expired` concerns refreshing, not signing in.
    let h = Harness::new(AuthScenario::RefreshExpired, Browser::Visits).await;
    let events = h.run(LoginMethod::Browser).await;
    assert_eq!(signed_in(&events).name, "E2EPlayer");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refusing_secret_store_fails_the_sign_in_and_leaves_no_account() {
    let h = Harness::new(AuthScenario::Ok, Browser::Visits).await;
    h.store.refuse_set.store(true, Ordering::SeqCst);
    let events = h.run(LoginMethod::Browser).await;
    assert_eq!(failed(&events).code, ErrorCode::CredentialStore);
    assert!(h.microsoft().is_empty());
    assert!(!h.store.has(PLAYER_A.uuid, SecretKind::Refresh));
    // The state is usable afterwards: once the store works again the sign-in succeeds.
    h.store.refuse_set.store(false, Ordering::SeqCst);
    let events = h.run(LoginMethod::Browser).await;
    // The refused attempt used up the mock's first authorization code (player A's).
    assert_eq!(signed_in(&events).name, "SecondPlayer");
    assert_eq!(h.microsoft().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn without_a_secret_store_sign_in_fails_before_the_browser_opens() {
    let h = Harness::new(AuthScenario::Ok, Browser::Silent).await;
    let auth = MicrosoftAuth::new("e2e-client".into(), auth_endpoints(&h.services.uri()));
    let opened = h.opened.clone();
    let accounts = Arc::new(Accounts::without_secret_store(
        &h.paths,
        "no credential manager".into(),
        Some(auth),
        Arc::new(move |u| {
            lock(&opened).push(u.to_string());
            Ok(())
        }),
    ));
    let events: Events = Arc::default();
    let method = LoginMethod::Browser;
    accounts.start_login(
        &h.ops,
        &h.hub,
        method,
        pages(),
        h.settings.clone(),
        sink_into(&events),
    );
    let seen = wait_terminal(&events).await;
    assert_eq!(failed(&seen).code, ErrorCode::CredentialStore);
    assert!(lock(&h.opened).is_empty());
}

#[test]
fn accounts_list_works_without_secret_store() {
    let (_dir, paths, _accounts, _store) = seeded(&[PLAYER_A]);
    let accounts = Accounts::new(
        &paths,
        Box::new(UnavailableSecretStore("no credential manager".into())),
        None,
        Arc::new(|_| Err(())),
    );
    let list = accounts.list(&Settings::default()).unwrap();
    let ms: Vec<&AccountDto> = list
        .accounts
        .iter()
        .filter(|a| a.kind == AccountKindDto::Microsoft)
        .collect();
    assert_eq!(ms.len(), 1);
    assert_eq!(ms[0].name, "E2EPlayer");
    assert!(ms[0].selected && !ms[0].sign_in_required);
    assert!(!list.can_sign_in, "no client id: no sign-in");
}

// ---- cancel and a second operation ----

#[tokio::test(flavor = "multi_thread")]
async fn cancel_ends_a_waiting_login() {
    let h = Harness::new(AuthScenario::Ok, Browser::Silent).await;
    let (op, events) = h.login(LoginMethod::Browser);
    wait_for(&events, |e| *e == LoginEvent::BrowserOpened).await;
    let port = h.port_of(0);
    assert!(
        TcpListener::bind(("127.0.0.1", port)).is_err(),
        "the sign-in listens on its port while it waits"
    );
    assert!(h.ops.cancel(op));
    let seen = wait_terminal(&events).await;
    assert_eq!(seen.last(), Some(&LoginEvent::Cancelled));
    assert_eq!(seen.iter().filter(|e| is_terminal(e)).count(), 1);
    assert!(h.microsoft().is_empty());
    assert!(port_is_free(port).await, "the port is free after cancel");
    // Cancelling again, or an unknown operation, does nothing.
    assert!(!h.ops.cancel(op));
    assert!(!h.ops.cancel(OperationId(9999)));
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(snapshot(&events).len(), seen.len());
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_works_in_every_phase() {
    // starting: the device code request is still on its way
    let slow = AuthScenario::Slow(Duration::from_millis(400));
    let h = Harness::new(slow, Browser::Silent).await;
    let (op, events) = h.login(LoginMethod::DeviceCode);
    assert!(h.ops.cancel(op));
    tokio::time::sleep(Duration::from_millis(900)).await;
    assert_eq!(
        snapshot(&events),
        [LoginEvent::Started { op }, LoginEvent::Cancelled]
    );

    // showing the code: waiting for the user to approve it
    let h = Harness::new(AuthScenario::DevicePending(u32::MAX), Browser::Silent).await;
    let (op, events) = h.login(LoginMethod::DeviceCode);
    wait_for(&events, |e| matches!(e, LoginEvent::DeviceCode { .. })).await;
    assert!(h.ops.cancel(op));
    let seen = wait_terminal(&events).await;
    assert_eq!(seen.last(), Some(&LoginEvent::Cancelled));
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(snapshot(&events).len(), seen.len());

    // completing: the Xbox and Minecraft requests are slow
    let slow = AuthScenario::Slow(Duration::from_millis(250));
    let h = Harness::new(slow, Browser::Visits).await;
    let (op, events) = h.login(LoginMethod::Browser);
    wait_for(&events, |e| *e == LoginEvent::Completing).await;
    assert!(h.ops.cancel(op));
    tokio::time::sleep(Duration::from_millis(1800)).await;
    let seen = snapshot(&events);
    assert_eq!(seen.last(), Some(&LoginEvent::Cancelled), "{seen:?}");
    assert_eq!(seen.iter().filter(|e| is_terminal(e)).count(), 1);
    assert!(h.microsoft().is_empty());
    assert!(!h.store.has(PLAYER_A.uuid, SecretKind::Minecraft));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_operation_that_is_saving_cannot_be_cancelled() {
    let ops = Operations::default();
    let id = ops.start(
        OpKind::Login,
        Box::new(|| panic!("must not be called")),
        |_| tokio::spawn(std::future::pending::<()>()).abort_handle(),
    );
    assert!(ops.try_commit(id));
    assert!(!ops.cancel(id));
    ops.cancel_all();
    assert!(
        ops.finish(id),
        "the operation still owns its terminal event"
    );
    assert!(!ops.finish(id));
    assert!(!ops.try_commit(id));
}

#[tokio::test(flavor = "multi_thread")]
async fn second_login_cancels_the_first() {
    let h = Harness::new(AuthScenario::DevicePending(u32::MAX), Browser::Silent).await;
    let (_, first) = h.login(LoginMethod::Browser);
    wait_for(&first, |e| *e == LoginEvent::BrowserOpened).await;
    let port = h.port_of(0);
    let (second_op, second) = h.login(LoginMethod::DeviceCode);
    let seen = wait_terminal(&first).await;
    assert_eq!(seen.last(), Some(&LoginEvent::Cancelled));
    wait_for(&second, |e| matches!(e, LoginEvent::DeviceCode { .. })).await;
    assert!(
        !snapshot(&second).iter().any(is_terminal),
        "the second sign-in keeps running"
    );
    assert_eq!(snapshot(&second)[0], LoginEvent::Started { op: second_op });
    assert!(
        port_is_free(port).await,
        "the port of the first sign-in is free"
    );
    // The first channel got nothing of the second.
    assert!(snapshot(&first)
        .iter()
        .all(|e| !matches!(e, LoginEvent::DeviceCode { .. })));
    assert!(h.ops.cancel(second_op));
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_all_ends_every_waiting_operation() {
    let h = Harness::new(AuthScenario::Ok, Browser::Silent).await;
    let (_, events) = h.login(LoginMethod::Browser);
    wait_for(&events, |e| *e == LoginEvent::BrowserOpened).await;
    let port = h.port_of(0);
    h.ops.cancel_all();
    let seen = wait_terminal(&events).await;
    assert_eq!(seen.last(), Some(&LoginEvent::Cancelled));
    assert!(port_is_free(port).await);
}

// ---- the browser address stays in Rust ----

#[tokio::test(flavor = "multi_thread")]
async fn reopen_uses_the_stored_address_and_refuses_unknown_operations() {
    let h = Harness::new(AuthScenario::Ok, Browser::Silent).await;
    let (op, events) = h.login(LoginMethod::Browser);
    wait_for(&events, |e| *e == LoginEvent::BrowserOpened).await;
    h.accounts.reopen_browser(&h.ops, op).unwrap();
    let opened = lock(&h.opened).clone();
    assert_eq!(opened.len(), 2);
    assert_eq!(opened[0], opened[1]);
    // A refusing browser is a typed error that names nothing of the address.
    h.open_refused.store(true, Ordering::SeqCst);
    let e = h.accounts.reopen_browser(&h.ops, op).unwrap_err();
    assert_eq!(e.code, ErrorCode::OpenFailed);
    assert!(!e.detail.contains("authorize") && !e.detail.contains("state"));
    // The sign-in goes on.
    assert!(!snapshot(&events).iter().any(is_terminal));
    h.ops.cancel(op);
    let e = h.accounts.reopen_browser(&h.ops, op).unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidInput);
    // A device-code sign-in has no address to open.
    let h = Harness::new(AuthScenario::DevicePending(u32::MAX), Browser::Silent).await;
    let (op, events) = h.login(LoginMethod::DeviceCode);
    wait_for(&events, |e| matches!(e, LoginEvent::DeviceCode { .. })).await;
    let e = h.accounts.reopen_browser(&h.ops, op).unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidInput);
    h.ops.cancel(op);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_browser_that_cannot_be_opened_ends_the_sign_in() {
    let h = Harness::new(AuthScenario::Ok, Browser::Silent).await;
    h.open_refused.store(true, Ordering::SeqCst);
    let events = h.run(LoginMethod::Browser).await;
    assert_eq!(failed(&events).code, ErrorCode::OpenFailed);
    assert!(!events.contains(&LoginEvent::BrowserOpened));
}

#[test]
fn page_texts_from_the_web_view_are_checked() {
    let good = LoginPagesDto {
        success_title: "a".into(),
        success_body: "b".into(),
        failure_title: "c".into(),
        failure_body: "d".into(),
    };
    assert!(browser_pages(good.clone()).is_ok());
    for bad in [
        LoginPagesDto {
            success_title: " ".into(),
            ..good.clone()
        },
        LoginPagesDto {
            failure_body: String::new(),
            ..good.clone()
        },
        LoginPagesDto {
            success_body: "x".repeat(501),
            ..good.clone()
        },
    ] {
        let e = browser_pages(bad).unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidInput);
    }
}

// ---- re-signing in ----

#[tokio::test(flavor = "multi_thread")]
async fn signing_in_again_as_another_player_adds_that_player_and_keeps_the_old_one_flagged() {
    let h = Harness::new(AuthScenario::Ok, Browser::Visits).await;
    h.run(LoginMethod::Browser).await; // A
    h.accounts.mark_sign_in_required(PLAYER_A.uuid).unwrap();
    let list = h.microsoft();
    assert!(list[0].sign_in_required && list[0].selected);

    // The user was asked to sign A in again, but the browser session belongs to B.
    let b = signed_in(&h.run(LoginMethod::Browser).await);
    assert_eq!(b.name, "SecondPlayer");
    assert!(b.selected && !b.sign_in_required);
    let list = h.microsoft();
    assert_eq!(
        list.len(),
        2,
        "B is added, A is neither replaced nor merged"
    );
    let a = list.iter().find(|x| x.id == PLAYER_A.uuid).unwrap();
    assert!(a.sign_in_required && !a.selected, "A is still signed out");
    assert!(
        h.store.has(PLAYER_A.uuid, SecretKind::Refresh),
        "A's secrets are untouched"
    );

    // The same player coming back clears the flag.
    let a = signed_in(&h.run(LoginMethod::Browser).await);
    assert_eq!(a.id, PLAYER_A.uuid);
    let list = h.microsoft();
    assert_eq!(list.len(), 2);
    let a = list.iter().find(|x| x.id == PLAYER_A.uuid).unwrap();
    assert!(!a.sign_in_required && a.selected);
    let e = h.accounts.mark_sign_in_required("nobody").unwrap_err();
    assert!(matches!(e, Error::Auth(AuthError::AccountNotFound { .. })));
}

// ---- sign-out with a refusing store ----

#[test]
fn a_secret_store_that_refuses_halfway_never_leaves_a_usable_looking_entry() {
    for refuse_from in [1, 2] {
        let (_dir, paths, accounts, store) = seeded(&[PLAYER_A, PLAYER_B]);
        store
            .refuse_delete_from
            .store(refuse_from, Ordering::SeqCst);
        let e = accounts.remove(PLAYER_B.uuid).unwrap_err();
        assert!(matches!(e, Error::CredentialStore(_)), "{e}");
        assert_eq!(error_dto(&e).code, ErrorCode::CredentialStore);
        // The file still parses and lists B, but flagged: not usable without sign-in.
        let list = accounts.list(&Settings::default()).unwrap();
        let b = list
            .accounts
            .iter()
            .find(|a| a.id == PLAYER_B.uuid)
            .unwrap();
        assert!(b.sign_in_required, "refuse_from {refuse_from}");
        let a = list
            .accounts
            .iter()
            .find(|a| a.id == PLAYER_A.uuid)
            .unwrap();
        assert!(!a.sign_in_required);
        let file = load_accounts_file(&paths.accounts_file()).unwrap();
        assert_eq!(file.accounts.len(), 2);
        // Once the store works, signing out completes.
        store.refuse_delete_from.store(0, Ordering::SeqCst);
        accounts.remove(PLAYER_B.uuid).unwrap();
        assert!(!store.has(PLAYER_B.uuid, SecretKind::Refresh));
        assert!(!store.has(PLAYER_B.uuid, SecretKind::Minecraft));
        let file = load_accounts_file(&paths.accounts_file()).unwrap();
        assert_eq!(file.accounts.len(), 1);
    }
}

// ---- files and settings ----

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_account_writes_leave_a_consistent_file() {
    let (_dir, paths, accounts, _store) = seeded(&[PLAYER_A, PLAYER_B]);
    let accounts = Arc::new(accounts);
    let settings = SettingsHandle::new(paths.clone(), Settings::default());
    let third = MockAccount {
        name: "ThirdPlayer",
        uuid: "00112233445566778899aabbccddeeff",
        minecraft_token: "test-mc-token-c",
        refresh_token: "test-refresh-c",
    };
    let mut tasks = Vec::new();
    for i in 0..40 {
        let (a, s) = (accounts.clone(), settings.clone());
        tasks.push(tokio::task::spawn_blocking(move || {
            let id = if i % 2 == 0 {
                PLAYER_A.uuid
            } else {
                PLAYER_B.uuid
            };
            a.select(id, &s).unwrap();
            if i % 4 == 0 {
                a.persist(&session(third), &s).unwrap();
            } else if i % 4 == 2 {
                // Another task may have removed it already.
                let _ = a.remove(third.uuid);
            }
        }));
    }
    for t in tasks {
        t.await.unwrap();
    }
    let file = load_accounts_file(&paths.accounts_file()).unwrap();
    assert!(file.accounts.len() == 2 || file.accounts.len() == 3);
    let default = file.default.clone().unwrap();
    assert!(file.accounts.iter().any(|a| a.uuid == default));
    assert!(!paths.accounts_file().with_extension("json.tmp").exists());
}

#[test]
fn a_new_login_clears_the_dev_account_flag_in_the_file() {
    let (_dir, paths, accounts, _store) = seeded(&[]);
    let settings = SettingsHandle::new(paths.clone(), Settings::default());
    settings
        .update(|s| {
            s.dev_account = true;
            Ok(())
        })
        .unwrap();
    assert_eq!(settings_json(&paths)["devAccount"], true);
    accounts.persist(&session(PLAYER_A), &settings).unwrap();
    assert_eq!(settings_json(&paths)["devAccount"], false);
    assert!(!settings.get().dev_account);
}

#[cfg(feature = "dev-offline")]
#[test]
fn selecting_the_dev_account_saves_the_setting_in_the_same_call() {
    let (_dir, paths, accounts, _store) = seeded(&[PLAYER_A]);
    let settings = SettingsHandle::new(paths.clone(), Settings::default());
    accounts.select(DEV_ACCOUNT_ID, &settings).unwrap();
    assert_eq!(settings_json(&paths)["devAccount"], true);
    accounts.select(PLAYER_A.uuid, &settings).unwrap();
    assert_eq!(settings_json(&paths)["devAccount"], false);
}

#[cfg(feature = "dev-offline")]
#[test]
fn dev_build_lists_and_selects_the_offline_account() {
    let (_dir, paths, accounts, _store) = seeded(&[PLAYER_A]);
    let settings = SettingsHandle::new(paths, Settings::default());
    let list = accounts.list(&settings.get()).unwrap();
    let dev = list
        .accounts
        .iter()
        .find(|a| a.id == DEV_ACCOUNT_ID)
        .unwrap();
    assert_eq!(dev.kind, AccountKindDto::OfflineDev);
    assert!(!dev.selected);
    accounts.select(DEV_ACCOUNT_ID, &settings).unwrap();
    let list = accounts.list(&settings.get()).unwrap();
    let dev = list
        .accounts
        .iter()
        .find(|a| a.id == DEV_ACCOUNT_ID)
        .unwrap();
    assert!(dev.selected);
    assert_eq!(list.accounts.iter().filter(|a| a.selected).count(), 1);
    // It has no tokens: it cannot be signed out.
    assert!(accounts.remove(DEV_ACCOUNT_ID).is_err());
}

#[cfg(not(feature = "dev-offline"))]
#[test]
fn release_build_never_lists_dev_account() {
    let (_dir, paths, accounts, _store) = seeded(&[PLAYER_A]);
    let settings = SettingsHandle::new(paths, Settings::default());
    settings
        .update(|s| {
            s.dev_account = true;
            Ok(())
        })
        .unwrap();
    let list = accounts.list(&settings.get()).unwrap();
    assert!(list.accounts.iter().all(|a| a.id != DEV_ACCOUNT_ID));
    // A stale setting does not deselect the real account.
    assert!(list
        .accounts
        .iter()
        .any(|a| a.selected && a.name == "E2EPlayer"));
    let e = accounts.select(DEV_ACCOUNT_ID, &settings).unwrap_err();
    assert!(matches!(e, Error::Auth(AuthError::AccountNotFound { .. })));
}

// ---- no token anywhere ----

#[tokio::test(flavor = "multi_thread")]
async fn login_events_never_contain_tokens() {
    let h = Harness::new(AuthScenario::Ok, Browser::Visits).await;
    let mut outputs: Vec<String> = Vec::new();
    let mut take = |what: &str, text: String| outputs.push(format!("{what}: {text}"));

    // Two complete sign-ins and the command results around them.
    for method in [LoginMethod::Browser, LoginMethod::DeviceCode] {
        let (op, events) = h.login(method);
        take("login_start result", serde_json::to_string(&op).unwrap());
        for e in wait_terminal(&events).await {
            take("event", serde_json::to_string(&e).unwrap());
        }
        take("accounts_list", serde_json::to_string(&h.list()).unwrap());
    }
    let cancelled = h.ops.cancel(OperationId(1));
    take(
        "login_cancel result",
        serde_json::to_string(&cancelled).unwrap(),
    );
    let opened = lock(&h.opened).clone();
    assert_eq!(opened.len(), 1, "the browser sign-in opened one address");

    // Failures after the Microsoft step carry the most data through the chain.
    for scenario in [
        AuthScenario::NotApproved,
        AuthScenario::NoXbox,
        AuthScenario::NoGame,
    ] {
        h.services.auth.set_scenario(scenario);
        for method in [LoginMethod::Browser, LoginMethod::DeviceCode] {
            for e in h.run(method).await {
                take("event", serde_json::to_string(&e).unwrap());
            }
        }
    }
    h.services.auth.set_scenario(AuthScenario::Ok);
    let opened = lock(&h.opened).clone();

    // The snapshot comes after the sign-ins.
    let secrets = h.services.auth.issued_secrets();
    assert!(!secrets.is_empty());
    for must in [
        PLAYER_A.minecraft_token.to_string(),
        PLAYER_A.refresh_token.to_string(),
        PLAYER_A.msa_access_token(),
        PLAYER_A.xsts_token(),
    ] {
        assert!(secrets.contains(&must), "the kit did not list {must}");
    }
    assert!(
        secrets.iter().any(|s| s.starts_with("test-code-")),
        "authorization code"
    );
    assert!(
        secrets.iter().any(|s| s.starts_with("test-device-")),
        "device code"
    );
    // The state of the authorize address and the PKCE verifier are in the list too.
    let authorize = tauri::Url::parse(&opened[0]).unwrap();
    let state = authorize
        .query_pairs()
        .find(|(k, _)| k == "state")
        .unwrap()
        .1
        .into_owned();
    assert!(secrets.contains(&state), "the kit saw the state");
    assert!(
        secrets
            .iter()
            .any(|s| s.len() == 43 && !s.starts_with("test-")),
        "the kit saw the verifier"
    );

    outputs.extend(lock(&h.log).iter().map(|l| format!("log: {l}")));
    assert!(outputs.iter().any(|o| o.starts_with("log: sign-in")));
    for out in &outputs {
        for secret in &secrets {
            assert!(!out.contains(secret.as_str()), "{secret} leaked into {out}");
        }
        // The authorization address and its parts are never output either.
        for part in opened.iter().map(String::as_str).chain([
            "state=",
            "code_verifier",
            "code_challenge",
            "device_code",
        ]) {
            assert!(!out.contains(part), "{part} leaked into {out}");
        }
    }
    // What the user must see is shown: the user code is in the events.
    assert!(outputs.iter().any(|o| o.contains("TEST-0")));
}
