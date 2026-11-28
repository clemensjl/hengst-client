//! The tray icon and the window while a game runs (DESIGN.md 11.11).
//!
//! | Game state           | Tray icon | Items                                | Tooltip                         |
//! |----------------------|-----------|--------------------------------------|---------------------------------|
//! | Idle, Preparing      | none      | (Open, separator, Quit)              | (no icon)                       |
//! | Running              | shown     | Open, Stop game, separator, Quit     | "Hengst Client: {name} läuft"   |
//! | Stopping             | shown     | Open, separator, Quit                | "Hengst Client: {name} läuft"   |
//!
//! - The game starts (to Running): the icon appears; with `minimize_on_launch` the window hides.
//! - The game ends (Running or Stopping to Idle): the icon goes; a hidden or minimized window is
//!   shown and restored, never focused (Windows may flash its taskbar button instead).
//! - Closing the window while a game runs hides it; otherwise the window closes and the app quits
//!   (a start being prepared is cancelled with the window, `startup::cancel_operations_on`). If the
//!   icon could not be shown, nothing hides into it: the window closes, the game runs on alone.
//! - "Quit" while a game runs (also while it stops) shows and focuses the window and asks the web
//!   view to confirm (`AppEvent::QuitRequested`); without a game it quits at once.
//! - "Stop game" is `play::stop`, so the exit counts as requested. If it reached no window, the
//!   window is shown and `AppEvent::StopNoWindow` tells the start page, which then offers the
//!   force stop: the game was not asked to close.
//!
//! The decisions are plain functions over `GameStateDto` (tested). The adapter gets the game's
//! states through an `EventHub` observer that only puts them into a channel: `Game::set` emits
//! while it holds the game lock, so nothing may run there. One task applies the states in order.
//! Menu and icon callbacks run on the main thread and only start tasks; stopping enumerates
//! windows and runs on a blocking thread inside `play::stop`.

use crate::dto::play::{GameStateDto, StopResult};
use crate::events::AppEvent;
use crate::settings::Language;
use crate::state::AppState;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime, WebviewWindow};

/// An entry of the tray menu, top to bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayItem {
    Open,
    StopGame,
    Separator,
    Quit,
}

/// A game process exists: running, or asked to stop and not exited yet.
pub fn game_runs(state: &GameStateDto) -> bool {
    matches!(
        state,
        GameStateDto::Running { .. } | GameStateDto::Stopping { .. }
    )
}

/// The menu for `state`. "Stop game" only while the game runs and no stop is under way; while it
/// stops the start page offers the force stop.
pub fn tray_items(state: &GameStateDto) -> Vec<TrayItem> {
    if matches!(state, GameStateDto::Running { .. }) {
        vec![
            TrayItem::Open,
            TrayItem::StopGame,
            TrayItem::Separator,
            TrayItem::Quit,
        ]
    } else {
        vec![TrayItem::Open, TrayItem::Separator, TrayItem::Quit]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowChange {
    Keep,
    /// Into the tray: hidden, no taskbar button.
    Hide,
    /// Shown and restored if hidden or minimized, without taking the focus.
    ShowRestore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reaction {
    /// The tray icon exists after the change.
    pub tray: bool,
    pub window: WindowChange,
}

/// What the move from `before` to `after` does to the tray and the window.
pub fn react(before: &GameStateDto, after: &GameStateDto, minimize_on_launch: bool) -> Reaction {
    let window = match (game_runs(before), game_runs(after)) {
        (false, true) if minimize_on_launch => WindowChange::Hide,
        (true, false) => WindowChange::ShowRestore,
        _ => WindowChange::Keep,
    };
    Reaction {
        tray: game_runs(after),
        window,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseAction {
    /// The close is prevented and the window hidden.
    HideToTray,
    /// The window closes and with it the app.
    Close,
}

pub fn on_close_requested(state: &GameStateDto) -> CloseAction {
    if game_runs(state) {
        CloseAction::HideToTray
    } else {
        CloseAction::Close
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuitAction {
    /// Quit now (a start being prepared is cancelled first).
    Exit,
    /// Show the window and let the web view ask first.
    Confirm,
}

pub fn on_quit(state: &GameStateDto) -> QuitAction {
    if game_runs(state) {
        QuitAction::Confirm
    } else {
        QuitAction::Exit
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    De,
    En,
}

/// The language setting, or the Windows display language without one (German for every German
/// variant, English otherwise), as the web view picks it.
pub fn lang_of(setting: Option<Language>) -> Lang {
    match setting {
        Some(Language::De) => Lang::De,
        Some(Language::En) => Lang::En,
        None => system_lang(),
    }
}

#[cfg(windows)]
fn system_lang() -> Lang {
    use windows_sys::Win32::Globalization::GetUserDefaultUILanguage;
    // SAFETY: plain call without arguments.
    let id = unsafe { GetUserDefaultUILanguage() };
    // The primary language is the low 10 bits; LANG_GERMAN is 0x07.
    if id & 0x3ff == 0x07 {
        Lang::De
    } else {
        Lang::En
    }
}

#[cfg(not(windows))]
fn system_lang() -> Lang {
    Lang::En
}

/// The tray's texts live with all others in the web view's translation files.
const DE: &str = include_str!("../../src/i18n/de.json");
const EN: &str = include_str!("../../src/i18n/en.json");

fn table(lang: Lang) -> &'static HashMap<String, String> {
    static DE_TABLE: OnceLock<HashMap<String, String>> = OnceLock::new();
    static EN_TABLE: OnceLock<HashMap<String, String>> = OnceLock::new();
    let (cell, text) = match lang {
        Lang::De => (&DE_TABLE, DE),
        Lang::En => (&EN_TABLE, EN),
    };
    cell.get_or_init(|| serde_json::from_str(text).unwrap_or_default())
}

/// The translation of `key`; the key itself if it is missing (a test keeps that from happening).
pub fn text(lang: Lang, key: &str) -> String {
    table(lang)
        .get(key)
        .cloned()
        .unwrap_or_else(|| key.to_string())
}

pub const TRAY_KEYS: [&str; 4] = [
    "tray.open",
    "tray.stopGame",
    "tray.quit",
    "tray.tooltip.running",
];

/// The label of a menu entry; `None` for the separator.
pub fn label(lang: Lang, item: TrayItem) -> Option<String> {
    let key = match item {
        TrayItem::Open => "tray.open",
        TrayItem::StopGame => "tray.stopGame",
        TrayItem::Quit => "tray.quit",
        TrayItem::Separator => return None,
    };
    Some(text(lang, key))
}

/// Windows shows at most 127 UTF-16 units of a tray tooltip (`NOTIFYICONDATAW::szTip`).
const TOOLTIP_MAX: usize = 127;

/// The embedding, override and isolate controls (U+202A to U+202E, U+2066 to U+2069), which
/// `lib/bidi.ts` drops in the web view too: in a name they could reorder the tooltip.
fn is_bidi_control(c: char) -> bool {
    matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// `name` without bidi controls, cut on a character boundary with "…" to at most `room` UTF-16
/// units.
fn fit_name(name: &str, room: usize) -> String {
    let clean: String = name.chars().filter(|&c| !is_bidi_control(c)).collect();
    if clean.encode_utf16().count() <= room {
        return clean;
    }
    let mut out = String::new();
    let mut used = 1; // the ellipsis
    for c in clean.chars() {
        if used + c.len_utf16() > room {
            break;
        }
        used += c.len_utf16();
        out.push(c);
    }
    out.push('…');
    out
}

/// The instance of the game that runs or stops; `None` otherwise (then there is no tray icon).
fn running_instance(state: &GameStateDto) -> Option<&str> {
    match state {
        GameStateDto::Running { instance, .. } | GameStateDto::Stopping { instance, .. } => {
            Some(instance)
        }
        _ => None,
    }
}

/// "Hengst Client: {name} läuft" with the name of the running instance `instance` (its id when the
/// name could not be read), cleaned and cut so the whole fits the tooltip and the "läuft" / "is
/// running" part always shows. The icon exists only while a game runs, so there is no other text.
pub fn tooltip(lang: Lang, instance: &str, name: Option<&str>) -> String {
    let template = text(lang, "tray.tooltip.running");
    let fixed = template.replace("{{instance}}", "").encode_utf16().count();
    let name = fit_name(name.unwrap_or(instance), TOOLTIP_MAX.saturating_sub(fixed));
    template.replace("{{instance}}", &name)
}

const TRAY_ID: &str = "main";
pub(crate) const MENU_OPEN: &str = "tray-open";
pub(crate) const MENU_STOP: &str = "tray-stop";
pub(crate) const MENU_QUIT: &str = "tray-quit";

/// The tray icon is in the notification area.
pub fn tray_shown<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.tray_by_id(TRAY_ID).is_some()
}

/// Shows the main window; `focus` only when the user asked for it (tray, second start).
pub fn show_window<R: Runtime>(app: &AppHandle<R>, focus: bool) {
    if let Some(window) = app.get_webview_window(crate::startup::WINDOW_LABEL) {
        let _ = window.unminimize();
        let _ = window.show();
        if focus {
            let _ = window.set_focus();
        }
    }
}

/// Shows a hidden window and restores a minimized one, without `set_focus` (which would force it
/// to the foreground). A window that is already shown and not minimized is left alone.
fn restore_without_focus<R: Runtime>(window: &WebviewWindow<R>) {
    if !window.is_visible().unwrap_or(true) {
        let _ = window.show();
    }
    if window.is_minimized().unwrap_or(false) {
        let _ = window.unminimize();
    }
}

/// Quits: what is still being prepared is cancelled, the tray icon is removed on the way out
/// (Tauri drops its tray icons before the exit).
pub fn exit_app<R: Runtime>(app: &AppHandle<R>) {
    app.state::<AppState>().operations.cancel_all();
    app.exit(0);
}

/// Watches the game's states and keeps the tray and the window in step. Call once, after the
/// window exists.
pub fn install<R: Runtime>(app: &AppHandle<R>) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<GameStateDto>();
    let state = app.state::<AppState>();
    state.events.observe(Arc::new(move |e| {
        if let AppEvent::GameState { state } = e {
            let _ = tx.send(state);
        }
    }));
    let mut before = state.game.state();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(after) = rx.recv().await {
            apply(&app, &before, &after).await;
            before = after;
        }
    });
}

async fn instance_name<R: Runtime>(app: &AppHandle<R>, state: &GameStateDto) -> Option<String> {
    let id = running_instance(state)?.to_string();
    let paths = app.state::<AppState>().paths.clone();
    tokio::task::spawn_blocking(move || hengst_core::instances::load(&paths, &id).ok())
        .await
        .ok()
        .flatten()
        .map(|i| i.config.name)
}

async fn apply<R: Runtime>(app: &AppHandle<R>, before: &GameStateDto, after: &GameStateDto) {
    let settings = app.state::<AppState>().settings.get();
    let reaction = react(before, after, settings.minimize_on_launch);
    let lang = lang_of(settings.language);
    let mut tray_shown = false;
    if reaction.tray {
        let name = instance_name(app, after).await;
        match show_tray(app, lang, after, name.as_deref()) {
            Ok(()) => tray_shown = true,
            Err(e) => eprintln!("tray: the tray icon could not be shown: {e}"),
        }
    } else {
        // Dropping the icon removes it from the notification area.
        drop(app.remove_tray_by_id(TRAY_ID));
    }
    let Some(window) = app.get_webview_window(crate::startup::WINDOW_LABEL) else {
        return;
    };
    match reaction.window {
        // Never hide into a tray that is not there: the window would be out of reach.
        WindowChange::Hide if tray_shown => {
            let _ = window.hide();
        }
        WindowChange::ShowRestore => restore_without_focus(&window),
        _ => {}
    }
}

fn menu<R: Runtime>(
    app: &AppHandle<R>,
    lang: Lang,
    state: &GameStateDto,
) -> tauri::Result<Menu<R>> {
    let menu = Menu::new(app)?;
    for item in tray_items(state) {
        let id = match item {
            TrayItem::Open => MENU_OPEN,
            TrayItem::StopGame => MENU_STOP,
            TrayItem::Quit => MENU_QUIT,
            TrayItem::Separator => {
                menu.append(&PredefinedMenuItem::separator(app)?)?;
                continue;
            }
        };
        let text = label(lang, item).unwrap_or_default();
        menu.append(&MenuItem::with_id(app, id, text, true, None::<&str>)?)?;
    }
    Ok(menu)
}

fn show_tray<R: Runtime>(
    app: &AppHandle<R>,
    lang: Lang,
    state: &GameStateDto,
    name: Option<&str>,
) -> tauri::Result<()> {
    let Some(instance) = running_instance(state) else {
        return Ok(());
    };
    let menu = menu(app, lang, state)?;
    let tip = tooltip(lang, instance, name);
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_menu(Some(menu))?;
        return tray.set_tooltip(Some(tip));
    }
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip(tip)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| on_menu(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_window(tray.app_handle(), true);
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// On the main thread: nothing here waits.
pub(crate) fn on_menu<R: Runtime>(app: &AppHandle<R>, id: &str) {
    match id {
        MENU_OPEN => show_window(app, true),
        MENU_STOP => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move { stop_from_tray(&app).await });
        }
        MENU_QUIT => match on_quit(&app.state::<AppState>().game.state()) {
            QuitAction::Confirm => {
                show_window(app, true);
                // Waits for the page if it is reloading right now.
                app.state::<AppState>().events.request_quit();
            }
            QuitAction::Exit => exit_app(app),
        },
        _ => {}
    }
}

async fn stop_from_tray<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>();
    let GameStateDto::Running { run_id, .. } = state.game.state() else {
        return;
    };
    match crate::play::stop(&state.game).await {
        Ok(StopResult::NoWindow) => {
            show_window(app, true);
            state.events.emit(AppEvent::StopNoWindow { run_id });
        }
        Ok(_) => {}
        Err(e) => eprintln!("tray: the game could not be asked to stop: {}", e.detail),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::play::{LastExit, RunId};

    fn idle() -> GameStateDto {
        GameStateDto::Idle { last_exit: None }
    }
    fn ended() -> GameStateDto {
        GameStateDto::Idle {
            last_exit: Some(LastExit {
                instance: "9.9".into(),
                run_id: RunId(1),
                code: 0,
                stop_requested: false,
            }),
        }
    }
    fn preparing() -> GameStateDto {
        GameStateDto::Preparing {
            instance: "9.9".into(),
        }
    }
    fn running() -> GameStateDto {
        GameStateDto::Running {
            instance: "9.9".into(),
            run_id: RunId(1),
            since: 1,
        }
    }
    fn stopping() -> GameStateDto {
        GameStateDto::Stopping {
            instance: "9.9".into(),
            run_id: RunId(1),
        }
    }

    #[test]
    fn tray_menu_lists_stop_game_only_while_a_game_runs() {
        use TrayItem::*;
        assert_eq!(tray_items(&running()), [Open, StopGame, Separator, Quit]);
        for state in [idle(), ended(), preparing(), stopping()] {
            assert_eq!(tray_items(&state), [Open, Separator, Quit], "{state:?}");
        }
    }

    #[test]
    fn close_hides_to_tray_while_a_game_runs() {
        assert_eq!(on_close_requested(&running()), CloseAction::HideToTray);
        assert_eq!(on_close_requested(&stopping()), CloseAction::HideToTray);
        for state in [idle(), ended(), preparing()] {
            assert_eq!(on_close_requested(&state), CloseAction::Close, "{state:?}");
        }
    }

    #[test]
    fn quit_while_running_needs_confirmation() {
        assert_eq!(on_quit(&running()), QuitAction::Confirm);
        assert_eq!(on_quit(&stopping()), QuitAction::Confirm);
        for state in [idle(), ended(), preparing()] {
            assert_eq!(on_quit(&state), QuitAction::Exit, "{state:?}");
        }
    }

    /// The window per state change, with and without "minimize to the notification area".
    #[test]
    fn the_window_hides_on_launch_only_when_asked_and_returns_when_the_game_ends() {
        use WindowChange::*;
        let rows = [
            // before, after, minimize_on_launch, tray, window
            (preparing(), running(), true, true, Hide),
            (preparing(), running(), false, true, Keep),
            (running(), stopping(), true, true, Keep),
            (running(), ended(), true, false, ShowRestore),
            (running(), ended(), false, false, ShowRestore),
            (stopping(), ended(), true, false, ShowRestore),
            (idle(), preparing(), true, false, Keep),
            (preparing(), idle(), true, false, Keep),
            (ended(), preparing(), true, false, Keep),
            (running(), running(), true, true, Keep),
        ];
        for (before, after, minimize, tray, window) in rows {
            assert_eq!(
                react(&before, &after, minimize),
                Reaction { tray, window },
                "{before:?} -> {after:?}, minimize {minimize}"
            );
        }
    }

    #[test]
    fn tooltip_names_the_running_instance_in_both_languages() {
        assert_eq!(
            tooltip(Lang::De, "9.9", Some("Mira_K Welt")),
            "Hengst Client: Mira_K Welt läuft"
        );
        assert_eq!(
            tooltip(Lang::En, "9.9", Some("Mira_K world")),
            "Hengst Client: Mira_K world is running"
        );
        // Without a readable name the id stands in.
        assert_eq!(
            tooltip(Lang::En, "9.9", None),
            "Hengst Client: 9.9 is running"
        );
    }

    /// Bidi controls in a name cannot reorder the tooltip, and a long name is cut so the "läuft" /
    /// "is running" part always fits into the 127 characters Windows shows.
    #[test]
    fn the_tooltip_name_is_cleaned_and_cut_so_the_state_stays_visible() {
        let tricky = "\u{202E}Mira\u{2066}_K\u{2069}\u{202A}";
        assert_eq!(
            tooltip(Lang::En, "9.9", Some(tricky)),
            "Hengst Client: Mira_K is running"
        );
        for lang in [Lang::De, Lang::En] {
            // 268 characters, some outside the BMP (two UTF-16 units each).
            let long: String = "ab\u{1F600}x".repeat(67);
            let tip = tooltip(lang, "9.9", Some(&long));
            assert!(tip.encode_utf16().count() <= 127, "{}", tip.len());
            let end = if lang == Lang::De {
                "… läuft"
            } else {
                "… is running"
            };
            assert!(tip.ends_with(end), "{tip}");
            assert!(tip.starts_with("Hengst Client: ab"), "{tip}");
        }
        // A name that fits is not touched.
        let fits = "x".repeat(64);
        assert!(!tooltip(Lang::En, "9.9", Some(&fits)).contains('…'));
    }

    #[test]
    fn menu_labels_follow_design_in_both_languages() {
        let de: Vec<_> = tray_items(&running())
            .into_iter()
            .map(|i| label(Lang::De, i))
            .collect();
        assert_eq!(
            de,
            [
                Some("Hengst Client öffnen".to_string()),
                Some("Spiel beenden".to_string()),
                None,
                Some("Hengst Client beenden".to_string()),
            ]
        );
        assert_eq!(
            label(Lang::En, TrayItem::Quit).as_deref(),
            Some("Quit Hengst Client")
        );
        assert_eq!(
            label(Lang::En, TrayItem::Open).as_deref(),
            Some("Open Hengst Client")
        );
        assert_eq!(
            label(Lang::En, TrayItem::StopGame).as_deref(),
            Some("Stop game")
        );
    }

    #[test]
    fn every_tray_text_exists_in_both_languages() {
        for lang in [Lang::De, Lang::En] {
            for key in TRAY_KEYS {
                let value = table(lang).get(key);
                assert!(
                    value.is_some_and(|v| !v.trim().is_empty()),
                    "{lang:?} {key}"
                );
            }
        }
    }

    #[test]
    fn an_explicit_language_setting_wins() {
        assert_eq!(lang_of(Some(Language::De)), Lang::De);
        assert_eq!(lang_of(Some(Language::En)), Lang::En);
    }
}
