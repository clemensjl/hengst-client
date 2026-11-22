use crate::dto::error::{ErrorDto, UrlTarget};
use crate::dto::instances::OpenTarget;
use crate::error::error_dto;
use crate::open::{explore_folder, open_folder, open_target, resolve_location};
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
#[tauri::command]
pub async fn open_location(state: State<'_, AppState>, target: OpenTarget) -> Result<(), ErrorDto> {
    let paths = state.paths.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let dir = resolve_location(&paths, &target).map_err(|e| match (&e, &target) {
            (hengst_core::Error::UnsafePath(_), OpenTarget::Instance { id }) => {
                error_dto(&hengst_core::Error::InstanceNotFound { id: id.clone() })
            }
            _ => error_dto(&e),
        })?;
        open_folder(&dir, explore_folder)
    })
    .await
    .map_err(|_| crate::accounts::internal("a background task failed"))?
}
