use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct VersionFilter {
    pub release: bool,
    pub snapshot: bool,
    pub beta: bool,
    pub alpha: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum GameVersionKind {
    Release,
    Snapshot,
    Beta,
    Alpha,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GameVersionDto {
    pub id: String,
    pub kind: GameVersionKind,
    pub release_time: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GameVersionsDto {
    pub versions: Vec<GameVersionDto>,
    pub latest_release: String,
    pub latest_snapshot: String,
    pub from_cache: bool,
}
