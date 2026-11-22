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
use hengst_desktop_lib::dto::accounts::{LoginEvent, LoginMethod};
use hengst_desktop_lib::dto::settings::SettingsDto;
use hengst_desktop_lib::error::error_dto;
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
    let list = instances_list_impl(&state).await.unwrap();
    assert_eq!(list.len(), 2);
    seen.push(serde_json::to_string(&list).unwrap());
    seen.push(serde_json::to_string(&loader_kinds_impl()).unwrap());
    let accounts = state.accounts.list(&state.settings.get()).unwrap();
    seen.push(serde_json::to_string(&accounts).unwrap());
    seen.push(serde_json::to_string(&app_info_impl(&state)).unwrap());
    seen.push(serde_json::to_string(&SettingsDto::new(state.settings.get(), None)).unwrap());
    seen.extend(run_mock_login_with_token(LoginMethod::Browser).await);
    seen.extend(run_mock_login_with_token(LoginMethod::DeviceCode).await);
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
