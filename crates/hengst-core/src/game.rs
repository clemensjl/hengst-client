//! The running game: started in its own job object without a console window, its stdout and
//! stderr read line by line, redacted, parsed into `LogLine`s for a sink and written to a launcher
//! log file, stoppable gracefully (close request to its windows) or by force (job terminated).
//!
//! Secrets: the only secret on the game's command line is the Minecraft access token of a
//! Microsoft account (`--accessToken`, and inside `--session token:<token>:<uuid>` of old
//! versions). `Redactor` replaces it as written, JSON-escaped and URL-encoded, before a line leaves
//! the reader thread. Not secret: the player name and UUID (public profile data the game itself
//! logs), the XUID (an account identifier like the UUID that grants nothing; it is stored in plain
//! text in `accounts.json` already), `clientid` (always `0`) and the offline account's token `0`.

use crate::args::{Account, AccountKind};
use crate::gamelog::{Entry, LogLine, LogTime, Parser, Stream};
use crate::launch::Prepared;
use crate::secret::REDACTED;
use memchr::memmem::Finder;
use std::fmt;
use std::sync::Arc;

#[cfg(windows)]
pub use self::windows::{spawn_game, GameProcess};

/// Receives every log line of a game run, in `seq` order, from the reader threads. It is called
/// while the run's output lock is held: it must return quickly (hand the line to a channel) and
/// must not wait for other lines. A panic in it is caught and the line dropped; reading goes on.
pub type LineSink = Arc<dyn Fn(LogLine) + Send + Sync>;

/// Raw output lines longer than this are cut (after redaction) and end with `CUT_MARKER`.
pub const LINE_LIMIT: usize = 8 * 1024;
/// Appended to a cut line.
pub const CUT_MARKER: &str = " [line cut: longer than 8 KiB]";
/// Launcher log files kept per instance (the newest).
pub const KEEP_GAME_LOGS: usize = 10;

/// Secret values shorter than this are not redacted: no real access token is that short (they
/// are JWTs of several hundred characters), and replacing, say, `0` everywhere would mangle the
/// whole log. An empty value would match between every two characters.
const MIN_SECRET_LEN: usize = 6;
/// Shortest piece of a secret that counts as a fragment when a secret is split over lines.
const MIN_FRAGMENT: usize = 8;

/// Replaces secrets in the game's output. Built only inside the core, from the `Prepared` launch
/// (its account); the web view and the sink only ever see its output.
///
/// For each secret it knows the forms the value can take in a log: as written, JSON-escaped (also
/// with `\/` and in Gson's HTML-safe form) and percent-encoded (upper- and lowercase hex). Beyond
/// whole occurrences it removes the pieces of a secret that a line break split apart: a line
/// ending in a prefix of a secret, a line starting with a suffix of one, a line that is entirely
/// an inner piece of one (each at least 8 bytes, ignoring surrounding whitespace). At most 7 bytes
/// of a secret can survive on either side of such a break.
#[derive(Clone)]
pub struct Redactor {
    /// Every form of every secret, longest first, each at least `MIN_SECRET_LEN` bytes, as a
    /// searcher built once.
    patterns: Vec<Finder<'static>>,
}

impl fmt::Debug for Redactor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Redactor")
            .field("patterns", &self.patterns.len())
            .finish()
    }
}

impl Redactor {
    /// The redactor for the account the game was prepared for.
    pub fn for_prepared(p: &Prepared) -> Redactor {
        Self::for_account(&p.account)
    }

    pub(crate) fn for_account(a: &Account) -> Redactor {
        match a.kind() {
            AccountKind::Microsoft => Self::new(&[a.access_token().expose()]),
            // The offline token is the constant `0`: nothing to hide.
            AccountKind::Offline => Self::new(&[]),
        }
    }

    pub(crate) fn new(secrets: &[&str]) -> Redactor {
        let mut forms: Vec<Vec<u8>> = Vec::new();
        for s in secrets.iter().filter(|s| s.len() >= MIN_SECRET_LEN) {
            for form in encodings(s) {
                if !forms.contains(&form) {
                    forms.push(form);
                }
            }
        }
        forms.sort_by_key(|p| std::cmp::Reverse(p.len()));
        let patterns = forms.iter().map(|f| Finder::new(f).into_owned()).collect();
        Redactor { patterns }
    }

    /// `text` with every secret replaced by `<redacted>`, line by line.
    pub fn redact(&self, text: &str) -> String {
        let mut out = Vec::with_capacity(text.len());
        for (i, line) in text.as_bytes().split(|&b| b == b'\n').enumerate() {
            if i > 0 {
                out.push(b'\n');
            }
            out.extend(splice(line, &self.spans(line, true), line.len()));
        }
        String::from_utf8_lossy(&out).into_owned()
    }

    fn longest(&self) -> usize {
        self.patterns.first().map_or(0, |f| f.needle().len())
    }

    /// How much of one line the reader keeps: enough that every secret starting before the cut at
    /// `LINE_LIMIT` is complete, so it is found and replaced as a whole.
    fn line_capacity(&self) -> usize {
        LINE_LIMIT + self.longest()
    }

    /// One raw output line (without its line break), redacted and cut. `complete` is false when
    /// the reader stopped keeping the line at `line_capacity` (its real end was dropped).
    fn redact_line(&self, raw: &[u8], complete: bool) -> Vec<u8> {
        // The end of an incomplete line is not its real end: no fragment check there.
        let spans = self.spans(raw, complete);
        let cut = !complete || raw.len() > LINE_LIMIT;
        let mut out = splice(raw, &spans, if cut { LINE_LIMIT } else { raw.len() });
        if cut {
            out.extend_from_slice(CUT_MARKER.as_bytes());
        }
        out
    }

    /// The byte ranges of `line` to replace, sorted and not overlapping.
    fn spans(&self, line: &[u8], check_end: bool) -> Vec<(usize, usize)> {
        let mut spans = Vec::new();
        let start = line
            .iter()
            .position(|b| !b.is_ascii_whitespace())
            .unwrap_or(line.len());
        let end = line
            .iter()
            .rposition(|b| !b.is_ascii_whitespace())
            .map_or(start, |i| i + 1);
        let seg = &line[start..end];
        for finder in &self.patterns {
            let p = finder.needle();
            let mut from = 0;
            while let Some(i) = finder.find(&line[from..]) {
                spans.push((from + i, from + i + p.len()));
                from += i + 1;
            }
            if p.len() <= MIN_FRAGMENT || seg.len() < MIN_FRAGMENT {
                continue;
            }
            // The whole line is an inner piece of the secret.
            if seg.len() < p.len() && find(p, seg).is_some() {
                spans.push((start, end));
                continue;
            }
            // The line starts with the rest of a secret begun on the previous line.
            let key = &seg[..MIN_FRAGMENT];
            let mut j = 1;
            while j + MIN_FRAGMENT <= p.len() {
                let Some(k) = find(&p[j..], key) else { break };
                let at = j + k;
                if seg.starts_with(&p[at..]) {
                    spans.push((start, start + p.len() - at));
                    break;
                }
                j = at + 1;
            }
            // The line ends with the beginning of a secret continued on the next line.
            if check_end {
                let lo = seg.len().saturating_sub(p.len() - 1);
                if let Some(s) = (lo..=seg.len() - MIN_FRAGMENT).find(|&s| p.starts_with(&seg[s..]))
                {
                    spans.push((start + s, end));
                }
            }
        }
        spans.sort_unstable();
        let mut merged: Vec<(usize, usize)> = Vec::with_capacity(spans.len());
        for (s, e) in spans {
            match merged.last_mut() {
                Some(last) if s < last.1 => last.1 = last.1.max(e),
                _ => merged.push((s, e)),
            }
        }
        merged
    }
}

/// The forms a secret takes in logs: as written, JSON string content (plain, with `\/`, Gson's
/// HTML-safe escaping) and percent-encoded with upper- and lowercase hex.
fn encodings(s: &str) -> Vec<Vec<u8>> {
    let json = |slash: bool, html: bool| {
        let mut out = String::new();
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '/' if slash => out.push_str("\\/"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                '\u{8}' => out.push_str("\\b"),
                '\u{c}' => out.push_str("\\f"),
                '<' | '>' | '&' | '=' | '\'' if html => {
                    out.push_str(&format!("\\u{:04x}", c as u32))
                }
                c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        out.into_bytes()
    };
    let percent = |upper: bool| {
        let mut out = String::new();
        for &b in s.as_bytes() {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
                out.push(b as char);
            } else if upper {
                out.push_str(&format!("%{b:02X}"));
            } else {
                out.push_str(&format!("%{b:02x}"));
            }
        }
        out.into_bytes()
    };
    vec![
        s.as_bytes().to_vec(),
        json(false, false),
        json(true, false),
        json(false, true),
        percent(true),
        percent(false),
    ]
}

/// The first position of `needle` in `hay` (one-off searches; the patterns have their `Finder`).
fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    memchr::memmem::find(hay, needle)
}

/// `line` up to `limit` with every span starting before `limit` replaced as a whole.
fn splice(line: &[u8], spans: &[(usize, usize)], limit: usize) -> Vec<u8> {
    let limit = limit.min(line.len());
    let mut out = Vec::with_capacity(limit + 16);
    let mut pos = 0;
    for &(s, e) in spans {
        if s >= limit {
            break;
        }
        out.extend_from_slice(&line[pos..s]);
        out.extend_from_slice(REDACTED.as_bytes());
        pos = e;
    }
    if pos < limit {
        out.extend_from_slice(&line[pos..limit]);
    }
    out
}

/// Splits a pipe into lines and hands each one, already redacted and cut, to `on_line`. A line
/// is held in memory up to `Redactor::line_capacity`; the rest of a longer line is read and
/// dropped. A line split over several reads is put together first, so a secret split between two
/// reads is still found. A trailing `\r` is removed.
fn read_lines(mut pipe: impl std::io::Read, redactor: &Redactor, mut on_line: impl FnMut(&str)) {
    let cap = redactor.line_capacity();
    let mut buf = vec![0u8; 8192];
    let mut line: Vec<u8> = Vec::new();
    let mut complete = true;
    let mut finish = |line: &mut Vec<u8>, complete: &mut bool| {
        if *complete && line.last() == Some(&b'\r') {
            line.pop();
        }
        let out = redactor.redact_line(line, *complete);
        on_line(&String::from_utf8_lossy(&out));
        line.clear();
        *complete = true;
    };
    loop {
        let n = match pipe.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        let mut chunk = &buf[..n];
        loop {
            let newline = chunk.iter().position(|&b| b == b'\n');
            let part = &chunk[..newline.unwrap_or(chunk.len())];
            if complete {
                let room = cap - line.len();
                if part.len() > room {
                    line.extend_from_slice(&part[..room]);
                    complete = false;
                } else {
                    line.extend_from_slice(part);
                }
            }
            match newline {
                Some(i) => {
                    finish(&mut line, &mut complete);
                    chunk = &chunk[i + 1..];
                }
                None => break,
            }
        }
    }
    if !line.is_empty() || !complete {
        finish(&mut line, &mut complete);
    }
}

/// Numbers the entries of both streams, writes them to the launcher log file and hands them to
/// the sink. One per game run, behind a mutex, so `seq` follows the order lines are delivered in.
struct Emitter {
    seq: u64,
    sink: LineSink,
    file: Option<std::io::BufWriter<std::fs::File>>,
    /// Local time minus UTC in seconds, for the times in the log file.
    utc_offset: i64,
}

impl Emitter {
    fn emit(&mut self, stream: Stream, entries: Vec<Entry>) {
        use std::io::Write;
        if entries.is_empty() {
            return;
        }
        for entry in entries {
            self.seq += 1;
            let line = entry.into_line(self.seq, stream);
            if let Some(f) = &mut self.file {
                if writeln!(f, "{}", format_line(&line, self.utc_offset)).is_err() {
                    // A full disk or a removed folder ends the file, not the game or the sink.
                    self.file = None;
                }
            }
            let sink = &self.sink;
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sink(line)));
        }
        if let Some(f) = &mut self.file {
            if f.flush().is_err() {
                self.file = None;
            }
        }
    }
}

/// A line as the launcher log file shows it: `[hh:mm:ss] [thread/LEVEL]: message` in local time
/// when time, thread and level are known, otherwise the message as it is.
fn format_line(line: &LogLine, utc_offset: i64) -> String {
    let time = match line.time {
        Some(LogTime::UnixMillis(ms)) => {
            let t = (i64::try_from(ms / 1000).unwrap_or(0) + utc_offset).rem_euclid(86_400);
            Some((t / 3600, t / 60 % 60, t % 60))
        }
        Some(LogTime::Clock {
            hour,
            minute,
            second,
        }) => Some((hour.into(), minute.into(), second.into())),
        None => None,
    };
    match (time, &line.thread, line.level) {
        (Some((h, m, s)), Some(thread), Some(level)) => format!(
            "[{h:02}:{m:02}:{s:02}] [{thread}/{}]: {}",
            level.as_str(),
            line.message
        ),
        _ => line.message.clone(),
    }
}

/// Days since 1970-01-01 of a date in the proleptic Gregorian calendar.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The sort key of a launcher log file of `instance`: `<instance>-<yyyymmdd>-<hhmmss>.log`, or
/// `...-<n>.log` when two runs started in the same second. Only names of exactly this shape match;
/// the instance part is compared without case (Windows folder names ignore it), and the fixed
/// digit groups keep `a` from matching the files of `a-b` or `a-20261123-120000`.
fn game_log_key(name: &str, instance: &str) -> Option<(u64, u64, u32)> {
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !name.get(..instance.len())?.eq_ignore_ascii_case(instance) {
        return None;
    }
    let rest = name[instance.len()..].strip_prefix('-')?;
    let ext = rest.len().checked_sub(4)?;
    if !rest.get(ext..)?.eq_ignore_ascii_case(".log") {
        return None;
    }
    let rest = &rest[..ext];
    let (date, rest) = (rest.get(..8)?, rest.get(8..)?);
    let rest = rest.strip_prefix('-')?;
    let (time, rest) = (rest.get(..6)?, rest.get(6..)?);
    if !digits(date) || !digits(time) {
        return None;
    }
    let n = match rest {
        "" => 0,
        _ => {
            let n = rest.strip_prefix('-')?;
            if !digits(n) || n.len() > 3 {
                return None;
            }
            n.parse().ok()?
        }
    };
    Some((date.parse().ok()?, time.parse().ok()?, n))
}

/// Creates `<dir>/<instance>-<stamp>.log`, never replacing a file (another run in the same second
/// gets `-1`, `-2`, ...).
fn create_game_log(
    dir: &std::path::Path,
    instance: &str,
    stamp: &str,
) -> std::io::Result<(std::path::PathBuf, std::fs::File)> {
    for n in 0..1000 {
        let name = match n {
            0 => format!("{instance}-{stamp}.log"),
            n => format!("{instance}-{stamp}-{n}.log"),
        };
        let path = dir.join(name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(f) => return Ok((path, f)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::ErrorKind::AlreadyExists.into())
}

/// Deletes all but the newest `keep` launcher log files of `instance` in `dir`, counting
/// `current` (the file this run just created) as one of them and never deleting it, also when its
/// name is older than the rest (the clock went back). Touches only regular files whose name
/// `game_log_key` accepts for this instance; links, folders and every other file stay. Best effort.
fn prune_game_logs(dir: &std::path::Path, instance: &str, keep: usize, current: &std::path::Path) {
    let current = current.file_name();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut logs: Vec<_> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter(|e| Some(e.file_name().as_os_str()) != current)
        .filter_map(|e| {
            let key = game_log_key(e.file_name().to_str()?, instance)?;
            Some((key, e.path()))
        })
        .collect();
    logs.sort();
    let excess = logs.len().saturating_sub(keep.saturating_sub(1));
    for (_, path) in logs.into_iter().take(excess) {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(windows)]
mod windows {
    use super::*;
    use crate::launch::{ensure_launch_allowed, DEV_OFFLINE};
    use crate::paths::validate_id;
    use crate::process::Job;
    use crate::{Error, Result};
    use std::io;
    use std::os::windows::io::{FromRawHandle, OwnedHandle};
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    use tokio::sync::watch;
    use windows_sys::core::BOOL;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, SYSTEMTIME};
    use windows_sys::Win32::System::SystemInformation::{GetLocalTime, GetSystemTime};
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetWindow, GetWindowThreadProcessId, IsWindowVisible,
        PostMessageW, GW_OWNER, SC_CLOSE, WM_SYSCOMMAND,
    };

    /// Exit code of a game ended by `force_stop`.
    pub const FORCE_STOP_EXIT_CODE: u32 = 1;

    /// After the game process has exited, how long `wait` lets the readers finish the last lines
    /// before it reports the exit. A process the game started may hold the pipes open longer;
    /// its output is still read and delivered afterwards.
    const READER_GRACE: Duration = Duration::from_secs(2);

    type Exit = std::result::Result<i32, (io::ErrorKind, String)>;

    /// A started game. Dropping it neither stops the game nor its output: the readers keep
    /// feeding the sink and the log file until the game closes its pipes.
    pub struct GameProcess {
        pid: u32,
        job: Job,
        exit: watch::Receiver<Option<Exit>>,
        log_path: Option<PathBuf>,
    }

    impl fmt::Debug for GameProcess {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("GameProcess")
                .field("pid", &self.pid)
                .field("log_path", &self.log_path)
                .finish()
        }
    }

    /// Starts the prepared game: only if `ensure_launch_allowed` lets this account play, in a new
    /// job object without kill-on-close (the game outlives the launcher), without a console window,
    /// stdin closed, stdout and stderr read by two threads. Each output line is redacted and cut
    /// in its reader thread, parsed, numbered and then written to the launcher log file
    /// `<data>/logs/game/<instance>-<yyyymmdd-hhmmss>.log` (local time; the 10 newest per
    /// instance are kept) and handed to `sink`. The log file is best effort: when it cannot be
    /// created the game starts without it (`log_path` is `None`). The command line is never
    /// written anywhere.
    pub async fn spawn_game(p: &Prepared, sink: LineSink) -> Result<GameProcess> {
        ensure_launch_allowed(p.account.kind(), DEV_OFFLINE)?;
        let p = p.clone();
        tokio::task::spawn_blocking(move || start(&p, sink))
            .await
            .map_err(|e| io::Error::other(format!("game start task failed: {e}")))?
    }

    fn start(p: &Prepared, sink: LineSink) -> Result<GameProcess> {
        validate_id(&p.instance)?;
        let redactor = Arc::new(Redactor::for_prepared(p));
        let (stamp, utc_offset) = local_clock();
        let log = std::fs::create_dir_all(&p.game_log_dir)
            .and_then(|()| create_game_log(&p.game_log_dir, &p.instance, &stamp))
            .ok();

        let job = Job::new_detached()?;
        let mut cmd = Command::new(&p.java);
        cmd.args(&p.args)
            .current_dir(&p.game_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = match job.spawn(&mut cmd) {
            Ok(child) => child,
            Err(e) => {
                if let Some((path, file)) = log {
                    drop(file);
                    let _ = std::fs::remove_file(path);
                }
                return Err(e.into());
            }
        };
        let (log_path, file) = match log {
            Some((path, file)) => {
                prune_game_logs(&p.game_log_dir, &p.instance, KEEP_GAME_LOGS, &path);
                (Some(path), Some(std::io::BufWriter::new(file)))
            }
            None => (None, None),
        };
        let pid = child.id();
        let emitter = Arc::new(Mutex::new(Emitter {
            seq: 0,
            sink,
            file,
            utc_offset,
        }));
        // Anything failing from here on must not leave the game running unwatched. The job is
        // detached, so dropping it would not end the game: terminate it, else kill the child.
        let fail = |job: &Job, child: &mut std::process::Child, e: io::Error| -> Error {
            if job.try_terminate(FORCE_STOP_EXIT_CODE).is_err() {
                let _ = child.kill();
            }
            e.into()
        };
        // The waiter thread starts first and gets the child only once everything else runs, so
        // the child is still at hand here when a thread cannot be started.
        let (start_tx, start_rx) =
            std::sync::mpsc::channel::<(std::process::Child, Vec<std::thread::JoinHandle<()>>)>();
        let (tx, exit) = watch::channel(None);
        let waiter = std::thread::Builder::new()
            .name("hengst-game-wait".into())
            .spawn(move || {
                let Ok((mut child, readers)) = start_rx.recv() else {
                    return;
                };
                let status = child.wait();
                let deadline = Instant::now() + READER_GRACE;
                while readers.iter().any(|r| !r.is_finished()) && Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(20));
                }
                for reader in readers.into_iter().filter(|r| r.is_finished()) {
                    let _ = reader.join();
                }
                tx.send_replace(Some(
                    status
                        .map(|s| s.code().unwrap_or(-1))
                        .map_err(|e| (e.kind(), e.to_string())),
                ));
            });
        if let Err(e) = waiter {
            return Err(fail(&job, &mut child, e));
        }
        let mut readers = Vec::new();
        let pipes: [(Option<Box<dyn io::Read + Send>>, Stream); 2] = [
            (
                child
                    .stdout
                    .take()
                    .map(|p| Box::new(p) as Box<dyn io::Read + Send>),
                Stream::Stdout,
            ),
            (
                child
                    .stderr
                    .take()
                    .map(|p| Box::new(p) as Box<dyn io::Read + Send>),
                Stream::Stderr,
            ),
        ];
        for (pipe, stream) in pipes {
            let Some(pipe) = pipe else { continue };
            let (redactor, emitter) = (redactor.clone(), emitter.clone());
            let reader = std::thread::Builder::new()
                .name(format!("hengst-game-{stream:?}").to_lowercase())
                .spawn(move || pump(pipe, stream, &redactor, &emitter));
            match reader {
                Ok(reader) => readers.push(reader),
                // Dropping `start_tx` with the error ends the waiter without a result.
                Err(e) => return Err(fail(&job, &mut child, e)),
            }
        }
        // The waiter only ends after `recv`, so the send cannot fail; it owns the child now.
        let _ = start_tx.send((child, readers));
        Ok(GameProcess {
            pid,
            job,
            exit,
            log_path,
        })
    }

    /// Reads one stream to its end: lines redacted and cut, then parsed and emitted.
    fn pump(pipe: impl io::Read, stream: Stream, redactor: &Redactor, emitter: &Mutex<Emitter>) {
        let emit = |entries: Vec<Entry>| {
            emitter
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .emit(stream, entries)
        };
        let mut parser = Parser::new();
        read_lines(pipe, redactor, |line| emit(parser.push(line)));
        emit(parser.finish());
    }

    impl GameProcess {
        /// The id of the game's first process (the java process).
        pub fn pid(&self) -> u32 {
            self.pid
        }

        /// The launcher log file of this run, if it could be created.
        pub fn log_path(&self) -> Option<&Path> {
            self.log_path.as_deref()
        }

        /// Waits until the game's process has exited (and its output is read, see
        /// `READER_GRACE`) and returns its exit code. Any number of callers may wait at the same
        /// time and after the end; all get the same result.
        pub async fn wait(&self) -> Result<i32> {
            let mut rx = self.exit.clone();
            let exit = rx
                .wait_for(Option::is_some)
                .await
                .map_err(|_| io::Error::other("the game's waiter thread ended without a result"))?;
            match &*exit {
                Some(Ok(code)) => Ok(*code),
                Some(Err((kind, message))) => Err(io::Error::new(*kind, message.clone()).into()),
                None => Err(io::Error::other("the game's exit is unknown").into()),
            }
        }

        /// Asks the game to close as if its window's close button was pressed: posts
        /// `WM_SYSCOMMAND`/`SC_CLOSE` to the top-level windows of the processes in its job (the
        /// game window belongs to the java process). Visible windows get it; when a process
        /// shows none (yet), its hidden unowned windows do. Owned windows (dialogs), input method
        /// windows and GLFW's helper window are left out (`SKIPPED_CLASSES`), and so is every window
        /// whose process is not in the job when it is checked on an open handle (a reused process
        /// id). Returns how many windows
        /// it was posted to: 0 when the game has ended or has no window, so it cannot be asked.
        /// Does not wait. Safe to call any number of times, also after the end.
        pub fn request_stop(&self) -> Result<usize> {
            let pids = self.job.process_ids()?;
            if pids.is_empty() {
                return Ok(0);
            }
            let mut posted = 0;
            for target in stop_targets(&self.job, &pids) {
                // The window must still belong to the process checked above; that process cannot
                // end and hand its id on while `target.process` holds it open.
                if window_pid(target.hwnd) != target.pid {
                    continue;
                }
                // SAFETY: plain call; a window that is gone by now makes it fail, nothing else.
                if unsafe { PostMessageW(target.hwnd, WM_SYSCOMMAND, SC_CLOSE as usize, 0) } != 0 {
                    posted += 1;
                }
            }
            Ok(posted)
        }

        /// Ends every process of the game at once (`TerminateJobObject`, exit code 1): unsaved
        /// progress is lost. Safe to call any number of times, also after the end.
        pub fn force_stop(&self) -> Result<()> {
            Ok(self.job.try_terminate(FORCE_STOP_EXIT_CODE)?)
        }
    }

    /// An unowned top-level window, its process and whether it is visible.
    pub(super) struct Window {
        pub(super) hwnd: HWND,
        pid: u32,
        visible: bool,
    }

    /// A window `request_stop` may post to: its process is open (so its id cannot be reused) and
    /// was checked to be in the game's job.
    pub(super) struct Target {
        pub(super) hwnd: HWND,
        pid: u32,
        #[allow(dead_code)] // held open, never read
        process: OwnedHandle,
    }

    /// Classes never asked to close: the input method windows Windows adds to every GUI thread,
    /// and GLFW's hidden helper window (its default handler would destroy it while the game
    /// window stays). The GLFW case follows GLFW's source and is not verified against a real game.
    const SKIPPED_CLASSES: [&str; 3] = ["IME", "MSCTFIME UI", "GLFW3 Helper"];

    /// The windows of `pids` to post the close request to: the visible ones, or the hidden ones
    /// when none is visible; each only if its process is in `job` right now. The ids come from the
    /// job's list, but a process can end and its id go to an unrelated process before the windows
    /// are enumerated, so membership is checked again on an open handle.
    pub(super) fn stop_targets(job: &Job, pids: &[u32]) -> Vec<Target> {
        let windows = top_level_windows(pids);
        let any_visible = windows.iter().any(|w| w.visible);
        windows
            .into_iter()
            .filter(|w| w.visible || !any_visible)
            .filter_map(|w| {
                // SAFETY: plain call; the result is checked before it is owned.
                let h = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, w.pid) };
                if h.is_null() {
                    return None;
                }
                // SAFETY: `h` is a valid process handle that nothing else owns.
                let process = unsafe { OwnedHandle::from_raw_handle(h) };
                job.contains(&process).ok()?.then_some(Target {
                    hwnd: w.hwnd,
                    pid: w.pid,
                    process,
                })
            })
            .collect()
    }

    fn window_pid(hwnd: HWND) -> u32 {
        let mut pid = 0;
        // SAFETY: `pid` is a writable u32 for the duration of the call.
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        pid
    }

    struct Search<'a> {
        pids: &'a [u32],
        found: Vec<Window>,
    }

    unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: `lparam` is the `&mut Search` `top_level_windows` passed, alive for the whole
        // enumeration, which calls back on this thread only.
        let search = unsafe { &mut *(lparam as *mut Search) };
        let pid = window_pid(hwnd);
        // SAFETY: plain calls on a window handle Windows just handed out.
        if !search.pids.contains(&pid) || !unsafe { GetWindow(hwnd, GW_OWNER) }.is_null() {
            return 1;
        }
        let mut class = [0u16; 64];
        // SAFETY: the buffer and its length in u16 match.
        let n = unsafe { GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32) };
        let class = String::from_utf16_lossy(&class[..n.max(0) as usize]);
        if SKIPPED_CLASSES.contains(&class.as_str()) {
            return 1;
        }
        // SAFETY: plain call.
        let visible = unsafe { IsWindowVisible(hwnd) } != 0;
        search.found.push(Window { hwnd, pid, visible });
        1
    }

    /// Unowned top-level windows of these processes, without `SKIPPED_CLASSES`.
    pub(super) fn top_level_windows(pids: &[u32]) -> Vec<Window> {
        let mut search = Search {
            pids,
            found: Vec::new(),
        };
        // SAFETY: `collect` matches WNDENUMPROC and only uses `lparam` as the `Search` passed
        // here, which outlives the call. A failure leaves `found` as far as it got.
        unsafe { EnumWindows(Some(collect), &mut search as *mut Search as LPARAM) };
        search.found
    }

    fn seconds(t: &SYSTEMTIME) -> i64 {
        days_from_civil(t.wYear.into(), t.wMonth.into(), t.wDay.into()) * 86_400
            + i64::from(t.wHour) * 3600
            + i64::from(t.wMinute) * 60
            + i64::from(t.wSecond)
    }

    /// The local time as `yyyymmdd-hhmmss` and the local offset from UTC in seconds (rounded to
    /// minutes).
    fn local_clock() -> (String, i64) {
        let mut local = SYSTEMTIME::default();
        let mut utc = SYSTEMTIME::default();
        // SAFETY: both point to writable SYSTEMTIME structs.
        unsafe {
            GetSystemTime(&mut utc);
            GetLocalTime(&mut local);
        }
        let offset = (seconds(&local) - seconds(&utc) + 30).div_euclid(60) * 60;
        let stamp = format!(
            "{:04}{:02}{:02}-{:02}{:02}{:02}",
            local.wYear, local.wMonth, local.wDay, local.wHour, local.wMinute, local.wSecond
        );
        (stamp, offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gamelog::Level;

    /// Every character that changes in JSON or in a URL, plus a long body.
    const TOKEN: &str = "eyJhbGciOiJIUzI1NiJ9.QZVJW/Tok+en\"with\\odd=chars'<&>.QZVJW-tail_end~";

    fn redactor() -> Redactor {
        Redactor::new(&[TOKEN])
    }

    /// Lines `read_lines` makes of these reads.
    fn lines_of(chunks: &[&[u8]], r: &Redactor) -> Vec<String> {
        /// Each read returns at most the rest of the current chunk, never two chunks at once.
        struct Chunks<'a>(std::slice::Iter<'a, &'a [u8]>, &'a [u8]);
        impl std::io::Read for Chunks<'_> {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                while self.1.is_empty() {
                    match self.0.next() {
                        Some(c) => self.1 = *c,
                        None => return Ok(0),
                    }
                }
                let n = self.1.len().min(buf.len());
                buf[..n].copy_from_slice(&self.1[..n]);
                self.1 = &self.1[n..];
                Ok(n)
            }
        }
        let mut out = Vec::new();
        read_lines(Chunks(chunks.iter(), &[]), r, |l| out.push(l.to_string()));
        out
    }

    /// No piece of the token of `MIN_FRAGMENT` bytes or more is in `text`.
    fn assert_no_token(text: &str) {
        for w in TOKEN.as_bytes().windows(MIN_FRAGMENT) {
            let w = std::str::from_utf8(w).unwrap();
            assert!(!text.contains(w), "{w:?} leaked in {text:?}");
        }
    }

    #[test]
    fn every_encoding_of_the_token_is_redacted() {
        let r = redactor();
        let json = serde_json::to_string(TOKEN).unwrap();
        let json = &json[1..json.len() - 1];
        let gson = json
            .replace('<', "\\u003c")
            .replace('>', "\\u003e")
            .replace('&', "\\u0026")
            .replace('=', "\\u003d")
            .replace('\'', "\\u0027");
        let percent = |upper: bool| -> String {
            TOKEN
                .bytes()
                .map(|b| match b {
                    b if b.is_ascii_alphanumeric() || b"-._~".contains(&b) => {
                        (b as char).to_string()
                    }
                    b if upper => format!("%{b:02X}"),
                    b => format!("%{b:02x}"),
                })
                .collect()
        };
        assert!(percent(true).contains("%2F") && percent(false).contains("%2f"));
        for form in [
            TOKEN.to_string(),
            json.to_string(),
            json.replace('/', "\\/"),
            gson,
            percent(true),
            percent(false),
        ] {
            let text = format!("before {form} after");
            assert_eq!(r.redact(&text), "before <redacted> after", "{form}");
        }
        let session = format!("--session token:{TOKEN}:0123");
        assert_eq!(r.redact(&session), "--session token:<redacted>:0123");
        assert_eq!(
            r.redact(&format!("{TOKEN}{TOKEN}\n{TOKEN}")),
            "<redacted><redacted>\n<redacted>"
        );
    }

    #[test]
    fn empty_and_short_secrets_redact_nothing_and_never_panic() {
        for secrets in [&[""][..], &["0"], &["abcde"], &[], &["", "0", "x"]] {
            let r = Redactor::new(secrets);
            for text in ["", "0", "abcde 0 x", "\u{e4}\u{fffd}"] {
                assert_eq!(r.redact(text), text);
            }
            assert_eq!(lines_of(&[b"0 abcde\n"], &r), ["0 abcde"]);
        }
        let offline = Redactor::for_account(&Account::offline("Mira_K"));
        assert_eq!(offline.redact("--accessToken 0"), "--accessToken 0");
        assert!(!format!("{:?}", redactor()).contains("QZVJW"));
    }

    #[test]
    fn a_token_split_between_two_reads_is_redacted() {
        let r = redactor();
        let line = format!("--accessToken {TOKEN} --version 1.21.11\n");
        for split in 1..line.len() {
            let (a, b) = line.as_bytes().split_at(split);
            let lines = lines_of(&[a, b], &r);
            assert_eq!(
                lines,
                ["--accessToken <redacted> --version 1.21.11"],
                "{split}"
            );
        }
    }

    #[test]
    fn a_token_split_over_lines_leaves_at_most_seven_bytes() {
        let r = redactor();
        for split in 1..TOKEN.len() {
            let text = format!("token {}\r\n{} rest\n", &TOKEN[..split], &TOKEN[split..]);
            let lines = lines_of(&[text.as_bytes()], &r);
            assert_eq!(lines.len(), 2);
            for l in &lines {
                assert_no_token(l);
            }
            assert!(lines[1].ends_with(" rest"), "{lines:?}");
        }
        // Wrapped over several lines: the middle ones are inner pieces.
        let wrapped: Vec<&str> = TOKEN
            .as_bytes()
            .chunks(10)
            .map(|c| std::str::from_utf8(c).unwrap())
            .collect();
        let text = format!("  {}\n", wrapped.join("\n  "));
        for l in lines_of(&[text.as_bytes()], &r) {
            assert_no_token(&l);
        }
    }

    #[test]
    fn long_lines_are_cut_after_redaction_and_never_through_a_token() {
        let r = redactor();
        let pad = TOKEN.len() + 4;
        for at in LINE_LIMIT - pad..LINE_LIMIT + 4 {
            let line = format!("{}{TOKEN}{}\n", "x".repeat(at), "y".repeat(20_000));
            // In one read and in reads of 1000 bytes.
            let chunks: Vec<&[u8]> = line.as_bytes().chunks(1000).collect();
            for lines in [lines_of(&[line.as_bytes()], &r), lines_of(&chunks, &r)] {
                assert_eq!(lines.len(), 1);
                let l = &lines[0];
                assert!(l.ends_with(CUT_MARKER), "{at}");
                // Not a single byte of the token's own letters made it.
                assert!(
                    !l.contains(['Q', 'Z', 'V', 'J', 'W']),
                    "{at}: {}",
                    &l[LINE_LIMIT - 100..]
                );
                assert!(l.len() <= LINE_LIMIT + REDACTED.len() + CUT_MARKER.len());
                if at < LINE_LIMIT {
                    assert!(l.contains(REDACTED), "{at}");
                }
            }
        }
    }

    #[test]
    fn an_endless_line_is_held_with_bounded_memory() {
        let r = redactor();
        let chunk = vec![b'x'; 8192];
        let mut chunks: Vec<&[u8]> = vec![&chunk; 256];
        chunks.push(b"\nnext\n");
        let lines = lines_of(&chunks, &r);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].len(), LINE_LIMIT + CUT_MARKER.len());
        assert_eq!(lines[1], "next");
        // Cut inside a multi-byte character, and output without a final line break.
        let text = format!("{}\u{e4}\u{e4}\u{e4}", "x".repeat(LINE_LIMIT - 1));
        let lines = lines_of(&[text.as_bytes()], &r);
        assert!(
            lines[0].ends_with(&format!("\u{fffd}{CUT_MARKER}")),
            "{}",
            &lines[0][LINE_LIMIT - 4..]
        );
        assert_eq!(lines_of(&[b"a\r\n\r\nb"], &r), ["a", "", "b"]);
    }

    #[test]
    fn game_log_names_match_their_instance_only() {
        let key = game_log_key;
        assert_eq!(
            key("1.21.11-20261123-120000.log", "1.21.11"),
            Some((20_261_123, 120_000, 0))
        );
        assert_eq!(
            key("1.21.11-20261123-120000-2.LOG", "1.21.11"),
            Some((20_261_123, 120_000, 2))
        );
        assert!(key("1.21.11-20261123-120000.log", "1.21.11-FABRIC").is_none());
        // Instance ids with dashes and dots never match another instance's files.
        assert!(key("1.21.11-fabric-20261123-120000.log", "1.21.11").is_none());
        assert!(key("1.21.11-fabric-20261123-120000.log", "1.21.11-fabric").is_some());
        assert!(key("1.21.11-fabric-20261123-120000.log", "1.21.1").is_none());
        assert!(key("a-20261123-120000-20261123-130000.log", "a").is_none());
        assert!(key("a-20261123-120000-20261123-130000.log", "a-20261123-120000").is_some());
        assert!(key("a-20261123-120000-3.log", "a-20261123-120000").is_none());
        assert!(key("A-20261123-120000.log", "a").is_some());
        for bad in [
            "a-20261123-120000.txt",
            "a-20261123-120000.log.bak",
            "a-2026112-1200000.log",
            "a-2026112x-120000.log",
            "a-20261123-120000-.log",
            "a-20261123-120000-1234.log",
            "a-20261123-120000-x.log",
            "a20261123-120000.log",
            "a-.log",
            "a",
            "\u{e4}-20261123-120000.log",
        ] {
            assert!(key(bad, "a").is_none(), "{bad}");
        }
    }

    #[test]
    fn pruning_keeps_the_ten_newest_of_the_instance_and_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        let touch = |name: &str| std::fs::write(dir.path().join(name), b"x").unwrap();
        let mine: Vec<String> = (0..12)
            .map(|i| format!("1.21.11-202611{:02}-120000.log", 10 + i))
            .collect();
        for name in &mine {
            touch(name);
        }
        touch("1.21.11-20261121-120000-1.log");
        let others = [
            "1.21.11-fabric-20200101-000000.log",
            "1.21.1-20200101-000000.log",
            "1.21.11-20200101-000000.txt",
            "notes.log",
        ];
        for name in others {
            touch(name);
        }
        std::fs::create_dir(dir.path().join("1.21.11-20000101-000000.log")).unwrap();
        prune_game_logs(
            dir.path(),
            "1.21.11",
            KEEP_GAME_LOGS,
            &dir.path().join(&mine[11]),
        );
        let left: std::collections::BTreeSet<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        // 13 log files of this instance: the three oldest go. The folder is not a log file and
        // stays although its name has the shape.
        for gone in &mine[..3] {
            assert!(!left.contains(gone), "{gone}");
        }
        for kept in &mine[3..] {
            assert!(left.contains(kept), "{kept}");
        }
        assert!(left.contains("1.21.11-20261121-120000-1.log"));
        for keep in others {
            assert!(left.contains(keep), "{keep}");
        }
        assert!(dir.path().join("1.21.11-20000101-000000.log").is_dir());
        assert_eq!(left.len(), KEEP_GAME_LOGS + others.len() + 1);
    }

    #[test]
    fn pruning_never_removes_the_file_of_the_current_run() {
        // The clock went back: the current run's file is older by name than ten others.
        let dir = tempfile::tempdir().unwrap();
        let touch = |name: &str| {
            let path = dir.path().join(name);
            std::fs::write(&path, b"x").unwrap();
            path
        };
        let others: Vec<std::path::PathBuf> = (0..10)
            .map(|i| touch(&format!("a-202611{:02}-120000.log", 10 + i)))
            .collect();
        let current = touch("a-20200101-000000.log");
        prune_game_logs(dir.path(), "a", KEEP_GAME_LOGS, &current);
        assert!(current.is_file());
        assert!(!others[0].exists());
        assert!(others[1..].iter().all(|p| p.is_file()));
    }

    /// A hidden top-level window of this test process, which is in no job of the test: stands in
    /// for a process that took over the id of an ended job member.
    #[cfg(windows)]
    #[test]
    fn a_window_of_a_process_outside_the_job_is_never_targeted() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow};
        let class: Vec<u16> = "STATIC".encode_utf16().chain([0]).collect();
        // SAFETY: a predefined class, no parent; created without WS_VISIBLE and never shown.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                class.as_ptr(),
                0,
                0,
                0,
                10,
                10,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        assert!(!hwnd.is_null());
        let own = std::process::id();
        // Control: by process id alone the window would be a target.
        let found = self::windows::top_level_windows(&[own]);
        let job = crate::process::Job::new_detached().unwrap();
        let targets = self::windows::stop_targets(&job, &[own]);
        // SAFETY: this thread's own window.
        unsafe { DestroyWindow(hwnd) };
        assert!(found.iter().any(|w| w.hwnd == hwnd));
        assert!(targets.iter().all(|t| t.hwnd != hwnd));
    }

    #[test]
    fn log_file_lines_use_local_time_and_the_plain_format() {
        let line = |time, level, thread: Option<&str>| LogLine {
            seq: 1,
            time,
            level,
            thread: thread.map(String::from),
            message: "Setting user: Mira_K".into(),
            stream: Stream::Stdout,
        };
        // 1795445575774 ms is 14:52:55 UTC.
        let event = line(
            Some(LogTime::UnixMillis(1_795_445_575_774)),
            Some(Level::Info),
            Some("Render thread"),
        );
        assert_eq!(
            format_line(&event, 3600),
            "[15:52:55] [Render thread/INFO]: Setting user: Mira_K"
        );
        assert_eq!(
            format_line(&event, -15 * 3600).get(..10),
            Some("[23:52:55]")
        );
        let clock = Some(LogTime::Clock {
            hour: 1,
            minute: 2,
            second: 3,
        });
        assert_eq!(
            format_line(&line(clock, Some(Level::Warn), Some("main")), 0),
            "[01:02:03] [main/WARN]: Setting user: Mira_K"
        );
        assert_eq!(
            format_line(&line(None, None, None), 0),
            "Setting user: Mira_K"
        );
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 1, 1), 10_957);
        assert_eq!(days_from_civil(2026, 11, 23), 1_795_445_575 / 86_400);
        assert_eq!(days_from_civil(1969, 12, 31), -1);
    }

    #[cfg(windows)]
    fn prepared(dir: &std::path::Path, account: Account) -> Prepared {
        Prepared {
            java: "cmd.exe".into(),
            // Leaves a marker when it runs at all.
            args: vec!["/d".into(), "/c".into(), "echo ran>marker.txt".into()],
            game_dir: dir.to_path_buf(),
            version_id: "1.21.11".into(),
            loader: None,
            loader_list_from_cache: false,
            account,
            instance: "1.21.11".into(),
            game_log_dir: dir.join("logs").join("game"),
        }
    }

    /// The gate is `ensure_launch_allowed`; without `dev-offline` it refuses before anything is
    /// created or started. Runs in `cargo test -p hengst-core --lib` (CI, no features).
    #[cfg(all(windows, not(feature = "dev-offline")))]
    #[tokio::test]
    async fn offline_account_cannot_spawn_in_release() {
        let dir = tempfile::tempdir().unwrap();
        let p = prepared(dir.path(), Account::offline("Mira_K"));
        let sink: LineSink = Arc::new(|_| panic!("no line expected"));
        let err = spawn_game(&p, sink).await.unwrap_err();
        assert!(matches!(err, crate::Error::OfflineLaunchDisabled), "{err}");
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(!dir.path().join("marker.txt").exists());
        assert!(!dir.path().join("logs").exists());
    }

    /// The counterpart: with `dev-offline` the same launch runs (so the test above would see it).
    #[cfg(all(windows, feature = "dev-offline"))]
    #[tokio::test]
    async fn offline_account_spawns_with_dev_offline() {
        let dir = tempfile::tempdir().unwrap();
        let p = prepared(dir.path(), Account::offline("Mira_K"));
        let game = spawn_game(&p, Arc::new(|_| {})).await.unwrap();
        assert_eq!(game.wait().await.unwrap(), 0);
        assert!(dir.path().join("marker.txt").is_file());
        assert!(game
            .log_path()
            .unwrap()
            .starts_with(dir.path().join("logs").join("game")));
    }
}
