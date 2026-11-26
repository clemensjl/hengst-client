//! The live log as the web view sees it (`log.rs`). Only redacted game output, never a path, a
//! token or the command line.

use crate::dto::play::RunId;
use hengst_core::game::CUT_MARKER;
use hengst_core::gamelog::{Level, LogLine, LogTime, Stream};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Severity as log4j names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum LogLevelDto {
    Fatal,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum LogStreamDto {
    Stdout,
    Stderr,
}

/// When a line was logged, as precisely as the game said it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LogTimeDto {
    /// A log4j event: milliseconds since the Unix epoch (UTC); the web view shows local time.
    UnixMillis {
        #[ts(type = "number")]
        ms: u64,
    },
    /// The local time of day of a plain line (old versions); no date.
    Clock { hour: u8, minute: u8, second: u8 },
}

/// One line of the game log. `message` may hold line breaks (a multi-line message, or a message
/// followed by its throwable). Text from the game: show it as text, never as markup.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LogLineDto {
    /// Strictly increasing within a run, from 1, without gaps (lines that left the buffer are
    /// counted in `LogBatch::dropped` instead).
    #[ts(type = "number")]
    pub seq: u64,
    pub time: Option<LogTimeDto>,
    pub level: Option<LogLevelDto>,
    pub thread: Option<String>,
    pub message: String,
    pub stream: LogStreamDto,
    /// A raw output line of it was longer than 8 KiB and was cut; the message holds the core's
    /// marker (`[line cut: longer than 8 KiB]`) where the cut is.
    pub cut: bool,
}

impl From<&LogLine> for LogLineDto {
    fn from(l: &LogLine) -> LogLineDto {
        LogLineDto {
            seq: l.seq,
            time: l.time.map(|t| match t {
                LogTime::UnixMillis(ms) => LogTimeDto::UnixMillis { ms },
                LogTime::Clock {
                    hour,
                    minute,
                    second,
                } => LogTimeDto::Clock {
                    hour,
                    minute,
                    second,
                },
            }),
            level: l.level.map(|v| match v {
                Level::Fatal => LogLevelDto::Fatal,
                Level::Error => LogLevelDto::Error,
                Level::Warn => LogLevelDto::Warn,
                Level::Info => LogLevelDto::Info,
                Level::Debug => LogLevelDto::Debug,
                Level::Trace => LogLevelDto::Trace,
            }),
            thread: l.thread.clone(),
            message: l.message.clone(),
            stream: match l.stream {
                Stream::Stdout => LogStreamDto::Stdout,
                Stream::Stderr => LogStreamDto::Stderr,
            },
            cut: l.message.contains(CUT_MARKER),
        }
    }
}

/// Why a run's batch stream ends. Nothing follows a batch that carries it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum LogEnd {
    /// The game closed its output: every line of the run has been sent (or counted as dropped);
    /// the buffer is final. The exit code is in `AppEvent::GameState`.
    Complete,
    /// The run left the buffer (two newer runs started); its log can no longer be shown.
    Expired,
}

/// Lines of one run on the channel of `log_attach`: first what the buffer holds from `sinceSeq`
/// on, then new lines as they come, at most every 100 ms or 200 lines at a time.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LogBatch {
    /// Hand it back with `log_ack` once the batch arrived: at most four batches with lines are on
    /// their way at a time, the next one waits for an ack (a page that stops reading gets nothing
    /// more; what leaves the buffer meanwhile is counted in `dropped`).
    #[ts(type = "number")]
    pub ack: u64,
    pub run_id: RunId,
    /// Lines that left the buffer before they could be sent, right before `lines[0]` (or before
    /// the next batch when `lines` is empty). The UI says so instead of showing a silent gap.
    #[ts(type = "number")]
    pub dropped: u64,
    pub lines: Vec<LogLineDto>,
    pub end: Option<LogEnd>,
}

/// A run whose log the app holds: the current one and the one before it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LogRunDto {
    pub run_id: RunId,
    pub instance: String,
    /// Unix milliseconds of the start (as `GameStateDto::Running::since`).
    #[ts(type = "number")]
    pub since: u64,
    /// The oldest line still held; lines before it were dropped (more than 10,000 lines or 16 MiB).
    #[ts(type = "number")]
    pub first_seq: u64,
    /// The seq the next line will get: the buffer holds `firstSeq..nextSeq`.
    #[ts(type = "number")]
    pub next_seq: u64,
    /// The game closed its output; no more lines come.
    pub closed: bool,
}

/// Which lines `log_copy` copies. The levels are the five groups of the log view's filter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogFilter {
    /// ERROR and FATAL.
    pub error: bool,
    /// WARN.
    pub warning: bool,
    /// INFO.
    pub info: bool,
    /// DEBUG and TRACE.
    pub debug: bool,
    /// Lines without a level.
    pub other: bool,
    /// Only lines whose message contains this text, ignoring case; empty: all lines. At most 256
    /// characters; plain text, no pattern.
    pub text: String,
}

/// What `log_copy` made for the clipboard.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LogCopyDto {
    /// The lines in the form of the launcher log file (`[hh:mm:ss] [thread/LEVEL]: message`, local
    /// time), one per line, oldest first.
    pub text: String,
    pub lines: u32,
    /// Matching lines left out because the copy is limited to 4 MiB: the oldest ones (the end of a
    /// log is what explains a crash).
    pub omitted: u32,
}
