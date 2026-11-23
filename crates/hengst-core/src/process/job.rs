//! A Windows job object that terminates every process in it when its last handle closes.

use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::os::windows::process::CommandExt;
use std::process::{Child, Command};

use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows_sys::Win32::System::Threading::{
    OpenThread, ResumeThread, CREATE_NO_WINDOW, CREATE_SUSPENDED, THREAD_SUSPEND_RESUME,
};

use crate::Result;

/// Takes ownership of a handle a Win32 call returned, or the last OS error if it failed (`null`
/// or `INVALID_HANDLE_VALUE`, depending on the call; both are refused).
fn owned(h: RawHandle) -> io::Result<OwnedHandle> {
    if h.is_null() || h == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `h` is a valid handle the caller just received from Windows and owns; nothing else
    // closes it, so `OwnedHandle` closes it exactly once.
    Ok(unsafe { OwnedHandle::from_raw_handle(h) })
}

/// Turns a Win32 `BOOL` result into the last OS error when it is zero.
fn check(ok: i32) -> io::Result<()> {
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Resumes every thread of a process that was created suspended (one: its main thread).
/// `std` keeps the main thread handle but exposes it only on nightly, so the thread is found
/// through a thread snapshot. The process id cannot be reused meanwhile: the caller holds a
/// handle to the (suspended, so still running) process.
fn resume_process(pid: u32) -> io::Result<()> {
    // SAFETY: plain call without pointers; the result is checked by `owned`.
    let snapshot = owned(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) })?;
    let mut entry = THREADENTRY32 {
        dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    let mut resumed = 0;
    // SAFETY: `snapshot` is a valid snapshot handle and `entry` a writable THREADENTRY32 with
    // `dwSize` set, as both calls require.
    let mut more = unsafe { Thread32First(snapshot.as_raw_handle(), &mut entry) } != 0;
    while more {
        if entry.th32OwnerProcessID == pid {
            // SAFETY: plain call without pointers; the result is checked by `owned`.
            let thread =
                owned(unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) })?;
            // SAFETY: `thread` is a valid thread handle opened with THREAD_SUSPEND_RESUME.
            if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
                return Err(io::Error::last_os_error());
            }
            resumed += 1;
        }
        // SAFETY: as for `Thread32First`.
        more = unsafe { Thread32Next(snapshot.as_raw_handle(), &mut entry) } != 0;
    }
    if resumed == 0 {
        return Err(io::Error::other("the new process has no thread to resume"));
    }
    Ok(())
}

/// Creation flags of every process `Job::spawn` starts: no console window (nothing flashes up,
/// also when the launcher is a GUI app without a console), and suspended until it is in the job.
pub(crate) const SPAWN_FLAGS: u32 = CREATE_NO_WINDOW | CREATE_SUSPENDED;

/// A job object with kill-on-close: when the `Job` is dropped, or the launcher process ends in
/// any way (also killed, or Ctrl+C in the CLI), Windows closes the handle and terminates every
/// process in the job. Processes started by a process in the job are in it too (no breakaway is
/// allowed), so a processor's helpers die with it.
///
/// If the launcher itself already runs inside a job (a terminal, a CI runner), the new job is
/// nested in it, which Windows 8 and later support.
#[derive(Debug)]
pub struct Job {
    handle: OwnedHandle,
}

impl Job {
    /// A new, unnamed job object with kill-on-close set.
    pub fn new_kill_on_close() -> Result<Job> {
        Ok(Self::create()?)
    }

    pub(super) fn create() -> io::Result<Job> {
        // SAFETY: null attributes (default security, handle not inheritable, so no child holds
        // the job open) and a null name are allowed; the result is checked by `owned`.
        let handle = owned(unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) })?;
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: `handle` is a valid job handle; the pointer and size describe `info`, which
        // lives for the duration of the call.
        check(unsafe {
            SetInformationJobObject(
                handle.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        })?;
        Ok(Job { handle })
    }

    fn assign_raw(&self, child: &Child) -> io::Result<()> {
        // SAFETY: both handles are valid for the duration of the call (borrowed from their owners).
        check(unsafe {
            AssignProcessToJobObject(self.handle.as_raw_handle(), child.as_raw_handle())
        })
    }

    /// Puts an already running child into the job. A child started without `Job::spawn` may
    /// already have started processes of its own before this call; those stay outside the job.
    /// Prefer `Job::spawn`, which has no such window.
    pub fn assign(&self, child: &Child) -> Result<()> {
        Ok(self.assign_raw(child)?)
    }

    /// Starts `cmd` inside the job: created suspended and without a console window, put into the
    /// job, then resumed, so not a single instruction of the child runs outside the job. This
    /// replaces any creation flags set on `cmd`. If the child cannot be put into the job or
    /// resumed, it is killed and the error returned.
    pub fn spawn(&self, cmd: &mut Command) -> io::Result<Child> {
        let mut child = cmd.creation_flags(SPAWN_FLAGS).spawn()?;
        if let Err(e) = self
            .assign_raw(&child)
            .and_then(|()| resume_process(child.id()))
        {
            // Suspended or already in the job: it has not run outside the job, and is ended here.
            let _ = child.kill();
            let _ = child.wait();
            return Err(e);
        }
        Ok(child)
    }

    /// Terminates every process in the job with exit code `code`. A failure is ignored: the
    /// processes still die when the job is dropped.
    pub fn terminate(&self, code: u32) {
        // SAFETY: the handle is valid for the duration of the call (owned by `self`).
        let _ = unsafe { TerminateJobObject(self.handle.as_raw_handle(), code) };
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::BufRead;
    use std::process::Stdio;
    use std::time::Duration;
    use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    };

    /// Generous: a loaded machine may take a while to tear a process down, but a process that
    /// should have died never ends within it (they all sleep for 60 s or more).
    pub(crate) const DEATH: Duration = Duration::from_secs(30);

    /// A handle to the process with this id, opened while it is known to run, so waiting on it
    /// later cannot hit a different process that got the same id.
    pub(crate) fn open_process(pid: u32) -> OwnedHandle {
        // SAFETY: plain call without pointers; the returned handle is checked before use.
        let h = unsafe {
            OpenProcess(
                PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
                0,
                pid,
            )
        };
        assert!(
            !h.is_null(),
            "process {pid} is not running: {}",
            std::io::Error::last_os_error()
        );
        // SAFETY: `h` is a valid process handle that nothing else owns.
        unsafe { OwnedHandle::from_raw_handle(h) }
    }

    /// True when the process ends within `within`.
    pub(crate) fn ends_within(process: &impl AsRawHandle, within: Duration) -> bool {
        let ms = u32::try_from(within.as_millis()).unwrap_or(u32::MAX);
        // SAFETY: the handle is valid for the duration of the call (borrowed from its owner).
        unsafe { WaitForSingleObject(process.as_raw_handle(), ms) == WAIT_OBJECT_0 }
    }

    /// `powershell -NoProfile -NonInteractive -c <script>` without a console window.
    pub(crate) fn powershell(script: &str) -> Command {
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-NonInteractive", "-c", script])
            .stdin(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW);
        cmd
    }

    /// Reads lines from `out` until one contains `prefix` and returns what follows it (libtest may
    /// have printed on the same line before).
    pub(crate) fn read_tagged(out: impl std::io::Read, prefix: &str) -> String {
        for line in std::io::BufReader::new(out).lines() {
            let line = line.unwrap();
            if let Some((_, rest)) = line.split_once(prefix) {
                return rest.trim().to_string();
            }
        }
        panic!("no line starting with {prefix}");
    }

    #[test]
    fn processor_child_dies_with_the_job() {
        let job = Job::new_kill_on_close().unwrap();
        let mut child = powershell("Start-Sleep 60")
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        if let Err(e) = job.assign(&child) {
            let _ = child.kill();
            panic!("assign failed: {e}");
        }
        assert!(!ends_within(&child, Duration::from_millis(200)));
        drop(job);
        let died = ends_within(&child, DEATH);
        if !died {
            let _ = child.kill();
        }
        assert!(died, "the child outlived its job");
        let _ = child.wait();
    }

    #[test]
    fn grandchild_started_through_spawn_dies_with_the_job() {
        let job = Job::new_kill_on_close().unwrap();
        // cmd.exe starts powershell, which prints its own id and sleeps.
        let mut cmd = Command::new("cmd");
        // Passed verbatim: cmd.exe does not read the `\"` escaping `arg` would use.
        cmd.raw_arg(
            "/d /c powershell -NoProfile -NonInteractive -c \"'PID '+$PID; Start-Sleep 60\"",
        )
        .stdin(Stdio::null())
        .stdout(Stdio::piped());
        let mut child = job.spawn(&mut cmd).unwrap();
        let pid: u32 = read_tagged(child.stdout.take().unwrap(), "PID ")
            .parse()
            .unwrap();
        let grandchild = open_process(pid);
        assert!(!ends_within(&grandchild, Duration::from_millis(200)));
        drop(job);
        assert!(
            ends_within(&grandchild, DEATH),
            "the grandchild outlived the job"
        );
        assert!(ends_within(&child, DEATH));
        let _ = child.wait();
    }

    #[test]
    fn terminate_ends_every_process_in_the_job() {
        let job = Job::new_kill_on_close().unwrap();
        let mut child = job
            .spawn(powershell("Start-Sleep 60").stdout(Stdio::null()))
            .unwrap();
        job.terminate(7);
        assert!(ends_within(&child, DEATH));
        assert_eq!(child.wait().unwrap().code(), Some(7));
    }

    #[test]
    fn spawn_failure_is_an_error_and_leaves_nothing_running() {
        let job = Job::new_kill_on_close().unwrap();
        let err = job
            .spawn(&mut Command::new("hengst-no-such-program-xyz.exe"))
            .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    /// Set in the helper process `children_die_when_the_launcher_dies` starts.
    const HELPER_ENV: &str = "HENGST_TEST_JOB_HELPER";

    /// The launcher dying (killed, or Ctrl+C with the default handler) closes its job handle, and
    /// with it every child goes. The test binary itself stands in for the launcher.
    #[test]
    fn children_die_when_the_launcher_dies() {
        if std::env::var_os(HELPER_ENV).is_some() {
            // Helper mode: start a child in a job, report its id, and wait to be killed.
            let job = Job::new_kill_on_close().unwrap();
            let mut child = job
                .spawn(powershell("Start-Sleep 120").stdout(Stdio::null()))
                .unwrap();
            println!("CHILD {}", child.id());
            std::thread::sleep(Duration::from_secs(120));
            drop(job);
            let _ = child.wait();
            return;
        }
        let mut launcher = Command::new(std::env::current_exe().unwrap())
            .args([
                "process::job::tests::children_die_when_the_launcher_dies",
                "--exact",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(HELPER_ENV, "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .unwrap();
        let pid: u32 = read_tagged(launcher.stdout.take().unwrap(), "CHILD ")
            .parse()
            .unwrap();
        let child = open_process(pid);
        launcher.kill().unwrap();
        let _ = launcher.wait();
        assert!(
            ends_within(&child, DEATH),
            "the child outlived the launcher"
        );
    }
}
