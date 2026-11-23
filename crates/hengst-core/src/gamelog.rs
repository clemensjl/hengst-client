//! The game's console output as log lines: log4j XML events (what the game prints with the log
//! configuration the launcher passes, `<log4j:Event ...>` over several lines with a CDATA message
//! and an optional throwable) and the plain lines of old versions (`[hh:mm:ss] [thread/LEVEL]: msg`).
//!
//! The parser sees lines that `game` has already redacted and cut to 8 KiB. Whatever it cannot
//! read degrades to a plain message line: it never panics and never drops text.

use regex::Regex;
use std::sync::OnceLock;

/// Severity of a log line, as log4j names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    Fatal,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl Level {
    /// The level for a log4j level name (`INFO`, `WARN`, ...), exactly as log4j writes it.
    pub fn parse(name: &str) -> Option<Level> {
        Some(match name {
            "FATAL" => Level::Fatal,
            "ERROR" => Level::Error,
            "WARN" => Level::Warn,
            "INFO" => Level::Info,
            "DEBUG" => Level::Debug,
            "TRACE" => Level::Trace,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Level::Fatal => "FATAL",
            Level::Error => "ERROR",
            Level::Warn => "WARN",
            Level::Info => "INFO",
            Level::Debug => "DEBUG",
            Level::Trace => "TRACE",
        }
    }
}

/// Which pipe of the game a line came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stream {
    Stdout,
    Stderr,
}

/// When a line was logged, as precisely as the game said it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogTime {
    /// A log4j event's `timestamp`: milliseconds since the Unix epoch (UTC).
    UnixMillis(u64),
    /// The local time of day of a plain line; plain lines carry no date.
    Clock { hour: u8, minute: u8, second: u8 },
}

/// One line of the game log as the launcher shows it. `message` may span several lines (a log4j
/// message with line breaks, or a message followed by its throwable).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogLine {
    /// Strictly increasing over both streams of one game run, starting at 1.
    pub seq: u64,
    pub time: Option<LogTime>,
    pub level: Option<Level>,
    pub thread: Option<String>,
    pub message: String,
    pub stream: Stream,
}

/// What the parser reads from the output; `game` numbers it and adds the stream to make a
/// `LogLine`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub time: Option<LogTime>,
    pub level: Option<Level>,
    pub thread: Option<String>,
    pub message: String,
}

impl Entry {
    fn text(message: &str) -> Entry {
        Entry {
            time: None,
            level: None,
            thread: None,
            message: message.to_string(),
        }
    }

    pub fn into_line(self, seq: u64, stream: Stream) -> LogLine {
        LogLine {
            seq,
            time: self.time,
            level: self.level,
            thread: self.thread,
            message: self.message,
            stream,
        }
    }
}

const EVENT_START: &str = "<log4j:Event";
const EVENT_END: &str = "</log4j:Event>";

/// An event still open after this many bytes or lines is given up: its lines come out as plain
/// lines, so memory stays bounded when the closing tag never comes.
const MAX_EVENT_BYTES: usize = 512 * 1024;
const MAX_EVENT_LINES: usize = 4096;

/// Turns the lines of one stream into entries. Feed it every line (without the line break) in
/// order with `push`, and call `finish` when the stream ends.
#[derive(Debug, Default)]
pub struct Parser {
    /// Lines of a log4j event whose closing tag has not come yet.
    pending: Vec<String>,
    pending_bytes: usize,
}

impl Parser {
    pub fn new() -> Parser {
        Parser::default()
    }

    /// The entries `line` completes: none while a log4j event is still open, one for a plain
    /// line or a closed event, several when an unreadable event degrades to plain lines.
    pub fn push(&mut self, line: &str) -> Vec<Entry> {
        let mut out = Vec::new();
        self.push_into(line, &mut out);
        out
    }

    /// Whatever is still open, as plain lines (the stream has ended).
    pub fn finish(&mut self) -> Vec<Entry> {
        let mut out = Vec::new();
        self.flush_raw(&mut out);
        out
    }

    fn push_into(&mut self, line: &str, out: &mut Vec<Entry>) {
        let starts_event = line.trim_start().starts_with(EVENT_START);
        if starts_event {
            // An event that never closed (its last line was cut, say) ends where the next begins.
            self.flush_raw(out);
        } else if self.pending.is_empty() {
            out.push(parse_plain(line));
            return;
        }
        if let Some(i) = line.find(EVENT_END) {
            let (event, rest) = line.split_at(i + EVENT_END.len());
            self.pending.push(event.to_string());
            self.close(out);
            if !rest.trim().is_empty() {
                self.push_into(rest, out);
            }
            return;
        }
        self.pending_bytes += line.len() + 1;
        self.pending.push(line.to_string());
        if self.pending_bytes > MAX_EVENT_BYTES || self.pending.len() > MAX_EVENT_LINES {
            self.flush_raw(out);
        }
    }

    fn close(&mut self, out: &mut Vec<Entry>) {
        match parse_event(&self.pending.join("\n")) {
            Some(entry) => {
                out.push(entry);
                self.pending.clear();
                self.pending_bytes = 0;
            }
            None => self.flush_raw(out),
        }
    }

    fn flush_raw(&mut self, out: &mut Vec<Entry>) {
        out.extend(self.pending.drain(..).map(|l| parse_plain(&l)));
        self.pending_bytes = 0;
    }
}

fn plain_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // `[12:34:56] [Render thread/INFO]: msg`; Forge adds its logger: `[main/INFO] [FML]: msg`.
        Regex::new(
            r"^\[([0-9]{2}):([0-9]{2}):([0-9]{2})\] \[(.+?)/(FATAL|ERROR|WARN|INFO|DEBUG|TRACE)\](?: \[([^\]]*)\])?: ?(.*)$",
        )
        .expect("valid pattern")
    })
}

/// A plain line, with time, thread and level when it has the shape of old versions' output.
fn parse_plain(line: &str) -> Entry {
    let Some(c) = plain_pattern().captures(line) else {
        return Entry::text(line);
    };
    let num = |i: usize| c[i].parse::<u8>().unwrap_or(u8::MAX);
    let (hour, minute, second) = (num(1), num(2), num(3));
    if hour > 23 || minute > 59 || second > 60 {
        return Entry::text(line);
    }
    let message = match c.get(6) {
        Some(logger) => format!("[{}] {}", logger.as_str(), &c[7]),
        None => c[7].to_string(),
    };
    Entry {
        time: Some(LogTime::Clock {
            hour,
            minute,
            second,
        }),
        level: Level::parse(&c[5]),
        thread: Some(c[4].to_string()),
        message,
    }
}

/// A complete `<log4j:Event ...>...</log4j:Event>`, or `None` when it cannot be read.
fn parse_event(text: &str) -> Option<Entry> {
    let text = text.trim_start();
    let open_end = text.find('>')?;
    let attrs = attributes(text.get(EVENT_START.len()..open_end)?);
    let body = &text[open_end + 1..];
    let attr = |name: &str| {
        attrs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
    };
    let (message, after) = element(body, "log4j:Message")?;
    let throwable = element(after, "log4j:Throwable").map(|(t, _)| t);
    let mut message = message;
    if let Some(t) = throwable {
        let t = t.trim_end_matches(['\n', '\r']);
        if !t.is_empty() {
            if !message.is_empty() {
                message.push('\n');
            }
            message.push_str(t);
        }
    }
    Some(Entry {
        time: attr("timestamp")
            .and_then(|t| t.parse().ok())
            .map(LogTime::UnixMillis),
        level: attr("level").as_deref().and_then(Level::parse),
        thread: attr("thread"),
        message,
    })
}

/// `name="value"` pairs of a start tag, values unescaped. Stops at the first thing that is not a
/// well-formed attribute.
fn attributes(mut s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    loop {
        s = s.trim_start();
        let Some(eq) = s.find('=') else { break };
        let name = s[..eq].trim();
        let rest = s[eq + 1..].trim_start();
        let Some(quote) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            break;
        };
        let Some(end) = rest[1..].find(quote) else {
            break;
        };
        out.push((name.to_string(), unescape(&rest[1..1 + end])));
        s = &rest[1 + end + 1..];
    }
    out
}

/// The text content of the first `<name>` element in `s` (CDATA sections taken as they are,
/// other text unescaped) and what follows its end tag; `None` without a complete element.
fn element<'a>(s: &'a str, name: &str) -> Option<(String, &'a str)> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let mut rest = &s[s.find(&open)? + open.len()..];
    let mut content = String::new();
    loop {
        if let Some(r) = rest.strip_prefix(&close) {
            return Some((content, r));
        }
        if let Some(r) = rest.strip_prefix("<![CDATA[") {
            let end = r.find("]]>")?;
            content.push_str(&r[..end]);
            rest = &r[end + 3..];
            continue;
        }
        // Text up to the next markup.
        let next = rest.find('<')?;
        if next == 0 {
            // Markup that is neither CDATA nor the end tag.
            return None;
        }
        content.push_str(&unescape(&rest[..next]));
        rest = &rest[next..];
    }
}

/// XML character references and the five predefined entities; anything else stays as written.
fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp..];
        let decoded = after.find(';').filter(|&semi| semi <= 10).and_then(|semi| {
            let name = &after[1..semi];
            let c = match name {
                "lt" => Some('<'),
                "gt" => Some('>'),
                "amp" => Some('&'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ => name
                    .strip_prefix("#x")
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .or_else(|| name.strip_prefix('#').and_then(|d| d.parse().ok()))
                    .and_then(char::from_u32),
            };
            c.map(|c| (c, semi + 1))
        });
        match decoded {
            Some((c, len)) => {
                out.push(c);
                rest = &after[len..];
            }
            None => {
                out.push('&');
                rest = &after[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_all(text: &str) -> Vec<Entry> {
        let mut p = Parser::new();
        let mut out: Vec<Entry> = text.lines().flat_map(|l| p.push(l)).collect();
        out.extend(p.finish());
        out
    }

    /// Recorded from a real start of Fabric 1.21.11 through the launcher (stdout of the game
    /// with the version's log4j configuration, captured 2026-11-23 in the companion-mod work).
    /// Scrubbed: the temp folder below the user profile is `C:/Temp/`, the offline name is
    /// `Mira_K`, line ends are LF. Everything else is as the game printed it.
    const RECORDED: &str = include_str!("../tests/fixtures/gamelog/log4j-1.21.11-fabric.txt");

    #[test]
    fn parses_log4j_xml_events_across_lines() {
        let entries = parse_all(RECORDED);
        assert_eq!(entries.len(), 16, "{entries:#?}");
        // Every entry is a parsed event: nothing degraded to raw XML.
        assert!(
            entries.iter().all(|e| e.level.is_some()
                && e.thread.is_some()
                && matches!(e.time, Some(LogTime::UnixMillis(_)))
                && !e.message.contains("log4j:")),
            "{entries:#?}"
        );
        assert_eq!(
            entries[0],
            Entry {
                time: Some(LogTime::UnixMillis(1_795_445_568_746)),
                level: Some(Level::Info),
                thread: Some("main".into()),
                message: "Loading Minecraft 1.21.11 with Fabric Loader 0.20.1".into(),
            }
        );
        // A message over many lines keeps its line breaks and tabs.
        assert!(entries[1]
            .message
            .starts_with("Loading 50 mods:\n\t- fabric-api 0.141.6+1.21.11\n\t   |-- "));
        assert!(entries[1].message.ends_with("\n\t- minecraft 1.21.11"));
        assert_eq!(entries[1].message.lines().count(), 51);
        let user = entries
            .iter()
            .find(|e| e.message == "Setting user: Mira_K")
            .unwrap();
        assert_eq!(user.thread.as_deref(), Some("Render thread"));
        // `]` right before the CDATA end belongs to the message.
        assert!(entries.iter().any(|e| e
            .message
            .ends_with("profilesHost=https://api.mojang.com, name=PROD]")));
        let warn = entries
            .iter()
            .filter(|e| e.level == Some(Level::Warn))
            .count();
        assert_eq!(warn, 1);
        // Errors carry their throwable after the message.
        let errors: Vec<&Entry> = entries
            .iter()
            .filter(|e| e.level == Some(Level::Error))
            .collect();
        assert_eq!(errors.len(), 2);
        assert!(errors[0].message.starts_with(
            "Failed to fetch user properties\ncom.mojang.authlib.exceptions.InvalidCredentialsException: Status: 401\n\tat "
        ));
        assert!(
            errors[0].message.ends_with("\t... 5 more"),
            "{}",
            errors[0].message
        );
        assert_eq!(errors[1].thread.as_deref(), Some("Download-2"));
        assert!(errors[1]
            .message
            .ends_with("at java.base/java.lang.Thread.run(Thread.java:1583)"));
    }

    #[test]
    fn parses_plain_lines_of_old_versions() {
        // The shape of the console output of versions without the XML log configuration
        // (written from the format, not recorded).
        let lines = parse_all(
            "[12:34:56] [Client thread/INFO]: Setting user: Mira_K\n\
             [23:59:60] [Server thread/WARN]: Can't keep up!\n\
             [00:00:01] [main/INFO] [FML]: Forge Mod Loader version 11.15.1\n\
             [08:15:00] [Thread/with/slash/ERROR]: boom\n\
             [08:15:00] [main/INFO]:\n\
             \tat net.minecraft.client.Main.main(Main.java:1)\n\
             [25:00:00] [main/INFO]: not a time\n\
             [12:00:00] [main/VERBOSE]: unknown level\n\
             2013-07-01 12:00:00 [INFO] even older\n",
        );
        let clock = |hour, minute, second| {
            Some(LogTime::Clock {
                hour,
                minute,
                second,
            })
        };
        assert_eq!(
            lines[0],
            Entry {
                time: clock(12, 34, 56),
                level: Some(Level::Info),
                thread: Some("Client thread".into()),
                message: "Setting user: Mira_K".into(),
            }
        );
        assert_eq!(lines[1].level, Some(Level::Warn));
        assert_eq!(lines[1].time, clock(23, 59, 60));
        assert_eq!(lines[2].message, "[FML] Forge Mod Loader version 11.15.1");
        assert_eq!(lines[2].thread.as_deref(), Some("main"));
        assert_eq!(lines[3].thread.as_deref(), Some("Thread/with/slash"));
        assert_eq!(lines[3].level, Some(Level::Error));
        assert_eq!(lines[4].message, "");
        // Anything else is kept as a plain message, unchanged.
        for (i, text) in [
            (5, "\tat net.minecraft.client.Main.main(Main.java:1)"),
            (6, "[25:00:00] [main/INFO]: not a time"),
            (7, "[12:00:00] [main/VERBOSE]: unknown level"),
            (8, "2013-07-01 12:00:00 [INFO] even older"),
        ] {
            assert_eq!(lines[i], Entry::text(text));
        }
        assert_eq!(lines.len(), 9);
    }

    #[test]
    fn xml_details_escapes_and_split_cdata() {
        let e = parse_all(
            "<log4j:Event logger=\"a\" timestamp=\"5\" level=\"DEBUG\" thread=\"T &amp; &quot;q&quot; &#65;&#x42;\"><log4j:Message><![CDATA[a]]>]]&gt;<![CDATA[b]]></log4j:Message></log4j:Event>",
        );
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].thread.as_deref(), Some("T & \"q\" AB"));
        assert_eq!(e[0].message, "a]]>b");
        assert_eq!(e[0].level, Some(Level::Debug));
        assert_eq!(e[0].time, Some(LogTime::UnixMillis(5)));
        // A throwable without a message line, and a level log4j does not know.
        let e = parse_all(
            "<log4j:Event level=\"LOUD\" thread=\"x\">\n<log4j:Message><![CDATA[]]></log4j:Message>\n<log4j:Throwable><![CDATA[java.lang.Error\n]]></log4j:Throwable>\n</log4j:Event>",
        );
        assert_eq!(e[0].message, "java.lang.Error");
        assert_eq!(e[0].level, None);
        assert_eq!(e[0].time, None);
    }

    #[test]
    fn malformed_events_degrade_to_plain_lines_and_keep_all_text() {
        let input = [
            // No message element.
            "<log4j:Event level=\"INFO\" thread=\"a\">",
            "  <log4j:Other>x</log4j:Other>",
            "</log4j:Event>",
            // Unterminated CDATA, then the next event starts.
            "<log4j:Event level=\"INFO\" thread=\"b\">",
            "<log4j:Message><![CDATA[cut off [line cut: longer than 8 KiB]",
            "<log4j:Event level=\"INFO\" thread=\"c\"><log4j:Message><![CDATA[ok]]></log4j:Message></log4j:Event> trailing text",
            // Never closed before the stream ends.
            "<log4j:Event level=\"WARN\" thread=\"d\">",
            "<log4j:Message><![CDATA[half",
        ];
        let out = parse_all(&input.join("\n"));
        let messages: Vec<&str> = out.iter().map(|e| e.message.as_str()).collect();
        assert_eq!(
            messages,
            [
                input[0],
                input[1],
                input[2],
                input[3],
                input[4],
                "ok",
                " trailing text",
                input[6],
                input[7]
            ]
        );
        assert_eq!(out[5].thread.as_deref(), Some("c"));
    }

    #[test]
    fn an_endless_event_is_given_up_with_bounded_memory() {
        let mut p = Parser::new();
        assert!(p
            .push("<log4j:Event level=\"INFO\" thread=\"a\">")
            .is_empty());
        let line = "x".repeat(8 * 1024);
        let mut emitted = 0;
        for _ in 0..200 {
            emitted += p.push(&line).len();
            assert!(p.pending_bytes <= MAX_EVENT_BYTES + line.len() + 1);
        }
        // Everything came out again, as plain lines.
        emitted += p.finish().len();
        assert_eq!(emitted, 201);
    }

    #[test]
    fn odd_input_never_panics() {
        let mut p = Parser::new();
        for line in [
            "",
            "<",
            "<log4j:Event",
            "<log4j:Event >",
            "<log4j:Event a=\"",
            "<log4j:Event a=b c=\"d\">",
            "</log4j:Event>",
            "<log4j:Event><log4j:Message>&#xD800;&#99999999;&;&amp</log4j:Message></log4j:Event>",
            "[\u{e4}\u{e4}:00:00] [x/INFO]: y",
            "\u{fffd}\u{fffd}",
        ] {
            let _ = p.push(line);
        }
        let _ = p.finish();
        let e = parse_all(
            "<log4j:Event><log4j:Message>&#xD800;&#65;&;&amp</log4j:Message></log4j:Event>",
        );
        assert_eq!(e[0].message, "&#xD800;A&;&amp");
    }
}
