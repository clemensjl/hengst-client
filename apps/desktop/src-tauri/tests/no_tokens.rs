//! No token type can be serialized, and nothing the web view receives contains a token.

use hengst_core::auth::flow::LoginSession;
use hengst_core::auth::minecraft::Profile;
use hengst_core::auth::msa::MsaTokens;
use hengst_core::auth::store::{AccountStore, MemorySecretStore};
use hengst_core::auth::{AuthError, MicrosoftAuth};
use hengst_core::launch::Endpoints;
use hengst_core::paths::Paths;
use hengst_core::secret::Secret;
use hengst_core::Error;
use hengst_desktop_lib::accounts::{Accounts, BrowserOpener, Operations};
use hengst_desktop_lib::commands::app::app_info_impl;
use hengst_desktop_lib::commands::instances::{instances_list_impl, loader_kinds_impl};
use hengst_desktop_lib::commands::play::play_impl;
use hengst_desktop_lib::dto::accounts::{LoginEvent, LoginMethod};
use hengst_desktop_lib::dto::play::{GameStateDto, PlayEvent};
use hengst_desktop_lib::dto::settings::SettingsDto;
use hengst_desktop_lib::error::error_dto;
use hengst_desktop_lib::events::AppEvent;
use hengst_desktop_lib::play::{force_stop, stop};
use hengst_desktop_lib::settings::{Settings, SettingsHandle};
use hengst_desktop_lib::state::AppState;
use hengst_testkit::{auth_endpoints, browser_visit, mount_auth, AuthScenario, MockAccount};
use static_assertions::assert_not_impl_any;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use wiremock::MockServer;

const FAKE: &str = "FAKE-ACCESS-TOKEN-7f3a";

#[test]
fn token_types_do_not_implement_serialize() {
    assert_not_impl_any!(Secret: serde::Serialize);
    assert_not_impl_any!(MsaTokens: serde::Serialize);
    assert_not_impl_any!(LoginSession: serde::Serialize);
}

/// A mock player whose tokens all contain `FAKE`.
fn fake_player() -> MockAccount {
    MockAccount {
        name: "FakeTokenPlayer",
        uuid: "aabbccddeeff00112233445566778899",
        minecraft_token: FAKE,
        refresh_token: "FAKE-ACCESS-TOKEN-7f3a-refresh",
    }
}

/// Signs `fake_player` in through `method` and returns every event as serialized for the web view.
async fn run_mock_login_with_token(method: LoginMethod) -> Vec<String> {
    let server = MockServer::builder().start().await;
    let mock = mount_auth(&server, AuthScenario::Ok, &[fake_player()]).await;
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let opener: BrowserOpener = Arc::new(|url| {
        let url = url.to_string();
        tokio::spawn(async move {
            let _ = browser_visit(&url).await;
        });
        Ok(())
    });
    let auth = MicrosoftAuth::new("e2e-client".into(), auth_endpoints(&server.uri()));
    let accounts = Arc::new(Accounts::new(
        &paths,
        Box::new(MemorySecretStore::default()),
        Some(auth),
        opener,
    ));
    let events: Arc<Mutex<Vec<LoginEvent>>> = Arc::default();
    let sink = {
        let events = events.clone();
        Arc::new(move |e| events.lock().unwrap().push(e))
    };
    let pages = hengst_core::auth::browser::BrowserLoginPages {
        success_title: "Done".into(),
        success_body: "You can return to the launcher.".into(),
        failure_title: "Not done".into(),
        failure_body: "Go back to the launcher.".into(),
    };
    let state = AppState::new(paths.clone(), Endpoints::default());
    accounts.start_login(
        &Operations::default(),
        &state.events,
        method,
        pages,
        SettingsHandle::new(paths.clone(), Settings::default()),
        sink,
    );
    let start = Instant::now();
    loop {
        let done = events.lock().unwrap().last().is_some_and(|e| {
            matches!(
                e,
                LoginEvent::SignedIn { .. } | LoginEvent::Failed { .. } | LoginEvent::Cancelled
            )
        });
        if done {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "sign-in timed out"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let seen = events.lock().unwrap();
    assert!(
        matches!(seen.last(), Some(LoginEvent::SignedIn { .. })),
        "{seen:?}"
    );
    // The check below is only meaningful if the chain really carried the fake token.
    assert!(mock.issued_secrets().iter().any(|s| s.contains(FAKE)));
    seen.iter()
        .map(|e| serde_json::to_string(e).unwrap())
        .collect()
}

struct SharedStore(Arc<MemorySecretStore>);

impl hengst_core::auth::store::SecretStore for SharedStore {
    fn get(&self, key: &str) -> hengst_core::Result<Option<Secret>> {
        self.0.get(key)
    }
    fn set(&self, key: &str, value: &Secret) -> hengst_core::Result<()> {
        self.0.set(key, value)
    }
    fn delete(&self, key: &str) -> hengst_core::Result<()> {
        self.0.delete(key)
    }
}

/// Starts an instance for an account whose token is `FAKE` (mock Mojang, `fake-java` printing its
/// arguments) and returns every play event, game state and stop answer as serialized for the web
/// view, plus the output lines (what the live log will carry).
async fn run_mock_play_with_token() -> Vec<String> {
    let server = MockServer::builder().start().await;
    hengst_testkit::mount_mojang(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().join("home"));
    std::fs::create_dir_all(paths.root()).unwrap();
    let secrets = Arc::new(MemorySecretStore::default());
    let store = AccountStore::new(paths.accounts_file(), SharedStore(secrets.clone()));
    store
        .save_session(&LoginSession {
            profile: Profile {
                id: "0123456789abcdef0123456789abcdef".into(),
                name: "Mira_K".into(),
            },
            xuid: "2535".into(),
            minecraft_token: Secret::new(FAKE),
            minecraft_expires_at: u64::MAX,
            refresh_token: Secret::new(format!("{FAKE}-refresh")),
        })
        .unwrap();
    store
        .set_default("0123456789abcdef0123456789abcdef")
        .unwrap();
    let accounts = Accounts::new(
        &paths,
        Box::new(SharedStore(secrets)),
        None,
        Arc::new(|_| Err(())),
    );
    let state = AppState::with_accounts(
        paths.clone(),
        hengst_testkit::endpoints(&server.uri()),
        accounts,
    );
    state
        .settings
        .update(|s| {
            s.java_override = Some(hengst_testkit::fake_java());
            Ok(())
        })
        .unwrap();
    let app_events: Arc<Mutex<Vec<String>>> = Arc::default();
    let kept = app_events.clone();
    state.events.subscribe(Arc::new(move |e: AppEvent| {
        kept.lock()
            .unwrap()
            .push(serde_json::to_string(&e).unwrap())
    }));
    let lines: Arc<Mutex<Vec<String>>> = Arc::default();
    let kept = lines.clone();
    state
        .game
        .set_line_handler(Arc::new(move |_, l| kept.lock().unwrap().push(l.message)));
    let inst = hengst_core::instances::create(
        &paths,
        &hengst_core::instances::NewInstance {
            name: "Mira_K world".into(),
            game: "9.9".into(),
            loader: None,
        },
        1,
    )
    .unwrap();
    std::fs::write(inst.dir.join("fake-java.txt"), "args\nsleep 60000\n").unwrap();
    let events: Arc<Mutex<Vec<PlayEvent>>> = Arc::default();
    let sink = events.clone();
    play_impl(
        &state,
        inst.id.clone(),
        Arc::new(move |e| sink.lock().unwrap().push(e)),
    )
    .await
    .unwrap();
    let start = Instant::now();
    while !lines
        .lock()
        .unwrap()
        .iter()
        .any(|l| l == "ARG --accessToken")
    {
        assert!(start.elapsed() < Duration::from_secs(60), "no game output");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let mut seen = vec![serde_json::to_string(&state.game.state()).unwrap()];
    seen.push(serde_json::to_string(&stop(&state.game).await.unwrap()).unwrap());
    assert!(force_stop(&state.game).await.unwrap());
    while !matches!(state.game.state(), GameStateDto::Idle { .. }) {
        assert!(
            start.elapsed() < Duration::from_secs(60),
            "the game did not end"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    seen.push(serde_json::to_string(&state.game.state()).unwrap());
    seen.extend(
        events
            .lock()
            .unwrap()
            .iter()
            .map(|e| serde_json::to_string(e).unwrap()),
    );
    seen.extend(app_events.lock().unwrap().iter().cloned());
    assert!(
        seen.iter().any(|s| s.contains("lastExit")),
        "the exit is covered"
    );
    let lines = lines.lock().unwrap().clone();
    // The game really got the token: the core replaced it in the output.
    assert!(lines.iter().any(|l| l == "ARG <redacted>"), "{lines:?}");
    seen.extend(lines);
    seen
}

/// Errors whose message can carry a token in a form a log or an HTTP library produces: a URL with
/// the token in its query, an `Authorization` header or a bearer token in the text, a JSON error
/// that quotes the offending value. (A token pasted into a free-text field with no such marker,
/// say a player name, cannot be recognized by anyone; the sign-in code never puts one there.)
fn every_error_with_message_containing(token: &str) -> Vec<Error> {
    let url = format!("https://example.invalid/path?access_token={token}&x=1");
    let bearer = format!("request failed, Authorization: Bearer {token}");
    let json = serde_json::from_str::<u64>(&format!("\"{token}\"")).unwrap_err();
    vec![
        Error::Status {
            url: url.clone(),
            status: 401,
        },
        Error::TooLarge {
            url: url.clone(),
            limit: 1,
        },
        Error::InvalidChecksumFile { url: url.clone() },
        Error::NoChecksum { url },
        Error::Installer(bearer.clone()),
        Error::InstallFailed(bearer.clone()),
        Error::CredentialStore(bearer.clone()),
        Error::Io(std::io::Error::other(bearer)),
        Error::Json(json),
        Error::Auth(AuthError::BadResponse("the Microsoft token endpoint")),
        Error::Auth(AuthError::AppNotApproved),
        Error::Auth(AuthError::LoginDeclined),
        Error::Auth(AuthError::MissingClientId),
    ]
}

#[tokio::test(flavor = "multi_thread")]
async fn no_dto_or_event_contains_a_token() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    // accounts.json with an account whose tokens (in a secret store of their own) contain FAKE.
    let store = AccountStore::new(paths.accounts_file(), MemorySecretStore::default());
    store
        .save_session(&LoginSession {
            profile: Profile {
                id: "0123456789abcdef0123456789abcdef".into(),
                name: "Mira_K".into(),
            },
            xuid: "2535".into(),
            minecraft_token: Secret::new(FAKE),
            minecraft_expires_at: 1,
            refresh_token: Secret::new(format!("{FAKE}-refresh")),
        })
        .unwrap();
    // An instance whose file carries the fake token in fields the app does not know (at the top
    // level and inside "loader") and in its Java path, plus an unreadable one that quotes it.
    let folder = paths.instance("9.9");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join("instance.json"),
        serde_json::json!({
            "format": 1, "name": "Mira_K world", "game": "9.9",
            "loader": {"kind": "fabric", "version": "0.99.0", "token": FAKE},
            "java": format!("C:\\{FAKE}\\javaw.exe"), "createdAt": 1, "notes": FAKE,
            "nested": {"deep": FAKE}
        })
        .to_string(),
    )
    .unwrap();
    std::fs::create_dir_all(paths.instance("broken")).unwrap();
    std::fs::write(
        paths.instance("broken").join("instance.json"),
        format!("{{ \"notes\": \"{FAKE}\" "),
    )
    .unwrap();
    let state = AppState::new(paths.clone(), Endpoints::default());

    let mut seen: Vec<String> = Vec::new();
    let list = instances_list_impl(&state).await.unwrap().instances;
    assert_eq!(list.len(), 2);
    seen.push(serde_json::to_string(&list).unwrap());
    seen.push(serde_json::to_string(&loader_kinds_impl()).unwrap());
    let accounts = state.accounts.list(&state.settings.get()).unwrap();
    seen.push(serde_json::to_string(&accounts).unwrap());
    seen.push(serde_json::to_string(&app_info_impl(&state)).unwrap());
    seen.push(serde_json::to_string(&SettingsDto::new(state.settings.get(), None)).unwrap());
    seen.extend(run_mock_login_with_token(LoginMethod::Browser).await);
    seen.extend(run_mock_login_with_token(LoginMethod::DeviceCode).await);
    seen.extend(run_mock_play_with_token().await);
    for err in every_error_with_message_containing(FAKE) {
        seen.push(serde_json::to_string(&error_dto(&err)).unwrap());
    }
    assert!(seen.len() > 12);
    assert!(seen.iter().any(|s| s.contains("Mira_K")));
    assert!(seen.iter().any(|s| s.contains("FakeTokenPlayer")));
    assert!(
        seen.iter().any(|s| s.contains("Mira_K world")),
        "the instance row is covered"
    );
    for s in &seen {
        assert!(!s.contains(FAKE), "token leaked into: {s}");
    }
}
