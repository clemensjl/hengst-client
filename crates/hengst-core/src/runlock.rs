//! The run lock of an instance: `<root>/tmp/run-<id>.lock`, held for exactly as long as the game
//! process of the instance lives, whatever happens to the launcher that started it (closed,
//! crashed, killed, restarted).
//!
//! How: `spawn_game` opens the file without any sharing and, while the new game process is still
//! suspended (`Job::spawn_with`), duplicates that handle into the game process; then it closes its
//! own. The game never uses the handle. Windows closes it when the game process exits, and only
//! then does the file open again. Nothing is inherited: the launcher's handle is not inheritable
//! (so no other process the launcher starts gets a copy) and the copy in the game is not either (so
//! the game's own child processes do not keep the lock alive).
//!
//! Held means "the file cannot be opened right now". A lock file that exists but opens (left over
//! after a reboot or a crash before the hand-over) is not held; such files are never deleted, an
//! empty file in `tmp` costs nothing.
//!
//! The hand-over happens before the game's first instruction, so there is no moment in which the
//! game runs without the lock. A game started by the CLI holds it too: `hengst launch`
//! (`launch::run`) takes the lock before the start and hands it to the game right after it.

use crate::paths::{validate_id, Paths};
use crate::Result;
use std::path::PathBuf;

#[cfg(windows)]
pub use self::windows::is_running;
#[cfg(windows)]
pub(crate) use self::windows::RunLock;

/// The run lock file of instance `id` (an instance folder name).
pub(crate) fn run_lock_path(paths: &Paths, id: &str) -> Result<PathBuf> {
    validate_id(id)?;
    Ok(paths.tmp().join(format!("run-{id}.lock")))
}

#[cfg(windows)]
mod windows {
    use super::run_lock_path;
    use crate::paths::Paths;
    use crate::Result;
    use std::fs::{File, OpenOptions};
    use std::io;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use std::path::Path;
    use std::time::Duration;

    use windows_sys::Win32::Foundation::{
        DuplicateHandle, DUPLICATE_SAME_ACCESS, ERROR_SHARING_VIOLATION, HANDLE,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    /// How often `acquire` tries again when the file is open elsewhere, and how long it waits
    /// between tries: a check (`is_held`) by another launcher opens the file for a moment, which
    /// looks the same as a running game.
    const ATTEMPTS: u32 = 10;
    const PAUSE: Duration = Duration::from_millis(20);

    /// Whether a game of instance `id` runs right now, started by this or by an earlier session of
    /// the app.
    pub fn is_running(paths: &Paths, id: &str) -> Result<bool> {
        Ok(is_held(&run_lock_path(paths, id)?)?)
    }

    fn sharing_violation(e: &io::Error) -> bool {
        e.raw_os_error() == Some(ERROR_SHARING_VIOLATION as i32)
    }

    /// The lock file at `path` is open without sharing somewhere. A missing file is not held.
    pub(crate) fn is_held(path: &Path) -> io::Result<bool> {
        let opened = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .open(path);
        match opened {
            Ok(_) => Ok(false),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) if sharing_violation(&e) => Ok(true),
            Err(e) => Err(e),
        }
    }

    /// The lock while the launcher still holds it, before `hand_to`. Dropping it releases the lock.
    #[derive(Debug)]
    pub(crate) struct RunLock {
        file: File,
    }

    impl RunLock {
        /// Takes the lock at `path` (creating the file and its folder), or `None` while it is held.
        pub(crate) fn acquire(path: &Path) -> io::Result<Option<RunLock>> {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            for attempt in 0..ATTEMPTS {
                let opened = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(false)
                    .share_mode(0)
                    .open(path);
                match opened {
                    Ok(file) => return Ok(Some(RunLock { file })),
                    Err(e) if sharing_violation(&e) => {
                        if attempt + 1 < ATTEMPTS {
                            std::thread::sleep(PAUSE);
                        }
                    }
                    Err(e) => return Err(e),
                }
            }
            Ok(None)
        }

        /// Gives the lock to `process` (a process handle with `PROCESS_DUP_HANDLE` access, as the
        /// one `CreateProcess` returns): from now on it is held until that process exits. The
        /// launcher's own handle is closed here.
        pub(crate) fn hand_to(self, process: &impl AsRawHandle) -> io::Result<()> {
            let mut copy: HANDLE = std::ptr::null_mut();
            // SAFETY: all three handles are valid for the duration of the call (borrowed from their
            // owners) and `copy` is writable. The copy lives in the other process's handle table:
            // it is not a handle of this process and is never closed here. Not inheritable (FALSE).
            let ok = unsafe {
                DuplicateHandle(
                    GetCurrentProcess(),
                    self.file.as_raw_handle(),
                    process.as_raw_handle(),
                    &mut copy,
                    0,
                    0,
                    DUPLICATE_SAME_ACCESS,
                )
            };
            if ok == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::process::job::tests::{
            ends_within, open_process, powershell, read_tagged, DEATH,
        };
        use crate::process::Job;
        use std::os::windows::process::CommandExt;
        use std::path::PathBuf;
        use std::process::{Command, Stdio};
        use std::time::Instant;

        fn lock_in(dir: &tempfile::TempDir) -> PathBuf {
            dir.path().join("tmp").join("run-9.9.lock")
        }

        /// Polls until the lock is no longer held; `false` if it still is after `DEATH`.
        fn released_within(path: &Path) -> bool {
            let deadline = Instant::now() + DEATH;
            while is_held(path).unwrap() {
                if Instant::now() >= deadline {
                    return false;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            true
        }

        #[test]
        fn the_lock_file_lives_in_tmp_and_takes_only_instance_ids() {
            let paths = Paths::new("C:/h");
            assert_eq!(
                run_lock_path(&paths, "9.9-fabric").unwrap(),
                PathBuf::from("C:/h/tmp/run-9.9-fabric.lock")
            );
            for bad in ["", "..", "a/b", "a\\b", "C:x"] {
                assert!(run_lock_path(&paths, bad).is_err(), "{bad:?}");
            }
        }

        #[test]
        fn a_missing_or_left_over_lock_file_is_not_held() {
            let dir = tempfile::tempdir().unwrap();
            let path = lock_in(&dir);
            assert!(!is_held(&path).unwrap());
            // As after a reboot: the file is there, nobody has it open.
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"").unwrap();
            assert!(!is_held(&path).unwrap());
            let lock = RunLock::acquire(&path).unwrap();
            assert!(lock.is_some(), "a left-over file blocks the lock");
        }

        #[test]
        fn a_taken_lock_is_held_until_it_is_dropped() {
            let dir = tempfile::tempdir().unwrap();
            let path = lock_in(&dir);
            let lock = RunLock::acquire(&path).unwrap().unwrap();
            assert!(is_held(&path).unwrap());
            assert!(RunLock::acquire(&path).unwrap().is_none());
            drop(lock);
            assert!(!is_held(&path).unwrap());
            assert!(RunLock::acquire(&path).unwrap().is_some());
        }

        /// The launcher's handle is not inheritable: a process it starts while it holds the lock
        /// does not keep the lock once the launcher lets go.
        #[test]
        fn a_process_started_while_the_lock_is_held_does_not_inherit_it() {
            let dir = tempfile::tempdir().unwrap();
            let path = lock_in(&dir);
            let job = Job::new_kill_on_close().unwrap();
            let lock = RunLock::acquire(&path).unwrap().unwrap();
            // `std` starts every child with handle inheritance on (its pipes need it).
            let mut child = job
                .spawn(powershell("Start-Sleep 60").stdout(Stdio::piped()))
                .unwrap();
            drop(lock);
            assert!(!is_held(&path).unwrap(), "the child inherited the lock");
            assert!(!ends_within(&child, Duration::from_millis(1)));
            drop(job);
            let _ = child.wait();
        }

        /// Handed over, the lock is held by the process alone: the launcher's handle is closed, and
        /// the process's own children do not get a copy. cmd.exe stands in for the game and starts
        /// powershell as its child; when cmd.exe is gone the lock is free although powershell runs
        /// on.
        #[test]
        fn a_handed_over_lock_is_held_by_that_process_alone_until_it_exits() {
            let dir = tempfile::tempdir().unwrap();
            let path = lock_in(&dir);
            let job = Job::new_kill_on_close().unwrap();
            let lock = RunLock::acquire(&path).unwrap().unwrap();
            let mut cmd = Command::new("cmd");
            // Passed verbatim: cmd.exe does not read the `\"` escaping `arg` would use.
            cmd.raw_arg(
                "/d /c powershell -NoProfile -NonInteractive -c \"'PID '+$PID; Start-Sleep 60\"",
            )
            .stdin(Stdio::null())
            .stdout(Stdio::piped());
            let mut game = job
                .spawn_with(&mut cmd, |child| lock.hand_to(child))
                .unwrap();
            let pid: u32 = read_tagged(game.stdout.take().unwrap(), "PID ")
                .parse()
                .unwrap();
            let grandchild = open_process(pid);
            assert!(
                is_held(&path).unwrap(),
                "the process does not hold the lock"
            );
            assert!(RunLock::acquire(&path).unwrap().is_none());
            game.kill().unwrap();
            let _ = game.wait();
            assert!(
                released_within(&path),
                "the lock outlived the process it was handed to"
            );
            assert!(
                !ends_within(&grandchild, Duration::from_millis(1)),
                "the child of the process is gone too, so the test proves nothing"
            );
            drop(job);
            assert!(ends_within(&grandchild, DEATH));
        }

        /// A hand-over that fails ends the new process: it never runs without the lock it was meant
        /// to hold. A process handle without `PROCESS_DUP_HANDLE` access makes `DuplicateHandle`
        /// fail.
        #[test]
        fn a_failed_hand_over_leaves_nothing_running_and_the_lock_free() {
            let dir = tempfile::tempdir().unwrap();
            let path = lock_in(&dir);
            let job = Job::new_kill_on_close().unwrap();
            let lock = RunLock::acquire(&path).unwrap().unwrap();
            let err = job
                .spawn_with(
                    powershell("Start-Sleep 60").stdout(Stdio::null()),
                    |child| {
                        // Limited rights: the handle may only be waited on and queried.
                        let weak = open_process(child.id());
                        lock.hand_to(&weak)
                    },
                )
                .unwrap_err();
            assert_eq!(err.raw_os_error(), Some(5), "{err}");
            assert!(!is_held(&path).unwrap());
            // Windows drops an ended process from the job's list shortly after it ended.
            let deadline = Instant::now() + DEATH;
            while !job.process_ids().unwrap().is_empty() {
                assert!(Instant::now() < deadline, "the process still runs");
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}
