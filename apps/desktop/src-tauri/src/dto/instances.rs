use hengst_core::loaders::LoaderKind;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A loader as the web view names it: the core's slug, nothing else. The display name comes from
/// `LoaderInfoDto` and `InstanceLoaderDto`, so there is one source for loader names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum LoaderKindDto {
    #[serde(rename = "fabric")]
    Fabric,
    #[serde(rename = "quilt")]
    Quilt,
    #[serde(rename = "legacy-fabric")]
    LegacyFabric,
    #[serde(rename = "forge")]
    Forge,
    #[serde(rename = "neoforge")]
    NeoForge,
}

/// The order the loader choice shows them in.
pub const LOADER_ORDER: [LoaderKindDto; 5] = [
    LoaderKindDto::Fabric,
    LoaderKindDto::Quilt,
    LoaderKindDto::LegacyFabric,
    LoaderKindDto::Forge,
    LoaderKindDto::NeoForge,
];

impl From<LoaderKindDto> for LoaderKind {
    fn from(kind: LoaderKindDto) -> LoaderKind {
        match kind {
            LoaderKindDto::Fabric => LoaderKind::Fabric,
            LoaderKindDto::Quilt => LoaderKind::Quilt,
            LoaderKindDto::LegacyFabric => LoaderKind::LegacyFabric,
            LoaderKindDto::Forge => LoaderKind::Forge,
            LoaderKindDto::NeoForge => LoaderKind::NeoForge,
        }
    }
}

impl From<LoaderKind> for LoaderKindDto {
    fn from(kind: LoaderKind) -> LoaderKindDto {
        match kind {
            LoaderKind::Fabric => LoaderKindDto::Fabric,
            LoaderKind::Quilt => LoaderKindDto::Quilt,
            LoaderKind::LegacyFabric => LoaderKindDto::LegacyFabric,
            LoaderKind::Forge => LoaderKindDto::Forge,
            LoaderKind::NeoForge => LoaderKindDto::NeoForge,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LoaderInfoDto {
    pub kind: LoaderKindDto,
    pub display_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LoaderVersionDto {
    pub id: String,
    pub stable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LoaderVersionsDto {
    /// Newest first, as the loader service lists them.
    pub versions: Vec<LoaderVersionDto>,
    /// The id of the build the core would pick without a request (`pick_version`).
    pub recommended: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct InstanceLoaderDto {
    pub kind: LoaderKindDto,
    pub display_name: String,
    /// `None` for a folder adopted from the CLI that has not been started yet.
    pub version: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum InstanceStatus {
    Ready,
    /// Saved by a newer build: listed, never changed or deleted by this one.
    TooNew,
    /// The file is damaged or cannot be read; the folder stays as it is.
    Broken,
}

/// What a row of the instance list shows. Not the folder path, not the Java path and none of the
/// unknown fields of `instance.json`. For `tooNew` and `broken` rows `name` is the id and `game`
/// is empty.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct InstanceDto {
    /// The folder name; the argument of `instance_select`, `instance_delete` and `open_location`.
    pub id: String,
    pub name: String,
    pub game: String,
    pub loader: Option<InstanceLoaderDto>,
    pub memory_mb: Option<u32>,
    /// Unix seconds.
    #[ts(type = "number | null")]
    pub last_played: Option<u64>,
    pub status: InstanceStatus,
    /// Only for `broken`: the core's fixed English text (it quotes no file content), for the
    /// "Details" disclosure.
    pub reason: Option<String>,
    pub selected: bool,
}

/// The answer of `instances_list`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct InstancesDto {
    pub instances: Vec<InstanceDto>,
    /// Some entries of the instances folder could not be read: the list may miss instances.
    pub incomplete: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LoaderRequestDto {
    pub kind: LoaderKindDto,
    /// `None`: the recommended build, which is then saved with the instance.
    pub version: Option<String>,
}

/// The web view sends slugs and ids, never display names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CreateInstanceRequest {
    pub name: String,
    pub game: String,
    pub loader: Option<LoaderRequestDto>,
}

/// A folder the web view may ask the app to open. Paths are chosen in Rust, from the id of an
/// instance or from the data folder; JavaScript never supplies one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OpenTarget {
    DataDir,
    Instance { id: String },
}
