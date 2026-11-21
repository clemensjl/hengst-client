//! The real capability (`capabilities/main.json` compiled into the context) is enforced by the
//! mock runtime too, so these tests exercise Tauri's access control, not just the JSON file.

use crate::state::AppState;
use hengst_core::launch::Endpoints;
use hengst_core::paths::Paths;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{get_ipc_response, mock_builder, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{Manager, WebviewWindowBuilder};

/// Not in `command_list.rs`, so no permission names it.
#[tauri::command]
fn not_in_the_list() -> &'static str {
    "reached"
}

fn call(
    app: &tauri::App<tauri::test::MockRuntime>,
    window: &str,
    cmd: &str,
) -> Result<serde_json::Value, serde_json::Value> {
    let webview = app.get_webview_window(window).unwrap();
    get_ipc_response(
        &webview,
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "http://tauri.localhost".parse().unwrap(),
            body: InvokeBody::default(),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .map(|body| body.deserialize().unwrap())
    .map_err(|e| serde_json::to_value(e).unwrap())
}

fn app() -> (tauri::App<tauri::test::MockRuntime>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new(Paths::new(dir.path()), Endpoints::default());
    let app = mock_builder()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            crate::commands::app::app_info,
            not_in_the_list
        ])
        .build(tauri::generate_context!())
        .unwrap();
    for label in ["main", "other"] {
        WebviewWindowBuilder::new(&app, label, Default::default())
            .build()
            .unwrap();
    }
    (app, dir)
}

#[test]
fn the_main_window_may_call_a_granted_command() {
    let (app, _dir) = app();
    assert!(call(&app, "main", "app_info").is_ok());
}

#[test]
fn a_window_without_a_capability_may_call_nothing() {
    let (app, _dir) = app();
    let err = call(&app, "other", "app_info").unwrap_err();
    assert!(err.to_string().contains("not allowed"), "{err}");
}

#[test]
fn a_command_that_no_permission_names_is_rejected_for_the_main_window() {
    let (app, _dir) = app();
    let err = call(&app, "main", "not_in_the_list").unwrap_err();
    assert!(err.to_string().contains("not allowed"), "{err}");
}

#[test]
fn plugin_commands_are_not_granted_to_the_main_window() {
    let (app, _dir) = app();
    let err = call(&app, "main", "plugin:opener|open_url").unwrap_err();
    assert!(!err.to_string().is_empty());
    let err = call(&app, "main", "plugin:window|inner_size").unwrap_err();
    assert!(!err.to_string().is_empty());
}
