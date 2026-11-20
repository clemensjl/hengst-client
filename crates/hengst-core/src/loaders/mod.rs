pub mod fabric;
pub mod forge;
pub mod merge;

use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use crate::download::{Download, Downloader};
use crate::libraries;
use crate::paths::Paths;
use crate::rules::Env;
use crate::version::VersionJson;
use crate::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoaderKind {
    Fabric,
    Quilt,
    LegacyFabric,
    NeoForge,
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
#[derive(Clone, Debug, PartialEq, Eq)]
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
pub struct LoaderSetup {
    pub resolved: ResolvedLoader,
    /// The loader's launcher profile, to be merged onto the vanilla version.
    pub child: VersionJson,
    /// Files beyond the profile's libraries that must be fetched (for Forge and NeoForge the
    /// installer's own libraries, processor jars included).
    pub extra_downloads: Vec<Download>,
    /// Forge and NeoForge: the verified installer, finished by `forge::finish_install` once
    /// every download is in place.
    pub forge: Option<forge::ForgeInstall>,
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
        return Err(Error::NoLoaderVersions {
            loader: kind.to_string(),
            game: game.to_string(),
        });
    }
    Ok(versions)
}

/// Resolves a loader request for a game version: picks the build and fetches its profile.
/// Fails before anything is downloaded.
pub async fn prepare_loader(
    dl: &Downloader,
    paths: &Paths,
    ep: &LoaderEndpoints,
    req: &LoaderRequest,
    game: &str,
    env: &Env,
) -> Result<LoaderSetup> {
    match req.kind {
        LoaderKind::Fabric | LoaderKind::Quilt | LoaderKind::LegacyFabric => {
            let version = match list_versions(dl.client(), ep, req.kind, game).await {
                Ok(list) => pick_version(&list, req.version.as_deref(), req.kind, game)?.clone(),
                // Service down but the user named a build: the profile fetch falls back to the
                // stored copy. The build is not checked against the list.
                Err(e) => match &req.version {
                    Some(id) if fabric::is_unreachable(&e) => LoaderVersion {
                        id: id.clone(),
                        maven: id.clone(),
                        stable: false,
                    },
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
            })
        }
        LoaderKind::Forge | LoaderKind::NeoForge => {
            let list = list_versions(dl.client(), ep, req.kind, game).await?;
            let version = pick_version(&list, req.version.as_deref(), req.kind, game)?.clone();
            let (installer, sha1) =
                forge::fetch_installer(dl, paths, req.kind, game, &version.maven, ep).await?;
            let (p, kind, g, maven) = (
                paths.clone(),
                req.kind,
                game.to_string(),
                version.maven.clone(),
            );
            let fi = tokio::task::spawn_blocking(move || {
                forge::open_installer(&p, kind, &g, &maven, installer, sha1)
            })
            .await
            .expect("installer task panicked")?;
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
            })
        }
    }
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
pub fn compare_versions(a: &str, b: &str) -> Ordering {
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
        // A missing prerelease segment is the empty text, which sorts below any other text.
        (Some(pa), Some(pb)) => cmp_segments(pa, pb, ""),
    })
}

pub fn sort_newest_first(v: &mut [LoaderVersion]) {
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
        return Err(Error::NoLoaderVersions {
            loader: kind.to_string(),
            game: game.to_string(),
        });
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

    #[test]
    fn default_endpoints_match_the_public_services() {
        let e = LoaderEndpoints::default();
        assert_eq!(e.fabric, "https://meta.fabricmc.net/v2");
        assert_eq!(e.neoforge_maven, "https://maven.neoforged.net");
    }
}
