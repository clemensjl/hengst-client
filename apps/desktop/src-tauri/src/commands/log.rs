use crate::dto::error::ErrorDto;
use crate::dto::log::{LogBatch, LogCopyDto, LogFilter, LogRunDto};
use crate::dto::play::RunId;
use crate::log::LogSink;
use crate::state::AppState;
use std::sync::Arc;
use tauri::ipc::Channel;
use tauri::State;

/// The runs whose log the app holds (the current and the previous one), newest first.
#[tauri::command]
pub fn log_runs(state: State<'_, AppState>) -> Result<Vec<LogRunDto>, ErrorDto> {
    Ok(state.log.runs())
}

/// Sends the log of `run_id` to `channel`: what the buffer holds from `since_seq` on (0: from the
/// oldest line), then new lines as they come (`LogBatch`). Replaces any earlier attach; a page
/// load or the window's end detaches. Only a run id and a number cross from the web view.
#[tauri::command]
pub async fn log_attach(
    state: State<'_, AppState>,
    run_id: RunId,
    since_seq: u64,
    channel: Channel<LogBatch>,
) -> Result<(), ErrorDto> {
    let sink: LogSink = Arc::new(move |b| channel.send(b).map_err(|_| ()));
    state.log.attach(run_id, since_seq, sink)
}

/// A batch with this `ack` token has arrived; the next one can go out.
#[tauri::command]
pub fn log_ack(state: State<'_, AppState>, ack: u64) -> Result<(), ErrorDto> {
    state.log.ack(ack);
    Ok(())
}

/// Stops the batches of the last attach. Idempotent.
#[tauri::command]
pub fn log_detach(state: State<'_, AppState>) -> Result<(), ErrorDto> {
    state.log.detach();
    Ok(())
}

/// The held lines of `run_id` that `filter` lets through, as text for the clipboard (at most
/// 4 MiB, newest lines kept). Built from the redacted buffer.
#[tauri::command]
pub async fn log_copy(
    state: State<'_, AppState>,
    run_id: RunId,
    filter: LogFilter,
) -> Result<LogCopyDto, ErrorDto> {
    state.log.copy(run_id, filter).await
}
