//! Child processes the launcher waits for (the installer processors): started inside a job
//! object without a console window, stopped as a whole tree on timeout or cancellation.

use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::loaders::forge::processors::ProcessRunner;

#[cfg(windows)]
pub(crate) mod job;
#[cfg(windows)]
pub use job::Job;

/// How often a running process is checked for exit, timeout and cancellation.
const POLL: Duration = Duration::from_millis(100);

/// Shared "stop now" request. Clones share the flag; once set it stays set.
#[derive(Clone, Debug, Default)]
pub struct CancelFlag(Arc<AtomicBool>);

impl CancelFlag {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Runs each process like `SystemRunner` and also stops it when `cancel` is set: the flag is
/// checked before the start and every 100 ms while it runs; then the whole process tree is
/// terminated and the run fails with `ErrorKind::Interrupted`, which `run_processors` reports as
/// `Error::Cancelled`.
#[derive(Clone, Debug)]
pub struct JobRunner {
    cancel: CancelFlag,
}

impl JobRunner {
    pub fn new(cancel: CancelFlag) -> Self {
        Self { cancel }
    }
}

impl ProcessRunner for JobRunner {
    fn run(&self, cmd: &mut Command, timeout: Duration) -> std::io::Result<Option<i32>> {
        run_in_job(cmd, timeout, Some(&self.cancel))
    }
}

/// Starts `cmd` in its own kill-on-close job without a console window and waits for it: its exit
/// code (-1 if it has none), `None` after `timeout` or `Err(Interrupted)` after `cancel` (the
/// whole tree terminated in both cases). Piped stdout and stderr are read and discarded while it
/// runs, so a child writing more than a pipe holds cannot block. When the child has exited, the
/// job is closed, which also ends any process it left behind.
pub(crate) fn run_in_job(
    cmd: &mut Command,
    timeout: Duration,
    cancel: Option<&CancelFlag>,
) -> std::io::Result<Option<i32>> {
    let cancelled = || cancel.is_some_and(CancelFlag::is_cancelled);
    if cancelled() {
        return Err(std::io::ErrorKind::Interrupted.into());
    }
    let (tree, mut child) = spawn_tree(cmd)?;
    let drains = [
        child.stdout.take().map(drain),
        child.stderr.take().map(drain),
    ];
    let deadline = Instant::now() + timeout;
    let outcome = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(Some(status.code().unwrap_or(-1))),
            Ok(None) => {}
            Err(e) => break Err(e),
        }
        if cancelled() {
            break Err(std::io::ErrorKind::Interrupted.into());
        }
        if Instant::now() >= deadline {
            break Ok(None);
        }
        std::thread::sleep(POLL);
    };
    if !matches!(outcome, Ok(Some(_))) {
        kill_tree(&tree);
        let _ = child.kill();
        let _ = child.wait();
    }
    // Ends whatever the child left running; only then do the pipes close and the readers end.
    drop(tree);
    for reader in drains.into_iter().flatten() {
        let _ = reader.join();
    }
    outcome
}

/// Reads a child's pipe to the end and discards it.
fn drain(mut pipe: impl std::io::Read + Send + 'static) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let _ = std::io::copy(&mut pipe, &mut std::io::sink());
    })
}

/// The child in a new kill-on-close job, started through `Job::spawn` (no console window).
#[cfg(windows)]
fn spawn_tree(cmd: &mut Command) -> std::io::Result<(Job, Child)> {
    let job = Job::create()?;
    let child = job.spawn(cmd)?;
    Ok((job, child))
}

#[cfg(windows)]
fn kill_tree(job: &Job) {
    job.terminate(1);
}

/// Outside Windows (not a supported platform) there is no job: only the child itself is stopped.
#[cfg(not(windows))]
fn spawn_tree(cmd: &mut Command) -> std::io::Result<((), Child)> {
    Ok(((), cmd.spawn()?))
}

#[cfg(not(windows))]
fn kill_tree(_: &()) {}

#[cfg(all(test, windows))]
pub(crate) mod tests {
    use super::job::tests::{ends_within, open_process, powershell, read_tagged, DEATH};
    use super::*;
    use std::process::Stdio;

    #[test]
    fn cancel_flag_is_shared_by_clones() {
        let a = CancelFlag::new();
        let b = a.clone();
        assert!(!b.is_cancelled());
        a.cancel();
        assert!(b.is_cancelled());
    }

    #[test]
    fn cancelled_before_the_start_starts_nothing() {
        let cancel = CancelFlag::new();
        cancel.cancel();
        // The program does not exist: had it been started, the error would be NotFound.
        let err = JobRunner::new(cancel)
            .run(
                &mut Command::new("hengst-no-such-program-xyz.exe"),
                Duration::from_secs(30),
            )
            .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::Interrupted);
    }

    #[test]
    fn cancel_terminates_the_running_tree() {
        let cancel = CancelFlag::new();
        let runner = JobRunner::new(cancel.clone());
        let dir = tempfile::tempdir().unwrap();
        let pid_file = dir.path().join("pid.txt");
        // cmd.exe starts powershell, which records its id and sleeps.
        let mut cmd = Command::new("cmd");
        // Passed verbatim: cmd.exe does not read the `\"` escaping `arg` would use.
        std::os::windows::process::CommandExt::raw_arg(
            &mut cmd,
            "/d /c powershell -NoProfile -NonInteractive -c \"Set-Content -LiteralPath $env:HENGST_PID_FILE -Value $PID; Start-Sleep 60\"",
        )
        .env("HENGST_PID_FILE", &pid_file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
        let watcher = std::thread::spawn(move || {
            let pid = wait_for_pid(&pid_file);
            let grandchild = open_process(pid);
            cancel.cancel();
            ends_within(&grandchild, DEATH)
        });
        let start = Instant::now();
        let err = runner.run(&mut cmd, Duration::from_secs(120)).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::Interrupted);
        assert!(
            start.elapsed() < Duration::from_secs(60),
            "cancel was not seen"
        );
        assert!(
            watcher.join().unwrap(),
            "the grandchild survived the cancel"
        );
    }

    /// Waits (up to a minute) until a test child has written its process id to `file`.
    pub(crate) fn wait_for_pid(file: &std::path::Path) -> u32 {
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            if let Some(pid) = std::fs::read_to_string(file)
                .ok()
                .and_then(|s| s.trim().parse().ok())
            {
                return pid;
            }
            assert!(
                Instant::now() < deadline,
                "no process id in {}",
                file.display()
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    #[test]
    fn chatty_piped_child_cannot_block_on_a_full_pipe() {
        // About 2 MB on each stream, far more than a pipe buffer holds.
        let mut cmd = powershell(
            "[Console]::Out.Write('x' * 2000000); [Console]::Error.Write('y' * 2000000); exit 4",
        );
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        let outcome = JobRunner::new(CancelFlag::new())
            .run(&mut cmd, Duration::from_secs(120))
            .unwrap();
        assert_eq!(outcome, Some(4));
    }

    #[test]
    fn cancel_stops_a_chatty_piped_child() {
        let cancel = CancelFlag::new();
        let mut cmd = powershell("while ($true) { [Console]::Out.Write('x' * 100000) }");
        cmd.stdout(Stdio::piped());
        let flag = cancel.clone();
        let t = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(2));
            flag.cancel();
        });
        let start = Instant::now();
        let err = JobRunner::new(cancel)
            .run(&mut cmd, Duration::from_secs(120))
            .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::Interrupted);
        assert!(start.elapsed() < Duration::from_secs(60));
        t.join().unwrap();
    }

    /// The child is created with `CREATE_NO_WINDOW` (`SPAWN_FLAGS`, the only flags `Job::spawn`
    /// sets), and a real child started through the runner has no console window at all
    /// (`GetConsoleWindow()` is NULL; without the flag it would share the runner's console, also
    /// under ConPTY or a headless runner, where that window is merely hidden).
    #[test]
    fn no_console_window_flag_is_set() {
        use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;
        assert_eq!(job::SPAWN_FLAGS & CREATE_NO_WINDOW, CREATE_NO_WINDOW);
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("console.txt");
        let mut cmd = powershell(
            "Add-Type -Namespace HengstTest -Name Con -MemberDefinition '\
             [DllImport(\"kernel32.dll\")] public static extern System.IntPtr GetConsoleWindow();'; \
             $h = [HengstTest.Con]::GetConsoleWindow(); \
             Set-Content -LiteralPath $env:HENGST_OUT -Value ('CONSOLE ' + ($h -ne [System.IntPtr]::Zero))",
        );
        cmd.env("HENGST_OUT", &out)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let code = JobRunner::new(CancelFlag::new())
            .run(&mut cmd, Duration::from_secs(120))
            .unwrap();
        assert_eq!(code, Some(0));
        let text = std::fs::read_to_string(&out).unwrap();
        assert_eq!(read_tagged(text.as_bytes(), "CONSOLE "), "False", "{text}");
    }
}
