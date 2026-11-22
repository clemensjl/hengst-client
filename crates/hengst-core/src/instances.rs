//! Instances: named game installations stored as `<root>/instances/<id>/instance.json`.
//!
//! The instance folder is also the game directory (the CLI uses the same folders). The `id` is the
//! folder name: the core builds it from the game version and loader (`default_id`), never from the
//! display name, and every id a caller passes in is checked against the id grammar and against the
//! folder that exists on disk (`instance_dir`) before it becomes a path. The functions are
//! synchronous and may wait for a per-instance lock file; async callers run them on a blocking
//! thread.

use crate::args::Account;
use crate::launch::LaunchOptions;
use crate::loaders::forge::{lock_file, LockTiming};
use crate::loaders::{LoaderKind, LoaderRequest, ResolvedLoader};
use crate::paths::{is_windows_reserved, remove_stale_temps, unique_temp, validate_id, Paths};
use crate::{Error, Result};
use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const INSTANCE_FILE: &str = "instance.json";

/// The format of `instance.json` this build reads and writes.
///
/// Migration rule: a file is read through `serde_json::Value`. A missing `format` makes the
/// instance `Broken`; a `format` above this constant makes it `TooNew` and the file is never
/// written (`save` refuses, the folder is never deleted). Every future increase brings a function
/// `v<n>_to_v<n+1>(Value) -> Value`, one fixture file per old format and a test; a migrated file is
/// only written back by the next `save`, in the new format. Unknown fields survive in `extra` and
/// in `save`, which merges onto the map that is on disk.
pub const INSTANCE_FORMAT: u32 = 1;

/// A metadata file is a few hundred bytes; anything above this is not read.
const MAX_FILE_BYTES: u64 = 1024 * 1024;
const MAX_NAME_CHARS: usize = 64;
const MAX_SUFFIX: u32 = 999;
const LOCK_TIMING: LockTiming = LockTiming {
    notice_after: Duration::from_secs(5),
    limit: Duration::from_secs(20),
};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InstanceLoader {
    pub kind: LoaderKind,
    /// `None` until the first start resolves it (folders adopted from the CLI).
    pub version: Option<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceConfig {
    pub format: u32,
    /// Display name only; never part of a path.
    pub name: String,
    pub game: String,
    pub loader: Option<InstanceLoader>,
    #[serde(default)]
    pub memory_mb: Option<u32>,
    #[serde(default)]
    pub java: Option<PathBuf>,
    pub created_at: u64,
    #[serde(default)]
    pub last_played: Option<u64>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Instance {
    pub id: String,
    pub dir: PathBuf,
    pub config: InstanceConfig,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Listed {
    Ready(Instance),
    /// Saved by a newer build; shown, never written or deleted by this one.
    TooNew {
        id: String,
        format: u32,
    },
    /// The file is missing data or damaged; the folder stays untouched.
    Broken {
        id: String,
        reason: String,
    },
}

#[derive(Clone, Debug)]
pub struct NewInstance {
    pub name: String,
    pub game: String,
    pub loader: Option<InstanceLoader>,
}

/// The folder name `prepare` uses without `--instance`: `<game>` or `<game>-<loader slug>`.
pub fn default_id(game: &str, loader: Option<LoaderKind>) -> String {
    match loader {
        Some(kind) => format!("{game}-{}", kind.slug()),
        None => game.to_string(),
    }
}

/// Reads a folder name as a default id: the whole name as a game version, else the longest loader
/// suffix whose rest is a game version. `is_game` knows the versions (the manifest).
pub fn parse_default_id(
    id: &str,
    is_game: impl Fn(&str) -> bool,
) -> Option<(String, Option<LoaderKind>)> {
    if is_game(id) {
        return Some((id.to_string(), None));
    }
    // `-legacy-fabric` before `-fabric` and `-neoforge` before `-forge`: the longer suffix wins.
    for kind in [
        LoaderKind::LegacyFabric,
        LoaderKind::NeoForge,
        LoaderKind::Fabric,
        LoaderKind::Quilt,
        LoaderKind::Forge,
    ] {
        let suffix = format!("-{}", kind.slug());
        if let Some(game) = id.strip_suffix(&suffix) {
            if is_game(game) {
                return Some((game.to_string(), Some(kind)));
            }
        }
    }
    None
}

/// A display name: trimmed, 1 to 64 characters, no control characters.
pub fn validate_name(name: &str) -> Result<String> {
    let name = name.trim();
    let bad = |why: &str| Err(Error::InvalidInstanceName(why.to_string()));
    if name.is_empty() {
        return bad("the name is empty");
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return bad("the name is longer than 64 characters");
    }
    if name.chars().any(char::is_control) {
        return bad("the name contains control characters");
    }
    Ok(name.to_string())
}

/// The id grammar of instance folders: `validate_id` plus what Windows would reinterpret (a
/// trailing dot, a device name such as `CON`).
fn check_id(id: &str) -> Result<()> {
    validate_id(id)?;
    if id.ends_with('.') || is_windows_reserved(id) {
        return Err(Error::UnsafePath(id.to_string()));
    }
    Ok(())
}

fn instances_root(paths: &Paths) -> PathBuf {
    paths.root().join("instances")
}

/// Symlinks, junctions and every other reparse point: never followed.
fn is_link(meta: &std::fs::Metadata) -> bool {
    let reparse = {
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
            meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        }
        #[cfg(not(windows))]
        {
            false
        }
    };
    meta.file_type().is_symlink() || reparse
}

/// True for a real directory (not a link) at `path`.
fn is_plain_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.is_dir() && !is_link(&m))
}

/// True for a real file (not a link) at `path`.
fn is_plain_file(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.is_file())
}

/// The folder of an existing instance. The only way an id from a caller becomes a path: the id
/// must match the id grammar, name exactly (in case) a folder inside `instances/` that is not a
/// link, and that folder must hold an `instance.json` that is a file. A folder without one (made by
/// the CLI) is not an instance here.
pub fn instance_dir(paths: &Paths, id: &str) -> Result<PathBuf> {
    check_id(id)?;
    let not_found = || Error::InstanceNotFound { id: id.to_string() };
    let dir = paths.instance(id);
    let meta = match std::fs::symlink_metadata(&dir) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(not_found()),
        Err(e) => return Err(e.into()),
    };
    if is_link(&meta) {
        return Err(Error::UnsafePath(id.to_string()));
    }
    if !meta.is_dir() {
        return Err(not_found());
    }
    // File systems that ignore case would let `Foo` open `foo`: the folder must carry the name.
    let real = std::fs::canonicalize(&dir)?;
    if real.file_name().and_then(|n| n.to_str()) != Some(id) {
        return Err(not_found());
    }
    if !is_plain_file(&dir.join(INSTANCE_FILE)) {
        return Err(not_found());
    }
    Ok(dir)
}

enum Parsed {
    Ready(serde_json::Map<String, serde_json::Value>),
    TooNew(u32),
    Broken(String),
}

enum Raw {
    Bytes(Vec<u8>),
    TooLarge,
    Io(std::io::Error),
}

fn read_capped(path: &Path) -> Raw {
    let read = || -> std::io::Result<Raw> {
        let file = std::fs::File::open(path)?;
        if file.metadata()?.len() > MAX_FILE_BYTES {
            return Ok(Raw::TooLarge);
        }
        let mut bytes = Vec::new();
        file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes)?;
        Ok(if bytes.len() as u64 > MAX_FILE_BYTES {
            Raw::TooLarge
        } else {
            Raw::Bytes(bytes)
        })
    };
    read().unwrap_or_else(Raw::Io)
}

/// Reads `instance.json`. The reasons are fixed texts: serde's own messages quote file content.
fn read_parsed(path: &Path) -> Parsed {
    let bytes = match read_capped(path) {
        Raw::Bytes(b) => b,
        Raw::TooLarge => {
            return Parsed::Broken(format!(
                "{INSTANCE_FILE} is larger than {MAX_FILE_BYTES} bytes"
            ))
        }
        Raw::Io(e) => {
            return Parsed::Broken(format!("{INSTANCE_FILE} cannot be read ({:?})", e.kind()))
        }
    };
    parse(&bytes)
}

fn parse(bytes: &[u8]) -> Parsed {
    let value = match serde_json::from_slice::<serde_json::Value>(bytes) {
        Ok(v) => v,
        Err(e) => {
            return Parsed::Broken(format!(
                "{INSTANCE_FILE} is not valid JSON (line {} column {})",
                e.line(),
                e.column()
            ))
        }
    };
    let serde_json::Value::Object(map) = value else {
        return Parsed::Broken(format!("{INSTANCE_FILE} is not a JSON object"));
    };
    let Some(format) = map.get("format").and_then(serde_json::Value::as_u64) else {
        return Parsed::Broken(format!("{INSTANCE_FILE} has no format number"));
    };
    if format > u64::from(INSTANCE_FORMAT) {
        return Parsed::TooNew(u32::try_from(format).unwrap_or(u32::MAX));
    }
    if format < u64::from(INSTANCE_FORMAT) {
        return Parsed::Broken(format!("{INSTANCE_FILE} has an unsupported format"));
    }
    Parsed::Ready(map)
}

enum State {
    Ready(InstanceConfig),
    TooNew(u32),
    Broken(String),
}

fn read_state(dir: &Path) -> State {
    match read_parsed(&dir.join(INSTANCE_FILE)) {
        Parsed::TooNew(f) => State::TooNew(f),
        Parsed::Broken(r) => State::Broken(r),
        Parsed::Ready(map) => match serde_json::from_value(serde_json::Value::Object(map)) {
            Ok(config) => State::Ready(config),
            Err(_) => State::Broken(format!("{INSTANCE_FILE} has missing or unexpected data")),
        },
    }
}

fn listed(paths: &Paths, id: String) -> Listed {
    let dir = paths.instance(&id);
    match read_state(&dir) {
        State::Ready(config) => Listed::Ready(Instance { id, dir, config }),
        State::TooNew(format) => Listed::TooNew { id, format },
        State::Broken(reason) => Listed::Broken { id, reason },
    }
}

/// The folder names in `instances/` that are plain directories matching the id grammar. Entries
/// that cannot be read, names that are not UTF-8, files, links and odd names are left out.
fn folder_ids(paths: &Paths) -> Result<Vec<String>> {
    let rd = match std::fs::read_dir(instances_root(paths)) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut ids = Vec::new();
    let mut errors = 0;
    for entry in rd {
        let Ok(entry) = entry else {
            // A failing iterator can fail on every call: do not spin on it.
            errors += 1;
            if errors > 16 {
                break;
            }
            continue;
        };
        let Ok(id) = entry.file_name().into_string() else {
            continue;
        };
        if check_id(&id).is_ok() && is_plain_dir(&entry.path()) {
            ids.push(id);
        }
    }
    Ok(ids)
}

/// Every folder in `instances/` that holds an `instance.json`: ready, too new or unreadable.
/// Sorted by `last_played` (newest first, never played last), then by name without regard to case.
pub fn list(paths: &Paths) -> Result<Vec<Listed>> {
    let mut out: Vec<Listed> = folder_ids(paths)?
        .into_iter()
        .filter(|id| is_plain_file(&paths.instance(id).join(INSTANCE_FILE)))
        .map(|id| listed(paths, id))
        .collect();
    let key = |l: &Listed| match l {
        Listed::Ready(i) => (
            i.config.last_played,
            i.config.name.to_lowercase(),
            i.id.clone(),
        ),
        Listed::TooNew { id, .. } | Listed::Broken { id, .. } => {
            (None, id.to_lowercase(), id.clone())
        }
    };
    out.sort_by(|a, b| {
        let (a, b) = (key(a), key(b));
        b.0.cmp(&a.0)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.cmp(&b.2))
    });
    Ok(out)
}

/// The instance `id`, or why it cannot be used: not there, a link, too new, unreadable.
pub fn load(paths: &Paths, id: &str) -> Result<Instance> {
    let dir = instance_dir(paths, id)?;
    match read_state(&dir) {
        State::Ready(config) => Ok(Instance {
            id: id.to_string(),
            dir,
            config,
        }),
        State::TooNew(format) => Err(Error::InstanceFormatTooNew {
            id: id.to_string(),
            format,
        }),
        State::Broken(reason) => Err(Error::InstanceUnreadable {
            id: id.to_string(),
            reason,
        }),
    }
}

/// An exclusive lock for one instance, held until the file is dropped. The lock file lives in
/// `<root>/tmp`, not in the instance folder, so a held lock never keeps the folder from being
/// moved away.
fn lock_instance(paths: &Paths, id: &str, timing: LockTiming) -> Result<std::fs::File> {
    check_id(id)?;
    let path = paths.tmp().join(format!("instance-{id}.lock"));
    lock_file(&path, &format!("instance {id}"), timing, &|_| {})
}

/// Temp file, flush, rename: a reader sees the old or the new file. The folder must exist; this
/// never creates it.
fn write_file(dir: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let file = dir.join(INSTANCE_FILE);
    let tmp = unique_temp(&file);
    let result = (|| {
        let mut f = std::fs::File::create_new(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, &file)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

fn config_bytes(
    config: &InstanceConfig,
    base: serde_json::Map<String, serde_json::Value>,
) -> Result<Vec<u8>> {
    let mut map = base;
    if let serde_json::Value::Object(known) = serde_json::to_value(config)? {
        map.extend(known);
    }
    Ok(serde_json::to_vec_pretty(&serde_json::Value::Object(map))?)
}

/// Makes a new instance: a new folder named by `default_id`, with `-2`, `-3`, ... up to `-999` when
/// that name (in any case) is taken. An existing folder is never used. `game` must be a concrete
/// version id; whether it exists is up to the caller.
pub fn create(paths: &Paths, new: &NewInstance, now: u64) -> Result<Instance> {
    let name = validate_name(&new.name)?;
    if matches!(
        new.game.as_str(),
        "latest" | "latest-release" | "latest-snapshot"
    ) {
        return Err(Error::UnknownVersion(new.game.clone()));
    }
    check_id(&new.game)?;
    if let Some(version) = new.loader.as_ref().and_then(|l| l.version.as_deref()) {
        validate_id(version)?;
    }
    let base = default_id(&new.game, new.loader.as_ref().map(|l| l.kind));
    check_id(&base)?;
    let root = instances_root(paths);
    std::fs::create_dir_all(&root)?;
    let taken: HashSet<String> = std::fs::read_dir(&root)?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_ascii_lowercase())
        .collect();
    let config = InstanceConfig {
        format: INSTANCE_FORMAT,
        name,
        game: new.game.clone(),
        loader: new.loader.clone(),
        memory_mb: None,
        java: None,
        created_at: now,
        last_played: None,
        extra: Default::default(),
    };
    for n in 1..=MAX_SUFFIX {
        let id = if n == 1 {
            base.clone()
        } else {
            format!("{base}-{n}")
        };
        if check_id(&id).is_err() {
            break;
        }
        if taken.contains(&id.to_ascii_lowercase()) {
            continue;
        }
        let _lock = lock_instance(paths, &id, LOCK_TIMING)?;
        let dir = paths.instance(&id);
        match std::fs::create_dir(&dir) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
        let written = config_bytes(&config, Default::default())
            .and_then(|bytes| write_file(&dir, &bytes).map_err(Error::from));
        if let Err(e) = written {
            // Only our own, still empty folder: `remove_dir` refuses anything else.
            let _ = std::fs::remove_dir(&dir);
            return Err(e);
        }
        return Ok(Instance { id, dir, config });
    }
    Err(Error::InstanceExists { id: base })
}

/// Writes the instance back: known fields from `inst`, everything else as it is on disk. Refuses a
/// file from a newer format (`InstanceFormatTooNew`) and a damaged one (`InstanceUnreadable`), and
/// never creates the folder. Only `inst.id` and `inst.config` are used; `inst.dir` is ignored.
pub fn save(paths: &Paths, inst: &Instance) -> Result<()> {
    let _lock = lock_instance(paths, &inst.id, LOCK_TIMING)?;
    save_locked(paths, inst)
}

fn save_locked(paths: &Paths, inst: &Instance) -> Result<()> {
    let dir = instance_dir(paths, &inst.id)?;
    let mut config = inst.config.clone();
    config.name = validate_name(&config.name)?;
    config.format = INSTANCE_FORMAT;
    let on_disk = match read_parsed(&dir.join(INSTANCE_FILE)) {
        Parsed::Ready(map) => map,
        Parsed::TooNew(format) => {
            return Err(Error::InstanceFormatTooNew {
                id: inst.id.clone(),
                format,
            })
        }
        Parsed::Broken(reason) => {
            return Err(Error::InstanceUnreadable {
                id: inst.id.clone(),
                reason,
            })
        }
    };
    let file = dir.join(INSTANCE_FILE);
    remove_stale_temps(&file);
    write_file(&dir, &config_bytes(&config, on_disk)?)?;
    Ok(())
}

/// Gives every folder in `instances/` without an `instance.json` one, when its name reads as a
/// default id of a game version `is_game` knows (`parse_default_id`): name `"<game> <Loader>"` or
/// `"<game>"`, loader version `None`. Other folders stay untouched and unseen. Best effort per
/// folder: one that cannot be written is skipped. Returns the adopted ids.
pub fn adopt(paths: &Paths, is_game: impl Fn(&str) -> bool, now: u64) -> Result<Vec<String>> {
    let mut adopted = Vec::new();
    for id in folder_ids(paths)? {
        let dir = paths.instance(&id);
        if std::fs::symlink_metadata(dir.join(INSTANCE_FILE)).is_ok() {
            continue;
        }
        let Some((game, kind)) = parse_default_id(&id, &is_game) else {
            continue;
        };
        let Ok(_lock) = lock_instance(paths, &id, LOCK_TIMING) else {
            continue;
        };
        // A create or another adopt may have finished while this one waited for the lock.
        if std::fs::symlink_metadata(dir.join(INSTANCE_FILE)).is_ok() || !is_plain_dir(&dir) {
            continue;
        }
        let wanted = match kind {
            Some(k) => format!("{game} {}", k.display_name()),
            None => game.clone(),
        };
        let name =
            validate_name(&wanted).unwrap_or_else(|_| id.chars().take(MAX_NAME_CHARS).collect());
        let config = InstanceConfig {
            format: INSTANCE_FORMAT,
            name,
            game,
            loader: kind.map(|kind| InstanceLoader {
                kind,
                version: None,
            }),
            memory_mb: None,
            java: None,
            created_at: now,
            last_played: None,
            extra: Default::default(),
        };
        let written = config_bytes(&config, Default::default())
            .ok()
            .is_some_and(|bytes| write_file(&dir, &bytes).is_ok());
        if written {
            adopted.push(id);
        }
    }
    Ok(adopted)
}

/// Moves the instance folder out with `trash` (the Recycle Bin in the app). Only the folder of an
/// instance that `instance_dir` accepts: unknown ids, links and folders without `instance.json` are
/// refused, and so is an instance from a newer format. If `trash` fails, or leaves the folder in
/// place, the error is returned and nothing else happens: there is no other way to remove it. The
/// shared `versions`, `libraries` and `assets` are never touched. Whether the instance is running is
/// for the caller to know.
pub fn delete(paths: &Paths, id: &str, trash: &dyn Fn(&Path) -> Result<()>) -> Result<()> {
    let _lock = lock_instance(paths, id, LOCK_TIMING)?;
    let dir = instance_dir(paths, id)?;
    if let State::TooNew(format) = read_state(&dir) {
        return Err(Error::InstanceFormatTooNew {
            id: id.to_string(),
            format,
        });
    }
    trash(&std::path::absolute(&dir)?)?;
    if std::fs::symlink_metadata(&dir).is_ok() {
        return Err(Error::Io(std::io::Error::other(format!(
            "the folder of instance {id} is still there after it was moved away"
        ))));
    }
    Ok(())
}

pub fn launch_options(
    inst: &Instance,
    account: Account,
    default_memory_mb: u32,
    default_java: Option<PathBuf>,
) -> LaunchOptions {
    let c = &inst.config;
    LaunchOptions {
        version: c.game.clone(),
        instance: Some(inst.id.clone()),
        account,
        java: c.java.clone().or(default_java),
        memory_mb: c.memory_mb.unwrap_or(default_memory_mb),
        loader: c.loader.as_ref().map(|l| LoaderRequest {
            kind: l.kind,
            version: l.version.clone(),
        }),
    }
}

/// Sets `last_played`. An instance whose loader version is still open takes the one the launch
/// resolved, once; a pinned version is never changed.
pub fn record_launch(
    paths: &Paths,
    id: &str,
    now: u64,
    resolved: Option<&ResolvedLoader>,
) -> Result<()> {
    let _lock = lock_instance(paths, id, LOCK_TIMING)?;
    let mut inst = load(paths, id)?;
    inst.config.last_played = Some(now);
    if let (Some(loader), Some(resolved)) = (inst.config.loader.as_mut(), resolved) {
        if loader.version.is_none() && loader.kind == resolved.kind {
            loader.version = Some(resolved.version.id.clone());
        }
    }
    save_locked(paths, &inst)
}

#[cfg(test)]
mod tests;
