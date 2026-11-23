//! Playing end to end against the test kit: mock Mojang and sign-in services, `fake-java` as
//! the game (it never shows a window; the stop test uses its hidden one). No real network, no
//! real game.

use super::*;
use crate::commands::instances::instance_delete_impl;
use crate::commands::play::{play_cancel_impl, play_impl};
use crate::state::AppState;
use hengst_core::auth::flow::LoginSession;
use hengst_core::auth::minecraft::Profile;
use hengst_core::auth::store::{AccountStore, MemorySecretStore, SecretStore};
use hengst_core::auth::MicrosoftAuth;
use hengst_core::instances::NewInstance;
use hengst_core::secret::Secret;
use hengst_core::Error;
use hengst_testkit::{
    auth_endpoints, endpoints, fake_java, mount_auth, mount_mojang, AuthScenario,
};
use hengst_testkit::{PLAYER_A, PLAYER_B};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Fake, with characters that change in JSON or in a URL.
const TOKEN: &str = "test-FAKE-ACCESS-TOKEN-7f3a.play/Tok+en=x";
const WAIT: Duration = Duration::from_secs(60);
const ID: &str = "9.9";

struct SharedStore(Arc<MemorySecretStore>);

impl SecretStore for SharedStore {
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

type Seen<T> = Arc<Mutex<Vec<T>>>;

struct Fx {
    server: MockServer,
    _dir: tempfile::TempDir,
    state: AppState,
    app_events: Seen<AppEvent>,
    lines: Seen<(RunId, String)>,
}

impl Drop for Fx {
    /// The game's job has no kill-on-close: end a game a failed test left running.
    fn drop(&mut self) {
        if let Phase::Running(run) = &lock(&self.state.game.inner).phase {
            let _ = run.process.force_stop();
        }
    }
}

enum Login {
    /// A Microsoft account whose saved token is valid for good (no refresh, no network).
    Valid,
    /// A Microsoft account whose token is due; the sign-in mock refuses every refresh.
    Expired,
    /// No account at all.
    None,
}

async fn fixture(login: Login) -> Fx {
    let server = MockServer::builder().start().await;
    mount_mojang(&server).await;
    let scenario = match login {
        Login::Expired => AuthScenario::RefreshExpired,
        _ => AuthScenario::Ok,
    };
    mount_auth(&server, scenario, &[PLAYER_A, PLAYER_B]).await;
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().join("home"));
    std::fs::create_dir_all(paths.root()).unwrap();
    let secrets = Arc::new(MemorySecretStore::default());
    let expires = match login {
        Login::Valid => u64::MAX,
        Login::Expired => 1,
        Login::None => 0,
    };
    if !matches!(login, Login::None) {
        let store = AccountStore::new(paths.accounts_file(), SharedStore(secrets.clone()));
        store
            .save_session(&LoginSession {
                profile: Profile {
                    id: PLAYER_A.uuid.into(),
                    name: "Mira_K".into(),
                },
                xuid: "2535400000000001".into(),
                minecraft_token: Secret::new(TOKEN),
                minecraft_expires_at: expires,
                refresh_token: Secret::new(PLAYER_A.refresh_token),
            })
            .unwrap();
        store.set_default(PLAYER_A.uuid).unwrap();
    }
    let auth = MicrosoftAuth::new("e2e-client".into(), auth_endpoints(&server.uri()));
    let accounts = Accounts::new(
        &paths,
        Box::new(SharedStore(secrets)),
        Some(auth),
        Arc::new(|_| Err(())),
    );
    let mut state = AppState::with_accounts(paths.clone(), endpoints(&server.uri()), accounts);
    state.trasher = Arc::new(|_| {
        Err(Error::Io(std::io::Error::other(
            "the test fixture has no Recycle Bin",
        )))
    });
    state
        .settings
        .update(|s| {
            s.java_override = Some(fake_java());
            Ok(())
        })
        .unwrap();
    let app_events: Seen<AppEvent> = Arc::default();
    let sink = app_events.clone();
    state
        .events
        .subscribe(Arc::new(move |e| sink.lock().unwrap().push(e)));
    let lines: Seen<(RunId, String)> = Arc::default();
    let kept = lines.clone();
    state
        .game
        .set_line_handler(Arc::new(move |run, line: LogLine| {
            kept.lock().unwrap().push((run, line.message))
        }));
    let made = instances::create(
        &paths,
        &NewInstance {
            name: "Mira_K world".into(),
            game: ID.into(),
            loader: None,
        },
        1,
    )
    .unwrap();
    assert_eq!(made.id, ID);
    Fx {
        server,
        _dir: dir,
        state,
        app_events,
        lines,
    }
}

impl Fx {
    /// What `fake-java` does when the instance starts.
    fn script(&self, script: &str) {
        std::fs::write(self.state.paths.instance(ID).join("fake-java.txt"), script).unwrap();
    }

    async fn play(&self) -> (OperationId, Seen<PlayEvent>) {
        let events: Seen<PlayEvent> = Arc::default();
        let sink = events.clone();
        let op = play_impl(
            &self.state,
            ID.into(),
            Arc::new(move |e| sink.lock().unwrap().push(e)),
        )
        .await
        .unwrap();
        (op, events)
    }

    fn states(&self) -> Vec<GameStateDto> {
        self.app_events
            .lock()
            .unwrap()
            .iter()
            .filter_map(|e| match e {
                AppEvent::GameState { state } => Some(state.clone()),
                _ => None,
            })
            .collect()
    }

    fn active(&self) -> Option<String> {
        self.state.active_instance.lock().unwrap().clone()
    }

    async fn running(&self) -> RunId {
        until("Running", || {
            matches!(self.state.game.state(), GameStateDto::Running { .. })
        })
        .await;
        match self.state.game.state() {
            GameStateDto::Running { run_id, .. } => run_id,
            other => panic!("{other:?}"),
        }
    }

    async fn idle(&self) -> Option<LastExit> {
        until("Idle", || {
            matches!(self.state.game.state(), GameStateDto::Idle { .. })
        })
        .await;
        match self.state.game.state() {
            GameStateDto::Idle { last_exit } => last_exit,
            other => panic!("{other:?}"),
        }
    }

    async fn line(&self, text: &str) {
        until(text, || {
            self.lines.lock().unwrap().iter().any(|(_, l)| l == text)
        })
        .await;
    }
}

async fn until(what: &str, done: impl Fn() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < WAIT, "no {what} within {WAIT:?}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn terminal(e: &PlayEvent) -> bool {
    matches!(
        e,
        PlayEvent::Started { .. } | PlayEvent::Failed { .. } | PlayEvent::Cancelled
    )
}

async fn ended(events: &Seen<PlayEvent>) -> PlayEvent {
    until("a terminal play event", || {
        events.lock().unwrap().iter().any(terminal)
    })
    .await;
    // Nothing follows the terminal event.
    tokio::time::sleep(Duration::from_millis(100)).await;
    let events = events.lock().unwrap();
    assert_eq!(
        events.iter().filter(|e| terminal(e)).count(),
        1,
        "{events:?}"
    );
    let last = events.last().unwrap().clone();
    assert!(terminal(&last), "{events:?}");
    last
}

/// A preparation that reports one installer step and then blocks a blocking thread, like the
/// processors do, until its cancel flag is set or `release` is; then it fails. `saw_cancel` tells
/// whether the flag (not an abort) ended it.
struct Blocking {
    release: Arc<AtomicBool>,
    saw_cancel: Arc<AtomicBool>,
}

fn blocking_preparer() -> (Preparer, Blocking) {
    let release = Arc::new(AtomicBool::new(false));
    let saw_cancel = Arc::new(AtomicBool::new(false));
    let (r, s) = (release.clone(), saw_cancel.clone());
    let preparer: Preparer = Arc::new(move |job: PrepareJob| {
        let (r, s) = (r.clone(), s.clone());
        Box::pin(async move {
            (job.progress)(Progress::InstallStep {
                index: 1,
                total: 3,
                name: "jarsplitter".into(),
            });
            tokio::task::spawn_blocking(move || {
                let start = Instant::now();
                while !r.load(Ordering::SeqCst) && start.elapsed() < WAIT {
                    if job.cancel.is_cancelled() {
                        s.store(true, Ordering::SeqCst);
                        return Err(Error::Cancelled);
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(Error::InstallFailed("released by the test".into()))
            })
            .await
            .unwrap()
        })
    });
    (
        preparer,
        Blocking {
            release,
            saw_cancel,
        },
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn plays_through_running_and_stops_on_request() {
    let fx = fixture(Login::Valid).await;
    fx.script("out hello\nwindow\nexit 0\n");
    let (op, events) = fx.play().await;
    let PlayEvent::Started { run_id } = ended(&events).await else {
        panic!("{:?}", events.lock().unwrap());
    };
    {
        let events = events.lock().unwrap();
        assert_eq!(events[0], PlayEvent::Preparing { op });
        let downloads = events
            .iter()
            .position(|e| {
                matches!(e, PlayEvent::Downloading { what: TransferDto::GameFiles, count } if *count > 0)
            })
            .unwrap_or_else(|| panic!("{events:?}"));
        let done = events
            .iter()
            .position(|e| {
                *e == PlayEvent::Downloaded {
                    what: TransferDto::GameFiles,
                }
            })
            .unwrap();
        assert!(downloads < done, "{events:?}");
    }
    assert_eq!(fx.running().await, run_id);
    assert_eq!(fx.active().as_deref(), Some(ID));
    fx.line("WINDOW READY").await;
    // Lines carry the run they belong to.
    assert!(fx.lines.lock().unwrap().iter().all(|(r, _)| *r == run_id));
    assert!(fx.lines.lock().unwrap().iter().any(|(_, l)| l == "hello"));
    // Recorded after the start.
    let inst = instances::load(&fx.state.paths, ID).unwrap();
    assert!(inst.config.last_played.is_some());

    assert_eq!(stop(&fx.state.game).await.unwrap(), StopResult::Asked);
    let exit = fx.idle().await.expect("an exit");
    assert_eq!(
        exit,
        LastExit {
            instance: ID.into(),
            run_id,
            code: 0,
            stop_requested: true
        }
    );
    fx.line("CLOSE REQUESTED").await;
    assert_eq!(fx.active(), None);
    let states = fx.states();
    assert!(
        matches!(
            &states[..],
            [
                GameStateDto::Preparing { .. },
                GameStateDto::Running { .. },
                GameStateDto::Stopping { .. },
                GameStateDto::Idle { last_exit: Some(_) }
            ]
        ),
        "{states:?}"
    );
    // After the end nothing is running.
    assert_eq!(stop(&fx.state.game).await.unwrap(), StopResult::NotRunning);
    assert!(!force_stop(&fx.state.game).await.unwrap());
    assert!(!play_cancel_impl(&fx.state, op));
}

#[tokio::test(flavor = "multi_thread")]
async fn second_play_is_busy() {
    let fx = fixture(Login::Valid).await;
    let (blocking, gate) = blocking_preparer();
    fx.state.game.set_preparer(blocking);
    let (_, events) = fx.play().await;
    until("the installer step", || events.lock().unwrap().len() >= 2).await;
    let busy = |r: Result<OperationId, ErrorDto>| r.unwrap_err().code == ErrorCode::Busy;
    let ignored: EventSink<PlayEvent> = Arc::new(|_| panic!("no event for a refused start"));
    assert!(busy(play_impl(&fx.state, ID.into(), ignored.clone()).await));
    gate.release.store(true, Ordering::SeqCst);
    assert!(matches!(ended(&events).await, PlayEvent::Failed { .. }));
    fx.idle().await;

    // And while it runs.
    fx.state.game.set_preparer(core_preparer());
    fx.script("sleep 60000\n");
    let (_, events) = fx.play().await;
    ended(&events).await;
    fx.running().await;
    assert!(busy(play_impl(&fx.state, ID.into(), ignored).await));
    assert!(force_stop(&fx.state.game).await.unwrap());
    fx.idle().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_during_downloads() {
    let fx = fixture(Login::Valid).await;
    // The client jar takes long, once.
    Mock::given(method("GET"))
        .and(path("/mojang/files/client.jar"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(b"client".to_vec())
                .set_delay(Duration::from_secs(30)),
        )
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&fx.server)
        .await;
    fx.script("exit 0\n");
    let (op, events) = fx.play().await;
    until("Downloading", || {
        events
            .lock()
            .unwrap()
            .iter()
            .any(|e| matches!(e, PlayEvent::Downloading { .. }))
    })
    .await;
    assert!(play_cancel_impl(&fx.state, op));
    assert_eq!(ended(&events).await, PlayEvent::Cancelled);
    assert_eq!(fx.idle().await, None);
    assert_eq!(fx.active(), None);
    assert!(!play_cancel_impl(&fx.state, op), "only once");
    // A new start has a fresh flag and gets through.
    let (_, events) = fx.play().await;
    assert!(matches!(ended(&events).await, PlayEvent::Started { .. }));
    assert_eq!(fx.idle().await.unwrap().code, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_during_the_blocking_install_stops_it_through_the_flag() {
    let fx = fixture(Login::Valid).await;
    let (blocking, gate) = blocking_preparer();
    fx.state.game.set_preparer(blocking);
    let (op, events) = fx.play().await;
    until("the installer step", || {
        events.lock().unwrap().iter().any(|e| {
            matches!(
                e,
                PlayEvent::InstallStep {
                    index: 1,
                    total: 3,
                    ..
                }
            )
        })
    })
    .await;
    assert!(play_cancel_impl(&fx.state, op));
    assert_eq!(ended(&events).await, PlayEvent::Cancelled);
    assert_eq!(fx.idle().await, None);
    // The abort does not reach the blocking thread; the flag does.
    until("the flag in the blocking phase", || {
        gate.saw_cancel.load(Ordering::SeqCst)
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn closing_the_window_while_preparing_cancels_the_start() {
    let fx = fixture(Login::Valid).await;
    let (blocking, gate) = blocking_preparer();
    fx.state.game.set_preparer(blocking);
    let (_, events) = fx.play().await;
    until("the installer step", || events.lock().unwrap().len() >= 2).await;
    // What the window's destroy hook and a page reload do.
    fx.state.operations.cancel_all();
    assert_eq!(ended(&events).await, PlayEvent::Cancelled);
    assert_eq!(fx.idle().await, None);
    until("the flag", || gate.saw_cancel.load(Ordering::SeqCst)).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_login_operation_id_does_not_cancel_a_start_and_the_reverse() {
    let fx = fixture(Login::Valid).await;
    let (blocking, gate) = blocking_preparer();
    fx.state.game.set_preparer(blocking);
    let (op, events) = fx.play().await;
    until("the installer step", || events.lock().unwrap().len() >= 2).await;
    assert!(!play_cancel_impl(&fx.state, OperationId(op.0 + 1000)));
    assert!(matches!(
        fx.state.game.state(),
        GameStateDto::Preparing { .. }
    ));
    gate.release.store(true, Ordering::SeqCst);
    ended(&events).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn expired_login_marks_account() {
    let fx = fixture(Login::Expired).await;
    fx.script("exit 0\n");
    let (_, events) = fx.play().await;
    let PlayEvent::Failed { error } = ended(&events).await else {
        panic!("{:?}", events.lock().unwrap());
    };
    assert_eq!(error.code, ErrorCode::LoginExpired);
    assert_eq!(fx.idle().await, None);
    let list = fx.state.accounts.list(&fx.state.settings.get()).unwrap();
    let account = list
        .accounts
        .iter()
        .find(|a| a.id == PLAYER_A.uuid)
        .unwrap();
    assert!(account.sign_in_required);
    assert!(fx
        .app_events
        .lock()
        .unwrap()
        .contains(&AppEvent::DataChanged {
            scope: DataScope::Accounts
        }));
    assert!(!fx
        .states()
        .iter()
        .any(|s| matches!(s, GameStateDto::Running { .. })));
}

#[tokio::test(flavor = "multi_thread")]
async fn exit_code_reaches_the_idle_state() {
    let fx = fixture(Login::Valid).await;
    for code in [0, 3] {
        fx.script(&format!("exit {code}\n"));
        let (_, events) = fx.play().await;
        let PlayEvent::Started { run_id } = ended(&events).await else {
            panic!("{:?}", events.lock().unwrap());
        };
        let expected = LastExit {
            instance: ID.into(),
            run_id,
            code,
            stop_requested: false,
        };
        assert_eq!(fx.idle().await, Some(expected.clone()));
        // The same through `game_state()` and the last event.
        assert_eq!(
            fx.state.game.state(),
            GameStateDto::Idle {
                last_exit: Some(expected.clone())
            }
        );
        assert_eq!(
            fx.states().last(),
            Some(&GameStateDto::Idle {
                last_exit: Some(expected)
            })
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn stop_requested_is_set_when_the_backend_stopped_the_game() {
    let fx = fixture(Login::Valid).await;
    // game_stop: covered with a window in `plays_through_running_and_stops_on_request`, here
    // through a window that ignores the request and then the force stop.
    fx.script("window ignore-close\n");
    let (_, events) = fx.play().await;
    ended(&events).await;
    fx.running().await;
    fx.line("WINDOW READY").await;
    assert_eq!(stop(&fx.state.game).await.unwrap(), StopResult::Asked);
    fx.line("CLOSE IGNORED").await;
    assert!(matches!(
        fx.state.game.state(),
        GameStateDto::Stopping { .. }
    ));
    assert!(force_stop(&fx.state.game).await.unwrap());
    let exit = fx.idle().await.unwrap();
    assert!(exit.stop_requested);
    assert_eq!(exit.code, 1);

    // game_force_stop alone.
    fx.script("sleep 60000\n");
    let (_, events) = fx.play().await;
    ended(&events).await;
    fx.running().await;
    assert!(force_stop(&fx.state.game).await.unwrap());
    let exit = fx.idle().await.unwrap();
    assert!(exit.stop_requested);
    assert_eq!(exit.code, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn stop_without_a_window_is_reported_and_not_counted_as_requested() {
    let fx = fixture(Login::Valid).await;
    fx.script("out up\nsleep 5000\nexit 3\n");
    let (_, events) = fx.play().await;
    ended(&events).await;
    fx.running().await;
    fx.line("up").await;
    assert_eq!(stop(&fx.state.game).await.unwrap(), StopResult::NoWindow);
    assert!(matches!(
        fx.state.game.state(),
        GameStateDto::Running { .. }
    ));
    let exit = fx.idle().await.unwrap();
    assert_eq!(exit.code, 3);
    assert!(!exit.stop_requested);
}

#[tokio::test(flavor = "multi_thread")]
async fn last_exit_is_cleared_by_the_next_start() {
    let fx = fixture(Login::Valid).await;
    fx.script("exit 3\n");
    let (_, events) = fx.play().await;
    ended(&events).await;
    assert_eq!(fx.idle().await.unwrap().code, 3);
    let (blocking, gate) = blocking_preparer();
    fx.state.game.set_preparer(blocking);
    let (_, events) = fx.play().await;
    assert_eq!(
        fx.state.game.state(),
        GameStateDto::Preparing {
            instance: ID.into()
        }
    );
    gate.release.store(true, Ordering::SeqCst);
    assert!(matches!(ended(&events).await, PlayEvent::Failed { .. }));
    assert_eq!(fx.idle().await, None);
    let states = fx.states();
    assert!(
        matches!(
            &states[states.len() - 2..],
            [
                GameStateDto::Preparing { .. },
                GameStateDto::Idle { last_exit: None }
            ]
        ),
        "{states:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_is_refused_while_preparing_or_running() {
    let fx = fixture(Login::Valid).await;
    let refused = |r: Result<(), ErrorDto>| r.unwrap_err().code == ErrorCode::InstanceRunning;
    let (blocking, gate) = blocking_preparer();
    fx.state.game.set_preparer(blocking);
    let (_, events) = fx.play().await;
    assert!(refused(instance_delete_impl(&fx.state, ID.into()).await));
    gate.release.store(true, Ordering::SeqCst);
    ended(&events).await;
    fx.idle().await;

    fx.state.game.set_preparer(core_preparer());
    fx.script("sleep 60000\n");
    let (_, events) = fx.play().await;
    ended(&events).await;
    fx.running().await;
    assert!(refused(instance_delete_impl(&fx.state, ID.into()).await));
    assert!(force_stop(&fx.state.game).await.unwrap());
    // Refused while stopping, too: the game has not exited before the state says so.
    fx.idle().await;
    // Idle: the delete gets past the check (and the fixture's trasher refuses it).
    let after = instance_delete_impl(&fx.state, ID.into())
        .await
        .unwrap_err();
    assert_ne!(after.code, ErrorCode::InstanceRunning);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_start_that_cannot_spawn_returns_to_idle_and_records_nothing() {
    let fx = fixture(Login::Valid).await;
    // Not a program.
    let java = fx.state.paths.root().join("jdk").join("java.exe");
    std::fs::create_dir_all(java.parent().unwrap()).unwrap();
    std::fs::write(&java, b"not a program").unwrap();
    fx.state
        .settings
        .update(|s| {
            s.java_override = Some(java.clone());
            Ok(())
        })
        .unwrap();
    let (_, events) = fx.play().await;
    let PlayEvent::Failed { error } = ended(&events).await else {
        panic!("{:?}", events.lock().unwrap());
    };
    assert!(!error
        .detail
        .contains(&*fx.state.paths.root().to_string_lossy()));
    assert_eq!(fx.idle().await, None);
    assert_eq!(fx.active(), None);
    let inst = instances::load(&fx.state.paths, ID).unwrap();
    assert_eq!(inst.config.last_played, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_panicking_start_returns_to_idle_with_an_internal_error() {
    let fx = fixture(Login::Valid).await;
    fx.state.game.set_preparer(Arc::new(|_| {
        Box::pin(async { panic!("the test preparer panics") })
    }));
    let (_, events) = fx.play().await;
    let PlayEvent::Failed { error } = ended(&events).await else {
        panic!("{:?}", events.lock().unwrap());
    };
    assert_eq!(error.code, ErrorCode::Internal);
    assert_eq!(fx.idle().await, None);
    assert_eq!(fx.active(), None);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unknown_instance_starts_nothing() {
    let fx = fixture(Login::Valid).await;
    let ignored: EventSink<PlayEvent> = Arc::new(|_| panic!("no event"));
    let err = play_impl(&fx.state, "nope".into(), ignored.clone())
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::InstanceNotFound);
    let err = play_impl(&fx.state, "..\\x".into(), ignored)
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::InstanceNotFound);
    assert!(fx.states().is_empty());
    assert_eq!(
        fx.state.game.state(),
        GameStateDto::Idle { last_exit: None }
    );
}

/// Without `dev-offline` (CI runs `cargo test -p hengst-desktop` so) the chosen development
/// account never becomes an offline game: the start looks for a Microsoft account instead.
#[cfg(not(feature = "dev-offline"))]
#[tokio::test(flavor = "multi_thread")]
async fn the_dev_account_never_starts_a_game_without_dev_offline() {
    let fx = fixture(Login::None).await;
    fx.state
        .settings
        .update(|s| {
            s.dev_account = true;
            Ok(())
        })
        .unwrap();
    fx.script("out started\n");
    let (_, events) = fx.play().await;
    let PlayEvent::Failed { error } = ended(&events).await else {
        panic!("{:?}", events.lock().unwrap());
    };
    assert_eq!(error.code, ErrorCode::NoAccount);
    assert_eq!(fx.idle().await, None);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(fx.lines.lock().unwrap().is_empty());
}

#[cfg(feature = "dev-offline")]
#[tokio::test(flavor = "multi_thread")]
async fn the_dev_account_starts_a_game_with_dev_offline() {
    let fx = fixture(Login::None).await;
    fx.state
        .settings
        .update(|s| {
            s.dev_account = true;
            Ok(())
        })
        .unwrap();
    fx.script("out started\nexit 0\n");
    let (_, events) = fx.play().await;
    assert!(matches!(ended(&events).await, PlayEvent::Started { .. }));
    fx.line("started").await;
    assert_eq!(fx.idle().await.unwrap().code, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn no_play_event_or_state_carries_the_token() {
    let fx = fixture(Login::Valid).await;
    fx.script("args\nexit 0\n");
    let (_, events) = fx.play().await;
    ended(&events).await;
    fx.idle().await;
    fx.line("ARG --accessToken").await;
    let mut seen: Vec<String> = events
        .lock()
        .unwrap()
        .iter()
        .map(|e| serde_json::to_string(e).unwrap())
        .collect();
    seen.extend(
        fx.app_events
            .lock()
            .unwrap()
            .iter()
            .map(|e| serde_json::to_string(e).unwrap()),
    );
    seen.push(serde_json::to_string(&fx.state.game.state()).unwrap());
    let lines: Vec<String> = fx
        .lines
        .lock()
        .unwrap()
        .iter()
        .map(|(_, l)| l.clone())
        .collect();
    // The output did carry the arguments around the token.
    assert!(lines.iter().any(|l| l == "ARG --accessToken"), "{lines:?}");
    seen.extend(lines);
    for s in &seen {
        assert!(!s.contains(TOKEN) && !s.contains("Tok+en"), "{s}");
    }
}

#[test]
fn progress_maps_one_to_one_and_keeps_paths_out() {
    let root = Path::new(r"C:\Users\Mira_K\AppData\Roaming\HengstClient");
    assert_eq!(
        play_event(
            Progress::InstallLog {
                path: root.join("logs").join("loader-install-forge-x.log")
            },
            root
        ),
        Some(PlayEvent::InstallLog {
            path: "logs/loader-install-forge-x.log".into()
        })
    );
    assert_eq!(
        play_event(
            Progress::InstallLog {
                path: r"D:\elsewhere\x.log".into()
            },
            root
        ),
        None
    );
    assert_eq!(
        play_event(Progress::InstallLog { path: root.into() }, root),
        None
    );
    let Some(PlayEvent::Notice { message }) = play_event(
        Progress::Notice {
            message: r"waiting for the lock C:\Users\Mira_K\AppData\x.lock: held".into(),
        },
        root,
    ) else {
        panic!()
    };
    assert!(!message.contains("Mira_K"), "{message}");
    assert_eq!(
        play_event(
            Progress::InstallFinished {
                elapsed: Duration::from_millis(1234)
            },
            root
        ),
        Some(PlayEvent::InstallFinished { elapsed_ms: 1234 })
    );
    assert_eq!(
        play_event(
            Progress::Downloading {
                what: Transfer::Java,
                count: 7
            },
            root
        ),
        Some(PlayEvent::Downloading {
            what: TransferDto::Java,
            count: 7
        })
    );
}
