//! Reading Forge and NeoForge installers (jars) in both formats. An installer is untrusted remote
//! data: every name taken from it is validated, only the entries the profile itself names as
//! installer-provided are extracted (each verified against the SHA-1 the profile declares, never
//! trusted from disk, never any sidecar file), and entry counts and sizes are capped.
//!
//! The modern format (`install_profile.json` + `version.json` + processors) is used from Forge
//! 1.12.2-14.23.5.2851 and by NeoForge. The legacy format (`install` + `versionInfo` in one JSON)
//! was used up to Forge 1.12.2-14.23.5.2847.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek};
use std::path::Path;

use serde::Deserialize;

use crate::download::sha1_file;
use crate::libraries::maven_path;
use crate::loaders::LoaderKind;
use crate::paths::{join_relative, remove_stale_temps, unique_temp, validate_id, validate_sha1};
use crate::version::{Artifact, Library, LibraryDownloads, VersionJson};
use crate::{Error, Result};

/// Largest single installer entry we read or extract. The biggest real ones (binary patches,
/// universal jars) are about 5 MB.
pub const MAX_ENTRY_BYTES: u64 = 64 * 1024 * 1024;
/// Largest total of bytes extracted from one installer in one `extract_embedded` call.
pub const MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024;
/// Largest JSON document (`install_profile.json`, `version.json`) we parse. Real ones are up to
/// about 130 KB.
pub const MAX_JSON_BYTES: u64 = 16 * 1024 * 1024;
/// Largest number of entries an installer jar may have (real ones: 350 to 650).
pub const MAX_ENTRIES: usize = 4096;

#[derive(Deserialize, Clone, Debug)]
pub struct SidedData {
    pub client: String,
    pub server: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Processor {
    pub jar: String,
    #[serde(default)]
    pub classpath: Vec<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub outputs: BTreeMap<String, String>,
    pub sides: Option<Vec<String>>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ModernProfile {
    pub spec: u32,
    pub minecraft: String,
    pub json: String,
    pub path: Option<String>,
    #[serde(default)]
    pub data: BTreeMap<String, SidedData>,
    #[serde(default)]
    pub processors: Vec<Processor>,
    #[serde(default)]
    pub libraries: Vec<Library>,
}

#[derive(Clone, Debug)]
pub enum InstallerKind {
    Modern(ModernProfile),
    /// `path` is the Maven coordinate of the universal jar, `file_path` its entry in the installer.
    Legacy {
        path: String,
        file_path: String,
        minecraft: String,
    },
}

#[derive(Clone, Debug)]
pub struct InstallerContents {
    pub kind: InstallerKind,
    pub version: VersionJson,
}

/// The installer must be for the game version being installed.
pub fn check_game(contents: &InstallerContents, game: &str) -> Result<()> {
    let minecraft = match &contents.kind {
        InstallerKind::Modern(p) => &p.minecraft,
        InstallerKind::Legacy { minecraft, .. } => minecraft,
    };
    let parent = contents.version.inherits_from.as_deref();
    if minecraft == game && parent == Some(game) {
        return Ok(());
    }
    Err(Error::ParentMismatch {
        expected: game.to_string(),
        found: if minecraft != game {
            minecraft
        } else {
            parent.unwrap_or("none")
        }
        .to_string(),
    })
}

#[derive(Deserialize)]
struct LegacyInstall {
    path: String,
    #[serde(rename = "filePath")]
    file_path: String,
    minecraft: String,
}

// Largest real entries seen: 4.9 MB (legacy universal jar), 3.2 MB (client.lzma).
const _: () =
    assert!(MAX_ENTRY_BYTES >= 32 * 1024 * 1024 && MAX_TOTAL_BYTES >= 4 * MAX_ENTRY_BYTES);

const MAVEN_DIR: &str = "maven/";

fn bad(msg: impl std::fmt::Display) -> Error {
    Error::Installer(msg.to_string())
}

fn open(installer: &Path, max_entries: usize) -> Result<zip::ZipArchive<File>> {
    let zip = zip::ZipArchive::new(File::open(installer)?)
        .map_err(|e| bad(format!("not a readable jar: {e}")))?;
    if zip.len() > max_entries {
        return Err(bad(format!(
            "installer has {} entries, more than the allowed {max_entries}",
            zip.len()
        )));
    }
    Ok(zip)
}

/// Reads a whole entry into memory, refusing it if it is larger than `limit`.
fn read_entry<R: Read + Seek>(
    zip: &mut zip::ZipArchive<R>,
    name: &str,
    limit: u64,
) -> Result<Vec<u8>> {
    let entry = zip
        .by_name(name)
        .map_err(|_| bad(format!("installer has no entry {name}")))?;
    if entry.size() > limit {
        return Err(too_large(name));
    }
    let mut buf = Vec::new();
    entry.take(limit + 1).read_to_end(&mut buf)?;
    if buf.len() as u64 > limit {
        return Err(too_large(name));
    }
    Ok(buf)
}

fn too_large(name: &str) -> Error {
    bad(format!(
        "installer entry {name} exceeds the size limit (per entry {MAX_ENTRY_BYTES} bytes, in total {MAX_TOTAL_BYTES} bytes, JSON {MAX_JSON_BYTES} bytes)"
    ))
}

/// A path or coordinate from the installer must resolve to a plain relative path.
fn check_relative(rel: &str) -> Result<()> {
    join_relative(Path::new("."), rel).map(|_| ())
}

fn check_coordinate(name: &str) -> Result<()> {
    let rel = maven_path(name).ok_or_else(|| Error::UnsafePath(name.to_string()))?;
    check_relative(&rel)
}

fn parse<T: for<'de> Deserialize<'de>>(bytes: &[u8], what: &str) -> Result<T> {
    serde_json::from_slice(bytes).map_err(|e| bad(format!("{what}: {e}")))
}

/// Parses `install_profile.json` (and `version.json` for the modern format) of an installer.
pub fn read_installer(installer: &Path) -> Result<InstallerContents> {
    let mut zip = open(installer, MAX_ENTRIES)?;
    let profile_bytes = read_entry(&mut zip, "install_profile.json", MAX_JSON_BYTES)?;
    let profile: serde_json::Value = parse(&profile_bytes, "install_profile.json")?;

    let contents = if profile.get("install").is_some() {
        #[derive(Deserialize)]
        struct Legacy {
            install: LegacyInstall,
            #[serde(rename = "versionInfo")]
            version_info: VersionJson,
        }
        let Legacy {
            install,
            version_info,
        } = parse(&profile_bytes, "install_profile.json")?;
        check_coordinate(&install.path)?;
        if !install.path.starts_with("net.minecraftforge:forge:") {
            return Err(bad(format!(
                "legacy installer path {} is not a net.minecraftforge:forge coordinate",
                install.path
            )));
        }
        check_relative(&install.file_path)?;
        InstallerContents {
            kind: InstallerKind::Legacy {
                path: install.path,
                file_path: install.file_path,
                minecraft: install.minecraft,
            },
            version: version_info,
        }
    } else {
        let modern: ModernProfile = parse(&profile_bytes, "install_profile.json")?;
        let json_entry = modern.json.trim_start_matches('/').to_string();
        check_relative(&json_entry)?;
        let version: VersionJson = parse(
            &read_entry(&mut zip, &json_entry, MAX_JSON_BYTES)?,
            &json_entry,
        )?;
        if let Some(path) = &modern.path {
            check_coordinate(path)?;
        }
        for p in &modern.processors {
            check_coordinate(&p.jar)?;
            p.classpath.iter().try_for_each(|c| check_coordinate(c))?;
        }
        for l in &modern.libraries {
            check_coordinate(&l.name)?;
        }
        InstallerContents {
            kind: InstallerKind::Modern(modern),
            version,
        }
    };

    validate_id(&contents.version.id)?;
    for l in &contents.version.libraries {
        check_coordinate(&l.name)?;
    }
    Ok(contents)
}

#[derive(Clone, Copy)]
struct Limits {
    entry: u64,
    total: u64,
    entries: usize,
}

const LIMITS: Limits = Limits {
    entry: MAX_ENTRY_BYTES,
    total: MAX_TOTAL_BYTES,
    entries: MAX_ENTRIES,
};

/// Copies one entry to `dest` (via a temp file unique to this call), enforcing the per-entry limit and the remaining
/// total budget. With `sha1` the copied bytes must match it before anything is put at `dest`.
fn extract_entry<R: Read + Seek>(
    zip: &mut zip::ZipArchive<R>,
    name: &str,
    dest: &Path,
    budget: &mut u64,
    limits: Limits,
    sha1: Option<&str>,
) -> Result<u64> {
    let mut entry = zip
        .by_name(name)
        .map_err(|_| bad(format!("installer has no entry {name}")))?;
    let limit = limits.entry.min(limits.total - *budget);
    if entry.size() > limit {
        return Err(too_large(name));
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    remove_stale_temps(dest);
    let tmp = unique_temp(dest);
    let mut out = File::create(&tmp)?;
    let copied = std::io::copy(&mut (&mut entry).take(limit + 1), &mut out);
    drop(out);
    let result = match copied {
        Ok(n) if n > limit => Err(too_large(name)),
        Ok(n) => match sha1 {
            Some(want) if !sha1_file(&tmp)?.eq_ignore_ascii_case(want) => Err(bad(format!(
                "installer entry {name} does not match the SHA-1 {want} its profile declares"
            ))),
            _ => Ok(n),
        },
        Err(e) => Err(e.into()),
    };
    match result {
        Ok(n) => {
            std::fs::rename(&tmp, dest)?;
            *budget += n;
            Ok(n)
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// File names that are checksum or signature sidecars. They are never taken from an installer:
/// a later download would trust a local `<file>.sha1` over the repository.
fn is_sidecar(rel: &str) -> bool {
    let lower = rel.to_ascii_lowercase();
    [
        ".sha1", ".md5", ".sha256", ".sha512", ".sha384", ".asc", ".sig", ".part",
    ]
    .iter()
    .any(|ext| lower.ends_with(ext))
}

/// The installer's own Maven namespace: a provided artifact outside it is refused, so a Forge
/// installer cannot provide a file at another loader's coordinate.
fn namespace(kind: LoaderKind) -> Result<&'static str> {
    match kind {
        LoaderKind::Forge => Ok("net.minecraftforge"),
        LoaderKind::NeoForge => Ok("net.neoforged"),
        other => Err(Error::LoaderUnsupported {
            loader: other.to_string(),
            game: String::new(),
            reason: "it has no installer".into(),
        }),
    }
}

/// The loader's own main artifacts (`group:artifact`). NeoForge for 1.20.1 still publishes under
/// `net.neoforged:forge`.
fn main_artifacts(kind: LoaderKind) -> &'static [&'static str] {
    match kind {
        LoaderKind::Forge => &["net.minecraftforge:forge"],
        LoaderKind::NeoForge => &["net.neoforged:neoforge", "net.neoforged:forge"],
        _ => &[],
    }
}

/// The artifacts a modern profile marks as installer-provided (empty `url`), as relative path and
/// declared SHA-1. Each must be a plain `jar` of the installer's own `group`, its path must be
/// exactly what the coordinate maps to, and the same file (compared case-insensitively) must not
/// be declared twice with different content. A provided artifact of the loader's own main
/// artifact must carry exactly the requested build `maven`. A missing or malformed hash is
/// refused.
fn provided_artifacts(
    profile: &ModernProfile,
    version: &VersionJson,
    kind: LoaderKind,
    maven: &str,
    max: usize,
) -> Result<BTreeMap<String, String>> {
    let group = namespace(kind)?;
    let mut out: BTreeMap<String, (String, String)> = BTreeMap::new();
    let provided = profile
        .libraries
        .iter()
        .chain(&version.libraries)
        .filter_map(|l| Some((&l.name, l.downloads.as_ref()?.artifact.as_ref()?)))
        .filter(|(_, a)| a.url.is_empty());
    for (name, a) in provided {
        if name.split_once('@').is_some_and(|(_, ext)| ext != "jar") {
            return Err(bad(format!("provided artifact {name} is not a jar")));
        }
        if name.split(':').next() != Some(group) {
            return Err(bad(format!("provided artifact {name} is outside {group}")));
        }
        let coord = name.split('@').next().unwrap_or(name);
        let parts: Vec<&str> = coord.split(':').collect();
        if parts.len() >= 3
            && main_artifacts(kind).contains(&format!("{}:{}", parts[0], parts[1]).as_str())
            && parts[2] != maven
        {
            return Err(bad(format!(
                "provided artifact {name} is not the requested build {maven}"
            )));
        }
        let expected = maven_path(name).ok_or_else(|| Error::UnsafePath(name.clone()))?;
        check_relative(&a.path)?;
        if a.path != expected {
            return Err(bad(format!(
                "provided artifact path {} is not the path of {name}",
                a.path
            )));
        }
        if is_sidecar(a.path.rsplit('/').next().unwrap_or(&a.path)) {
            return Err(bad(format!(
                "provided artifact {} is a sidecar file",
                a.path
            )));
        }
        validate_sha1(&a.sha1).map_err(|_| {
            bad(format!(
                "provided artifact {} has no valid SHA-1 in the profile",
                a.path
            ))
        })?;
        let entry = (a.path.clone(), a.sha1.to_ascii_lowercase());
        if out
            .insert(a.path.to_ascii_lowercase(), entry.clone())
            .is_some_and(|old| old != entry)
        {
            return Err(bad(format!(
                "profile declares {} twice with different content or case",
                a.path
            )));
        }
        if out.len() > max {
            return Err(bad("profile lists too many provided artifacts"));
        }
    }
    Ok(out.into_values().collect())
}

/// Extracts what the installer provides for the later steps into the shared libraries directory.
///
/// Modern: only the artifacts the profile marks as installer-provided (empty `url`) are taken from
/// `maven/<path>`. Each is checked against the SHA-1 the profile declares: a matching file on disk
/// is kept, anything else is replaced by the entry from this installer, but only after the entry
/// bytes matched; on a mismatch nothing is left at the destination and it is an error. Every other
/// entry (including sidecars such as `.sha1`) is ignored, so an installer cannot plant files in
/// the cache shared with other loaders. A provided artifact without a valid declared SHA-1 is
/// refused (all seven real installers declare one). Legacy: the universal jar goes to its Maven
/// path and the matching library in `contents.version` is rewritten to point at it (empty `url`,
/// real SHA-1 and size). That jar is rewritten on every install, which fails with an error while a
/// running game holds it open.
pub fn extract_embedded(
    installer: &Path,
    contents: &mut InstallerContents,
    libraries: &Path,
    kind: LoaderKind,
    maven: &str,
) -> Result<()> {
    extract_embedded_with(installer, contents, libraries, kind, maven, LIMITS)
}

fn extract_embedded_with(
    installer: &Path,
    contents: &mut InstallerContents,
    libraries: &Path,
    kind: LoaderKind,
    maven: &str,
    limits: Limits,
) -> Result<()> {
    let group = namespace(kind)?;
    let mut zip = open(installer, limits.entries)?;
    let mut budget = 0u64;
    match &contents.kind {
        InstallerKind::Modern(profile) => {
            validate_id(maven)?;
            let provided =
                provided_artifacts(profile, &contents.version, kind, maven, limits.entries)?;
            for (rel, sha1) in &provided {
                let dest = join_relative(libraries, rel)?;
                if dest.is_file() && sha1_file(&dest).is_ok_and(|h| h == *sha1) {
                    continue;
                }
                // A file with the wrong hash must not survive a failed extraction either.
                let _ = std::fs::remove_file(&dest);
                let name = format!("{MAVEN_DIR}{rel}");
                if zip.by_name(&name).is_err() {
                    return Err(bad(format!(
                        "installer is missing {name}, which its profile marks as provided"
                    )));
                }
                extract_entry(&mut zip, &name, &dest, &mut budget, limits, Some(sha1))?;
            }
        }
        InstallerKind::Legacy {
            path, file_path, ..
        } => {
            validate_id(maven)?;
            let expected = format!("{group}:forge:{maven}");
            if *path != expected || kind != LoaderKind::Forge {
                return Err(bad(format!(
                    "legacy installer path {path} is not {expected}, the build being installed"
                )));
            }
            let rel = maven_path(path).ok_or_else(|| Error::UnsafePath(path.clone()))?;
            let dest = join_relative(libraries, &rel)?;
            let (path, file_path) = (path.clone(), file_path.clone());
            // Always rewritten: the SHA-1 recorded below must be that of this installer jar.
            let size = extract_entry(&mut zip, &file_path, &dest, &mut budget, limits, None)?;
            let artifact = Artifact {
                path: rel,
                url: String::new(),
                sha1: sha1_file(&dest)?,
                size,
            };
            let slot = contents
                .version
                .libraries
                .iter_mut()
                .find(|l| l.name == path)
                .ok_or_else(|| bad(format!("version info has no library {path}")))?;
            slot.downloads = Some(LibraryDownloads {
                artifact: Some(artifact),
                classifiers: None,
            });
            slot.url = None;
        }
    }
    Ok(())
}

/// Extracts single named entries to explicit destinations, sharing one total budget and the usual
/// per-entry limit. Used for the data files the processors read (e.g. `data/client.lzma`); the
/// caller has already validated every destination. The destinations lie in the per-install work
/// directory and are rewritten from the installer on every run (never trusted from disk).
pub fn extract_files(installer: &Path, jobs: &[(String, std::path::PathBuf)]) -> Result<()> {
    let limits = LIMITS;
    let mut zip = open(installer, limits.entries)?;
    let mut budget = 0u64;
    for (name, dest) in jobs {
        extract_entry(&mut zip, name, dest, &mut budget, limits, None)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::download::sha1_hex;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    const LEGACY_189: &str =
        include_str!("../../../tests/fixtures/loaders/forge-1.8.9-install_profile.json");
    const PROFILE_2859: &str =
        include_str!("../../../tests/fixtures/loaders/forge-1.12.2-2859-install_profile.json");
    const VERSION_2859: &str =
        include_str!("../../../tests/fixtures/loaders/forge-1.12.2-2859-version.json");
    const PROFILE_1201: &str =
        include_str!("../../../tests/fixtures/loaders/forge-1.20.1-install_profile.json");
    const VERSION_1201: &str =
        include_str!("../../../tests/fixtures/loaders/forge-1.20.1-version.json");
    const PROFILE_NEO: &str =
        include_str!("../../../tests/fixtures/loaders/neoforge-26.3-install_profile.json");

    fn jar(dir: &Path, name: &str, entries: &[(&str, &[u8])]) -> std::path::PathBuf {
        let path = dir.join(name);
        let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
        for (n, data) in entries {
            zip.start_file(*n, SimpleFileOptions::default()).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
        path
    }

    const FORGE: LoaderKind = LoaderKind::Forge;
    const LEGACY_189_MAVEN: &str = "1.8.9-11.15.1.2318-1.8.9";
    const UNIVERSAL_189: &str = "forge-1.8.9-11.15.1.2318-1.8.9-universal.jar";
    const REAL_SHA_2859: &str = "bfc07a510e23e5181b6a32fdcea9c9d9a1445a46";
    const MAVEN_2859: &str = "1.12.2-14.23.5.2859";
    const FORGE_2859: &str =
        "maven/net/minecraftforge/forge/1.12.2-14.23.5.2859/forge-1.12.2-14.23.5.2859.jar";

    #[test]
    fn reads_legacy_installer_and_converts_universal_jar() {
        let tmp = tempfile::tempdir().unwrap();
        let libs = tmp.path().join("libs");
        let jar = jar(
            tmp.path(),
            "i.jar",
            &[
                ("install_profile.json", LEGACY_189.as_bytes()),
                (UNIVERSAL_189, b"universal"),
            ],
        );
        let mut c = read_installer(&jar).unwrap();
        assert!(matches!(&c.kind, InstallerKind::Legacy { minecraft, .. } if minecraft == "1.8.9"));
        assert_eq!(c.version.main_class, "net.minecraft.launchwrapper.Launch");
        assert!(c
            .version
            .minecraft_arguments
            .as_ref()
            .unwrap()
            .contains("net.minecraftforge.fml.common.launcher.FMLTweaker"));
        extract_embedded(&jar, &mut c, &libs, FORGE, LEGACY_189_MAVEN).unwrap();
        let rel =
            "net/minecraftforge/forge/1.8.9-11.15.1.2318-1.8.9/forge-1.8.9-11.15.1.2318-1.8.9.jar";
        assert_eq!(std::fs::read(libs.join(rel)).unwrap(), b"universal");
        let forge = c
            .version
            .libraries
            .iter()
            .find(|l| l.name.starts_with("net.minecraftforge:forge:"))
            .unwrap();
        let a = forge.downloads.as_ref().unwrap().artifact.as_ref().unwrap();
        assert_eq!(a.path, rel);
        assert_eq!(
            (a.url.as_str(), a.sha1.clone()),
            ("", sha1_hex(b"universal"))
        );
        assert_eq!(a.size, 9);
        assert!(forge.url.is_none());
        check_game(&c, "1.8.9").unwrap();
        assert!(matches!(
            check_game(&c, "1.8.8"),
            Err(Error::ParentMismatch { .. })
        ));
    }

    #[test]
    fn legacy_libraries_keep_their_checksums() {
        let tmp = tempfile::tempdir().unwrap();
        let jar = jar(
            tmp.path(),
            "i.jar",
            &[("install_profile.json", LEGACY_189.as_bytes())],
        );
        let c = read_installer(&jar).unwrap();
        let jline = c
            .version
            .libraries
            .iter()
            .find(|l| l.name == "jline:jline:2.13")
            .unwrap();
        assert_eq!(jline.clientreq, Some(false));
        assert_eq!(
            jline.checksums,
            ["2d9530d0a25daffaffda7c35037b046b627bb171"]
        );
    }

    #[test]
    fn reads_modern_spec0_without_processors() {
        let tmp = tempfile::tempdir().unwrap();
        let libs = tmp.path().join("libs");
        let declared = sha1_hex(b"forge");
        let profile = PROFILE_2859.replace(REAL_SHA_2859, &declared);
        let version = VERSION_2859.replace(REAL_SHA_2859, &declared);
        let jar = jar(
            tmp.path(),
            "i.jar",
            &[
                ("install_profile.json", profile.as_bytes()),
                ("version.json", version.as_bytes()),
                (FORGE_2859, b"forge"),
            ],
        );
        let mut c = read_installer(&jar).unwrap();
        let InstallerKind::Modern(p) = &c.kind else {
            panic!("expected the modern format")
        };
        assert_eq!((p.spec, p.processors.len()), (0, 0));
        assert_eq!(c.version.id, "1.12.2-forge-14.23.5.2859");
        extract_embedded(&jar, &mut c, &libs, FORGE, MAVEN_2859).unwrap();
        assert_eq!(
            std::fs::read(libs.join(
                "net/minecraftforge/forge/1.12.2-14.23.5.2859/forge-1.12.2-14.23.5.2859.jar"
            ))
            .unwrap(),
            b"forge"
        );
        check_game(&c, "1.12.2").unwrap();
    }

    #[test]
    fn reads_modern_spec1_with_processors_and_sides() {
        let tmp = tempfile::tempdir().unwrap();
        let jar = jar(
            tmp.path(),
            "i.jar",
            &[
                ("install_profile.json", PROFILE_1201.as_bytes()),
                ("version.json", VERSION_1201.as_bytes()),
            ],
        );
        let c = read_installer(&jar).unwrap();
        let InstallerKind::Modern(p) = &c.kind else {
            panic!("expected the modern format")
        };
        assert_eq!(p.spec, 1);
        assert_eq!(p.processors.len(), 10);
        let client_side = p
            .processors
            .iter()
            .filter(|x| {
                x.sides
                    .as_ref()
                    .is_none_or(|s| s.iter().any(|s| s == "client"))
            })
            .count();
        assert_eq!(client_side, 6);
        assert_eq!(
            p.data["PATCHED"].client,
            "[net.minecraftforge:forge:1.20.1-47.4.28:client]"
        );
        assert_eq!(
            c.version.main_class,
            "cpw.mods.bootstraplauncher.BootstrapLauncher"
        );
        assert!(p.path.is_none());
        check_game(&c, "1.20.1").unwrap();
    }

    #[test]
    fn reads_neoforge_profile_with_slash_json_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let version = VERSION_1201.replace("1.20.1-forge-47.4.28", "neoforge-26.3.0.115");
        let jar = jar(
            tmp.path(),
            "i.jar",
            &[
                ("install_profile.json", PROFILE_NEO.as_bytes()),
                ("version.json", version.as_bytes()),
            ],
        );
        let c = read_installer(&jar).unwrap();
        let InstallerKind::Modern(p) = &c.kind else {
            panic!("expected the modern format")
        };
        assert_eq!(
            (p.spec, p.json.as_str(), p.processors.len()),
            (1, "/version.json", 2)
        );
        assert_eq!(p.data["BINPATCH"].client, "/data/client.lzma");
        assert_eq!(c.version.id, "neoforge-26.3.0.115");
    }

    #[test]
    fn missing_provided_entry_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let libs = tmp.path().join("libs");
        // The profile marks the Forge jar as installer-provided, but the jar lacks it.
        let jar = jar(
            tmp.path(),
            "i.jar",
            &[
                ("install_profile.json", PROFILE_2859.as_bytes()),
                ("version.json", VERSION_2859.as_bytes()),
            ],
        );
        let mut c = read_installer(&jar).unwrap();
        let err = extract_embedded(&jar, &mut c, &libs, FORGE, MAVEN_2859).unwrap_err();
        assert!(
            matches!(&err, Error::Installer(m) if m.contains("forge-1.12.2-14.23.5.2859.jar")),
            "{err}"
        );
    }

    #[test]
    fn missing_legacy_universal_entry_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let jar = jar(
            tmp.path(),
            "i.jar",
            &[("install_profile.json", LEGACY_189.as_bytes())],
        );
        let mut c = read_installer(&jar).unwrap();
        let err = extract_embedded(
            &jar,
            &mut c,
            &tmp.path().join("libs"),
            FORGE,
            LEGACY_189_MAVEN,
        )
        .unwrap_err();
        assert!(
            matches!(&err, Error::Installer(m) if m.contains(UNIVERSAL_189)),
            "{err}"
        );
    }

    #[test]
    fn missing_json_entries_and_broken_json_are_installer_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let only_profile = jar(
            tmp.path(),
            "a.jar",
            &[("install_profile.json", PROFILE_2859.as_bytes())],
        );
        assert!(matches!(
            read_installer(&only_profile),
            Err(Error::Installer(_))
        ));
        let nothing = jar(tmp.path(), "b.jar", &[("readme.txt", b"x")]);
        assert!(matches!(read_installer(&nothing), Err(Error::Installer(_))));
        let broken = jar(tmp.path(), "c.jar", &[("install_profile.json", b"{ nope")]);
        assert!(matches!(read_installer(&broken), Err(Error::Installer(_))));
        let not_a_jar = tmp.path().join("d.jar");
        std::fs::write(&not_a_jar, b"plain text").unwrap();
        assert!(matches!(
            read_installer(&not_a_jar),
            Err(Error::Installer(_))
        ));
    }

    #[test]
    fn refuses_unsafe_version_id_and_coordinates() {
        let tmp = tempfile::tempdir().unwrap();
        let bad_version = VERSION_2859.replace("1.12.2-forge-14.23.5.2859", "../evil");
        let jar1 = jar(
            tmp.path(),
            "a.jar",
            &[
                ("install_profile.json", PROFILE_2859.as_bytes()),
                ("version.json", bad_version.as_bytes()),
            ],
        );
        assert!(matches!(read_installer(&jar1), Err(Error::UnsafePath(_))));

        let bad_profile = PROFILE_1201.replace(
            "net.minecraftforge:installertools:1.4.1",
            "net.minecraftforge:installertools:../../x",
        );
        assert_ne!(bad_profile, PROFILE_1201);
        let jar2 = jar(
            tmp.path(),
            "b.jar",
            &[
                ("install_profile.json", bad_profile.as_bytes()),
                ("version.json", VERSION_1201.as_bytes()),
            ],
        );
        assert!(matches!(read_installer(&jar2), Err(Error::UnsafePath(_))));

        let bad_legacy = LEGACY_189.replacen(
            "\"path\": \"net.minecraftforge:forge:1.8.9-11.15.1.2318-1.8.9\"",
            "\"path\": \"net.minecraftforge:forge:../../x\"",
            1,
        );
        assert_ne!(bad_legacy, LEGACY_189);
        let jar3 = jar(
            tmp.path(),
            "c.jar",
            &[("install_profile.json", bad_legacy.as_bytes())],
        );
        assert!(matches!(read_installer(&jar3), Err(Error::UnsafePath(_))));
    }

    #[test]
    fn oversized_json_entries_are_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let mut big = b"{\"x\":\"".to_vec();
        big.resize(MAX_JSON_BYTES as usize + 16, b'a');
        let jar = jar(tmp.path(), "a.jar", &[("install_profile.json", &big)]);
        let err = read_installer(&jar).unwrap_err();
        assert!(
            matches!(&err, Error::Installer(m) if m.contains("size limit")),
            "{err}"
        );
    }

    /// The coordinate whose Maven path is `path`, when `path` has the usual layout; otherwise a
    /// coordinate that does not match it.
    fn coord_for(path: &str) -> String {
        let seg: Vec<&str> = path.split('/').collect();
        if let [group @ .., artifact, version, file] = seg.as_slice() {
            if !group.is_empty() && *file == format!("{artifact}-{version}.jar") {
                return format!("{}:{artifact}:{version}", group.join("."));
            }
        }
        "net.minecraftforge:x:1".to_string()
    }

    type Lib<'a> = (&'a str, Option<&'a str>, &'a [u8]);

    /// A modern installer jar whose profile marks every `libs` entry (path, declared SHA-1
    /// override, jar bytes) as installer-provided, under the coordinate that belongs to the path.
    fn synth(dir: &Path, libs: &[Lib], extra: &[(&str, &[u8])]) -> std::path::PathBuf {
        let named: Vec<(String, &str, Option<&str>, &[u8])> = libs
            .iter()
            .map(|(p, s, b)| (coord_for(p), *p, *s, *b))
            .collect();
        let named: Vec<_> = named
            .iter()
            .map(|(n, p, s, b)| (n.as_str(), *p, *s, *b))
            .collect();
        synth_named(dir, &named, extra)
    }

    /// Like `synth`, with the library name given explicitly.
    fn synth_named(
        dir: &Path,
        libs: &[(&str, &str, Option<&str>, &[u8])],
        extra: &[(&str, &[u8])],
    ) -> std::path::PathBuf {
        let libraries: Vec<_> = libs
            .iter()
            .map(|(name, path, sha1, bytes)| {
                serde_json::json!({
                    "name": name,
                    "downloads": {"artifact": {
                        "path": path,
                        "url": "",
                        "sha1": sha1.map_or_else(|| sha1_hex(bytes), str::to_string),
                        "size": bytes.len()
                    }}
                })
            })
            .collect();
        let profile = serde_json::json!({
            "spec": 1, "minecraft": "1.20.1", "json": "/version.json", "libraries": libraries
        })
        .to_string();
        let version = serde_json::json!({
            "id": "t-1", "mainClass": "m", "inheritsFrom": "1.20.1", "libraries": []
        })
        .to_string();
        let names: Vec<String> = libs.iter().map(|(_, p, ..)| format!("maven/{p}")).collect();
        let mut entries: Vec<(&str, &[u8])> = vec![
            ("install_profile.json", profile.as_bytes()),
            ("version.json", version.as_bytes()),
        ];
        entries.extend(
            names
                .iter()
                .zip(libs)
                .map(|(n, (_, _, _, b))| (n.as_str(), *b)),
        );
        entries.extend_from_slice(extra);
        jar(dir, "synth.jar", &entries)
    }

    const SMALL: Limits = Limits {
        entry: 1000,
        total: 1500,
        entries: 8,
    };

    #[test]
    fn extracts_only_provided_artifacts_and_no_sidecars() {
        let tmp = tempfile::tempdir().unwrap();
        let libs = tmp.path().join("libs");
        let jar = synth(
            tmp.path(),
            &[("net/minecraftforge/b/1/b-1.jar", None, b"provided")],
            &[
                (
                    "maven/net/minecraftforge/b/1/b-1.jar.sha1",
                    sha1_hex(b"evil").as_bytes(),
                ),
                ("maven/x.sha1", b"x"),
                (
                    "maven/net/fabricmc/fabric-loader/1/fabric-loader-1.jar",
                    b"evil",
                ),
                (
                    "maven/net/fabricmc/fabric-loader/1/fabric-loader-1.jar.sha1",
                    b"x",
                ),
            ],
        );
        let mut c = read_installer(&jar).unwrap();
        extract_embedded(&jar, &mut c, &libs, FORGE, "x").unwrap();
        assert_eq!(
            std::fs::read(libs.join("net/minecraftforge/b/1/b-1.jar")).unwrap(),
            b"provided"
        );
        let mut files = Vec::new();
        collect(&libs, &mut files);
        assert_eq!(files.len(), 1, "{files:?}");
    }

    fn collect(p: &Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(p).unwrap() {
            let e = e.unwrap().path();
            if e.is_dir() {
                collect(&e, out);
            } else {
                out.push(e);
            }
        }
    }

    #[test]
    fn wrong_hash_file_at_destination_is_replaced_by_the_verified_one() {
        let tmp = tempfile::tempdir().unwrap();
        let libs = tmp.path().join("libs");
        std::fs::create_dir_all(libs.join("net/minecraftforge/b/1")).unwrap();
        std::fs::write(libs.join("net/minecraftforge/b/1/b-1.jar"), b"foreign").unwrap();
        let jar = synth(
            tmp.path(),
            &[("net/minecraftforge/b/1/b-1.jar", None, b"provided")],
            &[],
        );
        let mut c = read_installer(&jar).unwrap();
        extract_embedded(&jar, &mut c, &libs, FORGE, "x").unwrap();
        assert_eq!(
            std::fs::read(libs.join("net/minecraftforge/b/1/b-1.jar")).unwrap(),
            b"provided"
        );
    }

    #[test]
    fn matching_file_at_destination_is_kept() {
        let tmp = tempfile::tempdir().unwrap();
        let libs = tmp.path().join("libs");
        std::fs::create_dir_all(libs.join("net/minecraftforge/b/1")).unwrap();
        std::fs::write(libs.join("net/minecraftforge/b/1/b-1.jar"), b"provided").unwrap();
        // The jar entry is wrong, which would fail if it were extracted.
        let wrong = sha1_hex(b"provided");
        let jar = synth(
            tmp.path(),
            &[("net/minecraftforge/b/1/b-1.jar", Some(&wrong), b"other")],
            &[],
        );
        let mut c = read_installer(&jar).unwrap();
        extract_embedded(&jar, &mut c, &libs, FORGE, "x").unwrap();
        assert_eq!(
            std::fs::read(libs.join("net/minecraftforge/b/1/b-1.jar")).unwrap(),
            b"provided"
        );
    }

    #[test]
    fn entry_not_matching_the_declared_hash_is_refused_and_leaves_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let libs = tmp.path().join("libs");
        let declared = sha1_hex(b"what the profile promises");
        let jar = synth(
            tmp.path(),
            &[(
                "net/minecraftforge/b/1/b-1.jar",
                Some(&declared),
                b"tampered",
            )],
            &[],
        );
        let mut c = read_installer(&jar).unwrap();
        let err = extract_embedded(&jar, &mut c, &libs, FORGE, "x").unwrap_err();
        assert!(
            matches!(&err, Error::Installer(m) if m.contains("SHA-1")),
            "{err}"
        );
        let mut files = Vec::new();
        if libs.exists() {
            collect(&libs, &mut files);
        }
        assert!(files.is_empty(), "{files:?}");
    }

    #[test]
    fn stale_wrong_file_is_removed_when_the_entry_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let libs = tmp.path().join("libs");
        std::fs::create_dir_all(libs.join("net/minecraftforge/b/1")).unwrap();
        std::fs::write(libs.join("net/minecraftforge/b/1/b-1.jar"), b"foreign").unwrap();
        let declared = sha1_hex(b"what the profile promises");
        let jar = synth(
            tmp.path(),
            &[(
                "net/minecraftforge/b/1/b-1.jar",
                Some(&declared),
                b"tampered",
            )],
            &[],
        );
        let mut c = read_installer(&jar).unwrap();
        assert!(extract_embedded(&jar, &mut c, &libs, FORGE, "x").is_err());
        assert!(!libs.join("net/minecraftforge/b/1/b-1.jar").exists());
    }

    #[test]
    fn provided_artifact_without_a_declared_hash_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let libs = tmp.path().join("libs");
        let jar = synth(
            tmp.path(),
            &[("net/minecraftforge/b/1/b-1.jar", Some(""), b"x")],
            &[],
        );
        let mut c = read_installer(&jar).unwrap();
        let err = extract_embedded(&jar, &mut c, &libs, FORGE, "x").unwrap_err();
        assert!(
            matches!(&err, Error::Installer(m) if m.contains("SHA-1")),
            "{err}"
        );
        assert!(!libs.join("net/minecraftforge/b/1/b-1.jar").exists());
    }

    #[test]
    fn provided_sidecar_paths_are_refused() {
        for rel in [
            "net/minecraftforge/b/1/b-1.jar.sha1",
            "net/minecraftforge/b/1/b-1.jar.ASC",
            "a/b/1/b-1.md5",
        ] {
            let tmp = tempfile::tempdir().unwrap();
            let libs = tmp.path().join("libs");
            let jar = synth(tmp.path(), &[(rel, None, b"x")], &[]);
            let mut c = read_installer(&jar).unwrap();
            assert!(
                matches!(
                    extract_embedded(&jar, &mut c, &libs, FORGE, "x"),
                    Err(Error::Installer(_))
                ),
                "{rel}"
            );
            assert!(!libs.join(rel).exists());
        }
    }

    #[test]
    fn hostile_artifact_paths_are_refused_and_write_nothing() {
        for rel in [
            "../../evil.jar",
            "x/../../evil.jar",
            "x/.. /evil.jar",
            "x/.../evil.jar",
            "x/CON",
            "x/nul.jar",
            "a.jar:s",
            "//server/share/evil.jar",
            "\\\\server\\share\\evil.jar",
            "x\\evil.jar",
            "C:/evil.jar",
        ] {
            let tmp = tempfile::tempdir().unwrap();
            let libs = tmp.path().join("work").join("libs");
            let jar = synth(tmp.path(), &[(rel, None, b"evil")], &[]);
            let mut c = read_installer(&jar).unwrap();
            assert!(
                matches!(
                    extract_embedded(&jar, &mut c, &libs, FORGE, "x"),
                    Err(Error::UnsafePath(_))
                ),
                "{rel:?} must be refused"
            );
            let mut files = Vec::new();
            collect(tmp.path(), &mut files);
            let written: Vec<_> = files
                .iter()
                .filter(|f| f.file_name().is_some_and(|n| n != "synth.jar"))
                .collect();
            assert!(written.is_empty(), "{rel:?}: {written:?}");
        }
    }

    #[test]
    fn entry_count_is_capped() {
        let tmp = tempfile::tempdir().unwrap();
        let libs = tmp.path().join("libs");
        let names: Vec<String> = (0..SMALL.entries + 1)
            .map(|i| format!("maven/pad/{i}.txt"))
            .collect();
        let extra: Vec<(&str, &[u8])> = names.iter().map(|n| (n.as_str(), &b""[..])).collect();
        let jar = synth(
            tmp.path(),
            &[("net/minecraftforge/b/1/b-1.jar", None, b"x")],
            &extra,
        );
        let mut c = read_installer(&jar).unwrap();
        let err = extract_embedded_with(&jar, &mut c, &libs, FORGE, "x", SMALL).unwrap_err();
        assert!(
            matches!(&err, Error::Installer(m) if m.contains("entries")),
            "{err}"
        );
        assert!(!libs.exists());
        let mut ok = SMALL;
        ok.entries = 100;
        extract_embedded_with(&jar, &mut c, &libs, FORGE, "x", ok).unwrap();
    }

    #[test]
    fn oversized_entry_is_refused_and_leaves_no_file() {
        let tmp = tempfile::tempdir().unwrap();
        let libs = tmp.path().join("libs");
        // Highly compressible, so the jar itself stays tiny.
        let huge = vec![0u8; SMALL.entry as usize + 1];
        let jar = synth(
            tmp.path(),
            &[("net/minecraftforge/b/1/b-1.jar", None, &huge)],
            &[],
        );
        let mut c = read_installer(&jar).unwrap();
        let err = extract_embedded_with(&jar, &mut c, &libs, FORGE, "x", SMALL).unwrap_err();
        assert!(
            matches!(&err, Error::Installer(m) if m.contains("size limit")),
            "{err}"
        );
        assert!(!libs.join("net/minecraftforge/b/1/b-1.jar").exists());
        assert!(!libs.join("net/minecraftforge/b/1/b-1.jar.part").exists());
    }

    #[test]
    fn total_extraction_budget_is_enforced() {
        let tmp = tempfile::tempdir().unwrap();
        let libs = tmp.path().join("libs");
        // Two entries of 800 bytes fit the per-entry limit but not the total of 1500.
        let chunk = [vec![1u8; 800], vec![2u8; 800]];
        let jar = synth(
            tmp.path(),
            &[
                ("net/minecraftforge/b/1/b-1.jar", None, &chunk[0]),
                ("net/minecraftforge/b/2/b-2.jar", None, &chunk[1]),
            ],
            &[],
        );
        let mut c = read_installer(&jar).unwrap();
        let err = extract_embedded_with(&jar, &mut c, &libs, FORGE, "x", SMALL).unwrap_err();
        assert!(
            matches!(&err, Error::Installer(m) if m.contains("size limit")),
            "{err}"
        );
        extract_embedded(&jar, &mut c, &libs, FORGE, "x").unwrap();
    }

    /// Overwrites the declared uncompressed size in the local and central headers of every entry.
    fn lie_about_size(path: &Path, declared: u32) {
        let mut b = std::fs::read(path).unwrap();
        for (sig, off) in [([0x50u8, 0x4b, 3, 4], 22usize), ([0x50, 0x4b, 1, 2], 24)] {
            let mut i = 0;
            while i + 4 <= b.len() {
                if b[i..i + 4] == sig {
                    b[i + off..i + off + 4].copy_from_slice(&declared.to_le_bytes());
                    i += 4;
                } else {
                    i += 1;
                }
            }
        }
        std::fs::write(path, b).unwrap();
    }

    #[test]
    fn entry_with_a_lying_size_header_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let libs = tmp.path().join("libs");
        let big = vec![0u8; 2 * SMALL.entry as usize];
        let path = tmp.path().join("lie.jar");
        let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
        zip.start_file(
            "maven/net/minecraftforge/b/1/b-1.jar",
            SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(&big).unwrap();
        zip.finish().unwrap();
        lie_about_size(&path, 10);
        let mut zip = open(&path, 8).unwrap();
        let mut budget = 0;
        let dest = libs.join("net/minecraftforge/b/1/b-1.jar");
        let res = extract_entry(
            &mut zip,
            "maven/net/minecraftforge/b/1/b-1.jar",
            &dest,
            &mut budget,
            SMALL,
            None,
        );
        assert!(
            matches!(&res, Err(Error::Io(e)) if e.kind() == std::io::ErrorKind::InvalidData
                && e.to_string().contains("larger than its declared")),
            "{res:?}"
        );
        assert!(!dest.exists());
        assert!(!libs.join("net/minecraftforge/b/1/b-1.jar.part").exists());
    }

    #[test]
    fn legacy_path_outside_net_minecraftforge_forge_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let bad_legacy = LEGACY_189.replacen(
            "\"path\": \"net.minecraftforge:forge:1.8.9-11.15.1.2318-1.8.9\"",
            "\"path\": \"net.fabricmc:fabric-loader:0.17.3\"",
            1,
        );
        assert_ne!(bad_legacy, LEGACY_189);
        let jar = jar(
            tmp.path(),
            "a.jar",
            &[("install_profile.json", bad_legacy.as_bytes())],
        );
        assert!(
            matches!(read_installer(&jar), Err(Error::Installer(m)) if m.contains("net.minecraftforge:forge"))
        );
    }

    fn nothing_written(libs: &Path) -> bool {
        let mut files = Vec::new();
        if libs.exists() {
            collect(libs, &mut files);
        }
        files.is_empty()
    }

    fn refused_modern(kind: LoaderKind, libs: &[(&str, &str, Option<&str>, &[u8])], what: &str) {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("work").join("libs");
        let jar = synth_named(tmp.path(), libs, &[]);
        let mut c = read_installer(&jar).unwrap();
        let res = extract_embedded(&jar, &mut c, &dest, kind, "x");
        assert!(
            matches!(res, Err(Error::Installer(_) | Error::UnsafePath(_))),
            "{what}: {res:?}"
        );
        let mut files = Vec::new();
        collect(tmp.path(), &mut files);
        let written: Vec<_> = files
            .iter()
            .filter(|f| f.file_name().is_some_and(|n| n != "synth.jar"))
            .collect();
        assert!(written.is_empty(), "{what}: {written:?}");
    }

    const FABRIC_JAR: &str = "net/fabricmc/fabric-loader/0.17.3/fabric-loader-0.17.3.jar";
    const FABRIC: &str = "net.fabricmc:fabric-loader:0.17.3";

    #[test]
    fn a2_trailing_slash_sidecar_path_is_refused() {
        let planted = sha1_hex(b"x");
        refused_modern(
            FORGE,
            &[(
                FABRIC,
                &format!("{FABRIC_JAR}.sha1/"),
                Some(&sha1_hex(planted.as_bytes())),
                planted.as_bytes(),
            )],
            "fabric sidecar",
        );
        refused_modern(
            FORGE,
            &[(
                "net.minecraftforge:b:1",
                "net/minecraftforge/b/1/b-1.jar.sha1/",
                None,
                b"x",
            )],
            "forge sidecar",
        );
    }

    #[test]
    fn provided_artifact_in_a_foreign_group_is_refused() {
        refused_modern(FORGE, &[(FABRIC, FABRIC_JAR, None, b"evil")], "fabric");
        refused_modern(
            LoaderKind::NeoForge,
            &[(
                "net.minecraftforge:b:1",
                "net/minecraftforge/b/1/b-1.jar",
                None,
                b"x",
            )],
            "forge group in a NeoForge installer",
        );
        refused_modern(
            FORGE,
            &[("net.neoforged:b:1", "net/neoforged/b/1/b-1.jar", None, b"x")],
            "neoforge group in a Forge installer",
        );
    }

    fn extract_named(kind: LoaderKind, name: &str, maven: &str) -> (tempfile::TempDir, Result<()>) {
        let tmp = tempfile::tempdir().unwrap();
        let path = maven_path(name).unwrap();
        let jar = synth_named(tmp.path(), &[(name, &path, None, b"jar")], &[]);
        let mut c = read_installer(&jar).unwrap();
        let res = extract_embedded(&jar, &mut c, &tmp.path().join("libs"), kind, maven);
        (tmp, res)
    }

    #[test]
    fn provided_main_artifact_must_be_the_requested_build() {
        let neo = LoaderKind::NeoForge;
        for (kind, name, maven) in [
            (
                FORGE,
                "net.minecraftforge:forge:1.20.1-47.4.28",
                "1.20.1-47.4.28",
            ),
            (
                FORGE,
                "net.minecraftforge:forge:1.20.1-47.4.28:universal",
                "1.20.1-47.4.28",
            ),
            (neo, "net.neoforged:neoforge:26.3.0.115", "26.3.0.115"),
            // NeoForge for 1.20.1 still uses the old `forge` artifact name.
            (
                neo,
                "net.neoforged:forge:1.20.1-47.1.106",
                "1.20.1-47.1.106",
            ),
            // Not the main artifact: any version.
            (FORGE, "net.minecraftforge:fmlcore:9.9", "1.20.1-47.4.28"),
        ] {
            let (_t, res) = extract_named(kind, name, maven);
            res.unwrap_or_else(|e| panic!("{name}: {e}"));
        }
        for (kind, name, maven) in [
            (
                FORGE,
                "net.minecraftforge:forge:1.20.1-47.4.27",
                "1.20.1-47.4.28",
            ),
            (
                FORGE,
                "net.minecraftforge:forge:1.12.2-14.23.5.2859",
                "1.20.1-47.4.28",
            ),
            (neo, "net.neoforged:neoforge:26.3.0.114", "26.3.0.115"),
            (neo, "net.neoforged:forge:1.20.1-47.1.5", "1.20.1-47.1.106"),
        ] {
            let (tmp, res) = extract_named(kind, name, maven);
            assert!(
                matches!(&res, Err(Error::Installer(m)) if m.contains("requested build")),
                "{name}: {res:?}"
            );
            assert!(nothing_written(&tmp.path().join("libs")), "{name}");
        }
    }

    #[test]
    fn provided_artifact_path_must_be_the_coordinates_path() {
        refused_modern(
            FORGE,
            &[(
                "net.minecraftforge:b:1",
                "net/minecraftforge/b/2/b-2.jar",
                None,
                b"x",
            )],
            "other version",
        );
        refused_modern(
            FORGE,
            &[(
                "net.minecraftforge:b:1",
                "net/minecraftforge//b/1/b-1.jar",
                None,
                b"x",
            )],
            "double slash",
        );
        refused_modern(
            FORGE,
            &[(
                "net.minecraftforge:b:1@zip",
                "net/minecraftforge/b/1/b-1.zip",
                None,
                b"x",
            )],
            "not a jar",
        );
    }

    #[test]
    fn case_alias_duplicates_are_refused() {
        refused_modern(
            FORGE,
            &[
                (
                    "net.minecraftforge:B:1",
                    "net/minecraftforge/B/1/B-1.jar",
                    None,
                    b"one",
                ),
                (
                    "net.minecraftforge:b:1",
                    "net/minecraftforge/b/1/b-1.jar",
                    None,
                    b"two",
                ),
            ],
            "case alias",
        );
    }

    fn legacy_jar(dir: &Path, path: &str) -> std::path::PathBuf {
        let profile = LEGACY_189.replacen(
            "\"path\": \"net.minecraftforge:forge:1.8.9-11.15.1.2318-1.8.9\"",
            &format!("\"path\": \"{path}\""),
            1,
        );
        jar(
            dir,
            "legacy.jar",
            &[
                ("install_profile.json", profile.as_bytes()),
                (UNIVERSAL_189, b"universal"),
            ],
        )
    }

    #[test]
    fn legacy_path_must_be_exactly_the_requested_build() {
        for path in [
            // A3: sidecar through a classifier and extension
            "net.minecraftforge:forge:1.8.9-11.15.1.2318-1.8.9:installer@jar.sha1",
            // A9: another version, a classifier, another extension, another artifact
            "net.minecraftforge:forge:1.12.2-14.23.5.2859",
            "net.minecraftforge:forge:1.8.9-11.15.1.2318-1.8.9:universal",
            "net.minecraftforge:forge:1.8.9-11.15.1.2318-1.8.9@zip",
            "net.minecraftforge:fml:1.8.9-11.15.1.2318-1.8.9",
        ] {
            let tmp = tempfile::tempdir().unwrap();
            let libs = tmp.path().join("libs");
            let jar = legacy_jar(tmp.path(), path);
            let res = read_installer(&jar)
                .and_then(|mut c| extract_embedded(&jar, &mut c, &libs, FORGE, LEGACY_189_MAVEN));
            assert!(matches!(res, Err(Error::Installer(_))), "{path}: {res:?}");
            assert!(nothing_written(&libs), "{path}");
        }
        // The exact coordinate works; for another requested build it does not.
        let tmp = tempfile::tempdir().unwrap();
        let libs = tmp.path().join("libs");
        let jar = legacy_jar(
            tmp.path(),
            "net.minecraftforge:forge:1.8.9-11.15.1.2318-1.8.9",
        );
        let mut c = read_installer(&jar).unwrap();
        assert!(extract_embedded(&jar, &mut c, &libs, FORGE, "1.8.9-11.15.1.2319-1.8.9").is_err());
        assert!(nothing_written(&libs));
        extract_embedded(&jar, &mut c, &libs, FORGE, LEGACY_189_MAVEN).unwrap();
    }
}
