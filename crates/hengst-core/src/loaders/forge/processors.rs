//! Planning the processors of a modern Forge or NeoForge installer and checking whether an
//! installation is complete. Nothing here runs a JVM, an installer or a processor.
//!
//! Everything in the install profile is untrusted remote data that ends up on a command line and
//! in file paths, so each string is validated where it becomes a path or an argument.
//!
//! Intended use (Task 11): hash the installer, `plan`, and skip all processors only if
//! `install_state_valid` is true. Otherwise run the commands in order with the work directory as
//! current directory, calling `ProcessorCommand::verify_files` right before each spawn and
//! `check_outputs` after it, then `write_marker`.

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

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
    /// Per-install scratch directory the installer's data files are extracted to. Processors run
    /// with it as current directory.
    pub work: PathBuf,
}

/// One processor, ready to run as `java -cp <jar + classpath> <main_class> <args>` (no shell).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessorCommand {
    pub jar: PathBuf,
    /// SHA-1 the profile declares for `jar`.
    pub jar_sha1: String,
    pub classpath: Vec<PathBuf>,
    /// SHA-1 the profile declares for each classpath entry, in the same order.
    pub classpath_sha1: Vec<String>,
    pub main_class: String,
    pub args: Vec<String>,
    /// Files the processor produces with their expected SHA-1.
    pub outputs: Vec<(PathBuf, String)>,
}

impl ProcessorCommand {
    /// Re-checks the jar and the classpath files against their declared SHA-1. Call it right
    /// before starting the process.
    pub fn verify_files(&self) -> Result<()> {
        let files = std::iter::once((&self.jar, &self.jar_sha1))
            .chain(self.classpath.iter().zip(&self.classpath_sha1));
        for (path, sha) in files {
            if !has_sha1(path, sha) {
                return Err(bad(format!(
                    "{} does not match its declared SHA-1",
                    path.display()
                )));
            }
        }
        Ok(())
    }

    /// Checks the declared outputs after the processor ran.
    pub fn check_outputs(&self) -> Result<()> {
        for (path, sha) in &self.outputs {
            match sha1_file(path) {
                Ok(actual) if actual.eq_ignore_ascii_case(sha) => {}
                Ok(actual) => {
                    return Err(Error::Checksum {
                        path: path.clone(),
                        expected: sha.clone(),
                        actual,
                    })
                }
                Err(_) => {
                    return Err(bad(format!(
                        "processor output {} is missing or unreadable",
                        path.display()
                    )))
                }
            }
        }
        Ok(())
    }
}

/// The processors to run together with the data they were planned from, so the install state is
/// always checked against the same map.
#[derive(Clone, Debug)]
pub struct Plan {
    pub commands: Vec<ProcessorCommand>,
    pub data: HashMap<String, String>,
    /// Files below the libraries directory the processors produce without a declared hash (data
    /// coordinates that are neither downloaded libraries, declared outputs nor `PATCHED`).
    produced: Vec<PathBuf>,
}

/// Largest `META-INF/MANIFEST.MF` we read; real ones are a few hundred bytes.
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
/// Largest install marker we read; a real one is a few hundred bytes.
const MAX_MARKER_BYTES: u64 = 64 * 1024;
const MARKER_MAGIC: &str = "hengst-loader-marker 1";

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

/// A short, printable excerpt of untrusted text for error messages.
fn excerpt(s: &str) -> String {
    let mut out: String = s
        .chars()
        .take(40)
        .map(|c| if c.is_control() { '?' } else { c })
        .collect();
    if s.chars().count() > 40 {
        out.push_str("...");
    }
    out
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

/// Data key and resolved path of every data value that was a Maven coordinate.
type CoordinatePaths = Vec<(String, PathBuf)>;

/// Client-side values of the profile's `data`, plus the fixed tokens, and the paths of the
/// values that were coordinates. Coordinates resolve below the libraries directory, quoted values
/// are literals, anything else is an installer entry that is extracted below `env.work`. The
/// fixed tokens cannot be overridden by the profile.
///
/// The extracted data files are trusted only because the installer itself was verified by the
/// caller; they are rewritten from it on every call and never read back from an earlier run.
fn resolve_data(
    profile: &ModernProfile,
    env: &InstallEnv,
) -> Result<(HashMap<String, String>, CoordinatePaths)> {
    let mut data = HashMap::new();
    let mut coords = Vec::new();
    let mut jobs = Vec::new();
    for (key, sided) in &profile.data {
        let v = sided.client.as_str();
        let value = if let Some(coord) = as_coordinate(v) {
            let path = library_file(&env.libraries, coord)
                .map_err(|e| bad(format!("data {key}: {}", inner(e))))?;
            let value = path_string(&path);
            coords.push((key.clone(), path));
            value
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
    Ok((data, coords))
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

/// `(artifact path, sha1)` of every profile library, by Maven path of its name. A library without
/// a download artifact has empty strings.
type Declared = HashMap<String, Vec<(String, String)>>;

fn declared_libraries(profile: &ModernProfile) -> Declared {
    let mut declared = Declared::new();
    for l in &profile.libraries {
        let Some(rel) = maven_path(&l.name) else {
            continue;
        };
        let artifact = l.downloads.as_ref().and_then(|d| d.artifact.as_ref());
        declared.entry(rel).or_default().push(match artifact {
            Some(a) => (a.path.clone(), a.sha1.clone()),
            None => (String::new(), String::new()),
        });
    }
    declared
}

/// A processor jar or classpath entry: it must be one of the profile's declared libraries, stored
/// at the Maven path of its name, with a declared SHA-1, which the file on disk must match. It
/// must not carry the classpath separator. Returns the file and its SHA-1.
fn declared_file(coord: &str, declared: &Declared, libraries: &Path) -> Result<(PathBuf, String)> {
    let rel = maven_path(coord).ok_or_else(|| Error::UnsafePath(coord.to_string()))?;
    if rel.contains(';') {
        return Err(bad(format!(
            "{coord}: ';' is not allowed in a classpath entry"
        )));
    }
    let entries = declared
        .get(&rel)
        .ok_or_else(|| bad(format!("{coord} is not one of the profile's libraries")))?;
    let (path, sha1) = &entries[0];
    if entries.iter().any(|e| e != &entries[0]) {
        return Err(bad(format!(
            "{coord} is declared more than once, differently"
        )));
    }
    if path.is_empty() {
        return Err(bad(format!(
            "{coord} has no download artifact in the profile"
        )));
    }
    if path != &rel {
        return Err(bad(format!(
            "{coord}: artifact path differs from the Maven path of its name"
        )));
    }
    if validate_sha1(sha1).is_err() {
        return Err(bad(format!("{coord} has no declared SHA-1")));
    }
    let file = join_relative(libraries, &rel)?;
    if !has_sha1(&file, sha1) {
        return Err(bad(format!(
            "{coord}: file is missing or does not match its declared SHA-1"
        )));
    }
    Ok((file, sha1.clone()))
}

/// A path taken from the profile: absolute, no UNC or verbatim form, and below one of `roots`
/// (the root directory itself only if `allow_root`) or exactly one of `exact`. Every component
/// below a root must pass the same rules as downloaded paths (no trailing dot or space, no device
/// names, no `:`).
fn contained(s: &str, roots: &[&Path], exact: &[&Path], allow_root: bool) -> bool {
    if s.starts_with("\\\\") || s.starts_with("//") {
        return false;
    }
    let p = Path::new(s);
    if !p.is_absolute() {
        return false;
    }
    if exact.contains(&p) {
        return true;
    }
    roots.iter().any(|r| {
        let Ok(rest) = p.strip_prefix(r) else {
            return false;
        };
        let mut rel = Vec::new();
        for c in rest.components() {
            match c.as_os_str().to_str() {
                Some(part) if matches!(c, Component::Normal(_)) => rel.push(part),
                _ => return false,
            }
        }
        if rel.is_empty() {
            return allow_root;
        }
        join_relative(Path::new("x"), &rel.join("/")).is_ok()
    })
}

/// An argument that looks like a path (contains `/`, `\` or `:`) must be contained; so must the
/// value of a `--key=value` argument. Anything else is a plain option or value.
fn check_argument(arg: &str, roots: &[&Path], exact: &[&Path]) -> bool {
    let value = match arg.strip_prefix('-').and(arg.split_once('=')) {
        Some((_, v)) => v,
        None => arg,
    };
    !value.contains(['/', '\\', ':']) || contained(value, roots, exact, true)
}

fn plan_one(
    p: &Processor,
    data: &HashMap<String, String>,
    declared: &Declared,
    env: &InstallEnv,
) -> Result<ProcessorCommand> {
    let (jar, jar_sha1) = declared_file(&p.jar, declared, &env.libraries)?;
    let mut classpath = Vec::new();
    let mut classpath_sha1 = Vec::new();
    for c in &p.classpath {
        let (file, sha) = declared_file(c, declared, &env.libraries)?;
        classpath.push(file);
        classpath_sha1.push(sha);
    }
    let main_class = jar_main_class(&jar)?;
    let roots = [env.libraries.as_path(), env.work.as_path()];
    let exact = [env.minecraft_jar.as_path()];
    let mut args = Vec::new();
    for (j, a) in p.args.iter().enumerate() {
        let arg = substitute(a, data, &env.libraries)?;
        if !check_argument(&arg, &roots, &exact) {
            return Err(bad(format!(
                "argument {j} ({}) is a path outside the launcher directories",
                excerpt(&arg)
            )));
        }
        args.push(arg);
    }
    let mut outputs = Vec::new();
    for (key, value) in &p.outputs {
        let key = substitute(key, data, &env.libraries)?;
        if !contained(&key, &roots, &[], false) {
            return Err(bad(format!(
                "output {} is outside the libraries and work directories",
                excerpt(&key)
            )));
        }
        let sha = substitute(value, data, &env.libraries)?;
        validate_sha1(&sha)
            .map_err(|_| bad(format!("output hash {} is not a SHA-1", excerpt(&sha))))?;
        outputs.push((PathBuf::from(key), sha));
    }
    Ok(ProcessorCommand {
        jar,
        jar_sha1,
        classpath,
        classpath_sha1,
        main_class,
        args,
        outputs,
    })
}

/// The client-side processors of the profile in order, ready to run, with the data they were
/// planned from. Needs the processor jars on disk (they are checked against their declared SHA-1
/// before their manifest is read) and the installer (for its data files, extracted into
/// `env.work` on every call). An invalid processor is an error naming its index in the profile.
pub fn plan(profile: &ModernProfile, env: &InstallEnv) -> Result<Plan> {
    let (data, coords) = resolve_data(profile, env)?;
    let declared = declared_libraries(profile);
    let commands = profile
        .processors
        .iter()
        .enumerate()
        .filter(|(_, p)| is_client(p))
        .map(|(i, p)| {
            plan_one(p, &data, &declared, env)
                .map_err(|e| bad(format!("processor {i}: {}", inner(e))))
        })
        .collect::<Result<Vec<_>>>()?;
    let downloaded: HashSet<PathBuf> = profile
        .libraries
        .iter()
        .filter_map(|l| l.downloads.as_ref()?.artifact.as_ref())
        .filter_map(|a| join_relative(&env.libraries, &a.path).ok())
        .collect();
    let declared_outputs: HashSet<&PathBuf> = commands
        .iter()
        .flat_map(|c| &c.outputs)
        .map(|(p, _)| p)
        .collect();
    let mut produced: Vec<PathBuf> = coords
        .into_iter()
        .filter(|(k, p)| k != "PATCHED" && !downloaded.contains(p) && !declared_outputs.contains(p))
        .map(|(_, p)| p)
        .collect();
    produced.sort();
    produced.dedup();
    Ok(Plan {
        commands,
        data,
        produced,
    })
}

fn patched_ok(plan: &Plan) -> Option<(PathBuf, String)> {
    let patched = PathBuf::from(plan.data.get("PATCHED")?);
    let actual = sha1_file(&patched).ok()?;
    match plan.data.get("PATCHED_SHA") {
        Some(sha) if !actual.eq_ignore_ascii_case(sha) => None,
        _ => Some((patched, actual)),
    }
}

/// Records a finished installation: the installer's SHA-1, the actual SHA-1 of the patched jar
/// and of every produced file without a declared hash that exists. Declared outputs and
/// `PATCHED_SHA` are verified first; any mismatch is an error and no marker is written.
pub fn write_marker(plan: &Plan, marker: &Path, installer_sha1: &str) -> Result<()> {
    validate_sha1(installer_sha1)?;
    for c in &plan.commands {
        c.check_outputs()?;
    }
    let (_, patched_sha) = patched_ok(plan)
        .ok_or_else(|| bad("the patched client jar is missing or does not match PATCHED_SHA"))?;
    let mut text = format!("{MARKER_MAGIC}\ninstaller {installer_sha1}\npatched {patched_sha}\n");
    for path in &plan.produced {
        let Ok(sha) = sha1_file(path) else { continue };
        let p = path_string(path);
        if p.contains(['\n', '\r']) {
            return Err(Error::UnsafePath(excerpt(&p)));
        }
        text.push_str(&format!("output {sha} {p}\n"));
    }
    if let Some(dir) = marker.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut tmp = marker.as_os_str().to_owned();
    tmp.push(".part");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, marker)?;
    Ok(())
}

struct Marker {
    installer: String,
    patched: String,
    outputs: Vec<(String, String)>,
}

/// Reads the marker as untrusted local data: bounded size, strict format, `None` on any problem.
fn read_marker(path: &Path) -> Option<Marker> {
    let mut buf = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(MAX_MARKER_BYTES + 1)
        .read_to_end(&mut buf)
        .ok()?;
    if buf.len() as u64 > MAX_MARKER_BYTES {
        return None;
    }
    let text = String::from_utf8(buf).ok()?;
    let mut lines = text.lines();
    if lines.next()? != MARKER_MAGIC {
        return None;
    }
    let hash = |s: &str| validate_sha1(s).is_ok().then(|| s.to_ascii_lowercase());
    let installer = hash(lines.next()?.strip_prefix("installer ")?)?;
    let patched = hash(lines.next()?.strip_prefix("patched ")?)?;
    let mut outputs = Vec::new();
    for line in lines {
        let (sha, p) = line.strip_prefix("output ")?.split_once(' ')?;
        outputs.push((p.to_string(), hash(sha)?));
    }
    Some(Marker {
        installer,
        patched,
        outputs,
    })
}

/// True only when all of this holds: the marker (written by `write_marker`) names this installer;
/// the patched client jar (`PATCHED`, required) exists, matches `PATCHED_SHA` when the profile
/// has one and the SHA-1 the marker recorded; every declared output has its SHA-1; every file
/// the marker recorded still has the recorded SHA-1 and is one the plan produces; and every
/// installer-provided library of `version` (empty `url`) matches its SHA-1. A marker alone is
/// never trusted, and an unreadable or malformed marker just means "not valid".
pub fn install_state_valid(
    plan: &Plan,
    version: &VersionJson,
    libraries: &Path,
    marker: &Path,
    installer_sha1: &str,
) -> Result<bool> {
    let Some(m) = read_marker(marker) else {
        return Ok(false);
    };
    if !m.installer.eq_ignore_ascii_case(installer_sha1) {
        return Ok(false);
    }
    match patched_ok(plan) {
        Some((_, actual)) if actual == m.patched => {}
        _ => return Ok(false),
    }
    if !plan
        .commands
        .iter()
        .flat_map(|c| &c.outputs)
        .all(|(path, sha)| has_sha1(path, sha))
    {
        return Ok(false);
    }
    let produced: HashSet<String> = plan.produced.iter().map(|p| path_string(p)).collect();
    let mut seen = HashSet::new();
    for (p, sha) in &m.outputs {
        if !produced.contains(p) || !seen.insert(p) || !has_sha1(Path::new(p), sha) {
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
    use crate::libraries::maven_path;
    use serde_json::{json, Value};
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

    /// A library entry with a download artifact whose SHA-1 `stage_jars` fills in.
    fn lib(name: &str) -> Value {
        json!({"name": name, "downloads": {"artifact": {
            "path": maven_path(name).unwrap(), "sha1": "", "size": 1,
            "url": "https://example.invalid/x.jar"}}})
    }

    /// Puts a dummy jar with a manifest at the artifact path of every library of the profile and
    /// declares its real SHA-1, so the profile's own hashes (which belong to the real jars) do not
    /// matter.
    fn stage_jars(profile: &mut ModernProfile, env: &InstallEnv) {
        for l in &mut profile.libraries {
            let Some(a) = l.downloads.as_mut().and_then(|d| d.artifact.as_mut()) else {
                continue;
            };
            let path = join_relative(&env.libraries, &a.path).unwrap();
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            zip_file(&path, &[("META-INF/MANIFEST.MF", &manifest("test.Main"))]);
            a.sha1 = sha1_file(&path).unwrap();
        }
    }

    /// Parses, stages and plans a profile.
    fn build(profile: Value, game: &str) -> (tempfile::TempDir, InstallEnv, Plan) {
        let tmp = tempfile::tempdir().unwrap();
        let env = env(tmp.path(), game);
        let mut profile = parse(&profile.to_string());
        stage_jars(&mut profile, &env);
        let plan = plan(&profile, &env).unwrap();
        (tmp, env, plan)
    }

    fn real(json: &str, game: &str) -> (tempfile::TempDir, InstallEnv, Plan) {
        let tmp = tempfile::tempdir().unwrap();
        let env = env(tmp.path(), game);
        let mut profile = parse(json);
        stage_jars(&mut profile, &env);
        let plan = plan(&profile, &env).unwrap();
        (tmp, env, plan)
    }

    fn rel(env: &InstallEnv, s: &str) -> String {
        s.replace(&env.root.to_string_lossy().into_owned(), "<home>")
            .replace('\\', "/")
    }

    #[test]
    fn resolves_data_like_the_forge_installer() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env(tmp.path(), "1.20.1");
        let (d, _) = resolve_data(&parse(PROFILE_1201), &env).unwrap();
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
        assert_eq!(resolve_data(&p, &env).unwrap().0["SIDE"], "client");
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
        let (_tmp, env, plan) = real(PROFILE_1201, "1.20.1");
        let cmds = &plan.commands;
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
        assert_eq!(split.classpath_sha1.len(), 2);
        split.verify_files().unwrap();
        assert_eq!(plan.data["SIDE"], "client");

        let (_tmp2, neo_env, neo) = real(PROFILE_NEO, "26.3");
        assert_eq!(neo.commands.len(), 1);
        assert!(neo.commands[0]
            .args
            .iter()
            .any(|a| a == &format!("{}/libraries/", neo_env.root.to_string_lossy())));
        assert_eq!(
            &neo.commands[0].args[..2],
            ["--task", "PROCESS_MINECRAFT_JAR"]
        );
    }

    /// The full argument vectors of the real fixtures, relative to the launcher home.
    #[test]
    fn real_fixture_argument_vectors() {
        let l = "<home>/libraries";
        let c = format!("{l}/de/oceanlabs/mcp/mcp_config/1.20.1-20230612.114412/mcp_config-1.20.1-20230612.114412");
        let m = format!(
            "{l}/net/minecraft/client/1.20.1-20230612.114412/client-1.20.1-20230612.114412"
        );
        let (_t, env1, forge) = real(PROFILE_1201, "1.20.1");
        let got: Vec<Vec<String>> = forge
            .commands
            .iter()
            .map(|c| c.args.iter().map(|a| rel(&env1, a)).collect())
            .collect();
        let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            got,
            vec![
                v(&["--task", "MCP_DATA", "--input", &format!("{c}.zip"), "--output", &format!("{c}-mappings.txt"), "--key", "mappings"]),
                v(&["--task", "DOWNLOAD_MOJMAPS", "--version", "1.20.1", "--side", "client", "--output", &format!("{m}-mappings.txt")]),
                v(&["--task", "MERGE_MAPPING", "--left", &format!("{c}-mappings.txt"), "--right", &format!("{m}-mappings.txt"), "--output", &format!("{c}-mappings-merged.txt"), "--classes", "--reverse-right"]),
                v(&["--input", "<home>/versions/1.20.1/1.20.1.jar", "--slim", &format!("{m}-slim.jar"), "--extra", &format!("{m}-extra.jar"), "--srg", &format!("{c}-mappings-merged.txt")]),
                v(&["--input", &format!("{m}-slim.jar"), "--output", &format!("{m}-srg.jar"), "--names", &format!("{c}-mappings-merged.txt"), "--ann-fix", "--ids-fix", "--src-fix", "--record-fix"]),
                v(&["--clean", &format!("{m}-srg.jar"), "--output", &format!("{l}/net/minecraftforge/forge/1.20.1-47.4.28/forge-1.20.1-47.4.28-client.jar"), "--apply", "<home>/work/data/client.lzma"]),
            ]
        );
        let (_t2, env2, neo) = real(PROFILE_NEO, "26.3");
        let got: Vec<String> = neo.commands[0].args.iter().map(|a| rel(&env2, a)).collect();
        assert_eq!(
            got,
            v(&["--task", "PROCESS_MINECRAFT_JAR", "--no-mod-manifest", "--input", "<home>/versions/26.3/26.3.jar", "--output", &format!("{l}/net/neoforged/minecraft-client-patched/26.3.0.115/minecraft-client-patched-26.3.0.115.jar"), "--extract-libraries-to", &format!("{l}/"), "--apply-patches", "<home>/work/data/client.lzma"])
        );
    }

    #[test]
    fn server_only_processors_are_skipped_and_unsided_ones_kept() {
        let (_t, _e, plan) = build(
            json!({"spec": 1, "minecraft": "1.20.1", "json": "/version.json",
            "libraries": [lib("x:tool:1")],
            "processors": [
                {"jar": "x:tool:1", "args": ["unsided"]},
                {"jar": "x:tool:1", "sides": ["server"], "args": ["server"]},
                {"jar": "x:tool:1", "sides": ["client", "server"], "args": ["both"]},
                {"jar": "x:tool:1", "sides": ["client"], "args": ["client"]},
                {"jar": "x:tool:1", "sides": [], "args": ["empty"]}
            ]}),
            "1.20.1",
        );
        let got: Vec<&str> = plan.commands.iter().map(|c| c.args[0].as_str()).collect();
        assert_eq!(got, ["unsided", "both", "client"]);
    }

    fn processors_profile(processors: Value) -> Value {
        json!({"spec": 1, "minecraft": "1.20.1", "json": "/version.json",
            "libraries": [lib("x:tool:1"), lib("x:dep:2")], "processors": processors})
    }

    fn one_processor(extra: Value) -> Value {
        let mut p = json!({"jar": "x:tool:1", "args": []});
        p.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        processors_profile(json!([p]))
    }

    fn plan_error(profile: Value) -> String {
        let tmp = tempfile::tempdir().unwrap();
        let env = env(tmp.path(), "1.20.1");
        let mut profile = parse(&profile.to_string());
        stage_jars(&mut profile, &env);
        match plan(&profile, &env) {
            Err(Error::Installer(m)) => m,
            other => panic!("expected an installer error, got {other:?}"),
        }
    }

    fn assert_refused(profile: Value, index: usize, needle: &str) {
        let m = plan_error(profile);
        assert!(m.contains(&format!("processor {index}")), "{m}");
        assert!(m.contains(needle), "{m}");
    }

    #[test]
    fn hostile_profiles_are_refused_naming_the_processor() {
        let sha = "0123456789abcdef0123456789abcdef01234567";
        assert_refused(one_processor(json!({"args": ["{NOPE}"]})), 0, "NOPE");
        assert_refused(one_processor(json!({"args": ["{ROOT}/../evil"]})), 0, "..");
        assert_refused(one_processor(json!({"jar": "x:other:1"})), 0, "x:other:1");
        assert_refused(
            one_processor(json!({"classpath": ["x:other:1"]})),
            0,
            "x:other:1",
        );
        assert_refused(
            one_processor(json!({"classpath": ["x:dep:../../2"]})),
            0,
            "x:dep",
        );
        assert_refused(
            one_processor(json!({"classpath": ["x:dep:2;evil"]})),
            0,
            "x:dep",
        );
        // Outputs: outside the libraries and work directories (the launcher root is too wide),
        // not absolute, reserved or trailing-dot components, bad value, unknown token.
        for (key, needle) in [
            ("C:/Windows/evil.dll", "evil.dll"),
            ("relative.jar", "relative.jar"),
            ("{ROOT}/versions/x.jar", "outside"),
            ("{LIBRARY_DIR}/CON/x.jar", "outside"),
            ("{LIBRARY_DIR}/a.", "outside"),
            ("{LIBRARY_DIR}", "outside"),
            ("{LIBRARY_DIR}/a.jar:stream", "outside"),
        ] {
            assert_refused(one_processor(json!({"outputs": {key: sha}})), 0, needle);
        }
        assert_refused(
            one_processor(json!({"outputs": {"{LIBRARY_DIR}/a.jar": "not-a-hash"}})),
            0,
            "not-a-hash",
        );
        assert_refused(
            one_processor(json!({"outputs": {"{NOPE}": sha}})),
            0,
            "NOPE",
        );
    }

    #[test]
    fn outputs_inside_libraries_or_work_are_accepted() {
        let sha = "0123456789abcdef0123456789abcdef01234567";
        let (_t, env, plan) = build(
            one_processor(json!({"outputs": {
                "{LIBRARY_DIR}/a/b.jar": sha, "{ROOT}/work/c.bin": sha}})),
            "1.20.1",
        );
        let outs = &plan.commands[0].outputs;
        assert!(outs
            .iter()
            .any(|(p, _)| *p == env.libraries.join("a/b.jar")));
        assert!(outs.iter().any(|(p, _)| *p == env.work.join("c.bin")));
    }

    #[test]
    fn path_arguments_must_stay_in_the_launcher_directories() {
        let refuse = |literal: &str, arg: &str| {
            let mut p = one_processor(json!({"args": ["--output", arg]}));
            p["data"] = json!({"X": {"client": format!("'{literal}'"), "server": "'x'"}});
            let m = plan_error(p);
            assert!(m.contains("processor 0") && m.contains("argument 1"), "{m}");
            assert!(m.len() < 200, "message must stay short: {m}");
        };
        refuse("C:/Users/u/Documents/f", "{X}");
        refuse("C:\\Users\\u\\Documents\\f", "{X}");
        refuse("\\\\server\\share\\f", "{X}");
        refuse("\\\\?\\C:\\f", "{X}");
        refuse("//server/share/f", "{X}");
        refuse("/etc/passwd", "{X}");
        refuse("relative/path", "{X}");
        refuse("C:f", "{X}");
        refuse("x", "--out=C:/Users/u/f");
        refuse("x", "{LIBRARY_DIR}/CON/f");
        refuse("x", "{LIBRARY_DIR}/a./f");
        refuse("x", "{ROOT}/versions/other/other.jar");
        refuse(&"a/".repeat(100), "{X}");

        // Allowed: plain values, the libraries and work directories, the vanilla jar, and
        // --key=value with a contained value.
        let (_t, env, plan) = build(
            one_processor(json!({"args": [
                "--task", "1.20.1", "{LIBRARY_DIR}", "{LIBRARY_DIR}/a/b.jar", "{MINECRAFT_JAR}",
                "{ROOT}/work/data/x", "--out={LIBRARY_DIR}/y.jar", "-Dx=plain"]})),
            "1.20.1",
        );
        assert_eq!(plan.commands[0].args.len(), 8);
        assert_eq!(PathBuf::from(&plan.commands[0].args[4]), env.minecraft_jar);
    }

    #[test]
    fn processor_index_is_the_position_in_the_profile() {
        let m = plan_error(processors_profile(json!([
            {"jar": "x:tool:1", "sides": ["server"], "args": []},
            {"jar": "x:tool:1", "sides": ["server"], "args": []},
            {"jar": "x:tool:1", "args": ["{NOPE}"]}
        ])));
        assert!(m.contains("processor 2") && m.contains("NOPE"), "{m}");
    }

    #[test]
    fn server_only_hostile_processor_is_not_inspected() {
        let (_t, _e, plan) = build(
            one_processor(json!({"sides": ["server"], "args": ["{NOPE}"]})),
            "1.20.1",
        );
        assert!(plan.commands.is_empty());
    }

    #[test]
    fn missing_processor_jar_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env(tmp.path(), "1.20.1");
        let profile = parse(&one_processor(json!({})).to_string());
        assert!(plan(&profile, &env).is_err());
    }

    #[test]
    fn processor_files_must_match_their_declared_sha1() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env(tmp.path(), "1.20.1");
        let mut profile = parse(&one_processor(json!({"classpath": ["x:dep:2"]})).to_string());
        stage_jars(&mut profile, &env);
        plan(&profile, &env).unwrap();
        // The jar changes on disk after the profile's hash was declared.
        let jar = env.libraries.join("x/tool/1/tool-1.jar");
        zip_file(&jar, &[("META-INF/MANIFEST.MF", &manifest("evil.Main"))]);
        let Err(Error::Installer(m)) = plan(&profile, &env) else {
            panic!("a changed processor jar must be refused")
        };
        assert!(m.contains("processor 0") && m.contains("SHA-1"), "{m}");
        // The same for a classpath entry.
        stage_jars(&mut profile, &env);
        zip_file(
            &env.libraries.join("x/dep/2/dep-2.jar"),
            &[("a", b"changed")],
        );
        let Err(Error::Installer(m)) = plan(&profile, &env) else {
            panic!("a changed classpath entry must be refused")
        };
        assert!(m.contains("processor 0") && m.contains("x:dep:2"), "{m}");
    }

    #[test]
    fn processor_libraries_need_a_declared_hash_and_the_maven_path() {
        for (change, needle) in [
            (json!({"path": "other/place/tool-1.jar"}), "artifact path"),
            (json!({"sha1": ""}), "SHA-1"),
        ] {
            let mut profile = one_processor(json!({}));
            let artifact = &mut profile["libraries"][0]["downloads"]["artifact"];
            artifact
                .as_object_mut()
                .unwrap()
                .extend(change.as_object().unwrap().clone());
            let tmp = tempfile::tempdir().unwrap();
            let env = env(tmp.path(), "1.20.1");
            let mut p = parse(&profile.to_string());
            // Stage the jar at the Maven path, as a downloader following the name would.
            let at = env.libraries.join("x/tool/1/tool-1.jar");
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            zip_file(&at, &[("META-INF/MANIFEST.MF", &manifest("test.Main"))]);
            if needle == "SHA-1" {
                p.libraries[0]
                    .downloads
                    .as_mut()
                    .unwrap()
                    .artifact
                    .as_mut()
                    .unwrap()
                    .sha1
                    .clear();
            }
            let Err(Error::Installer(m)) = plan(&p, &env) else {
                panic!("{needle}: must be refused")
            };
            assert!(m.contains("processor 0") && m.contains(needle), "{m}");
        }
        // A library without any download artifact cannot be a processor jar.
        let mut profile = one_processor(json!({}));
        profile["libraries"][0] = json!({"name": "x:tool:1"});
        let m = plan_error(profile);
        assert!(
            m.contains("processor 0") && m.contains("no download artifact"),
            "{m}"
        );
    }

    #[test]
    fn command_checks_files_and_outputs() {
        let sha = sha1_hex(b"out");
        let (_t, env, plan) = build(
            one_processor(json!({"outputs": {"{LIBRARY_DIR}/o.bin": sha}})),
            "1.20.1",
        );
        let cmd = &plan.commands[0];
        assert!(cmd.check_outputs().is_err());
        write(&env.libraries.join("o.bin"), b"out");
        cmd.check_outputs().unwrap();
        write(&env.libraries.join("o.bin"), b"other");
        assert!(matches!(cmd.check_outputs(), Err(Error::Checksum { .. })));
        cmd.verify_files().unwrap();
        std::fs::write(&cmd.jar, b"tampered").unwrap();
        assert!(cmd.verify_files().is_err());
    }

    #[test]
    fn arguments_stay_single_uninterpreted_arguments() {
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
        let (tmp, _env, plan) = build(one_processor(json!({"args": nasty})), "1.20.1");
        assert_eq!(plan.commands[0].args, nasty);

        // The vector goes to a real process without a shell; each element arrives as one argument.
        let script = tmp.path().join("echo.ps1");
        std::fs::write(
            &script,
            "$args.Count\r\nforeach ($a in $args) { 'ARG[' + $a + ']' }\r\n",
        )
        .unwrap();
        let out = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
            ])
            .arg("-File")
            .arg(&script)
            .args(&plan.commands[0].args)
            .output()
            .expect(
                "powershell.exe is part of every supported Windows and is needed for this test",
            );
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
        plan: Plan,
        version: VersionJson,
        marker: PathBuf,
        out: PathBuf,
        mid: PathBuf,
        patched: PathBuf,
        provided: PathBuf,
    }

    const SHA_OF_INSTALLER: &str = "1111111111111111111111111111111111111111";

    fn data_entry(client: &str) -> Value {
        json!({"client": client, "server": "'x'"})
    }

    /// A complete, valid installation: one client processor with a declared output, a produced
    /// file without a declared hash (`MID`), a patched jar with `PATCHED_SHA`, an
    /// installer-provided library and the marker written by `write_marker`.
    fn install() -> Install {
        let (out_data, patched_data, provided_data) = (b"out".as_slice(), b"patched", b"provided");
        let (tmp, env, plan) = build(
            json!({"spec": 1, "minecraft": "1.20.1", "json": "/version.json",
                "data": {
                    "OUT": data_entry("[a:b:1:out]"),
                    "OUT_SHA": data_entry(&format!("'{}'", sha1_hex(out_data))),
                    "MID": data_entry("[a:b:1:mid]"),
                    "PATCHED": data_entry("[a:b:1:patched]"),
                    "PATCHED_SHA": data_entry(&format!("'{}'", sha1_hex(patched_data)))
                },
                "libraries": [lib("x:tool:1")],
                "processors": [{"jar": "x:tool:1", "args": ["{OUT}", "{MID}"],
                    "outputs": {"{OUT}": "{OUT_SHA}"}}]}),
            "1.20.1",
        );
        let out = PathBuf::from(&plan.data["OUT"]);
        let mid = PathBuf::from(&plan.data["MID"]);
        let patched = PathBuf::from(&plan.data["PATCHED"]);
        let provided = env.libraries.join("p/q/1/q-1.jar");
        write(&out, out_data);
        write(&mid, b"mid");
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
        write_marker(&plan, &marker, SHA_OF_INSTALLER).unwrap();
        Install {
            _tmp: tmp,
            env,
            plan,
            version,
            marker,
            out,
            mid,
            patched,
            provided,
        }
    }

    impl Install {
        fn valid_for(&self, installer: &str) -> bool {
            install_state_valid(
                &self.plan,
                &self.version,
                &self.env.libraries,
                &self.marker,
                installer,
            )
            .unwrap()
        }
        fn valid(&self) -> bool {
            self.valid_for(SHA_OF_INSTALLER)
        }
    }

    #[test]
    fn install_state_complete_is_valid() {
        let i = install();
        assert_eq!(i.plan.commands[0].outputs.len(), 1);
        let marker = std::fs::read_to_string(&i.marker).unwrap();
        assert!(marker.contains(&sha1_hex(b"patched")), "{marker}");
        assert!(marker.contains(&sha1_hex(b"mid")), "{marker}");
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
        assert!(!i.valid_for("2222222222222222222222222222222222222222"));
        // A marker written for another installer build.
        write_marker(
            &i.plan,
            &i.marker,
            "2222222222222222222222222222222222222222",
        )
        .unwrap();
        assert!(!i.valid());
    }

    #[test]
    fn install_state_output_missing_despite_marker() {
        let i = install();
        assert!(i.marker.is_file());
        std::fs::remove_file(&i.out).unwrap();
        assert!(!i.valid());
    }

    #[test]
    fn install_state_output_wrong_hash() {
        let i = install();
        write(&i.out, b"corrupt");
        assert!(!i.valid());
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
    fn install_state_recorded_undeclared_output_changed_or_missing() {
        let i = install();
        write(&i.mid, b"changed");
        assert!(!i.valid());
        std::fs::remove_file(&i.mid).unwrap();
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
    fn install_state_neoforge_style_patched_jar_changed_after_marker() {
        // No PATCHED_SHA, no declared outputs: only the marker's recorded hash protects the jar.
        let (_t, env, plan) = build(
            json!({"spec": 1, "minecraft": "1.20.1", "json": "/version.json",
                "data": {"PATCHED": data_entry("[n:patched:1]")},
                "libraries": [lib("x:tool:1")],
                "processors": [{"jar": "x:tool:1", "args": ["{PATCHED}"]}]}),
            "1.20.1",
        );
        let patched = PathBuf::from(&plan.data["PATCHED"]);
        write(&patched, b"the real patched jar");
        let marker = env.root.join("m");
        write_marker(&plan, &marker, SHA_OF_INSTALLER).unwrap();
        let version: VersionJson =
            serde_json::from_value(json!({"id": "v", "mainClass": "m"})).unwrap();
        let check = || {
            install_state_valid(&plan, &version, &env.libraries, &marker, SHA_OF_INSTALLER).unwrap()
        };
        assert!(check());
        write(&patched, b"the real patched ja");
        assert!(!check());
        write(&patched, b"the real patched jar");
        assert!(check());
        std::fs::remove_file(&patched).unwrap();
        assert!(!check());
    }

    #[test]
    fn install_state_patched_key_absent() {
        let (_t, env, plan) = build(
            json!({"spec": 1, "minecraft": "1.20.1", "json": "/version.json",
                "libraries": [lib("x:tool:1")],
                "processors": [{"jar": "x:tool:1", "args": []}]}),
            "1.20.1",
        );
        let marker = env.root.join("m");
        // Nothing to record: a marker cannot be written.
        assert!(write_marker(&plan, &marker, SHA_OF_INSTALLER).is_err());
        assert!(!marker.exists());
        // A hand-made, well-formed marker does not make the state valid.
        write(
            &marker,
            format!(
                "{MARKER_MAGIC}\ninstaller {SHA_OF_INSTALLER}\npatched {}\n",
                sha1_hex(b"x")
            )
            .as_bytes(),
        );
        let version: VersionJson =
            serde_json::from_value(json!({"id": "v", "mainClass": "m"})).unwrap();
        assert!(
            !install_state_valid(&plan, &version, &env.libraries, &marker, SHA_OF_INSTALLER)
                .unwrap()
        );
    }

    #[test]
    fn install_state_malformed_markers_are_invalid_not_errors() {
        let i = install();
        let good = std::fs::read_to_string(&i.marker).unwrap();
        let mid = i.plan.produced[0].to_string_lossy().into_owned();
        let sha = sha1_hex(b"mid");
        let cases: Vec<(&str, Vec<u8>)> = vec![
            ("empty", vec![]),
            ("binary", vec![0xff, 0xfe, 0, 1, 2]),
            (
                "wrong magic",
                good.replacen("marker 1", "marker 2", 1).into_bytes(),
            ),
            ("truncated", good.as_bytes()[..good.len() / 3].to_vec()),
            ("unknown line", format!("{good}surprise\n").into_bytes()),
            (
                "bad hash",
                good.replacen("patched ", "patched zz", 1).into_bytes(),
            ),
            (
                "duplicate output",
                format!("{good}output {sha} {mid}\n").into_bytes(),
            ),
            (
                "output outside the plan",
                format!("{good}output {sha} {}\n", i.env.root.join("evil").display()).into_bytes(),
            ),
            ("oversized", vec![b'a'; (MAX_MARKER_BYTES + 1) as usize]),
        ];
        for (name, bytes) in cases {
            write(&i.marker, &bytes);
            assert!(!i.valid(), "{name}");
        }
        write(&i.marker, good.as_bytes());
        assert!(i.valid());
        // A directory where the marker should be.
        std::fs::remove_file(&i.marker).unwrap();
        std::fs::create_dir(&i.marker).unwrap();
        assert!(!i.valid());
    }

    #[test]
    fn write_marker_refuses_a_broken_install() {
        let i = install();
        write(&i.out, b"corrupt");
        assert!(write_marker(&i.plan, &i.marker, SHA_OF_INSTALLER).is_err());
        write(&i.out, b"out");
        write(&i.patched, b"tampered");
        assert!(write_marker(&i.plan, &i.marker, SHA_OF_INSTALLER).is_err());
        write(&i.patched, b"patched");
        assert!(write_marker(&i.plan, &i.marker, "not-a-hash").is_err());
        write_marker(&i.plan, &i.marker, SHA_OF_INSTALLER).unwrap();
        assert!(i.valid());
    }
}
