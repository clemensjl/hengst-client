//! The game process end to end, with `fake-java` in place of java: the real launch path prepares
//! it (mock Mojang services, a Microsoft account from the account store), `spawn_game` starts it.
//! `fake-java` opens no visible window; the stop test uses a hidden one.
#![cfg(windows)]

use hengst_core::auth::flow::{launch_account, LoginSession};
use hengst_core::auth::minecraft::Profile;
use hengst_core::auth::store::{AccountStore, MemorySecretStore};
use hengst_core::auth::unix_now;
use hengst_core::game::{spawn_game, GameProcess, LineSink};
use hengst_core::gamelog::{Level, LogLine, Stream};
use hengst_core::instances;
use hengst_core::launch::{prepare, run, LaunchOptions, Prepared};
use hengst_core::paths::Paths;
use hengst_core::runlock::is_running;
use hengst_core::secret::Secret;
use hengst_core::Error;
use hengst_testkit::{endpoints, mount_mojang, PLAYER_A};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use wiremock::MockServer;

/// Fake, but with every character that changes in JSON or in a URL.
const TOKEN: &str = "test-eyJhbGciOiJIUzI1NiJ9.QZVJW/Tok+en\"q\\b=x'<&>.QZVJW-tail_end~";
const XUID: &str = "2535400000000001";
const WAIT: Duration = Duration::from_secs(60);

/// Ends a test's game however the test ends: its job has no kill-on-close.
struct Running(GameProcess);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.force_stop();
    }
}

impl std::ops::Deref for Running {
    type Target = GameProcess;
    fn deref(&self) -> &GameProcess {
        &self.0
    }
}

/// Prepares version `9.9` for a Microsoft account with `TOKEN`, with `fake-java` as java and
/// `script` as its script.
async fn prepared(script: &str) -> (tempfile::TempDir, Prepared) {
    let server = MockServer::builder().start().await;
    mount_mojang(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().join("data"));
    std::fs::create_dir_all(paths.root()).unwrap();
    let store = AccountStore::new(paths.accounts_file(), MemorySecretStore::default());
    store
        .save_session(&LoginSession {
            profile: Profile {
                id: PLAYER_A.uuid.into(),
                name: "Mira_K".into(),
            },
            xuid: XUID.into(),
            minecraft_token: Secret::new(TOKEN),
            minecraft_expires_at: u64::MAX,
            refresh_token: Secret::new("test-refresh-game"),
        })
        .unwrap();
    let account = launch_account(&store, None, None, unix_now())
        .await
        .unwrap();
    let opts = LaunchOptions {
        version: "9.9".into(),
        instance: Some("fake-9.9".into()),
        account,
        java: Some(env!("CARGO_BIN_EXE_fake-java").into()),
        memory_mb: 512,
        loader: None,
    };
    let p = prepare(&paths, &opts, &endpoints(&server.uri()))
        .await
        .unwrap();
    assert!(
        p.args.iter().any(|a| a == TOKEN),
        "the mock passes the token"
    );
    std::fs::write(p.game_dir.join("fake-java.txt"), script).unwrap();
    (dir, p)
}

/// A sink that keeps every line and also sends each message to a channel.
fn collector() -> (
    LineSink,
    Arc<Mutex<Vec<LogLine>>>,
    mpsc::UnboundedReceiver<String>,
) {
    let lines = Arc::new(Mutex::new(Vec::new()));
    let (tx, rx) = mpsc::unbounded_channel();
    let kept = lines.clone();
    let sink: LineSink = Arc::new(move |l: LogLine| {
        let _ = tx.send(l.message.clone());
        kept.lock().unwrap().push(l);
    });
    (sink, lines, rx)
}

async fn wait_for_line(rx: &mut mpsc::UnboundedReceiver<String>, text: &str) {
    tokio::time::timeout(WAIT, async {
        while let Some(m) = rx.recv().await {
            if m == text {
                return;
            }
        }
        panic!("output ended before {text:?}");
    })
    .await
    .unwrap_or_else(|_| panic!("no {text:?} within {WAIT:?}"));
}

/// No piece of the token of 8 bytes or more, and none of its encoded forms, is in `text`.
fn assert_no_token(text: &str, what: &str) {
    for w in TOKEN.as_bytes().windows(8) {
        let w = std::str::from_utf8(w).unwrap();
        assert!(!text.contains(w), "{what}: {w:?} leaked");
    }
    for form in ["Tok+en\\\"q\\\\b", "Tok%2Ben%22q%5Cb", "QZVJW"] {
        assert!(!text.contains(form), "{what}: {form:?} leaked");
    }
}

#[tokio::test]
async fn token_never_reaches_the_sink_or_the_file() {
    let (dir, p) = prepared("args\nvariants\n").await;
    let (sink, lines, _rx) = collector();
    let game = Running(spawn_game(&p, sink).await.unwrap());
    assert_eq!(
        tokio::time::timeout(WAIT, game.wait())
            .await
            .unwrap()
            .unwrap(),
        0
    );
    let lines = lines.lock().unwrap().clone();
    let all: String = lines.iter().map(|l| format!("{}\n", l.message)).collect();
    assert_no_token(&all, "sink");
    // The output did flow: the arguments around the token, the not-secret XUID, both streams.
    for expected in [
        "ARG --accessToken",
        "ARG <redacted>",
        &format!("ARG {XUID}"),
        "ARG Mira_K",
        "{\"accessToken\":\"<redacted>\",\"expires\":86400}",
        "GET https://example.invalid/session?token=<redacted>&x=1",
        "Exception: bad token <redacted>",
        // Two writes make one line again before it is redacted.
        "split between writes <redacted> end",
        // Split over two lines: both halves go.
        "wrapped <redacted>",
        "<redacted> continues",
    ] {
        assert!(
            lines.iter().any(|l| l.message == expected),
            "{expected}\n{all}"
        );
    }
    assert!(lines.iter().any(|l| l.stream == Stream::Stderr));
    let cut = lines
        .iter()
        .find(|l| l.message.starts_with("xxxx"))
        .unwrap();
    assert!(cut
        .message
        .ends_with("<redacted> [line cut: longer than 8 KiB]"));

    let file = std::fs::read_to_string(game.log_path().unwrap()).unwrap();
    assert_no_token(&file, "log file");
    assert!(
        file.contains("ARG --accessToken\nARG <redacted>\n"),
        "{file}"
    );
    assert_no_token(&format!("{:?} {:?}", *game, p), "Debug");
    assert!(game
        .log_path()
        .unwrap()
        .starts_with(dir.path().join("data").join("logs").join("game")));
}

#[tokio::test]
async fn lines_are_numbered_across_both_streams_and_parsed() {
    let (_dir, p) = prepared(
        "out first\nerr second\nxml WARN|Render thread|Hello <world>\nerr third\n\
         out [12:00:00] [main/INFO]: plain line\nexit 3\n",
    )
    .await;
    let (sink, lines, _rx) = collector();
    let game = Running(spawn_game(&p, sink).await.unwrap());
    assert_eq!(
        tokio::time::timeout(WAIT, game.wait())
            .await
            .unwrap()
            .unwrap(),
        3
    );
    let lines = lines.lock().unwrap().clone();
    let seqs: Vec<u64> = lines.iter().map(|l| l.seq).collect();
    assert_eq!(seqs, (1..=5).collect::<Vec<u64>>(), "{lines:#?}");
    let by = |m: &str| lines.iter().find(|l| l.message == m).unwrap().clone();
    assert_eq!(by("first").stream, Stream::Stdout);
    assert_eq!(by("second").stream, Stream::Stderr);
    assert_eq!(by("third").stream, Stream::Stderr);
    let event = by("Hello <world>");
    assert_eq!(event.level, Some(Level::Warn));
    assert_eq!(event.thread.as_deref(), Some("Render thread"));
    assert_eq!(by("plain line").level, Some(Level::Info));
    // Order within one stream is kept.
    assert!(by("second").seq < by("third").seq);
    assert!(by("first").seq < by("Hello <world>").seq);
}

#[tokio::test]
async fn graceful_stop_then_force_stop() {
    // A game that closes when asked.
    let (_dir, p) = prepared("window\nout closed\n").await;
    let (sink, lines, mut rx) = collector();
    let game = Running(spawn_game(&p, sink).await.unwrap());
    wait_for_line(&mut rx, "WINDOW READY").await;
    assert!(game.request_stop().unwrap() >= 1);
    assert_eq!(
        tokio::time::timeout(WAIT, game.wait())
            .await
            .unwrap()
            .unwrap(),
        0
    );
    let messages: Vec<String> = lines
        .lock()
        .unwrap()
        .iter()
        .map(|l| l.message.clone())
        .collect();
    assert!(
        messages.contains(&"CLOSE REQUESTED".to_string()),
        "{messages:?}"
    );
    assert!(messages.contains(&"closed".to_string()), "{messages:?}");
    // After the end both are harmless, any number of times.
    assert_eq!(game.request_stop().unwrap(), 0);
    game.force_stop().unwrap();
    game.force_stop().unwrap();
    assert_eq!(game.wait().await.unwrap(), 0);

    // A game that ignores the request: only the forced stop ends it.
    let (_dir, p) = prepared("window ignore-close\n").await;
    let (sink, _lines, mut rx) = collector();
    let game = Running(spawn_game(&p, sink).await.unwrap());
    wait_for_line(&mut rx, "WINDOW READY").await;
    assert!(game.request_stop().unwrap() >= 1);
    wait_for_line(&mut rx, "CLOSE IGNORED").await;
    assert!(
        tokio::time::timeout(Duration::from_secs(1), game.wait())
            .await
            .is_err(),
        "the game ended although it ignored the request"
    );
    game.force_stop().unwrap();
    assert_eq!(
        tokio::time::timeout(WAIT, game.wait())
            .await
            .unwrap()
            .unwrap(),
        1
    );
    assert_eq!(game.request_stop().unwrap(), 0);
    game.force_stop().unwrap();
}

/// Ends a fake-java that waits for `release` (script `wait release`) however the test ends.
struct Release(std::path::PathBuf);

impl Release {
    fn new(game_dir: &std::path::Path) -> Release {
        Release(game_dir.join("release"))
    }
    fn now(&self) {
        std::fs::write(&self.0, b"").unwrap();
    }
}

impl Drop for Release {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.0, b"");
    }
}

async fn until_free(paths: &Paths, id: &str) {
    tokio::time::timeout(WAIT, async {
        while is_running(paths, id).unwrap() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("the lock outlived the game");
}

/// A game outlives the launcher that started it (closed, crashed, killed). Its run lock goes with
/// the game, not with the launcher: once the launcher's state for it is gone (here: the
/// `GameProcess` dropped, which neither stops the game nor holds the lock), the instance still
/// counts as running. A second start and a delete are refused until the game exits; then the lock
/// is free without anyone cleaning up.
#[tokio::test]
async fn the_run_lock_stays_with_the_game_and_ends_with_it() {
    let (dir, p) = prepared(
        "out started
wait release
",
    )
    .await;
    let release = Release::new(&p.game_dir);
    let paths = Paths::new(dir.path().join("data"));
    let id = "fake-9.9";
    // An unreadable instance may be deleted: only the run lock stands in the way.
    std::fs::write(p.game_dir.join("instance.json"), "{ not json").unwrap();
    assert!(!is_running(&paths, id).unwrap());
    let (sink, _lines, mut rx) = collector();
    let game = spawn_game(&p, sink).await.unwrap();
    wait_for_line(&mut rx, "started").await;
    drop(game);

    assert!(
        is_running(&paths, id).unwrap(),
        "the lock went with the launcher"
    );
    let err = spawn_game(&p, Arc::new(|_| {})).await.unwrap_err();
    assert!(
        matches!(&err, Error::InstanceRunning { id: i } if i == id),
        "{err}"
    );
    let trashed = AtomicBool::new(false);
    let err = instances::delete(&paths, id, &|_| {
        trashed.store(true, Ordering::SeqCst);
        Ok(())
    })
    .unwrap_err();
    assert!(
        matches!(&err, Error::InstanceRunning { id: i } if i == id),
        "{err}"
    );
    assert!(!trashed.load(Ordering::SeqCst));

    // The game ends: the lock is free again.
    release.now();
    until_free(&paths, id).await;
    std::fs::write(
        p.game_dir.join("fake-java.txt"),
        "exit 0
",
    )
    .unwrap();
    let game = Running(spawn_game(&p, Arc::new(|_| {})).await.unwrap());
    assert_eq!(
        tokio::time::timeout(WAIT, game.wait())
            .await
            .unwrap()
            .unwrap(),
        0
    );
}

/// The CLI's `launch::run` takes the same run lock: refused while a game of the instance runs,
/// and while its own game runs a start through `spawn_game` is refused. The lock is handed to its
/// game, so it holds even if the CLI is killed.
#[tokio::test]
async fn the_cli_launch_takes_the_run_lock_too() {
    let (dir, p) = prepared(
        "out started
wait release
",
    )
    .await;
    let paths = Paths::new(dir.path().join("data"));
    let id = "fake-9.9";
    let release = Release::new(&p.game_dir);
    let (sink, _lines, mut rx) = collector();
    let game = Running(spawn_game(&p, sink).await.unwrap());
    wait_for_line(&mut rx, "started").await;
    let err = run(&p).await.unwrap_err();
    assert!(
        matches!(&err, Error::InstanceRunning { id: i } if i == id),
        "{err}"
    );
    release.now();
    assert_eq!(
        tokio::time::timeout(WAIT, game.wait())
            .await
            .unwrap()
            .unwrap(),
        0
    );
    until_free(&paths, id).await;

    std::fs::remove_file(&release.0).unwrap();
    let cli = {
        let p = p.clone();
        tokio::spawn(async move { run(&p).await })
    };
    tokio::time::timeout(WAIT, async {
        while !is_running(&paths, id).unwrap() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the CLI's game does not hold the lock");
    let err = spawn_game(&p, Arc::new(|_| {})).await.unwrap_err();
    assert!(
        matches!(&err, Error::InstanceRunning { id: i } if i == id),
        "{err}"
    );
    // The CLI goes away (as if killed): the lock was handed to its game and stays with it.
    cli.abort();
    assert!(cli.await.unwrap_err().is_cancelled());
    assert!(
        is_running(&paths, id).unwrap(),
        "the lock stayed with the CLI"
    );
    release.now();
    until_free(&paths, id).await;
}

/// A game that ends at once (before or during the hand-over of the lock) reports its exit code,
/// never the failed hand-over. Repeated, since the exit may come before or after the hand-over.
#[tokio::test]
async fn a_cli_game_that_exits_at_once_reports_its_exit_code() {
    let (_dir, p) = prepared(
        "exit 3
",
    )
    .await;
    for _ in 0..20 {
        let code = tokio::time::timeout(WAIT, run(&p)).await.unwrap().unwrap();
        assert_eq!(code, 3);
    }
}
