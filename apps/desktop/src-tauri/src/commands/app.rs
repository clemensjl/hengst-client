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
        e2e_build: cfg!(feature = "e2e"),
        installed_memory_mb: installed_memory_mb(),
    }
}

#[cfg(windows)]
fn installed_memory_mb() -> Option<u32> {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    // SAFETY: `status` is a writable MEMORYSTATUSEX with `dwLength` set, as the call requires.
    if unsafe { GlobalMemoryStatusEx(&mut status) } == 0 {
        return None;
    }
    u32::try_from(status.ullTotalPhys / (1024 * 1024)).ok()
}

#[cfg(not(windows))]
fn installed_memory_mb() -> Option<u32> {
    None
}

/// Whether `app_quit` may quit now: no game process exists and none is being created (a start
/// still being prepared is cancelled by the quit).
pub fn app_quit_allowed(state: &AppState) -> bool {
    let game = state.game.state();
    crate::tray::quit_allowed(
        &game,
        state
            .operations
            .has_committing(crate::accounts::OpKind::Play),
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use hengst_core::launch::Endpoints;
    use hengst_core::paths::Paths;

    #[test]
    fn app_info_says_whether_this_is_an_e2e_build() {
        // The e2e harness refuses any other build: a plain debug build reaches the real services.
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(Paths::new(dir.path()), Endpoints::default());
        assert_eq!(app_info_impl(&state).e2e_build, cfg!(feature = "e2e"));
    }

    /// A start whose process is being created counts as a running game for the quit dialog.
    #[tokio::test(flavor = "multi_thread")]
    async fn quitting_is_refused_while_a_start_is_committing() {
        use crate::accounts::OpKind;
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(Paths::new(dir.path()), Endpoints::default());
        assert!(app_quit_allowed(&state));
        let op = state.operations.start(OpKind::Play, Box::new(|| {}), |_| {
            tokio::spawn(std::future::pending::<()>()).abort_handle()
        });
        assert!(
            app_quit_allowed(&state),
            "preparing only: the quit cancels it"
        );
        assert!(state.operations.try_commit(op));
        assert!(!app_quit_allowed(&state));
        assert!(state.operations.finish(op));
        assert!(app_quit_allowed(&state));
    }

    #[test]
    fn app_info_reports_the_installed_memory() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(Paths::new(dir.path()), Endpoints::default());
        let mb = app_info_impl(&state).installed_memory_mb.unwrap();
        // Any machine that runs these tests has between 1 GB and 64 TB.
        assert!((1024..64 * 1024 * 1024).contains(&mb), "{mb}");
    }
}
