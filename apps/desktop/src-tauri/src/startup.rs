//! App start: debug arguments, plugins, navigation guard, the main window and its saved state.

use crate::settings::{window_theme, Theme};
use crate::state::AppState;
use crate::window_state::{self, MonitorArea, Placement, Size, Tracker, WindowState, DEFAULT_SIZE};
use hengst_core::launch::Endpoints;
use hengst_core::paths::Paths;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::window::Color;
use tauri::{
    AppHandle, LogicalSize, Manager, Monitor, PhysicalPosition, Url, WebviewUrl, WebviewWindow,
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

/// Release builds are single-instance. A debug build takes part in the lock only when it works on
/// the default data folder: a start with an explicit, different `--home` (an e2e run next to a
/// development instance) is its own instance.
pub fn single_instance_enabled(debug: bool, home: Option<&Path>, default_root: &Path) -> bool {
    !debug || home.is_none_or(|h| h == default_root)
}

/// The single-instance lock is named after the identifier, so a debug build gets its own and never
/// hands over to an installed release build.
pub fn app_identifier(base: &str, debug: bool) -> String {
    if debug {
        format!("{base}.dev")
    } else {
        base.to_string()
    }
}

/// The origin the app's pages are served from on Windows: `http://tauri.localhost` without a
/// port (`useHttpsScheme` is not set in `tauri.conf.json`, so Tauri's default `false` applies; the
/// `tauri://` and `https://tauri.localhost` forms belong to other platforms or to that option).
const APP_ORIGIN: &str = "http://tauri.localhost";

/// The main window may only show the app's own pages: exactly the app origin and, under the dev
/// condition (`cfg(dev)`), the dev server with its exact origin. Every other
/// navigation (another port, another host, userinfo, `file:`, `data:`, `blob:`, `javascript:`) is
/// refused. Tauri's own local-URL check ignores the port, so this guard is the one that counts.
pub fn navigation_allowed(url: &Url, dev_url: Option<&Url>) -> bool {
    // The scheme is compared too: the origin of a `blob:` URL is the origin of its inner URL.
    let same = |allowed: &Url| url.scheme() == allowed.scheme() && url.origin() == allowed.origin();
    let own = Url::parse(APP_ORIGIN).is_ok_and(|app| same(&app));
    let plain = url.username().is_empty() && url.password().is_none();
    (own || dev_url.is_some_and(same)) && plain
}

/// `--bg` of the resolved theme (DESIGN.md 2.1), as the native window and WebView background. The
/// frontend test `tokens.test.ts` checks these two values against `index.css`.
pub const BG_LIGHT: (u8, u8, u8) = (243, 244, 246);
pub const BG_DARK: (u8, u8, u8) = (18, 20, 23);

pub fn background_color(resolved: tauri::Theme) -> Color {
    let (r, g, b) = match resolved {
        tauri::Theme::Dark => BG_DARK,
        _ => BG_LIGHT,
    };
    Color(r, g, b, 255)
}

/// Sets the title bar theme (`System` follows the system again) and the background that matches
/// the resolved theme.
pub fn apply_window_theme<R: tauri::Runtime>(
    window: &WebviewWindow<R>,
    setting: Theme,
) -> tauri::Result<()> {
    window.set_theme(window_theme(setting))?;
    let resolved = match window_theme(setting) {
        Some(t) => t,
        None => window.theme()?,
    };
    window.set_background_color(Some(background_color(resolved)))
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
fn frame_of(window: &WebviewWindow) -> tauri::Result<Size> {
    let scale = window.scale_factor()?;
    let (outer, inner) = (window.outer_size()?, window.inner_size()?);
    Ok(logical_size(
        outer.width.saturating_sub(inner.width),
        outer.height.saturating_sub(inner.height),
        scale,
    ))
}

fn area_of(m: &Monitor) -> MonitorArea {
    let work = m.work_area();
    MonitorArea {
        x: work.position.x,
        y: work.position.y,
        width: work.size.width,
        height: work.size.height,
        scale: m.scale_factor(),
    }
}

/// The rectangle of a window that is neither maximized nor minimized; `None` otherwise.
fn normal_rectangle(window: &WebviewWindow) -> tauri::Result<Option<WindowState>> {
    if window.is_minimized()? || window.is_maximized()? {
        return Ok(None);
    }
    // Windows parks a minimized window at -32000.
    if window.outer_position()?.x <= -30000 {
        return Ok(None);
    }
    let size = logical_size(
        window.inner_size()?.width,
        window.inner_size()?.height,
        window.scale_factor()?,
    );
    let position = window.outer_position()?;
    Ok(Some(WindowState {
        format: 1,
        width: size.width,
        height: size.height,
        x: position.x,
        y: position.y,
        maximized: false,
    }))
}

fn create_window(
    app: &AppHandle,
    args: &StartupArgs,
    paths: &Paths,
    dev_url: Option<Url>,
    theme: Theme,
) -> tauri::Result<()> {
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
    // Created invisible and shown once size and position are set, with the title bar theme and the
    // `--bg` background of the setting; `System` is resolved right after creation.
    let window = WebviewWindowBuilder::new(app, WINDOW_LABEL, WebviewUrl::default())
        .title(title)
        .visible(false)
        .inner_size(
            f64::from(DEFAULT_SIZE.width),
            f64::from(DEFAULT_SIZE.height),
        )
        .theme(window_theme(theme))
        .background_color(background_color(
            window_theme(theme).unwrap_or(tauri::Theme::Light),
        ))
        // Ctrl+Plus, Ctrl+Minus and the mouse wheel zoom stay on (DESIGN.md 13).
        .zoom_hotkeys_enabled(true)
        .devtools(cfg!(debug_assertions))
        .on_navigation(move |url| navigation_allowed(url, dev_url.as_ref()))
        .build()?;
    apply_window_theme(&window, theme)?;

    let monitors = window.available_monitors()?;
    let areas: Vec<MonitorArea> = monitors.iter().map(area_of).collect();
    let mut normal: Option<WindowState> = None;
    let mut start_maximized = false;
    if !areas.is_empty() {
        // The target monitor: the one the saved window lands on, otherwise the primary monitor.
        let frame = frame_of(&window)?;
        let restoring = saved
            .as_ref()
            .and_then(|s| window_state::restore_monitor(s, frame, &areas))
            .zip(saved.as_ref());
        let primary = window
            .primary_monitor()?
            .and_then(|p| monitors.iter().position(|m| m.position() == p.position()))
            .unwrap_or(0);
        let target = restoring.map_or(primary, |(i, _)| i);
        // Move the invisible window into the target monitor first, so the frame, the scale and the
        // sizes below are those of that monitor.
        window.set_position(PhysicalPosition::new(areas[target].x, areas[target].y))?;
        let frame = frame_of(&window)?;
        let placement: Placement = match restoring {
            Some((i, s)) => window_state::restored(s, &areas, i, frame),
            None => window_state::first_start(&areas, target, args.window_size, frame),
        };
        let min = window_state::min_size(areas[target].work(), frame);
        window.set_min_size(Some(LogicalSize::new(
            f64::from(min.width),
            f64::from(min.height),
        )))?;
        window.set_size(LogicalSize::new(
            f64::from(placement.inner.width),
            f64::from(placement.inner.height),
        ))?;
        window.set_position(PhysicalPosition::new(placement.x, placement.y))?;
        normal = Some(WindowState {
            format: 1,
            width: placement.inner.width,
            height: placement.inner.height,
            x: placement.x,
            y: placement.y,
            maximized: false,
        });
        start_maximized = restoring.is_some_and(|(_, s)| s.maximized);
    }
    if start_maximized {
        window.maximize()?;
    }
    window.show()?;
    // Windows may show the window without activating it, behind the launching terminal.
    window.set_focus()?;
    let (visible, focused) = (window.is_visible()?, window.is_focused()?);
    if cfg!(debug_assertions) {
        eprintln!("[debug] main window after show: visible={visible} focused={focused}");
    }
    if !visible {
        window.show()?;
    }

    if let (true, Some(normal)) = (restore, normal) {
        // The last normal rectangle of this session, see `Tracker`; only the maximized flag is
        // taken from the window at close.
        let tracker = Arc::new(Mutex::new(Tracker::new(normal)));
        let watched = window.clone();
        let paths = paths.clone();
        window.on_window_event(move |event| {
            let mut tracker = tracker.lock().unwrap_or_else(|p| p.into_inner());
            match event {
                WindowEvent::Moved(_) => {
                    tracker.on_moved(normal_rectangle(&watched).ok().flatten());
                }
                WindowEvent::Resized(_) => {
                    let now = normal_rectangle(&watched).ok().flatten();
                    let maximized = watched.is_maximized().unwrap_or(false);
                    tracker.on_resized(now, maximized);
                }
                WindowEvent::CloseRequested { .. } => {
                    let state = tracker.saved();
                    if let Err(e) = window_state::save(&paths, &state) {
                        eprintln!("could not save the window state: {e}");
                    }
                }
                _ => {}
            }
        });
    }
    Ok(())
}

pub fn run() {
    let args = StartupArgs::from_env();
    let paths = Paths::new(args.home.clone().unwrap_or_else(Paths::default_root));
    let state = AppState::new(paths.clone(), Endpoints::default());
    let theme = state.settings.get().theme;
    let mut context = tauri::generate_context!();
    let debug = cfg!(debug_assertions);
    let identifier = app_identifier(&context.config().identifier, debug);
    context.config_mut().identifier = identifier;
    // `dev` is the cfg tauri-build sets when the app is built without `custom-protocol`: only
    // then does the window load the dev server, so only then is its URL allowed.
    let dev_url = if cfg!(dev) {
        context.config().build.dev_url.clone()
    } else {
        None
    };
    let mut builder = tauri::Builder::default();
    if single_instance_enabled(debug, args.home.as_deref(), &Paths::default_root()) {
        // Must be the first plugin.
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }));
    }
    builder
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(with_commands!(handler))
        .setup(move |app| {
            create_window(app.handle(), &args, &paths, dev_url, theme)?;
            Ok(())
        })
        .run(context)
        .expect("error while running the Hengst Client");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> StartupArgs {
        StartupArgs::parse(args.iter().map(|a| a.to_string()))
    }

    fn url(text: &str) -> Url {
        text.parse().unwrap()
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

    #[test]
    fn single_instance_decision() {
        let default = Path::new("C:\\data\\HengstClient");
        let other = Path::new("C:\\tmp\\e2e");
        // release: always one instance, whatever the arguments
        assert!(single_instance_enabled(false, None, default));
        assert!(single_instance_enabled(false, Some(other), default));
        // debug: the default data folder takes part, an explicit different one does not
        assert!(single_instance_enabled(true, None, default));
        assert!(single_instance_enabled(true, Some(default), default));
        assert!(!single_instance_enabled(true, Some(other), default));
    }

    #[test]
    fn debug_builds_have_their_own_identifier() {
        let base = "io.github.clemensjl.hengst-client";
        assert_eq!(app_identifier(base, false), base);
        assert_ne!(app_identifier(base, true), base);
    }

    #[test]
    fn the_window_background_is_bg_of_the_resolved_theme() {
        assert_eq!(
            background_color(tauri::Theme::Light),
            Color(243, 244, 246, 255)
        );
        assert_eq!(background_color(tauri::Theme::Dark), Color(18, 20, 23, 255));
    }

    #[test]
    fn only_the_exact_app_origin_and_in_dev_the_exact_dev_origin_may_be_shown() {
        let dev = url("http://localhost:1420");
        for ok in [
            "http://tauri.localhost/",
            "http://tauri.localhost/index.html",
            "http://tauri.localhost:80/",
            "http://TAURI.LOCALHOST/",
        ] {
            assert!(navigation_allowed(&url(ok), None), "{ok}");
        }
        for bad in [
            "http://tauri.localhost:8080/",
            "https://tauri.localhost/",
            "tauri://localhost/",
            "http://tauri.localhost.evil.example/",
            "http://tauri.localhost@evil.example/",
            "http://evil@tauri.localhost/",
            "http://evil.example/tauri.localhost",
            "https://example.com/",
            "file:///C:/Windows/win.ini",
            "javascript:alert(1)",
            "http://localhost:1420/",
            "data:text/html,x",
            "blob:http://tauri.localhost/1234",
            "about:blank",
        ] {
            assert!(!navigation_allowed(&url(bad), None), "{bad}");
        }
        assert!(navigation_allowed(
            &url("http://localhost:1420/x"),
            Some(&dev)
        ));
        assert!(!navigation_allowed(
            &url("http://localhost:1421/"),
            Some(&dev)
        ));
        assert!(!navigation_allowed(
            &url("https://localhost:1420/"),
            Some(&dev)
        ));
        assert!(!navigation_allowed(
            &url("https://example.com/"),
            Some(&dev)
        ));
        assert!(navigation_allowed(
            &url("http://tauri.localhost/"),
            Some(&dev)
        ));
    }
}
