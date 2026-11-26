use crate::dto::app::AppInfo;
use crate::dto::error::ErrorDto;
use crate::events::AppEvent;
use crate::state::AppState;
use std::sync::Arc;
use tauri::ipc::Channel;
use tauri::{AppHandle, Runtime, State};

pub fn app_info_impl(state: &AppState) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        data_dir: state.paths.root().display().to_string(),
        dev_build: crate::dev_build(),
        dev_offline: cfg!(feature = "dev-offline"),
    }
}

/// Whether `app_quit` may quit now: no game process exists (a start still being prepared is
/// cancelled by the quit).
pub fn app_quit_allowed(state: &AppState) -> bool {
    !crate::tray::game_runs(&state.game.state())
}

/// Quits the app, the end of the quit dialog (DESIGN.md 11.11) once the game has exited. `false`
/// while a game still runs: nothing happens, the dialog has to stop it first.
#[tauri::command]
pub fn app_quit<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<bool, ErrorDto> {
    if !app_quit_allowed(&state) {
        return Ok(false);
    }
    crate::tray::exit_app(&app);
    Ok(true)
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> Result<AppInfo, ErrorDto> {
    Ok(app_info_impl(&state))
}

/// Replaces any earlier subscription: the backend keeps one subscriber. Operations are not touched
/// here: a page that reloads cancels them through the window's page-load hook
/// (`startup::cancel_operations_on_page_load`).
pub fn subscribe_events_impl(state: &AppState, sink: crate::events::EventSink<AppEvent>) {
    state.events.subscribe(sink);
}

#[tauri::command]
pub fn subscribe_events(
    state: State<'_, AppState>,
    channel: Channel<AppEvent>,
) -> Result<(), ErrorDto> {
    subscribe_events_impl(
        &state,
        Arc::new(move |e| {
            let _ = channel.send(e);
        }),
    );
    Ok(())
}
