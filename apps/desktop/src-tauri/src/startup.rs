//! App start: debug arguments, plugins, the main window and its saved state.

use crate::state::AppState;
use crate::window_state::{self, Size, WindowState, DEFAULT_SIZE};
use hengst_core::launch::Endpoints;
use hengst_core::paths::Paths;
use std::path::PathBuf;
use tauri::window::Color;
use tauri::{
    AppHandle, LogicalSize, Manager, Monitor, PhysicalPosition, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};

const WINDOW_LABEL: &str = "main";

/// Debug-only start arguments. Release builds ignore all of them (the data folder is then
/// `HENGST_HOME` or the default).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StartupArgs {
    /// `--home <dir>`: the data folder.
    pub home: Option<PathBuf>,
    /// `--window-size <W>x<H>`: the inner size, clamped to the minimum and the work area.
    pub window_size: Option<Size>,
    /// `--e2e-base-url <url>`: wired into the endpoints by Task 7.
    pub e2e_base_url: Option<String>,
}

impl StartupArgs {
    pub fn from_env() -> StartupArgs {
        if cfg!(debug_assertions) {
            StartupArgs::parse(std::env::args().skip(1))
        } else {
            StartupArgs::default()
        }
    }

    pub fn parse(args: impl IntoIterator<Item = String>) -> StartupArgs {
        let mut out = StartupArgs::default();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            let (name, inline) = match arg.split_once('=') {
                Some((n, v)) => (n.to_string(), Some(v.to_string())),
                None => (arg, None),
            };
            if !matches!(name.as_str(), "--home" | "--window-size" | "--e2e-base-url") {
                continue;
            }
            let Some(value) = inline.or_else(|| args.next()) else {
                break;
            };
            match name.as_str() {
                "--home" => out.home = Some(PathBuf::from(value)),
                "--window-size" => out.window_size = parse_size(&value),
                _ => out.e2e_base_url = Some(value),
            }
        }
        out
    }
}

/// `960x600` (also `960X600`).
pub fn parse_size(text: &str) -> Option<Size> {
    let (w, h) = text.split_once(['x', 'X'])?;
    Some(Size {
        width: w.trim().parse().ok()?,
        height: h.trim().parse().ok()?,
    })
}

fn logical(size: u32, scale: f64) -> u32 {
    (f64::from(size) / scale).round() as u32
}

fn logical_size(width: u32, height: u32, scale: f64) -> Size {
    Size {
        width: logical(width, scale),
        height: logical(height, scale),
    }
}

/// Window frame (outer minus inner size) in logical pixels.
fn frame_of(window: &WebviewWindow, scale: f64) -> tauri::Result<Size> {
    let (outer, inner) = (window.outer_size()?, window.inner_size()?);
    Ok(logical_size(
        outer.width.saturating_sub(inner.width),
        outer.height.saturating_sub(inner.height),
        scale,
    ))
}

/// Whether the saved window still overlaps a connected monitor. The saved size is logical, so it
/// is turned into physical pixels with each monitor's own scale factor.
fn saved_fits(saved: &WindowState, monitors: &[Monitor]) -> bool {
    monitors.iter().any(|m| {
        let scale = m.scale_factor();
        let physical = WindowState {
            width: (f64::from(saved.width) * scale).round() as u32,
            height: (f64::from(saved.height) * scale).round() as u32,
            ..saved.clone()
        };
        window_state::fits_on_a_monitor(
            &physical,
            &[(
                m.position().x,
                m.position().y,
                m.size().width,
                m.size().height,
            )],
        )
    })
}

fn create_window(app: &AppHandle, args: &StartupArgs, paths: &Paths) -> tauri::Result<()> {
    let restore = window_state::should_restore(args.window_size.is_some(), cfg!(feature = "e2e"));
    let saved = if restore {
        window_state::load(paths)
    } else {
        None
    };
    let title = if crate::dev_build() {
        "Hengst Client (Development build)"
    } else {
        "Hengst Client"
    };
    // Created invisible and shown once size and position are set. The background is `--bg` of the
    // light theme until Task 3 reads the theme.
    let window = WebviewWindowBuilder::new(app, WINDOW_LABEL, WebviewUrl::default())
        .title(title)
        .visible(false)
        .inner_size(
            f64::from(DEFAULT_SIZE.width),
            f64::from(DEFAULT_SIZE.height),
        )
        .background_color(Color(243, 244, 246, 255))
        .devtools(cfg!(debug_assertions))
        .build()?;

    let monitors = window.available_monitors()?;
    let placed = match &saved {
        Some(s) if saved_fits(s, &monitors) => {
            window.set_position(PhysicalPosition::new(s.x, s.y))?;
            true
        }
        _ => false,
    };
    // The monitor of the saved position, otherwise the primary monitor.
    let monitor = if placed {
        window.current_monitor()?
    } else {
        None
    }
    .or(window.primary_monitor()?);

    if let Some(monitor) = monitor {
        let scale = monitor.scale_factor();
        let area = monitor.work_area();
        let work = logical_size(area.size.width, area.size.height, scale);
        let frame = frame_of(&window, window.scale_factor()?)?;
        let size = window_state::startup_size(
            args.window_size,
            saved.as_ref().map(|s| Size {
                width: s.width,
                height: s.height,
            }),
            restore,
            work,
            frame,
        );
        let min = window_state::min_size(work, frame);
        window.set_min_size(Some(LogicalSize::new(
            f64::from(min.width),
            f64::from(min.height),
        )))?;
        window.set_size(LogicalSize::new(
            f64::from(size.width),
            f64::from(size.height),
        ))?;
        if !placed {
            let outer = window.outer_size()?;
            let x = area.position.x + (area.size.width as i32 - outer.width as i32) / 2;
            let y = area.position.y + (area.size.height as i32 - outer.height as i32) / 2;
            window.set_position(PhysicalPosition::new(
                x.max(area.position.x),
                y.max(area.position.y),
            ))?;
        }
    }
    if restore && saved.as_ref().is_some_and(|s| s.maximized) {
        window.maximize()?;
    }
    window.show()?;

    if restore {
        let watched = window.clone();
        let paths = paths.clone();
        window.on_window_event(move |event| {
            if matches!(event, WindowEvent::CloseRequested { .. }) {
                save_state(&watched, &paths);
            }
        });
    }
    Ok(())
}

/// Remembers size and position when the window closes. A maximized window keeps the size and
/// position saved before (only the flag changes), a minimized one is not saved. Failures are
/// logged, never fatal.
fn save_state(window: &WebviewWindow, paths: &Paths) {
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        if window.is_minimized()? {
            return Ok(());
        }
        let state = if window.is_maximized()? {
            let base = window_state::load(paths).unwrap_or(WindowState {
                format: 1,
                width: DEFAULT_SIZE.width,
                height: DEFAULT_SIZE.height,
                x: 0,
                y: 0,
                maximized: true,
            });
            WindowState {
                maximized: true,
                ..base
            }
        } else {
            let scale = window.scale_factor()?;
            let inner = window.inner_size()?;
            let position = window.outer_position()?;
            let size = logical_size(inner.width, inner.height, scale);
            WindowState {
                format: 1,
                width: size.width,
                height: size.height,
                x: position.x,
                y: position.y,
                maximized: false,
            }
        };
        Ok(window_state::save(paths, &state)?)
    })();
    if let Err(e) = result {
        eprintln!("could not save the window state: {e}");
    }
}

pub fn run() {
    let args = StartupArgs::from_env();
    let paths = Paths::new(args.home.clone().unwrap_or_else(Paths::default_root));
    let state = AppState::new(paths.clone(), Endpoints::default());
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(with_commands!(handler))
        .setup(move |app| {
            create_window(app.handle(), &args, &paths)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running the Hengst Client");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> StartupArgs {
        StartupArgs::parse(args.iter().map(|a| a.to_string()))
    }

    #[test]
    fn debug_arguments_are_read_in_both_spellings() {
        let a = parse(&[
            "--home",
            "C:\\tmp\\h",
            "--window-size=960x600",
            "--e2e-base-url",
            "http://x",
        ]);
        assert_eq!(a.home, Some(PathBuf::from("C:\\tmp\\h")));
        assert_eq!(
            a.window_size,
            Some(Size {
                width: 960,
                height: 600
            })
        );
        assert_eq!(a.e2e_base_url.as_deref(), Some("http://x"));
        assert_eq!(parse(&[]), StartupArgs::default());
    }

    #[test]
    fn broken_arguments_are_ignored() {
        assert_eq!(parse(&["--window-size", "big"]).window_size, None);
        assert_eq!(parse(&["--window-size", "10x"]).window_size, None);
        assert_eq!(parse(&["--home"]).home, None);
        assert_eq!(parse(&["--unknown", "x"]), StartupArgs::default());
    }
}
