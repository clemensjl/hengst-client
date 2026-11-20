//! Planning the processors of a modern Forge or NeoForge installer and checking whether an
//! installation is complete. Nothing here runs a JVM, an installer or a processor.
//!
//! Everything in the install profile is untrusted remote data that ends up on a command line and
//! in file paths, so each string is validated where it becomes a path or an argument.

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};

use super::installer::{extract_files, ModernProfile, Processor};
use crate::download::sha1_file;
use crate::libraries::maven_path;
use crate::paths::{join_relative, validate_sha1};
use crate::version::VersionJson;
use crate::{Error, Result};

/// Where a processor run reads from and writes to.
pub struct InstallEnv {
    /// The launcher home (`{ROOT}`).
    pub root: PathBuf,
    pub libraries: PathBuf,
    pub minecraft_jar: PathBuf,
    pub game: String,
    pub installer: PathBuf,
    /// Scratch directory the installer's data files are extracted to.
    pub work: PathBuf,
}

/// One processor, ready to run as `java -cp <jar + classpath> <main_class> <args>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessorCommand {
    pub jar: PathBuf,
    pub classpath: Vec<PathBuf>,
    pub main_class: String,
    pub args: Vec<String>,
    /// Files the processor produces with their expected SHA-1.
    pub outputs: Vec<(PathBuf, String)>,
}

impl ProcessorCommand {
    /// True when the processor declares outputs and all of them exist with the expected SHA-1, so
    /// it need not run again.
    pub fn satisfied(&self) -> bool {
        !self.outputs.is_empty() && self.outputs.iter().all(|(p, sha)| has_sha1(p, sha))
    }
}

/// Largest `META-INF/MANIFEST.MF` we read; real ones are a few hundred bytes.
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

fn bad(msg: impl std::fmt::Display) -> Error {
    Error::Installer(msg.to_string())
}

/// The message of an error without the "invalid loader installer" prefix, for nesting.
fn inner(e: Error) -> String {
    match e {
        Error::Installer(m) => m,
        other => other.to_string(),
    }
}

fn path_string(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn has_sha1(path: &Path, expected: &str) -> bool {
    sha1_file(path).is_ok_and(|h| h.eq_ignore_ascii_case(expected))
}

/// `[group:artifact:version...]` as a whole string.
fn as_coordinate(s: &str) -> Option<&str> {
    s.strip_prefix('[')?.strip_suffix(']')
}

/// The file a Maven coordinate has below the libraries directory.
fn library_file(libraries: &Path, coord: &str) -> Result<PathBuf> {
    let rel = maven_path(coord).ok_or_else(|| Error::UnsafePath(coord.to_string()))?;
    join_relative(libraries, &rel)
}

/// Client-side values of the profile's `data`, plus the fixed tokens. Coordinates resolve below
/// the libraries directory, quoted values are literals, anything else is an installer entry that
/// is extracted below `env.work`. The fixed tokens cannot be overridden by the profile.
pub fn resolve_data(profile: &ModernProfile, env: &InstallEnv) -> Result<HashMap<String, String>> {
    let mut data = HashMap::new();
    let mut jobs = Vec::new();
    for (key, sided) in &profile.data {
        let v = sided.client.as_str();
        let value = if let Some(coord) = as_coordinate(v) {
            path_string(
                &library_file(&env.libraries, coord)
                    .map_err(|e| bad(format!("data {key}: {}", inner(e))))?,
            )
        } else if v.len() >= 2 && v.starts_with('\'') && v.ends_with('\'') {
            v[1..v.len() - 1].to_string()
        } else {
            let entry = v.strip_prefix('/').unwrap_or(v);
            let dest = join_relative(&env.work, entry)
                .map_err(|e| bad(format!("data {key}: {}", inner(e))))?;
            let value = path_string(&dest);
            jobs.push((entry.to_string(), dest));
            value
        };
        data.insert(key.clone(), value);
    }
    extract_files(&env.installer, &jobs)?;
    for (key, value) in [
        ("SIDE", "client".to_string()),
        ("MINECRAFT_JAR", path_string(&env.minecraft_jar)),
        ("MINECRAFT_VERSION", env.game.clone()),
        ("ROOT", path_string(&env.root)),
        ("INSTALLER", path_string(&env.installer)),
        ("LIBRARY_DIR", path_string(&env.libraries)),
    ] {
        data.insert(key.to_string(), value);
    }
    Ok(data)
}

fn is_token_name(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Resolves one processor argument: a whole `[coordinate]` becomes its library path, otherwise
/// every `{KEY}` is replaced in a single pass (replacement text is not expanded again). An unknown
/// key is an error, and so is a result with a `..` segment.
pub fn substitute(arg: &str, data: &HashMap<String, String>, libraries: &Path) -> Result<String> {
    if let Some(coord) = as_coordinate(arg) {
        return library_file(libraries, coord).map(|p| path_string(&p));
    }
    let mut out = String::new();
    let mut rest = arg;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('}') {
            Some(end) if is_token_name(&after[..end]) => {
                let key = &after[..end];
                let value = data
                    .get(key)
                    .ok_or_else(|| bad(format!("unknown installer token {{{key}}}")))?;
                out.push_str(value);
                rest = &after[end + 1..];
            }
            _ => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    if out.split(['/', '\\']).any(|seg| seg == "..") {
        return Err(Error::UnsafePath(out));
    }
    Ok(out)
}

/// The `Main-Class` of a jar's manifest. It ends up on a command line right after the classpath,
/// so only Java class name characters are accepted (nothing that could read as a JVM option).
pub fn jar_main_class(jar: &Path) -> Result<String> {
    let no_main = || bad(format!("{} has no Main-Class", jar.display()));
    let mut zip = zip::ZipArchive::new(std::fs::File::open(jar)?)
        .map_err(|e| bad(format!("{} is not a readable jar: {e}", jar.display())))?;
    let entry = zip.by_name("META-INF/MANIFEST.MF").map_err(|_| no_main())?;
    let mut buf = Vec::new();
    entry.take(MAX_MANIFEST_BYTES + 1).read_to_end(&mut buf)?;
    if buf.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(bad(format!("{} has an oversized manifest", jar.display())));
    }
    // Lines starting with a space continue the previous one; a blank line ends the main section.
    let text = String::from_utf8_lossy(&buf);
    let mut headers: Vec<String> = Vec::new();
    for line in text.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() {
            break;
        }
        match (line.strip_prefix(' '), headers.last_mut()) {
            (Some(more), Some(last)) => last.push_str(more),
            _ => headers.push(line.to_string()),
        }
    }
    let class = headers
        .iter()
        .filter_map(|h| h.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("Main-Class"))
        .map(|(_, value)| value.trim())
        .filter(|v| !v.is_empty())
        .ok_or_else(no_main)?;
    let valid = !class.starts_with(['.', '-'])
        && class
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'$'));
    if !valid {
        return Err(bad(format!("{} has an invalid Main-Class", jar.display())));
    }
    Ok(class.to_string())
}

fn is_client(p: &Processor) -> bool {
    p.sides
        .as_ref()
        .is_none_or(|s| s.iter().any(|s| s == "client"))
}

/// A processor jar or classpath entry: it must be one of the profile's declared libraries (so its
/// download is verified like any other) and must not carry the classpath separator.
fn declared_file(coord: &str, declared: &HashSet<String>, libraries: &Path) -> Result<PathBuf> {
    let rel = maven_path(coord).ok_or_else(|| Error::UnsafePath(coord.to_string()))?;
    if rel.contains(';') {
        return Err(bad(format!(
            "{coord}: ';' is not allowed in a classpath entry"
        )));
    }
    if !declared.contains(&rel) {
        return Err(bad(format!(
            "{coord} is not one of the profile's libraries"
        )));
    }
    join_relative(libraries, &rel)
}

fn plan_one(
    p: &Processor,
    data: &HashMap<String, String>,
    declared: &HashSet<String>,
    env: &InstallEnv,
) -> Result<ProcessorCommand> {
    let jar = declared_file(&p.jar, declared, &env.libraries)?;
    let classpath = p
        .classpath
        .iter()
        .map(|c| declared_file(c, declared, &env.libraries))
        .collect::<Result<Vec<_>>>()?;
    let main_class = jar_main_class(&jar)?;
    let args = p
        .args
        .iter()
        .map(|a| substitute(a, data, &env.libraries))
        .collect::<Result<Vec<_>>>()?;
    let roots = [&env.root, &env.libraries, &env.work];
    let mut outputs = Vec::new();
    for (key, value) in &p.outputs {
        let path = PathBuf::from(substitute(key, data, &env.libraries)?);
        if !path.is_absolute() || !roots.iter().any(|r| path.starts_with(r)) {
            return Err(bad(format!(
                "output {} is outside the launcher directories",
                path.display()
            )));
        }
        let sha = substitute(value, data, &env.libraries)?;
        validate_sha1(&sha).map_err(|_| bad(format!("output hash {sha} is not a SHA-1")))?;
        outputs.push((path, sha));
    }
    Ok(ProcessorCommand {
        jar,
        classpath,
        main_class,
        args,
        outputs,
    })
}

/// The client-side processors of the profile in order, ready to run. Needs the processor jars on
/// disk (for their `Main-Class`) and the installer (for its data files). An invalid processor is
/// an error naming its index in the profile.
pub fn plan(profile: &ModernProfile, env: &InstallEnv) -> Result<Vec<ProcessorCommand>> {
    let data = resolve_data(profile, env)?;
    let declared: HashSet<String> = profile
        .libraries
        .iter()
        .filter_map(|l| maven_path(&l.name))
        .collect();
    profile
        .processors
        .iter()
        .enumerate()
        .filter(|(_, p)| is_client(p))
        .map(|(i, p)| {
            plan_one(p, &data, &declared, env)
                .map_err(|e| bad(format!("processor {i}: {}", inner(e))))
        })
        .collect()
}

/// True only when the install marker names this installer, every processor output has its
/// expected SHA-1, the patched client jar exists (and matches `PATCHED_SHA` when the profile has
/// one) and every installer-provided library of `version` (empty `url`) matches its SHA-1. A
/// marker alone is never trusted.
pub fn install_state_valid(
    cmds: &[ProcessorCommand],
    data: &HashMap<String, String>,
    version: &VersionJson,
    libraries: &Path,
    marker: &Path,
    installer_sha1: &str,
) -> Result<bool> {
    if !std::fs::read_to_string(marker).is_ok_and(|m| m.trim() == installer_sha1) {
        return Ok(false);
    }
    if !cmds
        .iter()
        .flat_map(|c| &c.outputs)
        .all(|(path, sha)| has_sha1(path, sha))
    {
        return Ok(false);
    }
    if let Some(patched) = data.get("PATCHED") {
        let patched = Path::new(patched);
        let ok = match data.get("PATCHED_SHA") {
            Some(sha) => has_sha1(patched, sha),
            None => patched.is_file(),
        };
        if !ok {
            return Ok(false);
        }
    }
    for artifact in version
        .libraries
        .iter()
        .filter_map(|l| l.downloads.as_ref()?.artifact.as_ref())
        .filter(|a| a.url.is_empty())
    {
        if !has_sha1(&join_relative(libraries, &artifact.path)?, &artifact.sha1) {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::download::sha1_hex;
    use serde_json::json;
    use std::fs::File;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    const PROFILE_1201: &str =
        include_str!("../../../tests/fixtures/loaders/forge-1.20.1-install_profile.json");
    const PROFILE_NEO: &str =
        include_str!("../../../tests/fixtures/loaders/neoforge-26.3-install_profile.json");

    fn zip_file(path: &Path, entries: &[(&str, &[u8])]) {
        let mut zip = zip::ZipWriter::new(File::create(path).unwrap());
        for (n, data) in entries {
            zip.start_file(*n, SimpleFileOptions::default()).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
    }

    fn manifest(main: &str) -> Vec<u8> {
        format!("Manifest-Version: 1.0\r\nMain-Class: {main}\r\n\r\n").into_bytes()
    }

    fn write(path: &Path, data: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, data).unwrap();
    }

    fn env(tmp: &Path, game: &str) -> InstallEnv {
        let root = tmp.join("home");
        let installer = tmp.join("installer.jar");
        zip_file(
            &installer,
            &[("data/client.lzma", b"lzma"), ("data/server.lzma", b"lzma")],
        );
        InstallEnv {
            libraries: root.join("libraries"),
            minecraft_jar: root.join("versions").join(game).join(format!("{game}.jar")),
            game: game.to_string(),
            installer,
            work: root.join("work"),
            root,
        }
    }

    fn parse(json: &str) -> ModernProfile {
        serde_json::from_str(json).unwrap()
    }

    /// A dummy jar with a manifest for every processor jar (client and server) of the profile.
    fn stage_jars(profile: &ModernProfile, env: &InstallEnv) {
        for p in &profile.processors {
            let rel = maven_path(&p.jar).unwrap();
            let path = join_relative(&env.libraries, &rel).unwrap();
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            zip_file(&path, &[("META-INF/MANIFEST.MF", &manifest("test.Main"))]);
        }
    }

    #[test]
    fn resolves_data_like_the_forge_installer() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env(tmp.path(), "1.20.1");
        let d = resolve_data(&parse(PROFILE_1201), &env).unwrap();
        assert_eq!(d["SIDE"], "client");
        assert_eq!(d["MC_SLIM_SHA"], "de86b035d2da0f78940796bb95c39a932ed84834");
        assert_eq!(d["MINECRAFT_VERSION"], "1.20.1");
        assert_eq!(PathBuf::from(&d["ROOT"]), env.root);
        assert_eq!(PathBuf::from(&d["LIBRARY_DIR"]), env.libraries);
        assert_eq!(PathBuf::from(&d["INSTALLER"]), env.installer);
        assert_eq!(PathBuf::from(&d["MINECRAFT_JAR"]), env.minecraft_jar);
        assert_eq!(
            PathBuf::from(&d["PATCHED"]),
            env.libraries
                .join("net/minecraftforge/forge/1.20.1-47.4.28/forge-1.20.1-47.4.28-client.jar")
        );
        assert_eq!(
            PathBuf::from(&d["MAPPINGS"]),
            env.libraries.join("de/oceanlabs/mcp/mcp_config/1.20.1-20230612.114412/mcp_config-1.20.1-20230612.114412-mappings.txt")
        );
        assert_eq!(
            PathBuf::from(&d["BINPATCH"]),
            env.work.join("data").join("client.lzma")
        );
        assert_eq!(
            std::fs::read(env.work.join("data").join("client.lzma")).unwrap(),
            b"lzma"
        );
    }

    #[test]
    fn profile_data_cannot_override_fixed_values() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env(tmp.path(), "1.20.1");
        let p = parse(
            &json!({"spec": 1, "minecraft": "1.20.1", "json": "/version.json",
                "data": {"SIDE": {"client": "'server'", "server": "'server'"}}})
            .to_string(),
        );
        assert_eq!(resolve_data(&p, &env).unwrap()["SIDE"], "client");
    }

    #[test]
    fn data_values_that_would_escape_are_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env(tmp.path(), "1.20.1");
        for value in [
            "[a:b:../../x]",
            "[a:b:1@../../x]",
            "/../evil",
            "/data/../../evil",
            "/C:/evil",
            "/..\\evil",
            "[nonsense]",
            "",
        ] {
            let p = parse(
                &json!({"spec": 1, "minecraft": "1.20.1", "json": "/version.json",
                    "data": {"X": {"client": value, "server": value}}})
                .to_string(),
            );
            let err = resolve_data(&p, &env).unwrap_err();
            assert!(
                matches!(err, Error::UnsafePath(_) | Error::Installer(_)),
                "{value:?}: {err}"
            );
        }
        assert!(!tmp.path().join("evil").exists());
        assert!(!env.root.join("evil").exists());
    }

    #[test]
    fn missing_installer_entry_for_data_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env(tmp.path(), "1.20.1");
        let p = parse(
            &json!({"spec": 1, "minecraft": "1.20.1", "json": "/version.json",
                "data": {"X": {"client": "/data/nope.bin", "server": "/data/nope.bin"}}})
            .to_string(),
        );
        assert!(matches!(resolve_data(&p, &env), Err(Error::Installer(_))));
    }

    #[test]
    fn substitutes_tokens_and_coordinates() {
        let d = HashMap::from([
            ("ROOT".to_string(), "C:/h".to_string()),
            ("SIDE".to_string(), "client".to_string()),
        ]);
        let libs = Path::new("C:/h/libraries");
        assert_eq!(
            substitute("{ROOT}/libraries/", &d, libs).unwrap(),
            "C:/h/libraries/"
        );
        assert_eq!(substitute("--side", &d, libs).unwrap(), "--side");
        assert_eq!(
            PathBuf::from(
                substitute(
                    "[de.oceanlabs.mcp:mcp_config:1.20.1-20230612.114412@zip]",
                    &d,
                    libs
                )
                .unwrap()
            ),
            libs.join("de/oceanlabs/mcp/mcp_config/1.20.1-20230612.114412/mcp_config-1.20.1-20230612.114412.zip")
        );
        assert!(matches!(
            substitute("{NOPE}", &d, libs),
            Err(Error::Installer(_))
        ));
        assert!(matches!(
            substitute("{ROOT}/../evil", &d, libs),
            Err(Error::UnsafePath(_))
        ));
        assert!(matches!(
            substitute("{ROOT}\\..\\evil", &d, libs),
            Err(Error::UnsafePath(_))
        ));
        assert!(matches!(
            substitute("[a:b:../../x]", &d, libs),
            Err(Error::UnsafePath(_))
        ));
    }

    #[test]
    fn substitution_is_single_pass() {
        let d = HashMap::from([
            ("A".to_string(), "{B}".to_string()),
            ("B".to_string(), "boom".to_string()),
        ]);
        assert_eq!(substitute("x{A}y", &d, Path::new("l")).unwrap(), "x{B}y");
    }

    #[test]
    fn reads_main_class_with_continuation_lines() {
        let tmp = tempfile::tempdir().unwrap();
        for (name, mf) in [
            (
                "crlf.jar",
                "Manifest-Version: 1.0\r\nMain-Class: net.minecraftforge.binarypatcher.Consol\r\n eTool\r\n\r\n",
            ),
            (
                "lf.jar",
                "Manifest-Version: 1.0\nMain-Class: net.minecraftforge.binarypatcher.Consol\n eTool\n\n",
            ),
        ] {
            let jar = tmp.path().join(name);
            zip_file(&jar, &[("META-INF/MANIFEST.MF", mf.as_bytes())]);
            assert_eq!(
                jar_main_class(&jar).unwrap(),
                "net.minecraftforge.binarypatcher.ConsoleTool",
                "{name}"
            );
        }
    }

    #[test]
    fn missing_or_hostile_main_class_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let none = tmp.path().join("none.jar");
        zip_file(
            &none,
            &[("META-INF/MANIFEST.MF", b"Manifest-Version: 1.0\r\n\r\n")],
        );
        assert!(
            matches!(jar_main_class(&none), Err(Error::Installer(m)) if m.contains("has no Main-Class"))
        );
        let no_manifest = tmp.path().join("nm.jar");
        zip_file(&no_manifest, &[("a.txt", b"x")]);
        assert!(matches!(
            jar_main_class(&no_manifest),
            Err(Error::Installer(_))
        ));
        // Would be read as a JVM option if it came before the arguments.
        for hostile in ["-Dx=1", "a b", "a;b", ".x"] {
            let jar = tmp.path().join("h.jar");
            zip_file(&jar, &[("META-INF/MANIFEST.MF", &manifest(hostile))]);
            assert!(
                matches!(jar_main_class(&jar), Err(Error::Installer(_))),
                "{hostile}"
            );
        }
    }

    #[test]
    fn plans_only_client_processors() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env(tmp.path(), "1.20.1");
        let profile = parse(PROFILE_1201);
        stage_jars(&profile, &env);
        let cmds = plan(&profile, &env).unwrap();
        assert_eq!(cmds.len(), 6);
        assert_eq!(&cmds[0].args[..2], ["--task", "MCP_DATA"]);
        assert!(cmds.iter().all(|c| c.main_class == "test.Main"));
        let split = &cmds[3];
        assert_eq!(split.outputs.len(), 2);
        assert!(split.outputs.iter().any(|(p, h)| p
            .ends_with("client-1.20.1-20230612.114412-slim.jar")
            && h == "de86b035d2da0f78940796bb95c39a932ed84834"));
        assert!(split
            .args
            .contains(&env.minecraft_jar.to_string_lossy().into_owned()));
        assert_eq!(
            split.jar,
            env.libraries
                .join("net/minecraftforge/jarsplitter/1.1.4/jarsplitter-1.1.4.jar")
        );
        assert_eq!(split.classpath.len(), 2);

        let neo_profile = parse(PROFILE_NEO);
        let neo_env = InstallEnv {
            game: "26.3".into(),
            ..env
        };
        stage_jars(&neo_profile, &neo_env);
        let neo = plan(&neo_profile, &neo_env).unwrap();
        assert_eq!(neo.len(), 1);
        assert!(neo[0]
            .args
            .iter()
            .any(|a| a == &format!("{}/libraries/", neo_env.root.to_string_lossy())));
        assert_eq!(&neo[0].args[..2], ["--task", "PROCESS_MINECRAFT_JAR"]);
    }

    #[test]
    fn server_only_processors_are_skipped_and_unsided_ones_kept() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env(tmp.path(), "1.20.1");
        let profile = parse(
            &json!({"spec": 1, "minecraft": "1.20.1", "json": "/version.json",
            "libraries": [{"name": "x:tool:1"}],
            "processors": [
                {"jar": "x:tool:1", "args": ["unsided"]},
                {"jar": "x:tool:1", "sides": ["server"], "args": ["server"]},
                {"jar": "x:tool:1", "sides": ["client", "server"], "args": ["both"]},
                {"jar": "x:tool:1", "sides": ["client"], "args": ["client"]},
                {"jar": "x:tool:1", "sides": [], "args": ["empty"]}
            ]})
            .to_string(),
        );
        stage_jars(&profile, &env);
        let cmds = plan(&profile, &env).unwrap();
        let got: Vec<&str> = cmds.iter().map(|c| c.args[0].as_str()).collect();
        assert_eq!(got, ["unsided", "both", "client"]);
    }

    fn one_processor(extra: serde_json::Value) -> ModernProfile {
        let mut p = json!({"jar": "x:tool:1", "args": []});
        p.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        parse(
            &json!({"spec": 1, "minecraft": "1.20.1", "json": "/version.json",
                "libraries": [{"name": "x:tool:1"}, {"name": "x:dep:2"}],
                "processors": [p]})
            .to_string(),
        )
    }

    fn assert_refused(profile: &ModernProfile, env: &InstallEnv, needle: &str) {
        stage_jars(profile, env);
        match plan(profile, env) {
            Err(Error::Installer(m)) => {
                assert!(m.contains("processor 0"), "{m}");
                assert!(m.contains(needle), "{m}");
            }
            other => panic!("expected an installer error, got {other:?}"),
        }
    }

    #[test]
    fn hostile_profiles_are_refused_naming_the_processor() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env(tmp.path(), "1.20.1");
        let sha = "0123456789abcdef0123456789abcdef01234567";
        // Unknown placeholder.
        assert_refused(&one_processor(json!({"args": ["{NOPE}"]})), &env, "NOPE");
        // A path that leaves the allowed roots.
        assert_refused(
            &one_processor(json!({"args": ["{ROOT}/../evil"]})),
            &env,
            "..",
        );
        // Jar and classpath entries the profile does not declare as libraries.
        assert_refused(
            &one_processor(json!({"jar": "x:other:1"})),
            &env,
            "x:other:1",
        );
        assert_refused(
            &one_processor(json!({"classpath": ["x:other:1"]})),
            &env,
            "x:other:1",
        );
        // Escaping coordinates, and a classpath separator smuggled into one.
        assert_refused(
            &one_processor(json!({"classpath": ["x:dep:../../2"]})),
            &env,
            "x:dep",
        );
        assert_refused(
            &one_processor(json!({"classpath": ["x:dep:2;evil"]})),
            &env,
            "x:dep",
        );
        // Outputs: key outside the roots, key not absolute, value not a SHA-1, unknown token.
        assert_refused(
            &one_processor(json!({"outputs": {"C:/Windows/evil.dll": sha}})),
            &env,
            "evil.dll",
        );
        assert_refused(
            &one_processor(json!({"outputs": {"relative.jar": sha}})),
            &env,
            "relative.jar",
        );
        assert_refused(
            &one_processor(json!({"outputs": {"{ROOT}/a.jar": "not-a-hash"}})),
            &env,
            "not-a-hash",
        );
        assert_refused(
            &one_processor(json!({"outputs": {"{NOPE}": sha}})),
            &env,
            "NOPE",
        );
    }

    #[test]
    fn server_only_hostile_processor_is_not_inspected() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env(tmp.path(), "1.20.1");
        let profile = one_processor(json!({"sides": ["server"], "args": ["{NOPE}"]}));
        stage_jars(&profile, &env);
        assert!(plan(&profile, &env).unwrap().is_empty());
    }

    #[test]
    fn missing_processor_jar_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env(tmp.path(), "1.20.1");
        let profile = one_processor(json!({}));
        assert!(plan(&profile, &env).is_err());
    }

    #[test]
    fn arguments_stay_single_uninterpreted_arguments() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env(tmp.path(), "1.20.1");
        let nasty = [
            "two words",
            "say \"hi\"",
            "a&b",
            "a|b",
            "%PATH%",
            "x;y",
            "$(whoami)",
            "`id`",
            "a>b<c",
            "caret^here",
            "it's",
        ];
        let profile = one_processor(json!({"args": nasty}));
        stage_jars(&profile, &env);
        let cmds = plan(&profile, &env).unwrap();
        assert_eq!(cmds[0].args, nasty);

        // The vector goes to a real process without a shell; each element arrives as one argument.
        let script = tmp.path().join("echo.ps1");
        std::fs::write(
            &script,
            "$args.Count\r\nforeach ($a in $args) { 'ARG[' + $a + ']' }\r\n",
        )
        .unwrap();
        let out = match std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
            ])
            .arg("-File")
            .arg(&script)
            .args(&cmds[0].args)
            .output()
        {
            Ok(o) => o,
            Err(e) => {
                eprintln!("skipping the process check, no powershell: {e}");
                return;
            }
        };
        let text = String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n");
        let mut lines = text.lines();
        assert_eq!(
            lines.next(),
            Some(nasty.len().to_string().as_str()),
            "{text}"
        );
        for (line, arg) in lines.zip(nasty) {
            assert_eq!(line, format!("ARG[{arg}]"));
        }
    }

    // --- install state ---------------------------------------------------------------------

    struct Install {
        _tmp: tempfile::TempDir,
        env: InstallEnv,
        cmds: Vec<ProcessorCommand>,
        data: HashMap<String, String>,
        version: VersionJson,
        marker: PathBuf,
        out: PathBuf,
        patched: PathBuf,
        provided: PathBuf,
    }

    const SHA_OF_INSTALLER: &str = "1111111111111111111111111111111111111111";

    /// A complete, valid installation: one client processor with an output, a patched jar with
    /// `PATCHED_SHA`, an installer-provided library and the marker.
    fn install() -> Install {
        let tmp = tempfile::tempdir().unwrap();
        let env = env(tmp.path(), "1.20.1");
        let (out_data, patched_data, provided_data) = (b"out".as_slice(), b"patched", b"provided");
        let profile = parse(
            &json!({"spec": 1, "minecraft": "1.20.1", "json": "/version.json",
                "data": {
                    "OUT": {"client": "[a:b:1:out]", "server": "[a:b:1:out]"},
                    "OUT_SHA": {"client": format!("'{}'", sha1_hex(out_data)), "server": "'x'"},
                    "PATCHED": {"client": "[a:b:1:patched]", "server": "[a:b:1:patched]"},
                    "PATCHED_SHA": {"client": format!("'{}'", sha1_hex(patched_data)), "server": "'x'"}
                },
                "libraries": [{"name": "x:tool:1"}],
                "processors": [{"jar": "x:tool:1", "args": ["{OUT}"],
                    "outputs": {"{OUT}": "{OUT_SHA}"}}]})
            .to_string(),
        );
        stage_jars(&profile, &env);
        let cmds = plan(&profile, &env).unwrap();
        let data = resolve_data(&profile, &env).unwrap();
        let out = PathBuf::from(&data["OUT"]);
        let patched = PathBuf::from(&data["PATCHED"]);
        let provided = env.libraries.join("p/q/1/q-1.jar");
        write(&out, out_data);
        write(&patched, patched_data);
        write(&provided, provided_data);
        let version: VersionJson = serde_json::from_value(json!({
        "id": "1.20.1-forge-1", "mainClass": "m",
        "libraries": [
            {"name": "p:q:1", "downloads": {"artifact": {
                "path": "p/q/1/q-1.jar", "url": "", "size": 8,
                "sha1": sha1_hex(provided_data)}}},
            {"name": "r:s:1", "downloads": {"artifact": {
                "path": "r/s/1/s-1.jar", "size": 1, "sha1": "00",
                "url": "https://example.invalid/s-1.jar"}}}
        ]}))
        .unwrap();
        let marker = env.root.join("loader.marker");
        write(&marker, format!("{SHA_OF_INSTALLER}\n").as_bytes());
        Install {
            _tmp: tmp,
            env,
            cmds,
            data,
            version,
            marker,
            out,
            patched,
            provided,
        }
    }

    impl Install {
        fn valid(&self) -> bool {
            install_state_valid(
                &self.cmds,
                &self.data,
                &self.version,
                &self.env.libraries,
                &self.marker,
                SHA_OF_INSTALLER,
            )
            .unwrap()
        }
    }

    #[test]
    fn install_state_complete_is_valid() {
        let i = install();
        assert_eq!(i.cmds[0].outputs.len(), 1);
        assert!(i.cmds[0].satisfied());
        assert!(i.valid());
    }

    #[test]
    fn install_state_marker_missing() {
        let i = install();
        std::fs::remove_file(&i.marker).unwrap();
        assert!(!i.valid());
    }

    #[test]
    fn install_state_other_installer() {
        let i = install();
        write(&i.marker, b"2222222222222222222222222222222222222222");
        assert!(!i.valid());
    }

    #[test]
    fn install_state_output_missing_despite_marker() {
        let i = install();
        assert!(i.marker.is_file());
        std::fs::remove_file(&i.out).unwrap();
        assert!(!i.valid());
        assert!(!i.cmds[0].satisfied());
    }

    #[test]
    fn install_state_output_wrong_hash() {
        let i = install();
        write(&i.out, b"corrupt");
        assert!(!i.valid());
        assert!(!i.cmds[0].satisfied());
    }

    #[test]
    fn install_state_patched_jar_missing() {
        let i = install();
        std::fs::remove_file(&i.patched).unwrap();
        assert!(!i.valid());
    }

    #[test]
    fn install_state_patched_jar_wrong_hash() {
        let i = install();
        write(&i.patched, b"tampered");
        assert!(!i.valid());
    }

    #[test]
    fn install_state_provided_library_missing_or_wrong() {
        let i = install();
        write(&i.provided, b"tampered");
        assert!(!i.valid());
        std::fs::remove_file(&i.provided).unwrap();
        assert!(!i.valid());
    }

    #[test]
    fn install_state_without_patched_sha_only_needs_the_file() {
        let mut i = install();
        i.data.remove("PATCHED_SHA");
        write(&i.patched, b"anything");
        assert!(i.valid());
        std::fs::remove_file(&i.patched).unwrap();
        assert!(!i.valid());
    }

    #[test]
    fn processor_without_outputs_is_never_satisfied() {
        let cmd = ProcessorCommand {
            jar: PathBuf::new(),
            classpath: vec![],
            main_class: "m".into(),
            args: vec![],
            outputs: vec![],
        };
        assert!(!cmd.satisfied());
    }
}
