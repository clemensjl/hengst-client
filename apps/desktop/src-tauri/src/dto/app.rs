use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
    pub dev_build: bool,
    pub dev_offline: bool,
    /// Built with the `e2e` feature (mock services); the e2e harness refuses any other build.
    pub e2e_build: bool,
    /// The machine's physical memory in MiB (`GlobalMemoryStatusEx`), for the memory setting's
    /// help line and its warning; `None` if Windows does not say.
    pub installed_memory_mb: Option<u32>,
}
