//! `e2e` builds only (the feature does not compile in release builds, see lib.rs): what WebDriver
//! cannot do in the tests. The tray menu is a native menu and the window's close button is not part
//! of the page, so the e2e harness asks for those actions through a file in the data folder,
//! `e2e-request.txt`, holding one of `close`, `tray-open`, `tray-stop` or `tray-quit`. The app
//! polls for it every 100 ms, removes it and does what the user's action would do:
//!
//! - `close`: `WebviewWindow::close`, which raises the same `CloseRequested` as the title bar's
//!   close button (hide to the tray while a game runs, else close and quit).
//! - `tray-open`, `tray-stop`, `tray-quit`: the tray menu's handler with that item's id.
//!
//! The harness writes the file through a temp file and a rename, so it is never read half written.
//! Nothing comes back through the file: the tests watch the effect (window, page, process).

use std::path::Path;
use std::time::Duration;
use tauri::{AppHandle, Manager, Runtime};

pub const FILE: &str = "e2e-request.txt";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    Close,
    /// A tray menu item, by its menu id.
    Tray(&'static str),
}

pub fn parse(text: &str) -> Option<Request> {
    Some(match text.trim() {
        "close" => Request::Close,
        "tray-open" => Request::Tray(crate::tray::MENU_OPEN),
        "tray-stop" => Request::Tray(crate::tray::MENU_STOP),
        "tray-quit" => Request::Tray(crate::tray::MENU_QUIT),
        _ => return None,
    })
}

/// The request in `<root>/e2e-request.txt`, which is removed; `None` when there is none. An
/// unknown request is removed too and reported on stderr.
pub fn take(root: &Path) -> Option<Request> {
    let file = root.join(FILE);
    let text = std::fs::read_to_string(&file).ok()?;
    let _ = std::fs::remove_file(&file);
    let request = parse(&text);
    if request.is_none() {
        eprintln!("e2e: unknown request {:?}", text.trim());
    }
    request
}

/// Starts the poll for the request file of the data folder `root`. Call once, after the window
/// and the tray exist.
pub fn install<R: Runtime>(app: &AppHandle<R>, root: std::path::PathBuf) {
    let app = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(100));
        let Some(request) = take(&root) else {
            continue;
        };
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || match request {
            Request::Close => {
                if let Some(window) = handle.get_webview_window(crate::startup::WINDOW_LABEL) {
                    let _ = window.close();
                }
            }
            Request::Tray(id) => crate::tray::on_menu(&handle, id),
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_requests_map_to_the_close_and_the_tray_menu_ids() {
        assert_eq!(parse("close\n"), Some(Request::Close));
        assert_eq!(parse("tray-open"), Some(Request::Tray("tray-open")));
        assert_eq!(parse(" tray-stop "), Some(Request::Tray("tray-stop")));
        assert_eq!(parse("tray-quit"), Some(Request::Tray("tray-quit")));
        for unknown in ["", "quit", "tray-exit", "close close"] {
            assert_eq!(parse(unknown), None, "{unknown:?}");
        }
    }

    #[test]
    fn take_reads_and_removes_the_file_once() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(take(dir.path()), None);
        std::fs::write(dir.path().join(FILE), "tray-quit").unwrap();
        assert_eq!(take(dir.path()), Some(Request::Tray("tray-quit")));
        assert!(!dir.path().join(FILE).exists());
        assert_eq!(take(dir.path()), None);
        // An unknown request is dropped, not retried forever.
        std::fs::write(dir.path().join(FILE), "reboot").unwrap();
        assert_eq!(take(dir.path()), None);
        assert!(!dir.path().join(FILE).exists());
    }
}
