//! Reading Forge and NeoForge installers (jars) in both formats. An installer is untrusted remote
//! data: every name taken from it is validated, only the entries the later steps need are
//! extracted, and entry sizes are capped.
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
use crate::paths::{join_relative, validate_id};
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

fn open(installer: &Path) -> Result<zip::ZipArchive<File>> {
    zip::ZipArchive::new(File::open(installer)?)
        .map_err(|e| bad(format!("not a readable jar: {e}")))
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
    let mut zip = open(installer)?;
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
}

/// Copies one entry to `dest` (via a `.part` file), enforcing the per-entry limit and the remaining
/// total budget.
fn extract_entry<R: Read + Seek>(
    zip: &mut zip::ZipArchive<R>,
    name: &str,
    dest: &Path,
    budget: &mut u64,
    limits: Limits,
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
    let mut tmp = dest.as_os_str().to_owned();
    tmp.push(".part");
    let tmp = std::path::PathBuf::from(tmp);
    let mut out = File::create(&tmp)?;
    let copied = std::io::copy(&mut (&mut entry).take(limit + 1), &mut out);
    drop(out);
    let copied = match copied {
        Ok(n) if n <= limit => n,
        other => {
            let _ = std::fs::remove_file(&tmp);
            return Err(match other {
                Ok(_) => too_large(name),
                Err(e) => e.into(),
            });
        }
    };
    std::fs::rename(&tmp, dest)?;
    *budget += copied;
    Ok(copied)
}

/// Extracts what the installer ships for the later steps into the shared libraries directory.
///
/// Modern: every file below `maven/` goes to `libraries/<rest>` unless it is already there; a
/// library the profile marks as installer-provided (empty `url`) must be among them. Legacy: the
/// universal jar goes to its Maven path and the matching library in `contents.version` is rewritten
/// to point at it (empty `url`, real SHA-1 and size).
pub fn extract_embedded(
    installer: &Path,
    contents: &mut InstallerContents,
    libraries: &Path,
) -> Result<()> {
    let limits = Limits {
        entry: MAX_ENTRY_BYTES,
        total: MAX_TOTAL_BYTES,
    };
    extract_embedded_with(installer, contents, libraries, limits)
}

fn extract_embedded_with(
    installer: &Path,
    contents: &mut InstallerContents,
    libraries: &Path,
    limits: Limits,
) -> Result<()> {
    let mut zip = open(installer)?;
    let mut budget = 0u64;
    match &contents.kind {
        InstallerKind::Modern(profile) => {
            let names = zip
                .file_names()
                .map(|n| n.map(|n| n.into_owned()))
                .filter(|n| {
                    n.as_ref()
                        .map_or(true, |n| n.starts_with(MAVEN_DIR) && !n.ends_with('/'))
                })
                .collect::<std::result::Result<Vec<String>, _>>()
                .map_err(|e| bad(format!("unreadable entry name: {e}")))?;
            for name in &names {
                let dest = join_relative(libraries, &name[MAVEN_DIR.len()..])?;
                if !dest.is_file() {
                    extract_entry(&mut zip, name, &dest, &mut budget, limits)?;
                }
            }
            let provided = profile
                .libraries
                .iter()
                .chain(&contents.version.libraries)
                .filter_map(|l| l.downloads.as_ref()?.artifact.as_ref())
                .filter(|a| a.url.is_empty());
            for artifact in provided {
                check_relative(&artifact.path)?;
                if !names.iter().any(|n| n[MAVEN_DIR.len()..] == artifact.path) {
                    return Err(bad(format!(
                        "installer is missing maven/{}, which its profile marks as provided",
                        artifact.path
                    )));
                }
            }
        }
        InstallerKind::Legacy {
            path, file_path, ..
        } => {
            let rel = maven_path(path).ok_or_else(|| Error::UnsafePath(path.clone()))?;
            let dest = join_relative(libraries, &rel)?;
            let (path, file_path) = (path.clone(), file_path.clone());
            // Always rewritten: the SHA-1 recorded below must be that of the installer's jar.
            let size = extract_entry(&mut zip, &file_path, &dest, &mut budget, limits)?;
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

    const UNIVERSAL_189: &str = "forge-1.8.9-11.15.1.2318-1.8.9-universal.jar";
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
        extract_embedded(&jar, &mut c, &libs).unwrap();
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
        let jar = jar(
            tmp.path(),
            "i.jar",
            &[
                ("install_profile.json", PROFILE_2859.as_bytes()),
                ("version.json", VERSION_2859.as_bytes()),
                (FORGE_2859, b"forge"),
            ],
        );
        let mut c = read_installer(&jar).unwrap();
        let InstallerKind::Modern(p) = &c.kind else {
            panic!("expected the modern format")
        };
        assert_eq!((p.spec, p.processors.len()), (0, 0));
        assert_eq!(c.version.id, "1.12.2-forge-14.23.5.2859");
        extract_embedded(&jar, &mut c, &libs).unwrap();
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
    fn refuses_embedded_paths_that_escape() {
        let tmp = tempfile::tempdir().unwrap();
        let libs = tmp.path().join("work").join("libs");
        let jar = jar(
            tmp.path(),
            "i.jar",
            &[
                ("install_profile.json", PROFILE_2859.as_bytes()),
                ("version.json", VERSION_2859.as_bytes()),
                ("maven/../../evil.jar", b"evil"),
            ],
        );
        let mut c = read_installer(&jar).unwrap();
        assert!(matches!(
            extract_embedded(&jar, &mut c, &libs),
            Err(Error::UnsafePath(_))
        ));
        assert!(!libs.parent().unwrap().join("evil.jar").exists());
        assert!(!tmp.path().join("evil.jar").exists());
    }

    #[test]
    fn refuses_backslash_and_drive_entries() {
        for name in ["maven/..\\evil.jar", "maven/C:/evil.jar", "maven//evil.jar"] {
            let tmp = tempfile::tempdir().unwrap();
            let libs = tmp.path().join("libs");
            let jar = jar(
                tmp.path(),
                "i.jar",
                &[
                    ("install_profile.json", PROFILE_2859.as_bytes()),
                    ("version.json", VERSION_2859.as_bytes()),
                    (name, b"evil"),
                ],
            );
            let mut c = read_installer(&jar).unwrap();
            assert!(
                matches!(
                    extract_embedded(&jar, &mut c, &libs),
                    Err(Error::UnsafePath(_))
                ),
                "{name:?} must be refused"
            );
        }
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
        let err = extract_embedded(&jar, &mut c, &libs).unwrap_err();
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
        let err = extract_embedded(&jar, &mut c, &tmp.path().join("libs")).unwrap_err();
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

    const SMALL: Limits = Limits {
        entry: 1000,
        total: 1500,
    };

    #[test]
    fn oversized_embedded_entry_is_refused_and_leaves_no_file() {
        let tmp = tempfile::tempdir().unwrap();
        let libs = tmp.path().join("libs");
        // Highly compressible, so the jar itself stays tiny.
        let huge = vec![0u8; SMALL.entry as usize + 1];
        let jar = jar(
            tmp.path(),
            "a.jar",
            &[
                ("install_profile.json", PROFILE_2859.as_bytes()),
                ("version.json", VERSION_2859.as_bytes()),
                (FORGE_2859, &huge),
            ],
        );
        let mut c = read_installer(&jar).unwrap();
        let err = extract_embedded_with(&jar, &mut c, &libs, SMALL).unwrap_err();
        assert!(
            matches!(&err, Error::Installer(m) if m.contains("size limit")),
            "{err}"
        );
        let target = libs.join("net/minecraftforge/forge/1.12.2-14.23.5.2859");
        assert!(!target.join("forge-1.12.2-14.23.5.2859.jar").exists());
        assert!(!target.join("forge-1.12.2-14.23.5.2859.jar.part").exists());
    }

    #[test]
    fn total_extraction_budget_is_enforced() {
        let tmp = tempfile::tempdir().unwrap();
        let libs = tmp.path().join("libs");
        // Two entries of 800 bytes each fit the per-entry limit but not the total of 1500.
        let chunk = vec![0u8; 800];
        let jar = jar(
            tmp.path(),
            "a.jar",
            &[
                ("install_profile.json", PROFILE_NEO.as_bytes()),
                ("version.json", VERSION_1201.as_bytes()),
                ("maven/a/b/1/b-1.jar", &chunk),
                ("maven/a/b/2/b-2.jar", &chunk),
            ],
        );
        let mut c = read_installer(&jar).unwrap();
        let err = extract_embedded_with(&jar, &mut c, &libs, SMALL).unwrap_err();
        assert!(
            matches!(&err, Error::Installer(m) if m.contains("size limit")),
            "{err}"
        );
        extract_embedded(&jar, &mut c, &libs).unwrap();
    }

    #[test]
    fn existing_files_are_not_overwritten() {
        let tmp = tempfile::tempdir().unwrap();
        let libs = tmp.path().join("libs");
        let dest = libs.join("net/minecraftforge/forge/1.12.2-14.23.5.2859");
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(dest.join("forge-1.12.2-14.23.5.2859.jar"), b"old").unwrap();
        let jar = jar(
            tmp.path(),
            "a.jar",
            &[
                ("install_profile.json", PROFILE_2859.as_bytes()),
                ("version.json", VERSION_2859.as_bytes()),
                (FORGE_2859, b"new"),
            ],
        );
        let mut c = read_installer(&jar).unwrap();
        extract_embedded(&jar, &mut c, &libs).unwrap();
        assert_eq!(
            std::fs::read(dest.join("forge-1.12.2-14.23.5.2859.jar")).unwrap(),
            b"old"
        );
    }
}
