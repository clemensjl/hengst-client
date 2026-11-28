//! Moving a folder to the Recycle Bin, and never deleting it permanently.
//!
//! What was measured on this machine (Windows 11, `trash` 5.4.0, see the Task 10 report):
//! - `trash` runs `IFileOperation` with `FOF_NO_UI | FOF_ALLOWUNDO | FOF_WANTNUKEWARNING`. For a
//!   folder larger than the bin's capacity Windows opens a modal "permanently delete?" dialog
//!   (Yes/No) on the calling thread, even with `FOF_NO_UI`; the call waits for an answer, and
//!   "Yes" deletes for good.
//! - `FOFX_RECYCLEONDELETE` made the same call delete silently and permanently.
//! - A folder with a junction inside, one with an open file, and a very long path behave as
//!   described in the report (moved with the junction untouched; error and folder intact; moved).
//!
//! What was NOT measured, only assumed from the documentation of `FOF_WANTNUKEWARNING` and from
//! the names of the shell's error codes: what Windows does with the setting "Don't move files to
//! the Recycle Bin" (`NukeOnDelete=1`), with the `NoRecycleFiles` policy and on a drive without a
//! bin (a network share, a FAT stick). It may delete permanently without any dialog, so the dialog
//! watchdog alone cannot be trusted. The defence is in three layers:
//! 1. `precheck`: before the shell is called, the facts about the volume are read (drive type,
//!    `NukeOnDelete`, policy, capacity, folder size) and the call is refused unless all of them
//!    say the bin will take the folder. Any fact that cannot be read refuses (fail closed).
//! 2. A guarded call: the shell runs on its own thread; a watchdog answers every dialog it opens
//!    (No, then Cancel, then close) and gives up after a time limit.
//! 3. `confirm`: after a successful call the folder must be found in the bin (`trash::os_limited`),
//!    otherwise the outcome is `Unconfirmed` and the UI says so instead of "in the Recycle Bin".
//!    The bin is listed before and after the call; only an item that is new counts.
//!
//! All three layers look at ONE path: the parent of the folder is canonicalized once (links in it
//! are resolved, the `\\?\` prefix is dropped) and the folder name is joined to it; the final
//! component is never resolved. `trash` canonicalizes the parent the same way, so the volume that is
//! checked is the volume that is changed.
//!
//! Known limits: the `RecycleBinSize` policy and the legacy global `NukeOnDelete` are not read
//! (layers 2 and 3 cover them); `trash::os_limited::list` fails as a whole when one item of the bin
//! cannot be read, which refuses the move before anything happens (`Refused(Unchecked)`: the bin is
//! listed before the call). If listing the bin fails after a successful call, the move ends as
//! `Unconfirmed`, never as "in the bin". After a time-out the shell thread keeps running: a later
//! move is refused until it has ended, its outcome is reported as unknown, and the instance list
//! is announced as changed once it has ended.

use hengst_core::{Error, Result};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

/// Why the bin will not (or may not) take a folder. `key()` is the `reason` param of the error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinReason {
    /// The folder is larger than the bin's capacity allows (with a margin).
    TooLarge,
    /// The bin is off for this volume (`NukeOnDelete`) or by a policy.
    BinOff,
    /// The drive has no Recycle Bin (not a fixed drive).
    NoBin,
    /// A file in the folder is in use.
    InUse,
    /// Windows would not move the folder, for a reason it did not name.
    Other,
    /// Something the move depends on could not be read, so Windows was never asked: the Recycle
    /// Bin settings of the volume, the resolved path of the folder, or the current contents of the
    /// bin (listed before the call).
    Unchecked,
}

impl BinReason {
    pub fn key(self) -> &'static str {
        match self {
            BinReason::TooLarge => "tooLarge",
            BinReason::BinOff => "binOff",
            BinReason::NoBin => "noBin",
            BinReason::InUse => "inUse",
            BinReason::Other => "other",
            BinReason::Unchecked => "unchecked",
        }
    }
}

/// What a failed or doubtful move ended as. It travels inside the `io::Error` of the trasher
/// (`failure_of` takes it out again), so the command can tell the trasher's own failures from the
/// core's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecycleFailure {
    /// Refused before the shell was called; nothing was touched.
    Refused(BinReason),
    /// The shell reported that an item is in use.
    InUse(String),
    /// Windows asked to delete permanently and the app declined (the shell reported failure).
    Declined(String),
    /// The shell call did not finish in time; it may still be running. The outcome is unknown.
    TimedOut,
    /// The shell call stopped unexpectedly (its thread panicked). The outcome is unknown.
    Aborted(String),
    /// An earlier shell call has not ended yet (after a time-out); nothing was started.
    StillRunning,
    /// The shell reported another failure.
    Failed(String),
    /// The shell reported success, but the folder is gone and was not found in the bin (or the
    /// bin could not be read): it may have been removed permanently.
    Unconfirmed(String),
}

impl RecycleFailure {
    pub fn reason(&self) -> BinReason {
        match self {
            RecycleFailure::Refused(r) => *r,
            RecycleFailure::InUse(_) => BinReason::InUse,
            _ => BinReason::Other,
        }
    }
}

impl std::fmt::Display for RecycleFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecycleFailure::Refused(r) => write!(
                f,
                "the Recycle Bin check refused the move before anything was touched ({})",
                r.key()
            ),
            RecycleFailure::InUse(d) => write!(f, "a file in the folder is in use: {d}"),
            RecycleFailure::Declined(d) => write!(
                f,
                "Windows asked to delete the folder permanently instead of moving it to the Recycle Bin; the app declined: {d}"
            ),
            RecycleFailure::TimedOut => write!(f, "the Recycle Bin call did not finish in time"),
            RecycleFailure::Aborted(d) => write!(f, "the Recycle Bin call stopped unexpectedly: {d}"),
            RecycleFailure::StillRunning => {
                write!(f, "an earlier Recycle Bin call is still running")
            }
            RecycleFailure::Failed(d) => write!(f, "the Recycle Bin did not take the folder: {d}"),
            RecycleFailure::Unconfirmed(d) => write!(
                f,
                "Windows reported success, but the folder was not found in the Recycle Bin: {d}"
            ),
        }
    }
}

impl std::error::Error for RecycleFailure {}

fn fail(f: RecycleFailure) -> Error {
    Error::Io(std::io::Error::other(f))
}

/// The trasher's own failure inside a core error, if the error is one.
pub fn failure_of(e: &Error) -> Option<&RecycleFailure> {
    match e {
        Error::Io(io) => io.get_ref()?.downcast_ref::<RecycleFailure>(),
        _ => None,
    }
}

/// What is known about the volume of the folder. `None` is "could not be read" and refuses.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VolumeFacts {
    /// The drive type is `DRIVE_FIXED`.
    pub fixed_drive: Option<bool>,
    /// `NukeOnDelete` of the volume's key under `...\Explorer\BitBucket\Volume\{guid}`.
    pub nuke_on_delete: Option<u32>,
    /// The `NoRecycleFiles` policy is set (in HKCU or HKLM). A missing value is `Some(false)`.
    pub no_recycle_policy: Option<bool>,
    /// `MaxCapacity` of the volume's key, in MB.
    pub max_capacity_mb: Option<u64>,
    /// The size of the folder in bytes (a lower bound once it is above the limit).
    pub folder_bytes: Option<u64>,
}

/// The share of the capacity a folder may take. An assumption: Windows' exact rule was only
/// measured far outside it (200 GB against about 97 GB), so there is a margin.
const CAPACITY_SHARE_PERCENT: u64 = 80;

/// The largest folder in bytes the bin of a volume with `max_capacity_mb` is trusted to take.
pub fn size_limit(max_capacity_mb: u64) -> u64 {
    max_capacity_mb.saturating_mul(1024 * 1024) / 100 * CAPACITY_SHARE_PERCENT
}

/// Whether the bin will take the folder, from the facts alone. Fails closed: every fact must be
/// known and say yes.
pub fn precheck(f: &VolumeFacts) -> std::result::Result<(), BinReason> {
    match f.fixed_drive {
        Some(true) => {}
        Some(false) => return Err(BinReason::NoBin),
        None => return Err(BinReason::Unchecked),
    }
    match f.no_recycle_policy {
        Some(false) => {}
        Some(true) => return Err(BinReason::BinOff),
        None => return Err(BinReason::Unchecked),
    }
    match f.nuke_on_delete {
        Some(0) => {}
        Some(_) => return Err(BinReason::BinOff),
        None => return Err(BinReason::Unchecked),
    }
    let (Some(capacity), Some(bytes)) = (f.max_capacity_mb, f.folder_bytes) else {
        return Err(BinReason::Unchecked);
    };
    if bytes > size_limit(capacity) {
        return Err(BinReason::TooLarge);
    }
    Ok(())
}

/// One item of the bin, as far as the check needs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BinEntry {
    pub name: std::ffi::OsString,
    pub original_parent: PathBuf,
    /// Unix seconds.
    pub deleted_at: i64,
}

fn normalized(p: &Path) -> String {
    let text = p.to_string_lossy().replace('/', "\\").to_lowercase();
    text.strip_prefix("\\\\?\\")
        .unwrap_or(&text)
        .trim_end_matches('\\')
        .to_string()
}

/// Whether `after` holds an item named like `path` that came from its parent folder and is not in
/// `before` (the bin as it was before the call). An older item with the same name and parent
/// (delete, recreate the same id, delete again) does not count.
pub fn in_bin(before: &[BinEntry], after: &[BinEntry], path: &Path) -> bool {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return false;
    };
    let parent = normalized(parent);
    after
        .iter()
        .any(|e| e.name == name && normalized(&e.original_parent) == parent && !before.contains(e))
}

/// `\\?\C:\x` -> `C:\x`, `\\?\UNC\host\share` -> `\\host\share`, a path without the prefix
/// as it is. Any other verbatim form (`\\?\Volume{..}\`, `\\?\GLOBALROOT`, a device path) has no
/// plain spelling the volume lookup could use: `None`, which refuses.
pub fn strip_verbatim(p: &Path) -> Option<PathBuf> {
    let text = p.to_string_lossy();
    let Some(rest) = text.strip_prefix("\\\\?\\") else {
        return Some(p.to_path_buf());
    };
    if let Some(unc) = rest.strip_prefix("UNC\\") {
        return Some(PathBuf::from(format!("\\\\{unc}")));
    }
    let mut chars = rest.chars();
    let drive = chars.next().is_some_and(|c| c.is_ascii_alphabetic());
    (drive && chars.next() == Some(':')).then(|| PathBuf::from(rest))
}

/// The one path every layer looks at: the parent canonicalized (links resolved, no `\\?\`), the
/// name joined. The name itself is not resolved.
pub fn resolve_target(path: &Path) -> std::io::Result<PathBuf> {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(std::io::Error::other("the folder has no parent or no name"));
    };
    let canonical = parent.canonicalize()?;
    let plain = strip_verbatim(&canonical)
        .ok_or_else(|| std::io::Error::other("the folder is on a volume that has no plain path"))?;
    Ok(plain.join(name))
}

/// Resolves `path` and runs the pre-check on the facts read from the RESOLVED path.
pub fn checked_target(
    path: &Path,
    facts: &dyn Fn(&Path) -> VolumeFacts,
) -> std::result::Result<PathBuf, RecycleFailure> {
    let target = resolve_target(path).map_err(|_| RecycleFailure::Refused(BinReason::Unchecked))?;
    precheck(&facts(&target)).map_err(RecycleFailure::Refused)?;
    Ok(target)
}

/// While a shell call runs (also one that was given up on), no second one starts.
static SHELL_RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Held by the thread that calls the shell, until the call has really ended.
pub struct ShellSlot;

impl ShellSlot {
    pub fn claim() -> Option<ShellSlot> {
        SHELL_RUNNING
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
            .then_some(ShellSlot)
    }
}

/// A shell call is running, also one the caller gave up on after a time-out.
pub fn shell_running() -> bool {
    SHELL_RUNNING.load(Ordering::SeqCst)
}

impl Drop for ShellSlot {
    fn drop(&mut self) {
        SHELL_RUNNING.store(false, Ordering::SeqCst);
    }
}

/// The `HRESULT`s of the copy engine for "in use by another process" (source and destination).
pub fn is_sharing_violation(hresult: i32) -> bool {
    matches!(hresult as u32, 0x8027_0020 | 0x8027_0028)
}

/// What the watchdog does to a dialog, by how many rounds it has been seen for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    No,
    Cancel,
    Close,
}

/// "No" first (that is the answer that keeps the folder), then Cancel, then closing the window.
pub fn answer_for(round: u32) -> Answer {
    match round {
        0..=3 => Answer::No,
        4..=7 => Answer::Cancel,
        _ => Answer::Close,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum GuardError {
    TimedOut,
    Panicked,
}

/// Runs `f` on a thread of its own and, while it runs, answers every dialog it opens on that
/// thread (`answer_for`). Returns `f`'s result and the number of dialogs answered, or
/// `TimedOut` when `f` has not finished after `limit` (the thread is left behind; it ends when
/// the shell call does). A dedicated thread keeps the caller's blocking thread from hanging for
/// good on a dialog nobody answers.
pub fn run_guarded<T: Send + 'static>(
    f: impl FnOnce() -> T + Send + 'static,
    limit: Duration,
) -> std::result::Result<(T, u32), GuardError> {
    let thread = Arc::new(AtomicU32::new(0));
    let (tx, rx) = mpsc::channel();
    let id = thread.clone();
    std::thread::Builder::new()
        .name("recycle-bin".into())
        .spawn(move || {
            id.store(platform::current_thread_id(), Ordering::SeqCst);
            let _ = tx.send(f());
        })
        .map_err(|_| GuardError::Panicked)?;
    let deadline = Instant::now() + limit;
    let (mut round, mut answered) = (0u32, 0u32);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(GuardError::TimedOut);
        }
        match rx.recv_timeout(left.min(Duration::from_millis(25))) {
            Ok(value) => return Ok((value, answered)),
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err(GuardError::Panicked),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let id = thread.load(Ordering::SeqCst);
                if id != 0 {
                    let n = platform::answer_dialogs(id, answer_for(round));
                    if n > 0 {
                        answered += n;
                        round += 1;
                    }
                }
            }
        }
    }
}

/// How long the shell may take. Moving a folder within a volume is a rename; this only bounds a
/// dialog that cannot be answered.
const SHELL_LIMIT: Duration = Duration::from_secs(120);

/// The trasher the app uses. Blocking: call it from a blocking thread. On failure the error is a
/// core `Io` error carrying a `RecycleFailure` (`failure_of`).
pub fn move_to_recycle_bin(path: &Path) -> Result<()> {
    let target = checked_target(path, &platform::volume_facts).map_err(fail)?;
    let before =
        platform::bin_entries().map_err(|_| fail(RecycleFailure::Refused(BinReason::Unchecked)))?;
    let slot = ShellSlot::claim().ok_or_else(|| fail(RecycleFailure::StillRunning))?;
    let shell_target = target.clone();
    let outcome = run_guarded(
        move || {
            // Released when the shell call ends, also when the caller gave up on it.
            let _slot = slot;
            trash::delete(&shell_target)
        },
        SHELL_LIMIT,
    );
    match outcome {
        Err(GuardError::TimedOut) => Err(fail(RecycleFailure::TimedOut)),
        Err(GuardError::Panicked) => {
            Err(fail(RecycleFailure::Aborted("its thread panicked".into())))
        }
        Ok((Err(e), declined)) => Err(fail(classify(&e, declined))),
        Ok((Ok(()), _)) => confirm(&target, &before),
    }
}

fn classify(e: &trash::Error, declined: u32) -> RecycleFailure {
    let detail = e.to_string();
    match e {
        trash::Error::Os { code, .. } if is_sharing_violation(*code) => {
            RecycleFailure::InUse(detail)
        }
        _ if declined > 0 => RecycleFailure::Declined(detail),
        _ => RecycleFailure::Failed(detail),
    }
}

/// After a success: the folder must be in the bin as a new item. If it is not, say so: never "in
/// the bin".
fn confirm(path: &Path, before: &[BinEntry]) -> Result<()> {
    match platform::bin_entries() {
        Ok(after) if in_bin(before, &after, path) => Ok(()),
        Ok(_) => Err(fail(RecycleFailure::Unconfirmed(
            "no new matching item in the Recycle Bin".into(),
        ))),
        Err(e) => Err(fail(RecycleFailure::Unconfirmed(format!(
            "the Recycle Bin could not be read: {e}"
        )))),
    }
}

#[cfg(windows)]
mod platform {
    use super::{BinEntry, VolumeFacts};
    use std::path::Path;
    use windows_sys::Win32::Foundation::{
        ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, HWND, LPARAM, WPARAM,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        GetDriveTypeW, GetVolumeNameForVolumeMountPointW, GetVolumePathNameW,
    };
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE,
        KEY_READ, REG_DWORD,
    };
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumThreadWindows, GetClassNameW, IsWindowVisible, PostMessageW, WM_CLOSE, WM_COMMAND,
    };

    pub fn current_thread_id() -> u32 {
        unsafe { GetCurrentThreadId() }
    }

    pub fn bin_entries() -> Result<Vec<BinEntry>, String> {
        let items = trash::os_limited::list().map_err(|e| e.to_string())?;
        Ok(items
            .into_iter()
            .map(|i| BinEntry {
                name: i.name,
                original_parent: i.original_parent,
                deleted_at: i.time_deleted,
            })
            .collect())
    }

    const IDNO: WPARAM = 7;
    const IDCANCEL: WPARAM = 2;

    /// Answers every visible dialog (`#32770`) of `thread`; returns how many it found.
    pub fn answer_dialogs(thread: u32, answer: super::Answer) -> u32 {
        struct Job {
            answer: super::Answer,
            found: u32,
        }
        unsafe extern "system" fn each(window: HWND, job: LPARAM) -> i32 {
            let job = unsafe { &mut *(job as *mut Job) };
            let mut class = [0u16; 16];
            let n = unsafe { GetClassNameW(window, class.as_mut_ptr(), class.len() as i32) };
            let class = String::from_utf16_lossy(&class[..usize::try_from(n).unwrap_or(0)]);
            if class == "#32770" && unsafe { IsWindowVisible(window) } != 0 {
                match job.answer {
                    super::Answer::No => unsafe { PostMessageW(window, WM_COMMAND, IDNO, 0) },
                    super::Answer::Cancel => unsafe {
                        PostMessageW(window, WM_COMMAND, IDCANCEL, 0)
                    },
                    super::Answer::Close => unsafe { PostMessageW(window, WM_CLOSE, 0, 0) },
                };
                job.found += 1;
            }
            1
        }
        let mut job = Job { answer, found: 0 };
        unsafe { EnumThreadWindows(thread, Some(each), &mut job as *mut Job as LPARAM) };
        job.found
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    enum Read {
        Value(u32),
        Missing,
        Failed,
    }

    /// One `REG_DWORD`, read-only. A missing key or value is `Missing`, anything else wrong is
    /// `Failed`.
    fn dword(root: HKEY, key: &str, value: &str) -> Read {
        let (key, value) = (wide(key), wide(value));
        let mut handle: HKEY = std::ptr::null_mut();
        let opened = unsafe { RegOpenKeyExW(root, key.as_ptr(), 0, KEY_READ, &mut handle) };
        match opened {
            ERROR_SUCCESS => {}
            ERROR_FILE_NOT_FOUND => return Read::Missing,
            _ => return Read::Failed,
        }
        let (mut kind, mut data, mut size) = (0u32, 0u32, 4u32);
        let queried = unsafe {
            RegQueryValueExW(
                handle,
                value.as_ptr(),
                std::ptr::null(),
                &mut kind,
                &mut data as *mut u32 as *mut u8,
                &mut size,
            )
        };
        unsafe { RegCloseKey(handle) };
        match queried {
            ERROR_SUCCESS if kind == REG_DWORD && size == 4 => Read::Value(data),
            ERROR_FILE_NOT_FOUND => Read::Missing,
            _ => Read::Failed,
        }
    }

    const EXPLORER: &str = "Software\\Microsoft\\Windows\\CurrentVersion";

    /// `Some(true)` when the policy is set anywhere, `Some(false)` when it is set nowhere,
    /// `None` when a read failed.
    fn recycle_policy() -> Option<bool> {
        let key = format!("{EXPLORER}\\Policies\\Explorer");
        let mut set = false;
        for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
            match dword(root, &key, "NoRecycleFiles") {
                Read::Value(v) => set |= v != 0,
                Read::Missing => {}
                Read::Failed => return None,
            }
        }
        Some(set)
    }

    /// `\\?\Volume{guid}\` -> `{guid}`.
    fn volume_guid(volume_name: &str) -> Option<String> {
        let (a, b) = (volume_name.find('{')?, volume_name.find('}')?);
        (a < b).then(|| volume_name[a..=b].to_string())
    }

    fn from_wide(buf: &[u16]) -> String {
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..end])
    }

    /// Reads the facts about the volume `path` is on. Read-only: the registry is only opened with
    /// `KEY_READ`; nothing is written.
    pub fn volume_facts(path: &Path) -> VolumeFacts {
        use std::os::windows::ffi::OsStrExt;
        let mut facts = VolumeFacts::default();
        let file: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut root = [0u16; 261];
        if unsafe { GetVolumePathNameW(file.as_ptr(), root.as_mut_ptr(), root.len() as u32) } == 0 {
            return facts;
        }
        facts.fixed_drive = Some(unsafe { GetDriveTypeW(root.as_ptr()) } == 3);
        facts.no_recycle_policy = recycle_policy();
        let mut name = [0u16; 64];
        let named = unsafe {
            GetVolumeNameForVolumeMountPointW(root.as_ptr(), name.as_mut_ptr(), name.len() as u32)
        };
        if named != 0 {
            if let Some(guid) = volume_guid(&from_wide(&name)) {
                let key = format!("{EXPLORER}\\Explorer\\BitBucket\\Volume\\{guid}");
                let read = |value| match dword(HKEY_CURRENT_USER, &key, value) {
                    Read::Value(v) => Some(v),
                    Read::Missing | Read::Failed => None,
                };
                facts.nuke_on_delete = read("NukeOnDelete");
                facts.max_capacity_mb = read("MaxCapacity").map(u64::from);
            }
        }
        if let Some(mb) = facts.max_capacity_mb {
            facts.folder_bytes = super::folder_bytes_up_to(path, super::size_limit(mb));
        }
        facts
    }
}

#[cfg(not(windows))]
mod platform {
    use super::{Answer, BinEntry, VolumeFacts};
    use std::path::Path;

    pub fn current_thread_id() -> u32 {
        1
    }
    pub fn bin_entries() -> Result<Vec<BinEntry>, String> {
        Err("the Recycle Bin is read on Windows only".to_string())
    }
    pub fn answer_dialogs(_thread: u32, _answer: Answer) -> u32 {
        0
    }
    /// Nothing is known, so every move is refused.
    pub fn volume_facts(_path: &Path) -> VolumeFacts {
        VolumeFacts::default()
    }
}

/// The size of the files under `root` in bytes, counting without following links, and stopping
/// as soon as the sum is above `limit` (the result is then only a lower bound). `None` when
/// anything cannot be read.
pub fn folder_bytes_up_to(root: &Path, limit: u64) -> Option<u64> {
    let mut total = 0u64;
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).ok()? {
            let entry = entry.ok()?;
            let meta = std::fs::symlink_metadata(entry.path()).ok()?;
            if meta.file_type().is_symlink() || is_reparse_point(&meta) {
                continue;
            }
            if meta.is_dir() {
                pending.push(entry.path());
            } else {
                total = total.saturating_add(meta.len());
                if total > limit {
                    return Some(total);
                }
            }
        }
    }
    Some(total)
}

fn is_reparse_point(meta: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        let _ = meta;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good() -> VolumeFacts {
        VolumeFacts {
            fixed_drive: Some(true),
            nuke_on_delete: Some(0),
            no_recycle_policy: Some(false),
            max_capacity_mb: Some(1000),
            folder_bytes: Some(10 * 1024 * 1024),
        }
    }

    #[test]
    fn the_precheck_passes_only_when_every_fact_says_yes() {
        assert_eq!(precheck(&good()), Ok(()));
    }

    #[test]
    fn the_precheck_refuses_each_branch_with_its_reason() {
        let with = |f: fn(&mut VolumeFacts)| {
            let mut v = good();
            f(&mut v);
            precheck(&v)
        };
        assert_eq!(with(|v| v.fixed_drive = Some(false)), Err(BinReason::NoBin));
        assert_eq!(with(|v| v.nuke_on_delete = Some(1)), Err(BinReason::BinOff));
        assert_eq!(with(|v| v.nuke_on_delete = Some(7)), Err(BinReason::BinOff));
        assert_eq!(
            with(|v| v.no_recycle_policy = Some(true)),
            Err(BinReason::BinOff)
        );
        assert_eq!(
            with(|v| v.folder_bytes = Some(900 * 1024 * 1024)),
            Err(BinReason::TooLarge)
        );
    }

    #[test]
    fn the_precheck_fails_closed_on_every_unknown_fact() {
        let with = |f: fn(&mut VolumeFacts)| {
            let mut v = good();
            f(&mut v);
            precheck(&v)
        };
        assert_eq!(with(|v| v.fixed_drive = None), Err(BinReason::Unchecked));
        assert_eq!(with(|v| v.nuke_on_delete = None), Err(BinReason::Unchecked));
        assert_eq!(
            with(|v| v.no_recycle_policy = None),
            Err(BinReason::Unchecked)
        );
        assert_eq!(
            with(|v| v.max_capacity_mb = None),
            Err(BinReason::Unchecked)
        );
        assert_eq!(with(|v| v.folder_bytes = None), Err(BinReason::Unchecked));
        assert_eq!(precheck(&VolumeFacts::default()), Err(BinReason::Unchecked));
    }

    #[test]
    fn the_size_limit_keeps_a_margin_and_the_edge_is_exact() {
        assert_eq!(size_limit(1000), 1000 * 1024 * 1024 / 100 * 80);
        let mut v = good();
        v.folder_bytes = Some(size_limit(1000));
        assert_eq!(precheck(&v), Ok(()));
        v.folder_bytes = Some(size_limit(1000) + 1);
        assert_eq!(precheck(&v), Err(BinReason::TooLarge));
        // A capacity of 0 (bin switched off by size) takes nothing but empty folders.
        v.max_capacity_mb = Some(0);
        v.folder_bytes = Some(1);
        assert_eq!(precheck(&v), Err(BinReason::TooLarge));
    }

    #[test]
    fn the_order_of_reasons_names_the_drive_before_the_settings() {
        let v = VolumeFacts {
            fixed_drive: Some(false),
            nuke_on_delete: Some(1),
            no_recycle_policy: Some(true),
            max_capacity_mb: Some(1),
            folder_bytes: Some(u64::MAX),
        };
        assert_eq!(precheck(&v), Err(BinReason::NoBin));
    }

    fn entry(name: &str, parent: &str, at: i64) -> BinEntry {
        BinEntry {
            name: name.into(),
            original_parent: parent.into(),
            deleted_at: at,
        }
    }

    #[test]
    fn the_bin_check_wants_name_parent_and_a_new_item() {
        let path = Path::new("C:\\Data\\instances\\9.9");
        let ours = entry("9.9", "C:\\Data\\instances", 100);
        assert!(in_bin(&[], std::slice::from_ref(&ours), path));
        // Case, slashes and a verbatim prefix do not matter.
        let spelled = entry("9.9", "\\\\?\\c:/data/instances/", 100);
        assert!(in_bin(&[], &[spelled], path));
        // Wrong name, wrong parent: not this folder.
        assert!(!in_bin(
            &[],
            &[entry("9.8", "C:\\Data\\instances", 100)],
            path
        ));
        assert!(!in_bin(
            &[],
            &[entry("9.9", "C:\\Other\\instances", 100)],
            path
        ));
        assert!(!in_bin(&[], &[], path));
        assert!(!in_bin(&[], std::slice::from_ref(&ours), Path::new("C:\\")));
    }

    #[test]
    fn an_older_item_with_the_same_name_and_parent_does_not_confirm_a_new_delete() {
        // Delete, recreate the same id, delete again: the first item is in the bin before the
        // second call, so only an item that appeared during the call counts.
        let path = Path::new("C:\\Data\\instances\\9.9");
        let old = entry("9.9", "C:\\Data\\instances", 100);
        assert!(!in_bin(
            std::slice::from_ref(&old),
            std::slice::from_ref(&old),
            path
        ));
        let new = entry("9.9", "C:\\Data\\instances", 105);
        assert!(in_bin(
            std::slice::from_ref(&old),
            &[old.clone(), new],
            path
        ));
    }

    #[test]
    fn the_verbatim_prefix_is_dropped_in_both_forms() {
        let plain = |t: &str| strip_verbatim(Path::new(t));
        assert_eq!(plain("\\\\?\\C:\\Data\\x"), Some("C:\\Data\\x".into()));
        assert_eq!(
            plain("\\\\?\\UNC\\host\\share\\x"),
            Some("\\\\host\\share\\x".into())
        );
        assert_eq!(plain("C:\\x"), Some("C:\\x".into()));
        // Forms with no plain spelling are refused, not turned into a relative path.
        for odd in [
            "\\\\?\\Volume{a0dd90d6-f1fa-457b-ba89-8eb6e88c31d5}\\x",
            "\\\\?\\GLOBALROOT\\Device\\HarddiskVolume3\\x",
            "\\\\?\\",
            "\\\\?\\1:\\x",
        ] {
            assert_eq!(plain(odd), None, "{odd}");
        }
    }

    #[test]
    fn only_one_shell_call_runs_at_a_time() {
        // Serialized with other users of the one slot by taking it for the whole test.
        // The global flag is safe here: every fixture swaps in a fake trasher, and the only other
        // claimant of the slot is the ignored manual test.
        let first = ShellSlot::claim();
        assert!(first.is_some());
        assert!(ShellSlot::claim().is_none());
        drop(first);
        assert!(ShellSlot::claim().is_some());
    }

    #[cfg(windows)]
    #[test]
    fn every_layer_looks_at_the_resolved_path_not_at_the_path_through_a_junction() {
        // <tmp>/real/instances/9.9 reached through the junction <tmp>/link -> <tmp>/real.
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir_all(real.join("instances/9.9")).unwrap();
        let link = dir.path().join("link");
        let made = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&link)
            .arg(&real)
            .output()
            .unwrap();
        assert!(made.status.success(), "{made:?}");
        let through = link.join("instances").join("9.9");

        let resolved = resolve_target(&through).unwrap();
        let wanted = strip_verbatim(&real.canonicalize().unwrap())
            .unwrap()
            .join("instances")
            .join("9.9");
        assert_eq!(resolved, wanted);
        assert!(!resolved.to_string_lossy().starts_with("\\\\?\\"));
        // Not the path through the junction (compared as a path: the temp folder's own name may
        // contain any text).
        assert_ne!(resolved, through);
        assert!(!resolved.starts_with(&link));

        // The facts function is handed the resolved path, and so is the caller.
        let asked = std::cell::RefCell::new(Vec::new());
        let facts = |p: &Path| {
            asked.borrow_mut().push(p.to_path_buf());
            good()
        };
        assert_eq!(checked_target(&through, &facts).unwrap(), wanted);
        assert_eq!(*asked.borrow(), std::slice::from_ref(&wanted));

        // A refusing fact on the resolved volume refuses; an unresolvable parent refuses too.
        let off = |_: &Path| VolumeFacts {
            nuke_on_delete: Some(1),
            ..good()
        };
        assert_eq!(
            checked_target(&through, &off),
            Err(RecycleFailure::Refused(BinReason::BinOff))
        );
        assert_eq!(
            checked_target(&dir.path().join("missing/9.9"), &facts),
            Err(RecycleFailure::Refused(BinReason::Unchecked))
        );
        // The final component is not resolved: a link as the folder itself stays as it is.
        let final_link = dir.path().join("final");
        let made = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&final_link)
            .arg(&real)
            .output()
            .unwrap();
        assert!(made.status.success());
        let kept = resolve_target(&final_link).unwrap();
        assert_eq!(kept.file_name().unwrap(), "final");
    }

    #[test]
    fn sharing_violations_are_in_use_and_other_codes_are_not() {
        assert!(is_sharing_violation(0x8027_0028_u32 as i32));
        assert!(is_sharing_violation(0x8027_0020_u32 as i32));
        assert!(!is_sharing_violation(0x8027_0021_u32 as i32));
        assert!(!is_sharing_violation(0));
    }

    #[test]
    fn the_watchdog_escalates_no_then_cancel_then_close() {
        assert_eq!(answer_for(0), Answer::No);
        assert_eq!(answer_for(3), Answer::No);
        assert_eq!(answer_for(4), Answer::Cancel);
        assert_eq!(answer_for(7), Answer::Cancel);
        assert_eq!(answer_for(8), Answer::Close);
        assert_eq!(answer_for(u32::MAX), Answer::Close);
    }

    #[test]
    fn a_guarded_call_returns_its_value_and_a_hung_one_times_out() {
        let ok = run_guarded(|| 41 + 1, Duration::from_secs(5)).unwrap();
        assert_eq!(ok, (42, 0));
        let slow = run_guarded(
            || std::thread::sleep(Duration::from_millis(600)),
            Duration::from_millis(80),
        );
        assert_eq!(slow, Err(GuardError::TimedOut));
        let panicked = run_guarded(|| -> u8 { panic!("boom") }, Duration::from_secs(5));
        assert_eq!(panicked, Err(GuardError::Panicked));
    }

    #[test]
    fn a_failure_survives_the_trip_through_a_core_error() {
        let e = fail(RecycleFailure::Refused(BinReason::BinOff));
        assert_eq!(
            failure_of(&e),
            Some(&RecycleFailure::Refused(BinReason::BinOff))
        );
        assert_eq!(failure_of(&Error::Io(std::io::Error::other("x"))), None);
        assert_eq!(failure_of(&Error::Cancelled), None);
        assert_eq!(RecycleFailure::InUse("x".into()).reason(), BinReason::InUse);
        assert_eq!(
            RecycleFailure::Declined("x".into()).reason(),
            BinReason::Other
        );
    }

    #[test]
    fn the_folder_size_counts_files_below_and_stops_above_the_limit() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("a/b")).unwrap();
        std::fs::write(dir.path().join("x"), vec![0u8; 100]).unwrap();
        std::fs::write(dir.path().join("a/b/y"), vec![0u8; 50]).unwrap();
        assert_eq!(folder_bytes_up_to(dir.path(), 10_000), Some(150));
        let capped = folder_bytes_up_to(dir.path(), 120).unwrap();
        assert!(capped > 120);
        assert_eq!(folder_bytes_up_to(&dir.path().join("missing"), 10), None);
    }

    /// MANUAL, not part of the normal run: opens a real dialog on the screen for a moment.
    /// `cargo test -p hengst-desktop -j 6 recycle::tests::manual_a_yes_no_dialog -- --ignored`
    #[cfg(windows)]
    #[test]
    #[ignore = "opens a real message box"]
    fn manual_a_yes_no_dialog_opened_on_the_guarded_thread_is_answered() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, IDNO, MB_YESNO};
        let (answer, declined) = run_guarded(
            || {
                let text: Vec<u16> = "test dialog\0".encode_utf16().collect();
                unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), text.as_ptr(), MB_YESNO) }
            },
            Duration::from_secs(20),
        )
        .unwrap();
        assert_eq!(answer, IDNO);
        assert!(declined >= 1);
    }

    /// MANUAL, not part of the normal run: moves ONE uniquely named folder under `%TEMP%` to the
    /// real Recycle Bin through the whole path (facts, shell, check) and purges it from the bin
    /// again. `cargo test -p hengst-desktop -j 6 recycle::tests::manual_one_small -- --ignored`
    #[cfg(windows)]
    #[test]
    #[ignore = "uses the real Recycle Bin"]
    fn manual_one_small_temp_folder_goes_through_the_whole_path() {
        let name = format!("hengst-fix10-recycle-{}", std::process::id());
        let folder = std::env::temp_dir().join(&name);
        std::fs::create_dir_all(folder.join("saves")).unwrap();
        std::fs::write(folder.join("instance.json"), b"{}").unwrap();
        let facts = platform::volume_facts(&folder);
        println!("facts: {facts:?}");
        let result = move_to_recycle_bin(&folder);
        println!("result: {result:?}; folder exists: {}", folder.exists());
        let mine: Vec<_> = trash::os_limited::list()
            .unwrap()
            .into_iter()
            .filter(|i| i.name == name.as_str())
            .collect();
        println!("items named {name} in the bin: {}", mine.len());
        trash::os_limited::purge_all(mine).unwrap();
        let _ = std::fs::remove_dir_all(&folder);
        assert!(result.is_ok(), "{result:?}");
    }
}
