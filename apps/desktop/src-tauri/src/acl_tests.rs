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
    call_with(app, window, cmd, InvokeBody::default())
}

fn call_with(
    app: &tauri::App<tauri::test::MockRuntime>,
    window: &str,
    cmd: &str,
    body: InvokeBody,
) -> Result<serde_json::Value, serde_json::Value> {
    let webview = app.get_webview_window(window).unwrap();
    get_ipc_response(
        &webview,
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "http://tauri.localhost".parse().unwrap(),
            body,
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
    // The opener plugin is registered, so a denied call proves the ACL, not a missing plugin.
    let app = mock_builder()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            crate::commands::app::app_info,
            crate::commands::open::open_url,
            crate::commands::accounts::accounts_list,
            crate::commands::accounts::account_select,
            crate::commands::accounts::account_remove,
            crate::commands::accounts::login_start,
            crate::commands::accounts::login_cancel,
            crate::commands::accounts::login_reopen_browser,
            crate::commands::instances::loader_kinds,
            crate::commands::instances::loader_versions,
            crate::commands::instances::instances_list,
            crate::commands::instances::instance_create,
            crate::commands::instances::instance_select,
            crate::commands::instances::instance_delete,
            crate::commands::open::open_location,
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
    for cmd in ["plugin:opener|open_url", "plugin:window|inner_size"] {
        let err = call(&app, "main", cmd).unwrap_err();
        assert!(err.to_string().contains("not allowed"), "{cmd}: {err}");
    }
}

#[test]
fn open_url_refuses_a_free_url_from_the_web_view() {
    let (app, _dir) = app();
    for body in [
        serde_json::json!({ "target": "https://evil.example/" }),
        serde_json::json!({ "url": "https://evil.example/" }),
    ] {
        let err = call_with(&app, "main", "open_url", InvokeBody::Json(body)).unwrap_err();
        // Granted to the window, but the argument is not a `UrlTarget`: nothing is opened.
        assert!(!err.to_string().contains("not allowed"), "{err}");
    }
}

#[test]
fn the_main_window_may_call_all_six_account_commands_and_others_may_not() {
    let (app, _dir) = app();
    let list = call(&app, "main", "accounts_list").unwrap();
    // The development account is the only possible entry (builds with `dev-offline`).
    let microsoft = list["accounts"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["kind"] == "microsoft")
        .count();
    assert_eq!(microsoft, 0);
    assert_eq!(list["canSignIn"], false);
    let cancelled = call_with(
        &app,
        "main",
        "login_cancel",
        InvokeBody::Json(serde_json::json!({ "op": 5 })),
    );
    assert_eq!(cancelled.unwrap(), serde_json::json!(false));
    // An operation that does not run has no address to open again: a typed error, not a panic.
    let err = call_with(
        &app,
        "main",
        "login_reopen_browser",
        InvokeBody::Json(serde_json::json!({ "op": 5 })),
    )
    .unwrap_err();
    assert_eq!(err["code"], "invalidInput");
    // The other three are granted to the main window too: the call gets as far as its arguments.
    for (cmd, body) in [
        ("account_select", serde_json::json!({ "id": "nobody" })),
        ("account_remove", serde_json::json!({ "id": "nobody" })),
        (
            "login_start",
            serde_json::json!({ "method": "browser", "language": "en" }),
        ),
    ] {
        let err = call_with(&app, "main", cmd, InvokeBody::Json(body)).unwrap_err();
        assert!(!err.to_string().contains("not allowed"), "{cmd}: {err}");
    }
    for cmd in [
        "accounts_list",
        "account_select",
        "account_remove",
        "login_start",
        "login_cancel",
        "login_reopen_browser",
    ] {
        let err = call(&app, "other", cmd).unwrap_err();
        assert!(err.to_string().contains("not allowed"), "{cmd}: {err}");
    }
}

#[test]
fn the_main_window_may_call_the_instance_commands_and_others_may_not() {
    let (app, _dir) = app();
    // Granted: the call gets as far as its arguments and the data (no network is reached with
    // these values).
    let kinds = call(&app, "main", "loader_kinds").unwrap();
    assert_eq!(kinds.as_array().unwrap().len(), 5);
    assert_eq!(
        call(&app, "main", "instances_list").unwrap(),
        serde_json::json!({ "instances": [], "incomplete": false })
    );
    for (cmd, body) in [
        ("instance_select", serde_json::json!({ "id": "nobody" })),
        ("instance_delete", serde_json::json!({ "id": "..\\x" })),
        (
            "open_location",
            serde_json::json!({ "target": { "kind": "instance", "id": "nobody" } }),
        ),
    ] {
        let err = call_with(&app, "main", cmd, InvokeBody::Json(body)).unwrap_err();
        assert_eq!(err["code"], "instanceNotFound", "{cmd}: {err}");
    }
    for cmd in [
        "loader_kinds",
        "loader_versions",
        "instances_list",
        "instance_create",
        "instance_select",
        "instance_delete",
        "open_location",
    ] {
        let err = call(&app, "other", cmd).unwrap_err();
        assert!(err.to_string().contains("not allowed"), "{cmd}: {err}");
    }
}

#[test]
fn open_location_refuses_a_path_from_the_web_view() {
    let (app, _dir) = app();
    for body in [
        serde_json::json!({ "target": { "kind": "path", "path": "C:\\Windows" } }),
        serde_json::json!({ "target": "C:\\Windows" }),
        serde_json::json!({ "path": "C:\\Windows" }),
    ] {
        let err = call_with(&app, "main", "open_location", InvokeBody::Json(body)).unwrap_err();
        // Granted to the window, but the argument is not an `OpenTarget`: nothing is opened.
        assert!(!err.to_string().contains("not allowed"), "{err}");
        assert!(
            err.get("code").is_none(),
            "an argument error, not a command result: {err}"
        );
    }
}
