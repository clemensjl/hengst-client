use crate::accounts::{internal, login_pages, page_language};
use crate::dto::accounts::{AccountsDto, LoginEvent, LoginMethod, OperationId};
use crate::dto::error::ErrorDto;
use crate::error::error_dto;
use crate::events::{AppEvent, DataScope, EventSink};
use crate::state::AppState;
use std::sync::Arc;
use tauri::ipc::Channel;
use tauri::State;

fn join_error(_: tauri::Error) -> ErrorDto {
    internal("a background task failed")
}

/// Reads `accounts.json` only. Blocking file I/O runs on a blocking thread.
#[tauri::command]
pub async fn accounts_list(state: State<'_, AppState>) -> Result<AccountsDto, ErrorDto> {
    let (accounts, settings) = (state.accounts.clone(), state.settings.get());
    tauri::async_runtime::spawn_blocking(move || accounts.list(&settings))
        .await
        .map_err(join_error)?
        .map_err(|e| error_dto(&e))
}

/// Selects an account by the id of the list (`accounts_list`).
#[tauri::command]
pub async fn account_select(state: State<'_, AppState>, id: String) -> Result<(), ErrorDto> {
    let (accounts, settings) = (state.accounts.clone(), state.settings.clone());
    tauri::async_runtime::spawn_blocking(move || accounts.select(&id, &settings))
        .await
        .map_err(join_error)?
        .map_err(|e| error_dto(&e))?;
    state.events.emit(AppEvent::DataChanged {
        scope: DataScope::Accounts,
    });
    state.events.emit(AppEvent::DataChanged {
        scope: DataScope::Settings,
    });
    Ok(())
}

/// Signs an account out: its tokens and its entry go.
#[tauri::command]
pub async fn account_remove(state: State<'_, AppState>, id: String) -> Result<(), ErrorDto> {
    let accounts = state.accounts.clone();
    let removed = tauri::async_runtime::spawn_blocking(move || accounts.remove(&id))
        .await
        .map_err(join_error)?;
    // A refused sign-out may still have flagged the entry: the list changed either way.
    state.events.emit(AppEvent::DataChanged {
        scope: DataScope::Accounts,
    });
    removed.map_err(|e| error_dto(&e))
}

/// Starts a sign-in and returns at once; the progress arrives on `channel`, which gets `Started`
/// with the operation id first. A running sign-in is cancelled. The texts of the page the browser
/// shows afterwards are fixed in Rust, in the language of the settings.
#[tauri::command]
pub async fn login_start(
    state: State<'_, AppState>,
    method: LoginMethod,
    channel: Channel<LoginEvent>,
) -> Result<OperationId, ErrorDto> {
    let pages = login_pages(page_language(state.settings.get().language));
    let sink: EventSink<LoginEvent> = Arc::new(move |e| {
        let _ = channel.send(e);
    });
    Ok(state.accounts.start_login(
        &state.operations,
        &state.events,
        method,
        pages,
        state.settings.clone(),
        sink,
    ))
}

/// `true` if the operation was running and is now cancelled (its channel gets `Cancelled`).
#[tauri::command]
pub fn login_cancel(state: State<'_, AppState>, op: OperationId) -> Result<bool, ErrorDto> {
    Ok(state.operations.cancel(op))
}

/// Opens the sign-in page of the running browser sign-in again. The address stays in Rust.
#[tauri::command]
pub async fn login_reopen_browser(
    state: State<'_, AppState>,
    op: OperationId,
) -> Result<(), ErrorDto> {
    let (accounts, operations) = (state.accounts.clone(), state.operations.clone());
    tauri::async_runtime::spawn_blocking(move || accounts.reopen_browser(&operations, op))
        .await
        .map_err(join_error)?
}
