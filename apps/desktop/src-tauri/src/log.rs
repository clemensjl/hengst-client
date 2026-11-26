//! The live log: the output of the current and the previous game run in memory, for the log view.
//!
//! Lines come from the drain task of each run (`play.rs`), already redacted by the core (each raw
//! line, and the parsed log4j text a second time after entity decoding). Nothing here ever sees
//! the token or the command line, and no path leaves this module towards the web view.
//!
//! Limits, per run (`RUN_LINES`, `RUN_BYTES`): 10,000 lines and 16 MiB, counting message, thread
//! and `LINE_OVERHEAD` per line. Whichever is hit first, the oldest lines go; one log4j event can
//! be up to 512 KiB, so the byte limit is what holds with large events. Two runs are kept (a third
//! start drops the oldest), so the buffers take at most about 2 x 16 MiB. Lines of a run that is
//! no longer kept are dropped; lines that arrive after the exit go to their own run's buffer.
//!
//! The drain path (`push`) takes the lock, puts the line into its run's ring and drops the oldest
//! lines over the limits, and wakes the run's consumer when a batch is due. It allocates one
//! `Arc` per line, copies nothing and never waits for a consumer, so the core's unbounded channel
//! drains as fast as lines come. The lock is otherwise held only for bounded work: a batch reads
//! and copies at most about 8 KiB of text (at most 200 lines), a copy takes one `Arc` clone per
//! held line (its text is built outside the lock).
//!
//! Consumers: one at a time (the log view of the one window). `attach` replaces the previous
//! consumer; its flush task ends and its channel is dropped. The consumer has a cursor into the
//! ring: backfill and live lines are read from the same ring by the same task, so there is no
//! hand-over and no line can be sent twice or skipped. Lines the ring dropped before they were
//! sent are reported as `dropped` in the next batch.
//!
//! Nothing waits in Tauri: Tauri 2.12.7 evaluates a channel message in the page at once when its
//! JSON is shorter than 8 KiB (`INLINE_JSON`) and only queues longer ones until the page fetches
//! them, which a page that reloads never does. Every batch stays below that (`BATCH_JSON` for its
//! lines, the rest for the envelope); a line too long for one batch goes in parts, which share
//! its seq. So nothing is left behind by a reload, however often the page reloads.
//!
//! Flow control: a batch with lines needs one of `WINDOW` credits and gives it back when the web
//! view acknowledges it (`ack`) with the batch's unguessable token, which it only learns by
//! receiving the batch, or when its send fails (the channel is gone; the consumer ends then). A
//! page that stops reading gets nothing more: the cursor stays and the ring keeps only its
//! limits, so nothing piles up for it. A page load returns all credits (its batches were already
//! handed to the page, none wait in Tauri). A batch that was cut by its size is followed by the
//! next one at once, within the credits; one that is not full waits for `BATCH_INTERVAL`.

use crate::dto::error::{ErrorCode, ErrorDto};
use crate::dto::log::{
    LogBatch, LogCopyDto, LogEnd, LogFilter, LogLineDto, LogRunDto, LogRunStateDto,
};
use crate::dto::play::RunId;
use hengst_core::gamelog::{Level, LogLine};
use std::collections::{BTreeMap, VecDeque};
use std::hash::{BuildHasher, RandomState};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tokio::sync::{Notify, Semaphore};
use tokio::time::Instant;

/// Where a consumer's batches go (the web view's channel). `Err`: the channel is gone.
pub type LogSink = Arc<dyn Fn(LogBatch) -> Result<(), ()> + Send + Sync>;

/// Lines kept per run.
pub const RUN_LINES: usize = 10_000;
/// Bytes kept per run (message, thread and `LINE_OVERHEAD` per line).
pub const RUN_BYTES: usize = 16 * 1024 * 1024;
/// What a held line costs besides its text: the `LogLine` behind its `Arc` and the ring slot.
pub const LINE_OVERHEAD: usize = 128;
/// At most this many lines per batch.
pub const BATCH_LINES: usize = 200;
/// A batch that is not full goes out at most this long after the previous one.
pub const BATCH_INTERVAL: Duration = Duration::from_millis(100);
/// Batches with lines that may be on their way to the web view (not yet acknowledged).
pub const WINDOW: usize = 4;
/// Tauri 2.12.7 evaluates a channel message directly in the page when its JSON is shorter than
/// this (`MAX_JSON_DIRECT_EXECUTE_THRESHOLD`); a longer one waits in Tauri's queue until the page
/// fetches it.
pub const INLINE_JSON: usize = 8192;
/// The JSON of a batch's lines (with their separators) stays within this, so the whole batch
/// stays below `INLINE_JSON`.
pub const BATCH_JSON: usize = 7680;
/// The longest search text of `log_copy`, in characters.
pub const FILTER_TEXT_CHARS: usize = 256;
/// A part shorter than this is not started at the end of a batch that already has lines.
const MIN_PIECE: usize = 256;
/// The longest text `log_copy` returns, in bytes.
pub const COPY_BYTES: usize = 4 * 1024 * 1024;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn invalid_input(field: &str, detail: &str) -> ErrorDto {
    ErrorDto {
        code: ErrorCode::InvalidInput,
        params: BTreeMap::from([("field".to_string(), field.to_string())]),
        action: None,
        detail: detail.to_string(),
    }
}

/// The run is not (or no longer) held: only the current and the previous run are.
fn unknown_run() -> ErrorDto {
    invalid_input("runId", "the log of this run is not held")
}

/// The JSON length of `c` inside a string, as serde_json writes it.
fn json_len(c: char) -> usize {
    match c {
        '"' | '\\' | '\n' | '\r' | '\t' | '\u{8}' | '\u{c}' => 2,
        c if (c as u32) < 0x20 => 6,
        c => c.len_utf8(),
    }
}

/// The longest prefix of `s` whose JSON fits into `room`: its length in bytes and in JSON.
fn fitting_prefix(s: &str, room: usize) -> (usize, usize) {
    let mut json = 0;
    for (i, c) in s.char_indices() {
        let n = json_len(c);
        if json + n > room {
            return (i, json);
        }
        json += n;
    }
    (s.len(), json)
}

struct Held {
    line: LogLine,
    cost: usize,
}

struct RunLog {
    run_id: RunId,
    instance: String,
    since: u64,
    /// The launcher log file of the run; never sent to the web view.
    log_file: Option<PathBuf>,
    /// Local time minus UTC at the start, as in the launcher log file.
    utc_offset: i64,
    exit: Option<(i32, bool)>,
    lines: VecDeque<Arc<Held>>,
    bytes: usize,
    /// The seq the next line gets.
    next_seq: u64,
    closed: bool,
}

impl RunLog {
    fn first_seq(&self) -> u64 {
        self.next_seq - self.lines.len() as u64
    }

    fn dto(&self) -> LogRunDto {
        LogRunDto {
            run_id: self.run_id,
            instance: self.instance.clone(),
            since: self.since,
            first_seq: self.first_seq(),
            next_seq: self.next_seq,
            closed: self.closed,
            state: match self.exit {
                None => LogRunStateDto::Running,
                Some((code, stop_requested)) => LogRunStateDto::Ended {
                    code,
                    stop_requested,
                },
            },
        }
    }

    /// The held line with `seq` (which must be held).
    fn at(&self, seq: u64) -> &Arc<Held> {
        &self.lines[(seq - self.first_seq()) as usize]
    }
}

/// A line that is being sent in parts: the rest from `offset` on goes as part `part`.
struct Partial {
    held: Arc<Held>,
    offset: usize,
    part: u32,
}

struct Consumer {
    id: u64,
    run_id: RunId,
    /// The next seq to send (the seq of `partial` while one is open).
    cursor: u64,
    partial: Option<Partial>,
    /// The last batch was cut by its size or line count: the next one is due at once.
    full: bool,
    /// Lines before this seq were held when the consumer attached: they go out without waiting
    /// for the interval.
    backfill_end: u64,
    last_sent: Option<Instant>,
    sink: LogSink,
    wake: Arc<Notify>,
    task: Option<tokio::task::AbortHandle>,
}

#[derive(Default)]
struct Inner {
    /// Oldest first, at most two.
    runs: VecDeque<RunLog>,
    consumer: Option<Consumer>,
    /// Tokens of batches with lines that have not been acknowledged.
    in_flight: Vec<u64>,
    next_consumer: u64,
    next_batch: u64,
}

impl Inner {
    fn run(&self, run_id: RunId) -> Option<&RunLog> {
        self.runs.iter().find(|r| r.run_id == run_id)
    }

    fn run_mut(&mut self, run_id: RunId) -> Option<&mut RunLog> {
        self.runs.iter_mut().find(|r| r.run_id == run_id)
    }
}

/// The logs of the current and the previous run, and the one consumer.
pub struct LiveLog {
    inner: Mutex<Inner>,
    /// Keys the batch tokens, so a page cannot acknowledge a batch it has not received.
    tokens: RandomState,
    /// One `log_copy` at a time.
    pub(crate) copy_gate: Semaphore,
}

impl Default for LiveLog {
    fn default() -> LiveLog {
        LiveLog::new()
    }
}

/// What the flush task of a consumer does next.
enum Step {
    Send(LogBatch, LogSink),
    /// The last batch of this consumer.
    Last(LogBatch, LogSink),
    /// Until woken, or at most this long.
    Wait(Option<Duration>),
    Stop,
}

impl LiveLog {
    pub fn new() -> LiveLog {
        LiveLog {
            inner: Mutex::new(Inner::default()),
            tokens: RandomState::new(),
            copy_gate: Semaphore::new(1),
        }
    }

    /// A token for the next batch: 53 bits (exact in a JavaScript number) of a keyed hash of a
    /// counter, so it cannot be guessed from earlier ones.
    fn token(&self, counter: &mut u64) -> u64 {
        *counter += 1;
        self.tokens.hash_one(*counter) & ((1 << 53) - 1)
    }

    /// A run started: its buffer is created (empty), the oldest of three is dropped. Called before
    /// the run's first line and before the state says `Running`, so an attach that follows the
    /// state always finds it.
    pub fn start_run(
        &self,
        run_id: RunId,
        instance: &str,
        since: u64,
        log_file: Option<&Path>,
        utc_offset: i64,
    ) {
        let dropped = {
            let mut inner = lock(&self.inner);
            inner.runs.push_back(RunLog {
                run_id,
                instance: instance.to_string(),
                since,
                log_file: log_file.map(Path::to_path_buf),
                utc_offset,
                exit: None,
                lines: VecDeque::new(),
                bytes: 0,
                next_seq: 1,
                closed: false,
            });
            let dropped = (inner.runs.len() > 2).then(|| inner.runs.pop_front());
            if let Some(c) = &inner.consumer {
                if !inner.runs.iter().any(|r| r.run_id == c.run_id) {
                    c.wake.notify_one();
                }
            }
            dropped
        };
        // Its lines are freed outside the lock.
        drop(dropped);
    }

    /// One line of run `run_id` (the drain path). Its seq is the run's next one. Lines of a run
    /// that is not held are dropped.
    pub fn push(&self, run_id: RunId, mut line: LogLine) {
        let cost = line.message.len() + line.thread.as_ref().map_or(0, String::len) + LINE_OVERHEAD;
        let mut evicted = Vec::new();
        {
            let mut inner = lock(&self.inner);
            let inner = &mut *inner;
            let Some(run) = inner.runs.iter_mut().find(|r| r.run_id == run_id) else {
                return;
            };
            line.seq = run.next_seq;
            run.next_seq += 1;
            run.bytes += cost;
            run.lines.push_back(Arc::new(Held { line, cost }));
            while run.lines.len() > 1 && (run.lines.len() > RUN_LINES || run.bytes > RUN_BYTES) {
                if let Some(old) = run.lines.pop_front() {
                    run.bytes -= old.cost;
                    evicted.push(old);
                }
            }
            if let Some(c) = &inner.consumer {
                if c.run_id == run_id {
                    // Wake the flush task when the first unsent line comes and when a batch is
                    // full; in between its interval timer runs.
                    let unsent = run.next_seq.saturating_sub(c.cursor);
                    if unsent == 1 || unsent == BATCH_LINES as u64 {
                        c.wake.notify_one();
                    }
                }
            }
        }
        drop(evicted);
    }

    /// The game closed the output of `run_id`: no more lines come.
    pub fn close_run(&self, run_id: RunId) {
        let mut inner = lock(&self.inner);
        if let Some(run) = inner.run_mut(run_id) {
            run.closed = true;
        }
        if let Some(c) = inner.consumer.as_ref().filter(|c| c.run_id == run_id) {
            c.wake.notify_one();
        }
    }

    /// The process of `run_id` exited (from the game's state).
    pub fn run_ended(&self, run_id: RunId, code: i32, stop_requested: bool) {
        if let Some(run) = lock(&self.inner).run_mut(run_id) {
            run.exit = Some((code, stop_requested));
        }
    }

    /// The held runs, newest first.
    pub fn runs(&self) -> Vec<LogRunDto> {
        lock(&self.inner)
            .runs
            .iter()
            .rev()
            .map(RunLog::dto)
            .collect()
    }

    /// The launcher log file of a held run (Rust only: `open_location`).
    pub fn log_file(&self, run_id: RunId) -> Option<PathBuf> {
        lock(&self.inner).run(run_id)?.log_file.clone()
    }

    /// Makes `sink` the consumer of run `run_id` from `since_seq` on (0 or 1: from the oldest held
    /// line), replacing any earlier consumer. Unknown run: `InvalidInput` (`runId`); a seq beyond
    /// the next one: `InvalidInput` (`sinceSeq`). Needs a Tokio runtime (its flush task).
    pub fn attach(
        self: &Arc<Self>,
        run_id: RunId,
        since_seq: u64,
        sink: LogSink,
    ) -> Result<(), ErrorDto> {
        let wake = Arc::new(Notify::new());
        let (old, id) = {
            let mut inner = lock(&self.inner);
            let Some(run) = inner.run(run_id) else {
                return Err(unknown_run());
            };
            if since_seq > run.next_seq {
                return Err(invalid_input(
                    "sinceSeq",
                    "the run has no line with this seq yet",
                ));
            }
            let backfill_end = run.next_seq;
            inner.next_consumer += 1;
            let id = inner.next_consumer;
            let old = inner.consumer.replace(Consumer {
                id,
                run_id,
                cursor: since_seq.max(1),
                partial: None,
                full: false,
                backfill_end,
                last_sent: None,
                sink,
                wake: wake.clone(),
                task: None,
            });
            (old, id)
        };
        Self::release(old);
        let log = self.clone();
        let task = tokio::spawn(flush(log, id, wake)).abort_handle();
        let mut inner = lock(&self.inner);
        match inner.consumer.as_mut().filter(|c| c.id == id) {
            Some(c) => c.task = Some(task),
            // Replaced or detached already: the task stops at its next step anyway.
            None => task.abort(),
        }
        Ok(())
    }

    /// Ends a consumer's task and drops its channel, outside the lock.
    fn release(old: Option<Consumer>) {
        if let Some(old) = old {
            if let Some(task) = &old.task {
                task.abort();
            }
        }
    }

    /// Stops sending; idempotent.
    pub fn detach(&self) {
        let old = lock(&self.inner).consumer.take();
        Self::release(old);
    }

    /// The page is loading again: its consumer is gone and the batches it had not acknowledged
    /// will never be.
    pub fn page_reset(&self) {
        let old = {
            let mut inner = lock(&self.inner);
            inner.in_flight.clear();
            inner.consumer.take()
        };
        Self::release(old);
    }

    /// The send of the batch `token` to consumer `id` failed: its channel is gone. The credit
    /// comes back and the consumer ends.
    fn send_failed(&self, id: u64, token: u64) {
        let old = {
            let mut inner = lock(&self.inner);
            inner.in_flight.retain(|t| *t != token);
            match &inner.consumer {
                Some(c) if c.id == id => inner.consumer.take(),
                _ => None,
            }
        };
        drop(old);
    }

    /// A batch with this token has arrived; unknown tokens are ignored.
    pub fn ack(&self, token: u64) {
        let mut inner = lock(&self.inner);
        if let Some(i) = inner.in_flight.iter().position(|t| *t == token) {
            inner.in_flight.swap_remove(i);
            if let Some(c) = &inner.consumer {
                c.wake.notify_one();
            }
        }
    }

    /// What the flush task of consumer `id` does now.
    fn step(&self, id: u64, now: Instant) -> Step {
        let mut guard = lock(&self.inner);
        let inner = &mut *guard;
        let Some(c) = inner.consumer.as_mut().filter(|c| c.id == id) else {
            return Step::Stop;
        };
        let Some(run) = inner.runs.iter().find(|r| r.run_id == c.run_id) else {
            let batch = LogBatch {
                ack: 0,
                run_id: c.run_id,
                dropped: 0,
                lines: Vec::new(),
                end: Some(LogEnd::Expired),
            };
            return Step::Last(batch, c.sink.clone());
        };
        let first = run.first_seq();
        let mut dropped = 0;
        if c.partial.is_none() && c.cursor < first {
            dropped = first - c.cursor;
            c.cursor = first;
        }
        let available = run.next_seq - c.cursor;
        let credit = inner.in_flight.len() < WINDOW;
        if available == 0 || !credit {
            let end = (available == 0 && run.closed).then_some(LogEnd::Complete);
            if dropped == 0 && end.is_none() {
                return Step::Wait(None);
            }
            // Without lines a batch needs no credit: it is small and is not acknowledged.
            let batch = LogBatch {
                ack: 0,
                run_id: c.run_id,
                dropped,
                lines: Vec::new(),
                end,
            };
            let sink = c.sink.clone();
            return match end {
                Some(_) => Step::Last(batch, sink),
                None => Step::Send(batch, sink),
            };
        }
        let due = dropped > 0
            || c.full
            || c.partial.is_some()
            || available >= BATCH_LINES as u64
            || run.closed
            || c.cursor < c.backfill_end
            || c.last_sent.is_none_or(|t| now >= t + BATCH_INTERVAL);
        if !due {
            let at = c.last_sent.map_or(now, |t| t + BATCH_INTERVAL);
            return Step::Wait(Some(at.saturating_duration_since(now)));
        }
        // Whole lines while they fit into `BATCH_JSON`; a line that does not fit into an empty
        // batch either is split. At most about 8 KiB of text is handled here, under the lock.
        let mut lines: Vec<LogLineDto> = Vec::new();
        let (mut used, mut full) = (0usize, false);
        loop {
            if lines.len() >= BATCH_LINES {
                full = true;
                break;
            }
            let (held, offset, part, resumed) = match c.partial.take() {
                Some(p) => (p.held, p.offset, p.part, true),
                None if c.cursor < run.next_seq && c.cursor >= first => {
                    (run.at(c.cursor).clone(), 0, 0, false)
                }
                None => break,
            };
            let sep = usize::from(!lines.is_empty());
            let head = LogLineDto::piece(&held.line, offset..offset, part, false);
            let base = serde_json::to_string(&head).map_or(BATCH_JSON, |j| j.len());
            let rest = &held.line.message[offset..];
            let (len, json) = fitting_prefix(rest, BATCH_JSON.saturating_sub(used + sep + base));
            if len == rest.len() {
                lines.push(LogLineDto::piece(
                    &held.line,
                    offset..offset + len,
                    part,
                    false,
                ));
                used += sep + base + json;
                c.cursor = held.line.seq + 1;
                continue;
            }
            // It does not fit whole. One that fits an empty batch, or a part that would be tiny,
            // waits for the next batch.
            let alone = fitting_prefix(rest, BATCH_JSON.saturating_sub(base)).0 == rest.len();
            if !lines.is_empty() && (alone || len < MIN_PIECE) {
                if resumed {
                    c.partial = Some(Partial { held, offset, part });
                }
                full = true;
                break;
            }
            lines.push(LogLineDto::piece(
                &held.line,
                offset..offset + len,
                part,
                true,
            ));
            c.cursor = held.line.seq;
            c.partial = Some(Partial {
                held,
                offset: offset + len,
                part: part + 1,
            });
            full = true;
            break;
        }
        if lines.is_empty() {
            return Step::Wait(None);
        }
        c.full = full;
        c.last_sent = Some(now);
        let end = (run.closed && c.cursor == run.next_seq && c.partial.is_none())
            .then_some(LogEnd::Complete);
        let (run_id, sink) = (c.run_id, c.sink.clone());
        let ack = self.token(&mut inner.next_batch);
        inner.in_flight.push(ack);
        drop(guard);
        let batch = LogBatch {
            ack,
            run_id,
            dropped,
            lines,
            end,
        };
        match end {
            Some(_) => Step::Last(batch, sink),
            None => Step::Send(batch, sink),
        }
    }

    /// The held lines of a run, oldest first (cheap: one `Arc` clone per line).
    fn snapshot(&self, run_id: RunId) -> Option<(Vec<Arc<Held>>, i64)> {
        let inner = lock(&self.inner);
        let run = inner.run(run_id)?;
        Some((run.lines.iter().cloned().collect(), run.utc_offset))
    }

    /// The held lines of `run_id` that `filter` lets through, as text for the clipboard (see
    /// `LogCopyDto`), with the times of the launcher log file (the UTC offset of the run's start).
    /// The work runs on a blocking thread. One copy at a time: a second one while the first runs
    /// is refused with `Busy` (the web view copies on a click; it can click again).
    pub async fn copy(&self, run_id: RunId, filter: LogFilter) -> Result<LogCopyDto, ErrorDto> {
        if filter.text.chars().count() > FILTER_TEXT_CHARS {
            return Err(invalid_input("text", "the search text is too long"));
        }
        let _running = self.copy_gate.try_acquire().map_err(|_| ErrorDto {
            code: ErrorCode::Busy,
            params: BTreeMap::new(),
            action: None,
            detail: "the log is being copied already".to_string(),
        })?;
        let (held, offset) = self.snapshot(run_id).ok_or_else(unknown_run)?;
        tokio::task::spawn_blocking(move || copy_text(&held, &filter, offset, COPY_BYTES))
            .await
            .map_err(|_| crate::accounts::internal("the task copying the log failed"))
    }
}

/// Sends the batches of consumer `id` until it is replaced, detached or its run's log ends.
async fn flush(log: Arc<LiveLog>, id: u64, wake: Arc<Notify>) {
    loop {
        match log.step(id, Instant::now()) {
            Step::Send(batch, sink) => {
                let token = batch.ack;
                if !send(&sink, batch) {
                    log.send_failed(id, token);
                    return;
                }
            }
            Step::Last(batch, sink) => {
                let token = batch.ack;
                if !send(&sink, batch) {
                    log.send_failed(id, token);
                }
                return;
            }
            Step::Wait(None) => wake.notified().await,
            Step::Wait(Some(d)) => {
                tokio::select! {
                    _ = wake.notified() => {}
                    _ = tokio::time::sleep(d) => {}
                }
            }
            Step::Stop => return,
        }
    }
}

/// `false` when the channel refused the batch (a panicking sink counts as delivered).
fn send(sink: &LogSink, batch: LogBatch) -> bool {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sink(batch)))
        .unwrap_or(Ok(()))
        .is_ok()
}

fn matches(line: &LogLine, f: &LogFilter, needle: &str) -> bool {
    let level = match line.level {
        Some(Level::Fatal | Level::Error) => f.error,
        Some(Level::Warn) => f.warning,
        Some(Level::Info) => f.info,
        Some(Level::Debug | Level::Trace) => f.debug,
        None => f.other,
    };
    level && (needle.is_empty() || line.message.to_lowercase().contains(needle))
}

/// The newest matching lines that fit into `limit` bytes, oldest first, one per line.
fn copy_text(held: &[Arc<Held>], f: &LogFilter, utc_offset: i64, limit: usize) -> LogCopyDto {
    let needle = f.text.to_lowercase();
    let mut picked: Vec<String> = Vec::new();
    let (mut bytes, mut omitted) = (0usize, 0u32);
    for h in held.iter().rev().filter(|h| matches(&h.line, f, &needle)) {
        if omitted > 0 {
            omitted += 1;
            continue;
        }
        let text = hengst_core::game::format_line(&h.line, utc_offset);
        if bytes + text.len() + 1 > limit {
            omitted += 1;
            continue;
        }
        bytes += text.len() + 1;
        picked.push(text);
    }
    picked.reverse();
    let mut text = picked.join("\n");
    if !text.is_empty() {
        text.push('\n');
    }
    LogCopyDto {
        text,
        lines: u32::try_from(picked.len()).unwrap_or(u32::MAX),
        omitted,
    }
}

/// Closes the run's log when the drain task ends, also when it ends without reaching the end of
/// its loop (a panic, the runtime shutting down).
pub struct RunCloser {
    pub log: Arc<LiveLog>,
    pub run_id: RunId,
}

impl Drop for RunCloser {
    fn drop(&mut self) {
        self.log.close_run(self.run_id);
    }
}

#[cfg(test)]
#[path = "log_tests.rs"]
mod tests;
