//! Hengst Client desktop app: a thin Tauri layer over `hengst-core`.

#[cfg(all(feature = "e2e", not(debug_assertions)))]
compile_error!("the `e2e` feature is for debug builds only");

include!("command_list.rs");

macro_rules! names {
    ($($m:ident :: $n:ident),* $(,)?) => {
        &[$(stringify!($n)),*]
    };
}

macro_rules! handler {
    ($($m:ident :: $n:ident),* $(,)?) => {
        tauri::generate_handler![$($crate::commands::$m::$n),*]
    };
}

pub mod commands;
pub mod dto;
pub mod error;
pub mod events;
pub mod startup;
pub mod state;
pub mod window_state;

/// Names of all commands; the capability test and the TypeScript client test compare against it.
pub const COMMANDS: &[&str] = with_commands!(names);

/// The one definition of "development build": debug, `dev-offline` or `e2e`.
pub const fn dev_build() -> bool {
    cfg!(debug_assertions) || cfg!(feature = "dev-offline") || cfg!(feature = "e2e")
}

pub fn run() {
    startup::run();
}
