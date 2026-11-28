pub(crate) mod fabric;
pub mod forge;
pub(crate) mod merge;

use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use crate::download::{Download, Downloader};
use crate::libraries;
use crate::paths::Paths;
use crate::progress::Progress;
use crate::rules::Env;
use crate::version::VersionJson;
use crate::{Error, Result};

/// Serialized as its `slug()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LoaderKind {
    #[serde(rename = "fabric")]
    Fabric,
    #[serde(rename = "quilt")]
    Quilt,
    #[serde(rename = "legacy-fabric")]
    LegacyFabric,
    #[serde(rename = "neoforge")]
    NeoForge,
    #[serde(rename = "forge")]
    Forge,
}

impl LoaderKind {
    pub fn slug(self) -> &'static str {
        match self {
            LoaderKind::Fabric => "fabric",
            LoaderKind::Quilt => "quilt",
            LoaderKind::LegacyFabric => "legacy-fabric",
            LoaderKind::NeoForge => "neoforge",
            LoaderKind::Forge => "forge",
        }
    }

    /// The name a user sees; the source for every loader name in a UI or a message.
    pub fn display_name(self) -> &'static str {
        match self {
            LoaderKind::Fabric => "Fabric",
            LoaderKind::Quilt => "Quilt",
            LoaderKind::LegacyFabric => "Legacy Fabric",
            LoaderKind::NeoForge => "NeoForge",
            LoaderKind::Forge => "Forge",
        }
    }
}

/// A likely reason for "no loader versions", decided without any request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoaderHint {
    NeoForgeStartsAt1201,
    FabricStartsAt114,
    LegacyFabricServesOld,
}

impl fmt::Display for LoaderHint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            LoaderHint::NeoForgeStartsAt1201 => "NeoForge starts at 1.20.1",
            LoaderHint::FabricStartsAt114 => "Fabric starts at 1.14; try --loader legacy-fabric",
            LoaderHint::LegacyFabricServesOld => {
                "Legacy Fabric serves 1.13.2 and older; try --loader fabric"
            }
        })
    }
}

impl fmt::Display for LoaderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.slug())
    }
}

impl FromStr for LoaderKind {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, String> {
        match s.to_ascii_lowercase().as_str() {
            "fabric" => Ok(LoaderKind::Fabric),
            "quilt" => Ok(LoaderKind::Quilt),
            "legacy-fabric" => Ok(LoaderKind::LegacyFabric),
            "neoforge" => Ok(LoaderKind::NeoForge),
            "forge" => Ok(LoaderKind::Forge),
            _ => Err(format!(
                "unknown loader '{s}' (expected fabric, quilt, legacy-fabric, neoforge or forge)"
            )),
        }
    }
}

/// One installable loader build. `id` is the short form for display and typing, `maven` the full
/// Maven version.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LoaderVersion {
    pub id: String,
    pub maven: String,
    pub stable: bool,
}

#[derive(Clone, Debug)]
pub struct LoaderRequest {
    pub kind: LoaderKind,
    pub version: Option<String>,
}

#[derive(Clone, Debug)]
pub struct LoaderEndpoints {
    pub fabric: String,
    pub quilt: String,
    pub legacy_fabric: String,
    pub forge_files: String,
    pub forge_maven: String,
    pub neoforge_maven: String,
}

impl Default for LoaderEndpoints {
    fn default() -> Self {
        Self {
            fabric: "https://meta.fabricmc.net/v2".into(),
            quilt: "https://meta.quiltmc.org/v3".into(),
            legacy_fabric: "https://meta.legacyfabric.net/v2".into(),
            forge_files: "https://files.minecraftforge.net/net/minecraftforge/forge".into(),
            forge_maven: "https://maven.minecraftforge.net".into(),
            neoforge_maven: "https://maven.neoforged.net".into(),
        }
    }
}

/// The loader build a launch resolved to.
#[derive(Clone, Debug)]
pub struct ResolvedLoader {
    pub kind: LoaderKind,
    pub version: LoaderVersion,
}

/// Everything `prepare` needs from a resolved loader.
#[derive(Debug)]
pub(crate) struct LoaderSetup {
    pub resolved: ResolvedLoader,
    /// The loader's launcher profile, to be merged onto the vanilla version.
    pub child: VersionJson,
    /// Files beyond the profile's libraries that must be fetched (for Forge and NeoForge the
    /// installer's own libraries, processor jars included).
    pub extra_downloads: Vec<Download>,
    /// Forge and NeoForge: the verified installer, finished by `forge::finish_install` once
    /// every download is in place.
    pub forge: Option<forge::ForgeInstall>,
    /// The build was picked from the saved version list because the service was unreachable.
    pub list_from_cache: bool,
}

/// Loader builds for a game version, newest first. Errors with `NoLoaderVersions` when there are
/// none.
pub async fn list_versions(
    client: &reqwest::Client,
    ep: &LoaderEndpoints,
    kind: LoaderKind,
    game: &str,
) -> Result<Vec<LoaderVersion>> {
    let versions = match kind {
        LoaderKind::Fabric | LoaderKind::Quilt | LoaderKind::LegacyFabric => {
            fabric::list_versions(client, ep, kind, game).await?
        }
        LoaderKind::Forge => forge::list_forge(client, ep, game).await?,
        LoaderKind::NeoForge => forge::list_neoforge(client, ep, game).await?,
    };
    if versions.is_empty() {
        return Err(no_versions(kind, game));
    }
    Ok(versions)
}

/// True when `game` (`1.X[.Y]` or a year-based `26.x`) is older than 1.`minor`.`patch`; `None`
/// for ids that cannot be placed (snapshots, pre-releases).
fn older_than(game: &str, minor: u64, patch: u64) -> Option<bool> {
    let mut parts = game.split('.');
    let major: u64 = parts.next()?.parse().ok()?;
    match major {
        0 => None,
        1 => {
            let x: u64 = parts.next()?.parse().ok()?;
            let y: u64 = match parts.next() {
                Some(p) => p.parse().ok()?,
                None => 0,
            };
            Some((x, y) < (minor, patch))
        }
        _ => Some(false),
    }
}

/// `NoLoaderVersions` with a reason for the common mistakes, decided without any request.
pub(crate) fn no_versions(kind: LoaderKind, game: &str) -> Error {
    let hint = match kind {
        LoaderKind::NeoForge if older_than(game, 20, 1) == Some(true) => {
            Some(LoaderHint::NeoForgeStartsAt1201)
        }
        LoaderKind::Fabric if older_than(game, 14, 0) == Some(true) => {
            Some(LoaderHint::FabricStartsAt114)
        }
        LoaderKind::LegacyFabric if older_than(game, 14, 0) == Some(false) => {
            Some(LoaderHint::LegacyFabricServesOld)
        }
        _ => None,
    };
    Error::NoLoaderVersions {
        loader: kind.to_string(),
        game: game.to_string(),
        hint,
    }
}

/// Resolves a loader request for a game version: picks the build and fetches its profile.
/// Fails before anything is downloaded.
pub(crate) async fn prepare_loader(
    dl: &Downloader,
    paths: &Paths,
    ep: &LoaderEndpoints,
    req: &LoaderRequest,
    game: &str,
    env: &Env,
) -> Result<LoaderSetup> {
    dl.report(Progress::ResolvingLoader {
        kind: req.kind,
        game: game.to_string(),
    });
    match req.kind {
        LoaderKind::Fabric | LoaderKind::Quilt | LoaderKind::LegacyFabric => {
            let (version, list_from_cache) = match list_saved(dl, paths, ep, req.kind, game).await {
                Ok((list, cached)) => (
                    pick_version(&list, req.version.as_deref(), req.kind, game)?.clone(),
                    cached,
                ),
                // Service down, no saved list, but the user named a build: the profile fetch
                // falls back to the stored copy. The build is not checked against a list.
                Err(e) => match &req.version {
                    Some(id) if is_unreachable(&e) => (
                        LoaderVersion {
                            id: id.clone(),
                            maven: id.clone(),
                            stable: false,
                        },
                        false,
                    ),
                    _ => return Err(e),
                },
            };
            let child =
                fabric::fetch_profile(dl.client(), paths, ep, req.kind, game, &version.id).await?;
            Ok(LoaderSetup {
                resolved: ResolvedLoader {
                    kind: req.kind,
                    version,
                },
                child,
                extra_downloads: Vec::new(),
                forge: None,
                list_from_cache,
            })
        }
        LoaderKind::Forge | LoaderKind::NeoForge => {
            forge::check_home(paths, req.kind, game)?;
            // A named build needs no version list, so an installed one starts offline.
            let (version, installer, sha1, list_from_cache) = match &req.version {
                Some(requested) => {
                    let (version, installer, sha1) =
                        forge::fetch_named(dl, paths, ep, req.kind, game, requested).await?;
                    (version, installer, sha1, false)
                }
                None => {
                    let (list, cached) = list_saved(dl, paths, ep, req.kind, game).await?;
                    let version = pick_version(&list, None, req.kind, game)?.clone();
                    let (installer, sha1) =
                        forge::fetch_installer(dl, paths, req.kind, game, &version.maven, ep)
                            .await?;
                    (version, installer, sha1, cached)
                }
            };
            let (p, kind, g, maven) = (
                paths.clone(),
                req.kind,
                game.to_string(),
                version.maven.clone(),
            );
            let mut fi = tokio::task::spawn_blocking(move || {
                forge::open_installer(&p, kind, &g, &maven, installer, sha1)
            })
            .await
            .map_err(|e| std::io::Error::other(format!("installer task failed: {e}")))??;
            fi.version = version.id.clone();
            let extra_downloads = match &fi.contents.kind {
                forge::installer::InstallerKind::Modern(profile) => {
                    libraries::resolve(&profile.libraries, env, paths)?.downloads
                }
                forge::installer::InstallerKind::Legacy { .. } => Vec::new(),
            };
            Ok(LoaderSetup {
                resolved: ResolvedLoader {
                    kind: req.kind,
                    version,
                },
                child: fi.contents.version.clone(),
                extra_downloads,
                forge: Some(fi),
                list_from_cache,
            })
        }
    }
}

/// The service could not be reached or failed on its side (not "this does not exist").
fn is_unreachable(e: &Error) -> bool {
    match e {
        Error::Http { .. } => true,
        Error::Status { status, .. } => *status >= 500 || *status == 429,
        _ => false,
    }
}

/// Where the last successful version list of a loader for a game version is kept.
fn saved_list_path(paths: &Paths, kind: LoaderKind, game: &str) -> Result<std::path::PathBuf> {
    crate::paths::validate_id(game)?;
    Ok(paths
        .loader_dir(kind.slug())
        .join(format!("versions-{game}.json")))
}

/// The version list from the service, saved after each success; when the service is unreachable,
/// the saved list (validated like remote data) with `true`.
async fn list_saved(
    dl: &Downloader,
    paths: &Paths,
    ep: &LoaderEndpoints,
    kind: LoaderKind,
    game: &str,
) -> Result<(Vec<LoaderVersion>, bool)> {
    let file = saved_list_path(paths, kind, game)?;
    match list_versions(dl.client(), ep, kind, game).await {
        Ok(list) => {
            // Best effort: a list that cannot be saved only means no offline fallback.
            if let Ok(bytes) = serde_json::to_vec(&list) {
                let _ = crate::paths::write_atomic(&file, &bytes).await;
            }
            Ok((list, false))
        }
        Err(e) if is_unreachable(&e) => {
            let saved = read_bounded(&file, forge::installer::MAX_JSON_BYTES)
                .await
                .and_then(|b| serde_json::from_slice::<Vec<LoaderVersion>>(&b).ok())
                .filter(|l| {
                    !l.is_empty()
                        && l.iter().all(|v| {
                            crate::paths::validate_id(&v.id).is_ok()
                                // A Fabric-family maven is a coordinate (`group:artifact:version`)
                                // that never becomes a path; Forge and NeoForge ones do.
                                && (matches!(
                                    kind,
                                    LoaderKind::Fabric | LoaderKind::Quilt | LoaderKind::LegacyFabric
                                ) || crate::paths::validate_id(&v.maven).is_ok())
                        })
                });
            match saved {
                Some(list) => Ok((list, true)),
                None => Err(e),
            }
        }
        Err(e) => Err(e),
    }
}

/// The file's bytes, or `None` when it cannot be read or is larger than `limit`.
async fn read_bounded(file: &std::path::Path, limit: u64) -> Option<Vec<u8>> {
    use tokio::io::AsyncReadExt;
    let mut buf = Vec::new();
    tokio::fs::File::open(file)
        .await
        .ok()?
        .take(limit + 1)
        .read_to_end(&mut buf)
        .await
        .ok()?;
    (buf.len() as u64 <= limit).then_some(buf)
}

/// An empty segment counts as the number 0, so it equals a missing one.
fn is_num(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_digit())
}

/// Numbers sort before text. Two numbers compare numerically without parsing (so any length is
/// fine), two texts lexically.
fn cmp_segment(a: &str, b: &str) -> Ordering {
    match (is_num(a), is_num(b)) {
        (true, true) => {
            let (a, b) = (a.trim_start_matches('0'), b.trim_start_matches('0'));
            a.len().cmp(&b.len()).then_with(|| a.cmp(b))
        }
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (false, false) => a.cmp(b),
    }
}

/// Compares dot-separated segments; a missing segment counts as `missing`.
fn cmp_segments(a: &str, b: &str, missing: &str) -> Ordering {
    let a: Vec<&str> = a.split('.').collect();
    let b: Vec<&str> = b.split('.').collect();
    for i in 0..a.len().max(b.len()) {
        let ord = cmp_segment(
            a.get(i).copied().unwrap_or(missing),
            b.get(i).copied().unwrap_or(missing),
        );
        if ord != Ordering::Equal {
            return ord;
        }
    }
    Ordering::Equal
}

/// Total ordering for loader version strings. Never panics, whatever the input.
pub(crate) fn compare_versions(a: &str, b: &str) -> Ordering {
    fn split(s: &str) -> (&str, Option<&str>) {
        let s = s.split('+').next().unwrap_or("");
        match s.split_once('-') {
            Some((base, pre)) => (base, Some(pre)),
            None => (s, None),
        }
    }
    let ((base_a, pre_a), (base_b, pre_b)) = (split(a), split(b));
    cmp_segments(base_a, base_b, "0").then_with(|| match (pre_a, pre_b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        // A missing prerelease segment is the empty text, which counts as the number 0: it equals
        // a `0` segment and sorts before any text segment.
        (Some(pa), Some(pb)) => cmp_segments(pa, pb, ""),
    })
}

pub(crate) fn sort_newest_first(v: &mut [LoaderVersion]) {
    v.sort_by(|a, b| compare_versions(&b.id, &a.id));
}

/// Picks the requested version, or the first stable one (else the first) of a newest-first list.
pub fn pick_version<'a>(
    versions: &'a [LoaderVersion],
    requested: Option<&str>,
    kind: LoaderKind,
    game: &str,
) -> Result<&'a LoaderVersion> {
    if versions.is_empty() {
        return Err(no_versions(kind, game));
    }
    match requested {
        Some(req) => versions
            .iter()
            .find(|v| v.id == req || v.maven == req)
            .ok_or_else(|| Error::UnknownLoaderVersion {
                loader: kind.to_string(),
                game: game.to_string(),
                version: req.to_string(),
            }),
        None => Ok(versions.iter().find(|v| v.stable).unwrap_or(&versions[0])),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_real_loader_versions() {
        use std::cmp::Ordering::*;
        assert_eq!(compare_versions("0.31.0-beta.6", "0.30.2"), Greater);
        assert_eq!(compare_versions("0.30.2", "0.30.2-beta.1"), Greater);
        assert_eq!(compare_versions("0.20.0-beta.10", "0.20.0-beta.9"), Greater);
        assert_eq!(compare_versions("26.3.0.115", "26.3.0.71-beta"), Greater);
        assert_eq!(compare_versions("10.13.4.1614", "10.13.4.1566"), Greater);
        assert_eq!(compare_versions("0.4.8+build.155", "0.4.8"), Equal);
        assert_eq!(compare_versions("21.1.267", "21.1.267"), Equal);
    }

    #[test]
    fn comparison_is_total_on_garbage() {
        use std::cmp::Ordering::*;
        let huge = "99999999999999999999999999999999999999";
        let huger = "100000000000000000000000000000000000000";
        assert_eq!(compare_versions(huger, huge), Greater);
        assert_eq!(compare_versions(&format!("1.{huge}"), "1.5"), Greater);
        assert_eq!(compare_versions("1.007", "1.7"), Equal);
        assert_eq!(compare_versions("", ""), Equal);
        assert_eq!(compare_versions("", "0"), Equal);
        assert_eq!(compare_versions("1.0", "1"), Equal);
        assert_eq!(compare_versions("1.2", "1.x"), Less);

        let inputs = [
            "",
            "-",
            "+",
            ".",
            "..",
            "-.",
            "1..2",
            "a",
            "1.a",
            "1_2-3",
            "1.2.3-",
            "-beta",
            "1.2-beta.",
            "1.2-beta.x.9",
            "+build",
            "1+2+3",
            "1-2-3",
            huge,
            huger,
            "1.2.3-beta+meta",
            "x.y.z",
            "0",
            "00",
            "1.0.0.0.0",
        ];
        for a in inputs {
            for b in inputs {
                let ab = compare_versions(a, b);
                assert_eq!(ab, compare_versions(b, a).reverse(), "{a:?} vs {b:?}");
                for c in inputs {
                    if ab == Less && compare_versions(b, c) == Less {
                        assert_eq!(compare_versions(a, c), Less, "{a:?} {b:?} {c:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn parses_and_prints_loader_kinds() {
        assert_eq!(
            "legacy-fabric".parse::<LoaderKind>().unwrap(),
            LoaderKind::LegacyFabric
        );
        assert_eq!(
            "NeoForge".parse::<LoaderKind>().unwrap(),
            LoaderKind::NeoForge
        );
        assert_eq!(LoaderKind::Forge.to_string(), "forge");
        assert!("ornithe"
            .parse::<LoaderKind>()
            .unwrap_err()
            .contains("legacy-fabric"));
    }

    fn v(id: &str, maven: &str, stable: bool) -> LoaderVersion {
        LoaderVersion {
            id: id.into(),
            maven: maven.into(),
            stable,
        }
    }

    #[test]
    fn picks_latest_stable_then_falls_back() {
        let list = [
            v("0.31.0-beta.6", "0.31.0-beta.6", false),
            v("0.30.2", "0.30.2", true),
        ];
        assert_eq!(
            pick_version(&list, None, LoaderKind::Quilt, "1.21.11")
                .unwrap()
                .id,
            "0.30.2"
        );
        let betas = [v("26.1.1.1-beta", "26.1.1.1-beta", false)];
        assert_eq!(
            pick_version(&betas, None, LoaderKind::NeoForge, "26.1.1")
                .unwrap()
                .id,
            "26.1.1.1-beta"
        );
    }

    #[test]
    fn explicit_version_matches_short_or_full_form() {
        let list = [v("10.13.4.1614", "1.7.10-10.13.4.1614-1.7.10", true)];
        for req in ["10.13.4.1614", "1.7.10-10.13.4.1614-1.7.10"] {
            assert_eq!(
                pick_version(&list, Some(req), LoaderKind::Forge, "1.7.10")
                    .unwrap()
                    .maven,
                "1.7.10-10.13.4.1614-1.7.10"
            );
        }
        let err = pick_version(&list, Some("0.0.1"), LoaderKind::Forge, "1.7.10").unwrap_err();
        assert_eq!(
            err.to_string(),
            "forge 0.0.1 is not available for Minecraft 1.7.10 (see `hengst loaders forge 1.7.10`)"
        );
        assert!(matches!(
            pick_version(&[], None, LoaderKind::Fabric, "1.8.9"),
            Err(Error::NoLoaderVersions { .. })
        ));
    }

    #[test]
    fn display_names_are_the_user_facing_names() {
        assert_eq!(LoaderKind::Fabric.display_name(), "Fabric");
        assert_eq!(LoaderKind::Quilt.display_name(), "Quilt");
        assert_eq!(LoaderKind::LegacyFabric.display_name(), "Legacy Fabric");
        assert_eq!(LoaderKind::NeoForge.display_name(), "NeoForge");
        assert_eq!(LoaderKind::Forge.display_name(), "Forge");
    }

    #[test]
    fn hints_are_typed_and_keep_their_text() {
        let e = no_versions(LoaderKind::Fabric, "1.8.9");
        assert!(matches!(
            e,
            Error::NoLoaderVersions {
                hint: Some(LoaderHint::FabricStartsAt114),
                ..
            }
        ));
        assert_eq!(
            e.to_string(),
            "no fabric versions available for Minecraft 1.8.9 (Fabric starts at 1.14; try --loader legacy-fabric)"
        );
        assert!(matches!(
            no_versions(LoaderKind::NeoForge, "1.19.4"),
            Error::NoLoaderVersions {
                hint: Some(LoaderHint::NeoForgeStartsAt1201),
                ..
            }
        ));
        assert!(matches!(
            no_versions(LoaderKind::LegacyFabric, "1.21.11"),
            Error::NoLoaderVersions {
                hint: Some(LoaderHint::LegacyFabricServesOld),
                ..
            }
        ));
        assert_eq!(
            no_versions(LoaderKind::LegacyFabric, "1.21.11").to_string(),
            "no legacy-fabric versions available for Minecraft 1.21.11 (Legacy Fabric serves 1.13.2 and older; try --loader fabric)"
        );
    }

    #[test]
    fn no_versions_names_the_common_mistakes() {
        let m = |kind, game| no_versions(kind, game).to_string();
        assert_eq!(
            m(LoaderKind::NeoForge, "1.19.4"),
            "no neoforge versions available for Minecraft 1.19.4 (NeoForge starts at 1.20.1)"
        );
        assert!(m(LoaderKind::NeoForge, "1.20").contains("starts at 1.20.1"));
        assert!(!m(LoaderKind::NeoForge, "1.20.1").contains('('));
        assert!(m(LoaderKind::Fabric, "1.8.9").contains("--loader legacy-fabric"));
        assert!(m(LoaderKind::LegacyFabric, "1.21.11").contains("--loader fabric"));
        assert!(m(LoaderKind::LegacyFabric, "26.1").contains("--loader fabric"));
        // No hint where the launcher cannot tell, or nothing is known to be wrong.
        for (kind, game) in [
            (LoaderKind::Fabric, "1.21.11"),
            (LoaderKind::Fabric, "24w14a"),
            (LoaderKind::Fabric, "1.14 Pre-Release 1"),
            (LoaderKind::LegacyFabric, "1.8.9"),
            (LoaderKind::LegacyFabric, "24w14a"),
            (LoaderKind::Quilt, "1.8.9"),
            (LoaderKind::Forge, "1.21.11"),
        ] {
            assert!(!m(kind, game).contains('('), "{kind} {game}");
        }
    }

    #[test]
    fn sorts_newest_first() {
        let mut list = [
            v("0.30.2", "0.30.2", true),
            v("0.31.0-beta.6", "0.31.0-beta.6", false),
            v("0.9.0", "0.9.0", true),
        ];
        sort_newest_first(&mut list);
        let ids: Vec<_> = list.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(ids, ["0.31.0-beta.6", "0.30.2", "0.9.0"]);
    }

    #[tokio::test]
    async fn version_list_is_saved_and_used_when_the_service_fails_on_its_side() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let dl = Downloader::new(reqwest::Client::new());
        let serve = |status: u16| async move {
            let server = MockServer::start().await;
            for (p, body) in [
                (
                    "/f/maven-metadata.json",
                    r#"{"1.20.1": ["1.20.1-47.4.28"]}"#,
                ),
                ("/f/promotions_slim.json", r#"{"promos": {}}"#),
            ] {
                Mock::given(method("GET"))
                    .and(path(p))
                    .respond_with(ResponseTemplate::new(status).set_body_string(body))
                    .mount(&server)
                    .await;
            }
            server
        };
        let ep_for = |s: &MockServer| LoaderEndpoints {
            forge_files: format!("{}/f", s.uri()),
            ..LoaderEndpoints::default()
        };
        let up = serve(200).await;
        let (list, cached) = list_saved(&dl, &paths, &ep_for(&up), LoaderKind::Forge, "1.20.1")
            .await
            .unwrap();
        assert!(!cached);
        assert_eq!(list[0].maven, "1.20.1-47.4.28");
        assert!(saved_list_path(&paths, LoaderKind::Forge, "1.20.1")
            .unwrap()
            .starts_with(paths.root().join("meta")));
        for status in [503, 429] {
            let down = serve(status).await;
            let (again, cached) =
                list_saved(&dl, &paths, &ep_for(&down), LoaderKind::Forge, "1.20.1")
                    .await
                    .unwrap();
            assert!(cached, "{status}");
            assert_eq!(again, list);
        }
        // "Does not exist" is an answer, not an outage: no fallback.
        let missing = serve(404).await;
        assert!(
            list_saved(&dl, &paths, &ep_for(&missing), LoaderKind::Forge, "1.20.1")
                .await
                .is_err()
        );
        // A damaged saved list is not used.
        let file = saved_list_path(&paths, LoaderKind::Forge, "1.20.1").unwrap();
        std::fs::write(
            &file,
            r#"[{"id": "../x", "maven": "../x", "stable": true}]"#,
        )
        .unwrap();
        let down = serve(503).await;
        assert!(matches!(
            list_saved(&dl, &paths, &ep_for(&down), LoaderKind::Forge, "1.20.1").await,
            Err(Error::Status { status: 503, .. })
        ));
        // A valid list padded beyond the size limit is not read.
        let mut big = serde_json::to_vec(&list).unwrap();
        big.resize(forge::installer::MAX_JSON_BYTES as usize + 1, b' ');
        std::fs::write(&file, &big).unwrap();
        assert!(serde_json::from_slice::<Vec<LoaderVersion>>(&big).is_ok());
        assert!(matches!(
            list_saved(&dl, &paths, &ep_for(&down), LoaderKind::Forge, "1.20.1").await,
            Err(Error::Status { status: 503, .. })
        ));
    }

    #[test]
    fn default_endpoints_match_the_public_services() {
        let e = LoaderEndpoints::default();
        assert_eq!(e.fabric, "https://meta.fabricmc.net/v2");
        assert_eq!(e.neoforge_maven, "https://maven.neoforged.net");
    }
}
