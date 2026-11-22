//! Moving a folder to the Recycle Bin, and never deleting it permanently.
//!
//! `trash` 5.4 on Windows runs `IFileOperation` with `FOF_NO_UI | FOF_ALLOWUNDO |
//! FOF_WANTNUKEWARNING`. When Windows cannot recycle an item (too large for the bin, a drive
//! without one, the setting "Don't move files to the Recycle Bin"), `FOF_WANTNUKEWARNING` makes it
//! ask "permanently delete?" in a modal dialog, even with `FOF_NO_UI`; the call then waits for an
//! answer, and "Yes" deletes for good. That is neither silent nor safe, so a watchdog answers every
//! dialog the shell opens on the calling thread with "No" while the call runs: the item is then
//! skipped and the call fails. (`FOFX_RECYCLEONDELETE` is no way out: measured, it deletes silently
//! and permanently when the item is too large.)

use hengst_core::{Error, Result};
use std::path::Path;

/// The trasher the app uses. Blocking: call it from a blocking thread.
pub fn move_to_recycle_bin(path: &Path) -> Result<()> {
    let (result, declined) = guarded(|| trash::delete(path));
    result.map_err(|e| {
        let why = if declined > 0 {
            "Windows asked to delete the folder permanently instead of moving it to the Recycle Bin; the app declined"
        } else {
            "the Recycle Bin did not take the folder"
        };
        Error::Io(std::io::Error::other(format!("{why}: {e}")))
    })
}

/// Runs `f` on this thread while a watchdog answers every dialog `f` opens on this thread with
/// "No". Returns `f`'s result and the number of dialogs answered.
#[cfg(windows)]
fn guarded<T>(f: impl FnOnce() -> T) -> (T, u32) {
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::Arc;
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;

    let stop = Arc::new(AtomicBool::new(false));
    let answered = Arc::new(AtomicU32::new(0));
    let thread = unsafe { GetCurrentThreadId() };
    let watchdog = {
        let (stop, answered) = (stop.clone(), answered.clone());
        std::thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                let n = decline_dialogs(thread);
                answered.fetch_add(n, Ordering::SeqCst);
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
        })
    };
    // Stops the watchdog also when `f` panics.
    struct Stop(Arc<AtomicBool>);
    impl Drop for Stop {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let result = {
        let _stop = Stop(stop);
        f()
    };
    let _ = watchdog.join();
    (result, answered.load(Ordering::SeqCst))
}

#[cfg(not(windows))]
fn guarded<T>(f: impl FnOnce() -> T) -> (T, u32) {
    (f(), 0)
}

/// Posts "No" (`IDNO`) to every visible dialog (`#32770`) of `thread`; returns how many it found.
#[cfg(windows)]
fn decline_dialogs(thread: u32) -> u32 {
    use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumThreadWindows, GetClassNameW, IsWindowVisible, PostMessageW, WM_COMMAND,
    };
    const IDNO: WPARAM = 7;

    unsafe extern "system" fn each(window: HWND, found: LPARAM) -> i32 {
        let mut class = [0u16; 16];
        let n = unsafe { GetClassNameW(window, class.as_mut_ptr(), class.len() as i32) };
        let class = String::from_utf16_lossy(&class[..usize::try_from(n).unwrap_or(0)]);
        if class == "#32770" && unsafe { IsWindowVisible(window) } != 0 {
            unsafe { PostMessageW(window, WM_COMMAND, IDNO, 0) };
            unsafe { *(found as *mut u32) += 1 };
        }
        1
    }

    let mut found = 0u32;
    unsafe { EnumThreadWindows(thread, Some(each), &mut found as *mut u32 as LPARAM) };
    found
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, IDNO, MB_YESNO};

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    #[test]
    fn a_yes_no_dialog_opened_during_the_call_is_answered_with_no() {
        // The same kind of window the shell opens for "permanently delete?": a modal `#32770`
        // dialog with Yes and No, opened on the thread the guard runs on.
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let (answer, declined) = guarded(|| unsafe {
                let (text, title) = (wide("test dialog"), wide("hengst recycle test"));
                MessageBoxW(
                    std::ptr::null_mut(),
                    text.as_ptr(),
                    title.as_ptr(),
                    MB_YESNO,
                )
            });
            let _ = tx.send((answer, declined));
        });
        let (answer, declined) = rx
            .recv_timeout(std::time::Duration::from_secs(20))
            .expect("the dialog was not answered");
        assert_eq!(answer, IDNO);
        assert!(declined >= 1);
    }

    #[test]
    fn without_a_dialog_the_guard_changes_nothing() {
        let (value, declined) = guarded(|| 41 + 1);
        assert_eq!((value, declined), (42, 0));
    }
}
