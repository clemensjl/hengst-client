//! Core of the Hengst Client launcher: version metadata, downloads,
//! Java runtimes and launch argument construction.
//!
//! # Public surface
//!
//! Only what the CLI, the desktop app, the test kit and the integration tests use is public;
//! everything else is crate-private.
//!
//! - [`launch`]: prepare a version (`prepare*`, `LaunchOptions`, `Endpoints`, `Prepared`), resolve
//!   it from metadata alone (`resolve`, `Resolution`), start it (`run`, `ensure_launch_allowed`).
//! - [`game`] and [`gamelog`]: a started game with its parsed, redacted output.
//! - [`instances`], [`paths`], [`runlock`]: the data folder and the instances in it.
//! - [`manifest`], [`loaders`]: version and loader metadata; [`loaders::forge`] only names the
//!   install marker and log files.
//! - [`auth`]: sign-in (`MicrosoftAuth`, browser and device code), the account store and the
//!   launch account. The raw token chain (`msa`, `xbox`, `minecraft`) is private; only
//!   `DeviceCode`, `MsaTokens` and `Profile` are re-exported.
//! - [`process`]: job objects and the process runners of installer processors
//!   (`ProcessRunner`, `SystemRunner`, `JobRunner`).
//! - [`args::Account`], [`secret::Secret`], [`progress`], [`download::http_client`],
//!   [`download::sha1_hex`], [`libraries::maven_path`], [`Error`] and [`Result`].
//!
//! Types holding tokens (`Secret`, `MsaTokens`, `LoginSession`, `Account`) never print them in
//! `Debug` and do not implement `Serialize`.

pub mod args;
mod assets;
pub mod auth;
pub mod download;
pub mod error;
pub mod game;
pub mod gamelog;
pub mod instances;
mod java;
pub mod launch;
pub mod libraries;
pub mod loaders;
pub mod manifest;
pub mod paths;
pub mod process;
pub mod progress;
mod rules;
pub mod runlock;
pub mod secret;
mod version;

pub use error::{Error, Result};
