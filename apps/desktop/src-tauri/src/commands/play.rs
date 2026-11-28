use crate::accounts::OpKind;
use crate::dto::accounts::OperationId;
use crate::dto::error::ErrorDto;
use crate::dto::play::{GameStateDto, PlayEvent, StopResult};
use crate::events::EventSink;
use crate::state::AppState;
use std::sync::Arc;
use tauri::ipc::Channel;
use tauri::State;

/// Starts the game of an instance and returns at once; the progress arrives on `channel`, which
/// gets `Preparing` with the operation id first. Only the instance id crosses from the web view:
/// the account, Java, memory and every argument are chosen in Rust. `Busy` while a game is being
/// prepared or runs.
pub async fn play_impl(
    state: &AppState,
    instance_id: String,
    sink: EventSink<PlayEvent>,
) -> Result<OperationId, ErrorDto> {
    crate::play::play(state, instance_id, sink).await
}

#[tauri::command]
pub async fn play(
    state: State<'_, AppState>,
    instance_id: String,
    channel: Channel<PlayEvent>,
) -> Result<OperationId, ErrorDto> {
    let sink: EventSink<PlayEvent> = Arc::new(move |e| {
        let _ = channel.send(e);
    });
    play_impl(&state, instance_id, sink).await
}

/// `true` if the start was still being prepared and is now cancelled (its channel gets
/// `Cancelled`). Once the game process is being started it cannot be cancelled: `false`.
pub fn play_cancel_impl(state: &AppState, op: OperationId) -> bool {
    state.operations.cancel_of(OpKind::Play, op)
}

#[tauri::command]
pub fn play_cancel(state: State<'_, AppState>, op: OperationId) -> Result<bool, ErrorDto> {
    Ok(play_cancel_impl(&state, op))
}

/// Asks the running game to close, as its window's close button would.
#[tauri::command]
pub async fn game_stop(state: State<'_, AppState>) -> Result<StopResult, ErrorDto> {
    crate::play::stop(&state.game).await
}

/// Ends the running game at once (unsaved progress is lost). `false` when no game runs.
#[tauri::command]
pub async fn game_force_stop(state: State<'_, AppState>) -> Result<bool, ErrorDto> {
    crate::play::force_stop(&state.game).await
}

/// The game's state now, the same the last `AppEvent::GameState` carried (after a reload too).
#[tauri::command]
pub fn game_state(state: State<'_, AppState>) -> Result<GameStateDto, ErrorDto> {
    Ok(state.game.state())
}
