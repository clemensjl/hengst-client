// The only list of commands. `build.rs` (permissions) and `lib.rs` (handler and `COMMANDS`)
// include this file and call the macro with their own callback.
macro_rules! with_commands {
    ($callback:ident) => {
        $callback!(
            app::app_info,
            app::subscribe_events,
            app::app_quit,
            versions::game_versions,
            open::open_url,
            open::open_location,
            settings::settings_get,
            settings::settings_update,
            settings::java_choose,
            accounts::accounts_list,
            accounts::account_select,
            accounts::account_remove,
            accounts::login_start,
            accounts::login_cancel,
            accounts::login_reopen_browser,
            instances::loader_kinds,
            instances::loader_versions,
            instances::instances_list,
            instances::instance_create,
            instances::instance_select,
            instances::instance_delete,
            play::play,
            play::play_cancel,
            play::game_stop,
            play::game_force_stop,
            play::game_state,
            log::log_runs,
            log::log_attach,
            log::log_ack,
            log::log_detach,
            log::log_copy
        )
    };
}
