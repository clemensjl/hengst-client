//! Planning and running the processors of a modern Forge or NeoForge installer and checking
//! whether an installation is complete. Processes are started only through a `ProcessRunner`.
//!
//! Everything in the install profile is untrusted remote data that ends up on a command line and
//! in file paths, so each string is validated where it becomes a path or an argument.
//!
//! Use (see `forge::finish_install`): hash the installer, `plan`, and skip all processors only if
//! `install_state_valid` is true. Otherwise `run_processors` (which checks the files before and
//! the outputs after every processor), then `write_marker`.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

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
    /// Position of the processor in the profile (server-side ones counted).
    pub index: usize,
    /// Maven coordinate of the processor jar, as the profile names it.
    pub coordinate: String,
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

/// The names of all `{KEY}` tokens in an argument.
fn token_names(arg: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = arg;
    while let Some(start) = rest.find('{') {
        let after = &rest[start + 1..];
        match after.find('}') {
            Some(end) if is_token_name(&after[..end]) => {
                out.push(&after[..end]);
                rest = &after[end + 1..];
            }
            _ => rest = after,
        }
    }
    out
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

/// An argument that looks like a path (contains `/`, `\` or `:`) must be contained and must not
/// carry a list separator (`;` or `,`), which could smuggle a second path past the check. For a
/// dashed `-key=value` argument both parts are checked. Anything else is a plain option or value.
fn check_argument(arg: &str, roots: &[&Path], exact: &[&Path]) -> bool {
    let parts = match arg
        .strip_prefix('-')
        .and_then(|a| a.trim_start_matches('-').split_once('='))
    {
        Some((k, v)) => vec![k, v],
        None => vec![arg],
    };
    parts.iter().all(|p| {
        !p.contains(['/', '\\', ':'])
            || (!p.contains([';', ',']) && contained(p, roots, exact, true))
    })
}

fn plan_one(
    index: usize,
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
        index,
        coordinate: p.jar.clone(),
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
    let client: Vec<(usize, &Processor)> = profile
        .processors
        .iter()
        .enumerate()
        .filter(|(_, p)| is_client(p))
        .collect();
    // Without PATCHED nothing ties a finished install to its result, and PATCHED without client
    // processors cannot be produced. Both are refused before anything is extracted or run.
    match (client.is_empty(), profile.data.contains_key("PATCHED")) {
        (false, false) => return Err(bad("profile has client processors but no PATCHED data")),
        (true, true) => return Err(bad("profile declares PATCHED but has no client processors")),
        _ => {}
    }
    let (data, coords) = resolve_data(profile, env)?;
    let declared = declared_libraries(profile);
    let commands = client
        .iter()
        .map(|&(i, p)| {
            plan_one(i, p, &data, &declared, env)
                .map_err(|e| bad(format!("processor {i}: {}", inner(e))))
        })
        .collect::<Result<Vec<_>>>()?;
    // Data keys the client processors refer to. Coordinates only server processors write (such
    // as MC_UNPACKED) are never produced on the client.
    let used: HashSet<&str> = client
        .iter()
        .flat_map(|(_, p)| &p.args)
        .flat_map(|a| token_names(a))
        .collect();
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
        .filter(|(k, p)| {
            k != "PATCHED"
                && used.contains(k.as_str())
                && !downloaded.contains(p)
                && !declared_outputs.contains(p)
        })
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
/// and of every produced file without a declared hash (each must exist). Declared outputs and
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
        let sha = sha1_file(path).map_err(|_| {
            bad(format!(
                "{} should have been produced by the processors but is missing",
                path.display()
            ))
        })?;
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

/// Starts one prepared processor command and waits for it. Injected so tests can run the loop
/// without a JVM.
pub trait ProcessRunner: Send + Sync {
    /// Runs `cmd` as built by `run_processors` and returns its exit code (-1 if it has none).
    fn run(&self, cmd: &mut Command) -> std::io::Result<i32>;
}

/// Starts the real process.
pub struct SystemRunner;

impl ProcessRunner for SystemRunner {
    fn run(&self, cmd: &mut Command) -> std::io::Result<i32> {
        Ok(cmd.status()?.code().unwrap_or(-1))
    }
}

/// Variables a JVM reads options or a classpath from; never inherited by a processor.
pub const CLEARED_JAVA_ENV: [&str; 4] = [
    "JAVA_TOOL_OPTIONS",
    "_JAVA_OPTIONS",
    "JDK_JAVA_OPTIONS",
    "CLASSPATH",
];

fn processor_name(c: &ProcessorCommand) -> String {
    format!("processor {} ({})", c.index, c.coordinate)
}

/// `java -cp <jar;classpath> <main_class> <args...>` with `cwd` as working directory, no shell,
/// every argument a separate OS argument, the JVM option variables removed and stdin closed.
/// Nothing from the profile comes before the main class.
fn processor_command(java: &Path, c: &ProcessorCommand, cwd: &Path) -> Result<Command> {
    let classpath = std::env::join_paths(std::iter::once(&c.jar).chain(&c.classpath))
        .map_err(|_| bad(format!("{}: invalid classpath", processor_name(c))))?;
    let mut cmd = Command::new(java);
    cmd.arg("-cp")
        .arg(classpath)
        .arg(&c.main_class)
        .args(&c.args)
        .current_dir(cwd)
        .stdin(Stdio::null());
    for key in CLEARED_JAVA_ENV {
        cmd.env_remove(key);
    }
    Ok(cmd)
}

/// Runs the commands one after another. stdout and stderr of every processor go to `log`
/// (truncated first), each preceded by a header line naming the processor and its arguments.
/// Right before each start the jar and classpath are checked against their declared SHA-1, and
/// after a zero exit the declared outputs. Any failure stops the run with an error naming the
/// processor and the log; a non-zero exit is `Error::Processor`.
pub fn run_processors(
    runner: &dyn ProcessRunner,
    java: &Path,
    cmds: &[ProcessorCommand],
    cwd: &Path,
    log: &Path,
) -> Result<()> {
    if let Some(dir) = log.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut out = std::fs::File::create(log)?;
    let see = || log.display().to_string();
    for c in cmds {
        let name = processor_name(c);
        let fail = |e: Error| bad(format!("{name}: {}; see {}", inner(e), see()));
        c.verify_files().map_err(fail)?;
        writeln!(out, "== {name}: {} {}", c.main_class, c.args.join(" "))?;
        out.flush()?;
        let mut cmd = processor_command(java, c, cwd)?;
        cmd.stdout(out.try_clone()?).stderr(out.try_clone()?);
        let code = runner.run(&mut cmd).map_err(|e| {
            bad(format!(
                "{name}: could not start {}: {e}; see {}",
                java.display(),
                see()
            ))
        })?;
        if code != 0 {
            return Err(Error::Processor {
                jar: name,
                code,
                log: log.to_path_buf(),
            });
        }
        c.check_outputs().map_err(fail)?;
    }
    Ok(())
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
            "data": {"PATCHED": data_entry("[n:patched:1]")},
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

    /// A profile with the given processors, two libraries and a `PATCHED` coordinate.
    fn processors_profile(processors: Value) -> Value {
        json!({"spec": 1, "minecraft": "1.20.1", "json": "/version.json",
            "data": {"PATCHED": data_entry("[n:patched:1]")},
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
            p["data"]["X"] = json!({"client": format!("'{literal}'"), "server": "'x'"});
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
    fn path_arguments_with_list_separators_are_refused() {
        for arg in [
            "{LIBRARY_DIR}/a.jar;C:/evil.jar",
            "{LIBRARY_DIR}/a.jar,b.jar",
            "--cp={LIBRARY_DIR}/a.jar;{LIBRARY_DIR}/b.jar",
            "--list={ROOT}/work/a,{ROOT}/work/b",
        ] {
            let m = plan_error(one_processor(json!({"args": [arg]})));
            assert!(
                m.contains("processor 0") && m.contains("argument 0"),
                "{arg}: {m}"
            );
        }
        // Separators in plain values stay allowed.
        let (_t, _e, plan) = build(
            one_processor(json!({"args": ["a;b", "x,y", "--k=a,b"]})),
            "1.20.1",
        );
        assert_eq!(plan.commands[0].args, ["a;b", "x,y", "--k=a,b"]);
    }

    #[test]
    fn key_part_of_dashed_arguments_is_checked_too() {
        for arg in [
            "-C:/Users/u/evil=x",
            "--{ROOT}/versions/x.jar=1",
            "-D\\\\server\\s=1",
        ] {
            let m = plan_error(one_processor(json!({"args": [arg]})));
            assert!(m.contains("argument 0"), "{arg}: {m}");
        }
        let (_t, _e, plan) = build(
            one_processor(json!({"args": ["-{LIBRARY_DIR}/k=v", "-Dx=1"]})),
            "1.20.1",
        );
        assert_eq!(plan.commands[0].args.len(), 2);
    }

    #[test]
    fn client_processors_without_patched_are_refused() {
        let m = plan_error(
            json!({"spec": 1, "minecraft": "1.20.1", "json": "/version.json",
            "libraries": [lib("x:tool:1")],
            "processors": [{"jar": "x:tool:1", "args": []}]}),
        );
        assert!(m.contains("PATCHED"), "{m}");
        // Server-only processors and no PATCHED: nothing to run, nothing to refuse.
        let (_t, _e, plan) = build(
            json!({"spec": 1, "minecraft": "1.20.1", "json": "/version.json",
                "libraries": [lib("x:tool:1")],
                "processors": [{"jar": "x:tool:1", "sides": ["server"], "args": []}]}),
            "1.20.1",
        );
        assert!(plan.commands.is_empty());
    }

    #[test]
    fn patched_without_client_processors_is_refused() {
        let m = plan_error(
            json!({"spec": 1, "minecraft": "1.20.1", "json": "/version.json",
            "data": {"PATCHED": data_entry("[n:patched:1]")}}),
        );
        assert!(m.contains("PATCHED"), "{m}");
    }

    #[test]
    fn only_coordinates_used_by_client_processors_count_as_produced() {
        let (_t, env, plan) = real(PROFILE_1201, "1.20.1");
        let produced: Vec<String> = plan
            .produced
            .iter()
            .map(|p| rel(&env, &path_string(p)))
            .collect();
        // MC_UNPACKED is only written by server processors.
        assert!(
            !produced.iter().any(|p| p.contains("unpacked")),
            "{produced:?}"
        );
        for needle in ["mappings.txt", "mappings-merged.txt", "srg.jar"] {
            assert!(
                produced.iter().any(|p| p.ends_with(needle)),
                "{needle}: {produced:?}"
            );
        }
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
        let mut profile = one_processor(json!({"sides": ["server"], "args": ["{NOPE}"]}));
        profile.as_object_mut().unwrap().remove("data");
        let (_t, _e, plan) = build(profile, "1.20.1");
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

    // --- running ---------------------------------------------------------------------------

    #[derive(Debug)]
    struct Call {
        program: PathBuf,
        args: Vec<String>,
        cwd: Option<PathBuf>,
        envs: Vec<(String, Option<String>)>,
    }

    /// Records every command instead of starting it; exits with `codes[i]` (default 0).
    #[derive(Default)]
    struct Recorder {
        calls: std::sync::Mutex<Vec<Call>>,
        codes: Vec<i32>,
        /// Called with the index of each run, as if the process did something.
        effect: Option<Box<dyn Fn(usize) + Send + Sync>>,
    }

    impl ProcessRunner for Recorder {
        fn run(&self, cmd: &mut Command) -> std::io::Result<i32> {
            let mut calls = self.calls.lock().unwrap();
            let i = calls.len();
            calls.push(Call {
                program: PathBuf::from(cmd.get_program()),
                args: cmd
                    .get_args()
                    .map(|a| a.to_string_lossy().into_owned())
                    .collect(),
                cwd: cmd.get_current_dir().map(Path::to_path_buf),
                envs: cmd
                    .get_envs()
                    .map(|(k, v)| {
                        (
                            k.to_string_lossy().into_owned(),
                            v.map(|v| v.to_string_lossy().into_owned()),
                        )
                    })
                    .collect(),
            });
            if let Some(effect) = &self.effect {
                effect(i);
            }
            Ok(self.codes.get(i).copied().unwrap_or(0))
        }
    }

    /// Two client processors: the first with a declared output that is already in place.
    fn two_step() -> (tempfile::TempDir, InstallEnv, Plan) {
        let sha = sha1_hex(b"out");
        let (tmp, env, plan) = build(
            processors_profile(json!([
                {"jar": "x:tool:1", "classpath": ["x:dep:2"], "args": ["--first", "{LIBRARY_DIR}/o.bin"],
                    "outputs": {"{LIBRARY_DIR}/o.bin": sha}},
                {"jar": "x:dep:2", "args": ["--second", "two words"]}
            ])),
            "1.20.1",
        );
        write(&env.libraries.join("o.bin"), b"out");
        std::fs::create_dir_all(&env.work).unwrap();
        (tmp, env, plan)
    }

    #[test]
    fn runs_processors_in_order_without_a_shell_and_with_a_clean_environment() {
        let (_t, env, plan) = two_step();
        let java = env.root.join("java/bin/java.exe");
        let log = env.root.join("logs/install.log");
        let runner = Recorder::default();
        run_processors(&runner, &java, &plan.commands, &env.work, &log).unwrap();
        let calls = runner.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        let cp = format!(
            "{};{}",
            plan.commands[0].jar.display(),
            plan.commands[0].classpath[0].display()
        );
        let first = &calls[0];
        assert_eq!(first.program, java);
        assert_eq!(first.cwd.as_deref(), Some(env.work.as_path()));
        assert_eq!(first.args[..3], ["-cp".to_string(), cp, "test.Main".into()]);
        assert_eq!(first.args[3], "--first");
        let second = &calls[1];
        assert_eq!(second.args[1], plan.commands[1].jar.display().to_string());
        assert_eq!(second.args[2..], ["test.Main", "--second", "two words"]);
        for call in calls.iter() {
            let mut removed: Vec<String> = call
                .envs
                .iter()
                .filter(|(_, v)| v.is_none())
                .map(|(k, _)| k.to_ascii_uppercase())
                .collect();
            removed.sort();
            assert_eq!(
                removed,
                [
                    "CLASSPATH",
                    "JAVA_TOOL_OPTIONS",
                    "JDK_JAVA_OPTIONS",
                    "_JAVA_OPTIONS"
                ]
            );
            // Nothing is added to the inherited environment.
            assert!(call.envs.iter().all(|(_, v)| v.is_none()), "{call:?}");
        }
        let text = std::fs::read_to_string(&log).unwrap();
        assert!(text.contains("processor 0 (x:tool:1)"), "{text}");
        assert!(text.contains("processor 1 (x:dep:2)"), "{text}");
    }

    #[test]
    fn failing_processor_stops_the_run_and_names_index_and_log() {
        let (_t, env, plan) = two_step();
        let log = env.root.join("logs/install.log");
        let runner = Recorder {
            codes: vec![3],
            ..Recorder::default()
        };
        let err = run_processors(&runner, Path::new("java"), &plan.commands, &env.work, &log)
            .unwrap_err();
        assert!(
            matches!(&err, Error::Processor { jar, code: 3, log: l } if jar == "processor 0 (x:tool:1)" && *l == log),
            "{err}"
        );
        assert_eq!(runner.calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn tampered_processor_jar_is_not_started() {
        let (_t, env, plan) = two_step();
        // The first processor replaces the jar the second one is about to run.
        let second = plan.commands[1].jar.clone();
        let log = env.root.join("logs/install.log");
        let runner = Recorder {
            effect: Some(Box::new(move |_| {
                std::fs::write(&second, b"tampered").unwrap()
            })),
            ..Recorder::default()
        };
        let err = run_processors(&runner, Path::new("java"), &plan.commands, &env.work, &log)
            .unwrap_err();
        let m = err.to_string();
        assert!(
            m.contains("processor 1") && m.contains("SHA-1") && m.contains("install.log"),
            "{m}"
        );
        // The first ran, the second was refused before it started.
        assert_eq!(runner.calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn wrong_output_after_a_processor_is_an_error_naming_it() {
        let (_t, env, plan) = two_step();
        write(&env.libraries.join("o.bin"), b"not what was declared");
        let log = env.root.join("logs/install.log");
        let runner = Recorder::default();
        let err = run_processors(&runner, Path::new("java"), &plan.commands, &env.work, &log)
            .unwrap_err();
        let m = err.to_string();
        assert!(
            m.contains("processor 0") && m.contains("o.bin") && m.contains("install.log"),
            "{m}"
        );
        assert_eq!(runner.calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn real_failing_child_process_is_reported_with_its_output_in_the_log() {
        let (_t, env, plan) = two_step();
        let log = env.root.join("logs/install.log");
        // where.exe rejects `-cp` and exits with a non-zero code; it stands in for a JVM.
        let helper = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32")
            .join("where.exe");
        let err =
            run_processors(&SystemRunner, &helper, &plan.commands, &env.work, &log).unwrap_err();
        assert!(
            matches!(&err, Error::Processor { jar, code, .. } if jar.starts_with("processor 0") && *code != 0),
            "{err}"
        );
        // The helper's own error message (console code page, localized) follows the header.
        let text = String::from_utf8_lossy(&std::fs::read(&log).unwrap()).into_owned();
        let mut lines = text.lines();
        assert!(
            lines
                .next()
                .unwrap()
                .starts_with("== processor 0 (x:tool:1)"),
            "{text}"
        );
        assert!(lines.next().is_some_and(|l| !l.trim().is_empty()), "{text}");
    }

    #[test]
    fn missing_executable_is_an_error_naming_the_processor() {
        let (_t, env, plan) = two_step();
        let log = env.root.join("logs/install.log");
        let err = run_processors(
            &SystemRunner,
            &env.root.join("no-such-java.exe"),
            &plan.commands,
            &env.work,
            &log,
        )
        .unwrap_err();
        let m = err.to_string();
        assert!(
            m.contains("processor 0") && m.contains("install.log"),
            "{m}"
        );
    }

    /// A real JVM with a main class that does not exist exits with 1. Needs a Java runtime: set
    /// `HENGST_TEST_JAVA` to a java.exe, or have the managed runtime of a live run under
    /// `%TEMP%\hengst-live`.
    #[test]
    #[ignore]
    fn real_java_with_missing_main_class_fails() {
        let java = std::env::var_os("HENGST_TEST_JAVA")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::temp_dir().join("hengst-live/java/java-runtime-delta/bin/java.exe")
            });
        assert!(java.is_file(), "no Java at {}", java.display());
        let (_t, env, mut plan) = two_step();
        plan.commands[0].main_class = "does.not.Exist".into();
        let log = env.root.join("logs/install.log");
        let err =
            run_processors(&SystemRunner, &java, &plan.commands, &env.work, &log).unwrap_err();
        assert!(matches!(&err, Error::Processor { code: 1, .. }), "{err}");
        let text = std::fs::read_to_string(&log).unwrap();
        assert!(text.contains("does.not.Exist"), "{text}");
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
    fn install_state_patched_jar_absent_after_run() {
        let (_t, env, plan) = build(
            json!({"spec": 1, "minecraft": "1.20.1", "json": "/version.json",
                "data": {"PATCHED": data_entry("[n:patched:1]")},
                "libraries": [lib("x:tool:1")],
                "processors": [{"jar": "x:tool:1", "args": ["{PATCHED}"]}]}),
            "1.20.1",
        );
        let marker = env.root.join("m");
        // The patched jar was never produced: a marker cannot be written.
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
    fn write_marker_refuses_a_missing_produced_file() {
        let i = install();
        std::fs::remove_file(&i.mid).unwrap();
        std::fs::remove_file(&i.marker).unwrap();
        let err = write_marker(&i.plan, &i.marker, SHA_OF_INSTALLER).unwrap_err();
        assert!(err.to_string().contains("missing"), "{err}");
        assert!(!i.marker.exists());
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
