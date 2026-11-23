use crate::dto::error::{ErrorDto, UrlTarget};
use crate::dto::instances::OpenTarget;
use crate::error::{error_dto, instance_id_error};
use crate::open::{
    explore_folder, log_error, open_file, open_folder, open_target, resolve_location,
    resolve_log_path, shell_open_file,
};
use crate::state::AppState;
use tauri::State;
use tauri_plugin_opener::OpenerExt;

/// Opens one of the fixed pages in the browser. The opener plugin is used from Rust only; the
/// capability grants no `opener:` permission.
#[tauri::command]
pub fn open_url<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    target: UrlTarget,
) -> Result<(), ErrorDto> {
    open_target(&target, |url| app.opener().open_url(url, None::<&str>))
}

/// Opens the data folder or the folder of an instance in Explorer. The path is made in Rust from
/// the target (`resolve_location`); an instance that is a link, does not exist or has an invalid id
/// is refused. It is shown with `ShellExecuteW` and the verb "explore" (`explore_folder`), which
/// only exists for folders: a file swapped in for the folder is not run. The opener plugin is not
/// used for this: its "open" verb would run it.
///
/// A log file (`LogFile`) is opened in the program Windows has for `.log` files, after
/// `resolve_log_path` checked the path the web view sent; a refused path is `OpenFailed`.
#[tauri::command]
pub async fn open_location(state: State<'_, AppState>, target: OpenTarget) -> Result<(), ErrorDto> {
    let paths = state.paths.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let OpenTarget::LogFile { path } = &target {
            let file = resolve_log_path(&paths, path).map_err(|e| log_error(&e))?;
            return open_file(&file, shell_open_file);
        }
        let dir = resolve_location(&paths, &target).map_err(|e| match &target {
            OpenTarget::Instance { id } => instance_id_error(&e, id),
            OpenTarget::DataDir | OpenTarget::LogFile { .. } => error_dto(&e),
        })?;
        open_folder(&dir, explore_folder)
    })
    .await
    .map_err(|_| crate::accounts::internal("a background task failed"))?
}
