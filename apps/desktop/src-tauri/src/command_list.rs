// The only list of commands. `build.rs` (permissions) and `lib.rs` (handler and `COMMANDS`)
// include this file and call the macro with their own callback.
macro_rules! with_commands {
    ($callback:ident) => {
        $callback!(
            app::app_info,
            app::subscribe_events,
            versions::game_versions,
            open::open_url,
            settings::settings_get,
            settings::settings_update,
            accounts::accounts_list,
            accounts::account_select,
            accounts::account_remove,
            accounts::login_start,
            accounts::login_cancel,
            accounts::login_reopen_browser
        )
    };
}
