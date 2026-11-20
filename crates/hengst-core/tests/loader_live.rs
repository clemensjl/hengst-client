//! Live launches of the Fabric family against the real services. Run one game at a time:
//! `cargo test -p hengst-core --features dev-offline --test loader_live -- --ignored --test-threads=1`
#![cfg(all(windows, feature = "dev-offline"))]

use hengst_core::args::{Account, AccountKind};
use hengst_core::launch::{ensure_launch_allowed, prepare, Endpoints, LaunchOptions, DEV_OFFLINE};
use hengst_core::loaders::{LoaderKind, LoaderRequest};
use hengst_core::paths::Paths;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

static LOCK: Mutex<()> = Mutex::new(());

const LWJGL_MARKERS: [&str; 2] = ["Backend library: LWJGL version", "LWJGL Version: "];
const START_TIMEOUT: Duration = Duration::from_secs(240);
const SETTLE: Duration = Duration::from_secs(15);

/// Sets the master volume to zero in `<game_dir>/options.txt`, keeping every other line.
fn mute_sound(game_dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(game_dir)?;
    let file = game_dir.join("options.txt");
    let existing = std::fs::read_to_string(&file).unwrap_or_default();
    let mut lines: Vec<String> = Vec::new();
    let mut replaced = false;
    for line in existing.lines() {
        if line.starts_with("soundCategory_master:") {
            if !replaced {
                lines.push("soundCategory_master:0.0".into());
                replaced = true;
            }
        } else {
            lines.push(line.to_string());
        }
    }
    if !replaced {
        lines.push("soundCategory_master:0.0".into());
    }
    std::fs::write(&file, lines.join("\n") + "\n")
}

/// Kills the game and waits for it on every exit path, including panics.
struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn live_home() -> PathBuf {
    match std::env::var_os("HENGST_LIVE_HOME") {
        Some(p) => PathBuf::from(p),
        None => std::env::temp_dir().join("hengst-live"),
    }
}

/// Java processes started from the live home's managed runtimes, i.e. games of earlier runs.
/// Other Java programs on the machine (IDEs, build daemons) are not ours and are ignored.
fn game_processes(home: &Path) -> String {
    let script = format!(
        "Get-CimInstance Win32_Process | Where-Object {{ $_.ExecutablePath -and $_.ExecutablePath.StartsWith('{}', 'OrdinalIgnoreCase') -and $_.Name -like 'java*.exe' }} | ForEach-Object {{ $_.ProcessId.ToString() + ' ' + $_.ExecutablePath }}",
        home.display()
    );
    let out = Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output()
        .expect("powershell runs");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn pump<R: std::io::Read + Send + 'static>(
    r: R,
    log: Arc<Mutex<std::fs::File>>,
    tx: mpsc::Sender<String>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        for line in BufReader::new(r).lines().map_while(Result::ok) {
            if let Ok(mut f) = log.lock() {
                let _ = writeln!(f, "{line}");
            }
            let _ = tx.send(line);
        }
    })
}

// The lock is held on purpose for the whole launch: only one game may run at a time.
#[allow(clippy::await_holding_lock)]
async fn live_launch(version: &str, kind: LoaderKind, memory_mb: u32, banner: Option<&str>) {
    let _serial = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // A killed process can linger for a moment; give it a few seconds before judging.
    let mut running = game_processes(&live_home());
    for _ in 0..10 {
        if running.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_secs(1));
        running = game_processes(&live_home());
    }
    assert!(
        running.is_empty(),
        "a java process from an earlier run is still alive:\n{running}"
    );

    let paths = Paths::new(live_home());
    let opts = LaunchOptions {
        version: version.into(),
        instance: None,
        account: Account::offline("Dev"),
        java: None,
        memory_mb,
        loader: Some(LoaderRequest {
            kind,
            version: None,
        }),
    };
    let t0 = Instant::now();
    let p = prepare(&paths, &opts, &Endpoints::default())
        .await
        .expect("prepare against the live services");
    println!("prepared in {:?}: {}", t0.elapsed(), p.version_id);

    ensure_launch_allowed(AccountKind::Offline, DEV_OFFLINE).expect("dev-offline launch allowed");
    mute_sound(&p.game_dir).expect("mute sound");
    let log_dir = p.game_dir.join("logs");
    std::fs::create_dir_all(&log_dir).unwrap();
    let log = Arc::new(Mutex::new(
        std::fs::File::create(log_dir.join("hengst-live.log")).unwrap(),
    ));

    let mut child = Command::new(&p.java)
        .args(&p.args)
        .current_dir(&p.game_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the game");
    let (tx, rx) = mpsc::channel();
    let _h1 = pump(child.stdout.take().unwrap(), log.clone(), tx.clone());
    let _h2 = pump(child.stderr.take().unwrap(), log.clone(), tx);
    let mut guard = ChildGuard(child);

    let started = Instant::now();
    let mut lwjgl_at: Option<Duration> = None;
    let mut banner_line: Option<String> = None;
    let mut settle_deadline: Option<Instant> = None;
    loop {
        let deadline = settle_deadline.unwrap_or(started + START_TIMEOUT);
        let Some(left) = deadline.checked_duration_since(Instant::now()) else {
            break;
        };
        match rx.recv_timeout(left.min(Duration::from_millis(500))) {
            Ok(line) => {
                if lwjgl_at.is_none() && LWJGL_MARKERS.iter().any(|m| line.contains(m)) {
                    lwjgl_at = Some(started.elapsed());
                    settle_deadline = Some(Instant::now() + SETTLE);
                }
                if let Some(b) = banner {
                    if banner_line.is_none() && line.contains(b) {
                        banner_line = Some(line);
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        if let Ok(Some(status)) = guard.0.try_wait() {
            panic!("game exited early with {status}; see logs/hengst-live.log");
        }
    }

    let lwjgl = lwjgl_at.expect("no LWJGL line within 240 s; see logs/hengst-live.log");
    println!("{version} {kind:?}: LWJGL line after {lwjgl:?}");
    if let Some(b) = banner {
        let line = banner_line.unwrap_or_else(|| panic!("banner {b:?} never appeared"));
        println!("banner line: {line}");
    }
    assert!(
        guard.0.try_wait().unwrap().is_none(),
        "game died within 15 s after start"
    );
    let crashes = p.game_dir.join("crash-reports");
    let n = std::fs::read_dir(&crashes).map(|d| d.count()).unwrap_or(0);
    assert_eq!(n, 0, "crash reports present in {}", crashes.display());
    drop(guard);
}

#[tokio::test]
#[ignore]
async fn fabric_1_21_11() {
    live_launch(
        "1.21.11",
        LoaderKind::Fabric,
        2048,
        Some("with Fabric Loader"),
    )
    .await
}

#[tokio::test]
#[ignore]
async fn fabric_latest() {
    live_launch(
        "latest",
        LoaderKind::Fabric,
        2048,
        Some("with Fabric Loader"),
    )
    .await
}

#[tokio::test]
#[ignore]
async fn quilt_1_21_11() {
    live_launch(
        "1.21.11",
        LoaderKind::Quilt,
        2048,
        Some("with Quilt Loader"),
    )
    .await
}

#[tokio::test]
#[ignore]
async fn legacy_fabric_1_8_9() {
    live_launch(
        "1.8.9",
        LoaderKind::LegacyFabric,
        2048,
        Some("with Fabric Loader"),
    )
    .await
}

#[tokio::test]
#[ignore]
async fn legacy_fabric_1_12_2() {
    live_launch(
        "1.12.2",
        LoaderKind::LegacyFabric,
        2048,
        Some("with Fabric Loader"),
    )
    .await
}

#[test]
fn mute_sound_rewrites_only_master_volume() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("options.txt"),
        "soundCategory_master:1.0\nfov:0.5\n",
    )
    .unwrap();
    mute_sound(dir.path()).unwrap();
    let text = std::fs::read_to_string(dir.path().join("options.txt")).unwrap();
    assert_eq!(text, "soundCategory_master:0.0\nfov:0.5\n");

    // Master volume missing: appended, other lines kept.
    std::fs::write(dir.path().join("options.txt"), "fov:0.5\n").unwrap();
    mute_sound(dir.path()).unwrap();
    let text = std::fs::read_to_string(dir.path().join("options.txt")).unwrap();
    assert_eq!(text, "fov:0.5\nsoundCategory_master:0.0\n");

    // No options.txt yet.
    let fresh = tempfile::tempdir().unwrap();
    let game = fresh.path().join("instances").join("x");
    mute_sound(&game).unwrap();
    assert_eq!(
        std::fs::read_to_string(game.join("options.txt")).unwrap(),
        "soundCategory_master:0.0\n"
    );
}
