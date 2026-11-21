use crate::dto::error::{ErrorDto, UrlTarget};
use crate::open::open_target;
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
