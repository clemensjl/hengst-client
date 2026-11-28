//! Live launches of the Fabric family, Forge and NeoForge against the real services. Run one game
//! at a time:
//! `cargo test -p hengst-core --features dev-offline --test loader_live -- --ignored --test-threads=1`
//!
//! Forge and NeoForge are prepared twice before the start: a modern build must run its installer
//! processors once (the marker of an earlier run is removed to observe that) and start none on
//! the second prepare; legacy and spec-0 builds must start none and leave no marker.
//!
//! A launch passes when, in this order, the game prints an LWJGL line (window and GL context
//! exist), prints a late-loading marker (texture atlases created after the resource reload:
//! `blocks.png-atlas` on modern versions, `textures-atlas` on 1.8.9 and 1.12.2,
//! `textures/blocks-atlas` on 1.7.10, where the 16x16 placeholder atlases created before mod
//! loading do not count), the loader banner has appeared, the process is still alive 15 s after the late marker, and no new file
//! appeared in `crash-reports`. That proves the loader, libraries, natives and arguments are
//! sound up to the end of initial resource loading. It does not prove the title screen is drawn
//! (no pixels are checked), that worlds load, or that mods work; a crash later than 15 s after
//! the marker is not seen.
#![cfg(all(windows, feature = "dev-offline"))]

use hengst_core::args::{Account, AccountKind};
use hengst_core::launch::{
    ensure_launch_allowed, prepare_with_runner, Endpoints, LaunchOptions, Prepared, DEV_OFFLINE,
};
use hengst_core::loaders::forge;
use hengst_core::loaders::{LoaderKind, LoaderRequest};
use hengst_core::paths::Paths;
use hengst_core::process::{ProcessRunner, SystemRunner};
use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

static LOCK: Mutex<()> = Mutex::new(());

const LWJGL_MARKERS: [&str; 2] = ["Backend library: LWJGL version", "LWJGL Version: "];
const LOADED_MARKERS: [&str; 3] = [
    "blocks.png-atlas",
    "textures-atlas",
    "textures/blocks-atlas",
];
/// 1.7.10 creates empty 16x16 atlases before the mods are loaded.
const PLACEHOLDER_ATLAS: &str = "Created: 16x16 ";
/// Lines worth printing as evidence of which loader started.
const LOADER_HINTS: [&str; 6] = [
    "ModLauncher",
    "Forge Mod Loader",
    "Forge mod loading",
    "NeoForge",
    "Launching target",
    "FML",
];
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

/// Java processes started from inside the live home, i.e. games of earlier runs. Other Java
/// programs on the machine (IDEs, build daemons) are not ours and are ignored. The home path is
/// handed over through the environment so no character in it can break the script.
fn game_processes(home: &Path) -> String {
    let mut prefix = home.to_string_lossy().into_owned();
    if !prefix.ends_with('\\') {
        prefix.push('\\');
    }
    let script = "Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -and $_.ExecutablePath.StartsWith($env:HENGST_LIVE_PREFIX, 'OrdinalIgnoreCase') -and $_.Name -like 'java*.exe' } | ForEach-Object { $_.ProcessId.ToString() + ' ' + $_.ExecutablePath }";
    let out = Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .env("HENGST_LIVE_PREFIX", prefix)
        .output()
        .expect("powershell runs");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn crash_reports(dir: &Path) -> BTreeSet<String> {
    std::fs::read_dir(dir)
        .map(|d| {
            d.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
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

/// Starts every processor through the real runner and counts the starts.
#[derive(Default)]
struct CountingRunner(AtomicUsize);

impl ProcessRunner for CountingRunner {
    fn run(&self, cmd: &mut Command, timeout: Duration) -> std::io::Result<Option<i32>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        SystemRunner.run(cmd, timeout)
    }
}

/// Prepares once and returns the result with the number of processors started.
async fn prepare_counted(paths: &Paths, opts: &LaunchOptions) -> (Prepared, usize, Duration) {
    let runner = Arc::new(CountingRunner::default());
    let t0 = Instant::now();
    let p = prepare_with_runner(paths, opts, &Endpoints::default(), runner.clone())
        .await
        .expect("prepare against the live services");
    (p, runner.0.load(Ordering::SeqCst), t0.elapsed())
}

/// Free physical memory in MB.
fn free_memory_mb() -> u64 {
    let out = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "(Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory",
        ])
        .output()
        .expect("powershell runs");
    let kb: u64 = String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .expect("free memory in KB");
    kb / 1024
}

/// Another agent may run one game of its own; never start ours below 3 GB free memory. Waits up
/// to 10 minutes for memory to come back.
fn wait_for_memory() {
    const NEEDED_MB: u64 = 3 * 1024;
    let start = Instant::now();
    loop {
        let free = free_memory_mb();
        if free >= NEEDED_MB {
            println!("free memory before the launch: {free} MB");
            return;
        }
        assert!(
            start.elapsed() < Duration::from_secs(600),
            "only {free} MB free memory after waiting 10 minutes"
        );
        println!("only {free} MB free memory; waiting");
        std::thread::sleep(Duration::from_secs(15));
    }
}

/// What the processor stage of a Forge or NeoForge build looks like.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Install {
    /// Fabric family: nothing to install.
    None,
    /// Legacy and spec-0 Forge: no processors, no marker.
    NoProcessors,
    /// Modern Forge and NeoForge: processors on the first prepare, none on the next.
    Processors,
}

async fn live_launch(version: &str, kind: LoaderKind, memory_mb: u32, banner: Option<&str>) {
    live_launch_with(version, kind, None, Install::None, memory_mb, banner).await
}

// The lock is held on purpose for the whole launch: only one game may run at a time.
#[allow(clippy::await_holding_lock)]
async fn live_launch_with(
    version: &str,
    kind: LoaderKind,
    loader_version: Option<&str>,
    install: Install,
    memory_mb: u32,
    banner: Option<&str>,
) {
    let _serial = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // Offline accounts may only start the game in dev-offline builds; fail before any download.
    ensure_launch_allowed(AccountKind::Offline, DEV_OFFLINE).expect("dev-offline launch allowed");
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
            version: loader_version.map(String::from),
        }),
    };
    let (mut p, started, took) = prepare_counted(&paths, &opts).await;
    println!(
        "prepared in {took:?}: {} ({started} processors started)",
        p.version_id
    );
    let marker = forge::marker_path(&paths, kind, &p.version_id);
    match install {
        Install::None => assert_eq!(started, 0),
        Install::NoProcessors => {
            assert_eq!(started, 0, "this build has no processor stage");
            assert!(!marker.exists(), "no install marker without processors");
        }
        Install::Processors => {
            assert!(marker.is_file(), "install marker {}", marker.display());
            if started == 0 {
                // Installed by an earlier run: make the stage run once more to observe it.
                std::fs::remove_file(&marker).unwrap();
                let (again, started, took) = prepare_counted(&paths, &opts).await;
                println!(
                    "re-installed after removing the marker in {took:?}: {started} processors"
                );
                assert!(started > 0, "processors ran after the marker was removed");
                assert!(marker.is_file());
                assert_eq!(again.args, p.args);
                p = again;
            }
            let log = forge::install_log(&paths, kind, &p.version_id);
            let headers = std::fs::read_to_string(&log)
                .unwrap()
                .lines()
                .filter(|l| l.starts_with("== processor "))
                .map(|l| l.split_once("): ").map_or(l, |(h, _)| h).to_string() + ")")
                .collect::<Vec<_>>();
            println!("install log {}: {headers:?}", log.display());
            let (second, started, took) = prepare_counted(&paths, &opts).await;
            println!("second prepare in {took:?}: {started} processors started");
            assert_eq!(started, 0, "a valid install skips the processor stage");
            assert_eq!(second.args, p.args);
        }
    }

    wait_for_memory();
    mute_sound(&p.game_dir).expect("mute sound");
    let log_dir = p.game_dir.join("logs");
    std::fs::create_dir_all(&log_dir).unwrap();
    let log = Arc::new(Mutex::new(
        std::fs::File::create(log_dir.join("hengst-live.log")).unwrap(),
    ));
    // Crash reports of earlier runs stay as evidence; only new ones fail this run.
    let crashes = p.game_dir.join("crash-reports");
    let crashes_before = crash_reports(&crashes);

    let mut guard = ChildGuard(
        Command::new(&p.java)
            .args(&p.args)
            .current_dir(&p.game_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn the game"),
    );
    let (tx, rx) = mpsc::channel();
    let _h1 = pump(guard.0.stdout.take().unwrap(), log.clone(), tx.clone());
    let _h2 = pump(guard.0.stderr.take().unwrap(), log.clone(), tx);

    let started = Instant::now();
    let mut lwjgl_at: Option<Duration> = None;
    let mut loaded_at: Option<Duration> = None;
    let mut banner_line: Option<String> = None;
    let mut loader_lines: Vec<String> = Vec::new();
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
                }
                if lwjgl_at.is_some()
                    && loaded_at.is_none()
                    && line.contains("Created: ")
                    && !line.contains(PLACEHOLDER_ATLAS)
                    && LOADED_MARKERS.iter().any(|m| line.contains(m))
                {
                    loaded_at = Some(started.elapsed());
                    settle_deadline = Some(Instant::now() + SETTLE);
                }
                if loader_lines.len() < 8
                    && !line.contains("<log4j:Event")
                    && LOADER_HINTS.iter().any(|h| line.contains(h))
                {
                    loader_lines.push(line.clone());
                }
                if let Some(b) = banner {
                    if banner_line.is_none() && banner_matches(&line, b) {
                        banner_line = Some(line);
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // Both pipes closed: the game is gone or closing. Never a pass.
                let status = guard.0.wait().expect("wait for the game");
                panic!("game exited with {status} before the settle window ended; see logs/hengst-live.log");
            }
        }
        if let Ok(Some(status)) = guard.0.try_wait() {
            panic!("game exited early with {status}; see logs/hengst-live.log");
        }
    }

    let lwjgl = lwjgl_at.expect("no LWJGL line within 240 s; see logs/hengst-live.log");
    let loaded = loaded_at.expect("no late-loading marker (atlas created) within 240 s");
    println!("{version} {kind:?}: LWJGL line after {lwjgl:?}, loaded marker after {loaded:?}");
    for l in &loader_lines {
        println!("loader line: {l}");
    }
    if let Some(b) = banner {
        let line = banner_line.unwrap_or_else(|| panic!("banner {b:?} never appeared"));
        println!("banner line: {line}");
    }
    assert!(
        guard.0.try_wait().unwrap().is_none(),
        "game died within 15 s after the loaded marker"
    );
    let new: Vec<_> = crash_reports(&crashes)
        .difference(&crashes_before)
        .cloned()
        .collect();
    assert!(
        new.is_empty(),
        "new crash reports in {}: {new:?}",
        crashes.display()
    );
    let killed = Instant::now();
    drop(guard);
    println!("guard kill and wait took {:?}", killed.elapsed());
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

// Loader evidence lines, taken from the first real run of each family:
// - Forge 1.13 and later: FML's "Forge mod loading, version <build>, for MC <game>" (after
//   ModLauncher has started the `forgeclient` target).
// - NeoForge: "NeoForge mod loading, version <build>, for MC <game>".
// - Forge before 1.13 (LaunchWrapper with FMLTweaker): "Forge Mod Loader version <build> for
//   Minecraft <game> loading".

const MODERN_FORGE: &str = "Forge mod loading, version ";
const LEGACY_FORGE: &str = "Forge Mod Loader version ";
const NEOFORGE: &str = "NeoForge mod loading, version ";

/// The Forge banners are substrings of the NeoForge one: a line that names NeoForge is evidence
/// for NeoForge only. (The NeoForge banner itself contains "NeoForge" by construction.)
fn banner_matches(line: &str, banner: &str) -> bool {
    line.contains(banner) && (banner == NEOFORGE || !line.contains("NeoForge"))
}

#[test]
fn a_neoforge_line_is_not_a_forge_banner() {
    let neo = "NeoForge mod loading, version 21.1.267, for MC 1.21.1";
    let forge = "Forge mod loading, version 47.4.28, for MC 1.20.1";
    assert!(banner_matches(neo, NEOFORGE));
    assert!(!banner_matches(neo, MODERN_FORGE));
    assert!(banner_matches(forge, MODERN_FORGE));
    assert!(!banner_matches(forge, NEOFORGE));
}

#[tokio::test]
#[ignore]
async fn forge_1_7_10() {
    live_launch_with(
        "1.7.10",
        LoaderKind::Forge,
        None,
        Install::NoProcessors,
        2048,
        Some(LEGACY_FORGE),
    )
    .await
}

#[tokio::test]
#[ignore]
async fn forge_1_8_9() {
    live_launch_with(
        "1.8.9",
        LoaderKind::Forge,
        None,
        Install::NoProcessors,
        2048,
        Some(LEGACY_FORGE),
    )
    .await
}

/// The recommended 1.12.2 build (14.23.5.2859, install profile spec 0).
#[tokio::test]
#[ignore]
async fn forge_1_12_2() {
    live_launch_with(
        "1.12.2",
        LoaderKind::Forge,
        None,
        Install::NoProcessors,
        2048,
        Some(LEGACY_FORGE),
    )
    .await
}

/// The last 1.12.2 build with the legacy installer format.
#[tokio::test]
#[ignore]
async fn forge_1_12_2_legacy_format() {
    live_launch_with(
        "1.12.2",
        LoaderKind::Forge,
        Some("14.23.5.2847"),
        Install::NoProcessors,
        2048,
        Some(LEGACY_FORGE),
    )
    .await
}

#[tokio::test]
#[ignore]
async fn forge_1_16_5() {
    live_launch_with(
        "1.16.5",
        LoaderKind::Forge,
        None,
        Install::Processors,
        2048,
        Some(MODERN_FORGE),
    )
    .await
}

#[tokio::test]
#[ignore]
async fn forge_1_20_1() {
    live_launch_with(
        "1.20.1",
        LoaderKind::Forge,
        None,
        Install::Processors,
        3072,
        Some(MODERN_FORGE),
    )
    .await
}

#[tokio::test]
#[ignore]
async fn forge_latest() {
    live_launch_with(
        "latest",
        LoaderKind::Forge,
        None,
        Install::Processors,
        3072,
        Some(MODERN_FORGE),
    )
    .await
}

#[tokio::test]
#[ignore]
async fn neoforge_1_20_1() {
    live_launch_with(
        "1.20.1",
        LoaderKind::NeoForge,
        None,
        Install::Processors,
        3072,
        Some(NEOFORGE),
    )
    .await
}

#[tokio::test]
#[ignore]
async fn neoforge_1_21_1() {
    live_launch_with(
        "1.21.1",
        LoaderKind::NeoForge,
        None,
        Install::Processors,
        3072,
        Some(NEOFORGE),
    )
    .await
}

#[tokio::test]
#[ignore]
async fn neoforge_latest() {
    live_launch_with(
        "latest",
        LoaderKind::NeoForge,
        None,
        Install::Processors,
        3072,
        Some(NEOFORGE),
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

    // Duplicate master volume lines collapse to exactly one.
    std::fs::write(
        dir.path().join("options.txt"),
        "soundCategory_master:1.0\nfov:0.5\nsoundCategory_master:0.3\n",
    )
    .unwrap();
    mute_sound(dir.path()).unwrap();
    let text = std::fs::read_to_string(dir.path().join("options.txt")).unwrap();
    assert_eq!(text, "soundCategory_master:0.0\nfov:0.5\n");

    // No options.txt yet.
    let fresh = tempfile::tempdir().unwrap();
    let game = fresh.path().join("instances").join("x");
    mute_sound(&game).unwrap();
    assert_eq!(
        std::fs::read_to_string(game.join("options.txt")).unwrap(),
        "soundCategory_master:0.0\n"
    );
}
