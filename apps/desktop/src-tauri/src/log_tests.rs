//! The live log on its own: buffer limits, runs, batches, flow control and copying.

use super::*;
use hengst_core::gamelog::{LogTime, Stream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Weak;
use tokio::time::Instant;

const WAIT: Duration = Duration::from_secs(30);
const R1: RunId = RunId(1);
const R2: RunId = RunId(2);
const R3: RunId = RunId(3);

fn line(message: &str) -> LogLine {
    LogLine {
        seq: 0,
        time: Some(LogTime::UnixMillis(1_795_445_575_774)),
        level: Some(Level::Info),
        thread: Some("Render thread".into()),
        message: message.to_string(),
        stream: Stream::Stdout,
        cut: false,
    }
}

fn leveled(level: Option<Level>, message: &str) -> LogLine {
    LogLine {
        level,
        ..line(message)
    }
}

fn log_with_run() -> Arc<LiveLog> {
    let log = Arc::new(LiveLog::new());
    log.start_run(R1, "9.9", 1000, None, 0);
    log
}

fn push_n(log: &LiveLog, run: RunId, from: usize, n: usize) {
    for i in from..from + n {
        log.push(run, line(&format!("line {i}")));
    }
}

type Batches = Arc<Mutex<Vec<LogBatch>>>;
type Times = Arc<Mutex<Vec<Instant>>>;

/// A consumer like the web view: it records every batch (and when it was sent) and, with `ack`,
/// acknowledges it.
fn timed_recorder(log: &Arc<LiveLog>, ack: bool) -> (LogSink, Batches, Times) {
    let seen: Batches = Arc::default();
    let times: Times = Arc::default();
    let (kept, at, weak) = (seen.clone(), times.clone(), Arc::downgrade(log));
    let sink: LogSink = Arc::new(move |b: LogBatch| {
        let token = b.ack;
        at.lock().unwrap().push(Instant::now());
        kept.lock().unwrap().push(b);
        if let (true, Some(log)) = (ack, Weak::upgrade(&weak)) {
            log.ack(token);
        }
        Ok(())
    });
    (sink, seen, times)
}

fn recorder(log: &Arc<LiveLog>, ack: bool) -> (LogSink, Batches) {
    let (sink, seen, _) = timed_recorder(log, ack);
    (sink, seen)
}

/// A sink that keeps `marker` alive as long as the consumer holds it.
fn holding(marker: &Arc<()>) -> LogSink {
    let held = marker.clone();
    Arc::new(move |_| {
        let _ = &held;
        Ok(())
    })
}

async fn until(what: &str, done: impl Fn() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < WAIT, "no {what} within {WAIT:?}");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

fn ended(seen: &Batches) -> bool {
    seen.lock().unwrap().iter().any(|b| b.end.is_some())
}

/// Checks the stream from `from` on: every batch is sent inline by Tauri (its JSON is shorter
/// than `INLINE_JSON`), every line has the next seq (the parts of a long line share it, in
/// order), gaps only where `dropped` says so, nothing after the end. Returns the seq after the
/// last line and the dropped total.
fn walk(seen: &[LogBatch], from: u64) -> (u64, u64) {
    let (mut next, mut dropped) = (from, 0);
    let mut open: Option<(u64, u32)> = None;
    for (i, b) in seen.iter().enumerate() {
        let json = serde_json::to_string(b).unwrap().len();
        assert!(json < INLINE_JSON, "batch {i}: {json} bytes of JSON");
        assert!(b.lines.len() <= BATCH_LINES, "batch {i} too long");
        next += b.dropped;
        dropped += b.dropped;
        for l in &b.lines {
            match open {
                Some((seq, part)) => {
                    assert_eq!((l.seq, l.part), (seq, part), "batch {i}: part out of order");
                    assert!(l.thread.is_none() && l.time.is_none(), "batch {i}");
                }
                None => {
                    assert_eq!(l.seq, next, "batch {i}: gap or duplicate");
                    assert_eq!(l.part, 0, "batch {i}");
                }
            }
            open = l.more.then_some((l.seq, l.part + 1));
            if !l.more {
                next = l.seq + 1;
            }
        }
        if b.end.is_some() {
            assert_eq!(i, seen.len() - 1, "a batch after the end");
            if b.end == Some(LogEnd::Complete) {
                assert!(open.is_none(), "the end in the middle of a line");
            }
        }
    }
    (next, dropped)
}

/// The messages of the stream, parts put together.
fn messages(seen: &[LogBatch]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for l in seen.iter().flat_map(|b| b.lines.iter()) {
        if l.part == 0 {
            out.push(l.message.clone());
        } else {
            out.last_mut().unwrap().push_str(&l.message);
        }
    }
    out
}

fn lines_received(seen: &[LogBatch]) -> u64 {
    seen.iter()
        .flat_map(|b| b.lines.iter())
        .filter(|l| !l.more)
        .count() as u64
}

#[test]
fn a_held_line_costs_at_most_the_line_overhead_besides_its_text() {
    // The `Arc` allocation (two counters and `Held`) and the ring's pointer.
    let held = std::mem::size_of::<Held>() + 2 * std::mem::size_of::<usize>();
    assert!(
        held + std::mem::size_of::<usize>() <= LINE_OVERHEAD,
        "{held}"
    );
}

#[test]
fn the_buffer_keeps_the_newest_10000_lines() {
    let log = log_with_run();
    push_n(&log, R1, 1, RUN_LINES + 50);
    let run = &log.runs()[0];
    assert_eq!(run.next_seq, RUN_LINES as u64 + 51);
    assert_eq!(run.first_seq, 51);
    let held = log.snapshot(R1).unwrap().0;
    assert_eq!(held.len(), RUN_LINES);
    assert_eq!(held[0].line.message, "line 51");
    assert_eq!(held[0].line.seq, 51);
}

#[test]
fn the_byte_limit_holds_with_large_log4j_events() {
    let log = log_with_run();
    let big = "x".repeat(512 * 1024);
    for _ in 0..40 {
        log.push(R1, line(&big));
    }
    let held = log.snapshot(R1).unwrap().0;
    let bytes: usize = held.iter().map(|h| h.cost).sum();
    assert!(bytes <= RUN_BYTES, "{bytes}");
    assert_eq!(
        held.len(),
        RUN_BYTES / (big.len() + "Render thread".len() + LINE_OVERHEAD)
    );
    let run = &log.runs()[0];
    assert_eq!(run.next_seq - run.first_seq, held.len() as u64);
    // One line larger than the whole limit still stays (alone), it is not lost on arrival.
    log.push(R1, line(&"y".repeat(RUN_BYTES)));
    assert_eq!(log.snapshot(R1).unwrap().0.len(), 1);
}

#[test]
fn only_two_runs_are_kept_and_late_lines_go_to_their_own_run() {
    let log = log_with_run();
    push_n(&log, R1, 1, 3);
    log.start_run(R2, "1.21.11", 2000, None, 0);
    // A line of the earlier run that arrives after the next run started stays with its run.
    log.push(R1, line("late"));
    push_n(&log, R2, 1, 2);
    let runs = log.runs();
    assert_eq!(
        runs.iter()
            .map(|r| (r.run_id, r.next_seq))
            .collect::<Vec<_>>(),
        [(R2, 3), (R1, 5)]
    );
    assert_eq!(runs[0].instance, "1.21.11");
    assert_eq!(runs[0].since, 2000);
    assert_eq!(log.snapshot(R1).unwrap().0[3].line.message, "late");
    log.start_run(R3, "9.9", 3000, None, 0);
    log.push(R1, line("too late"));
    assert_eq!(
        log.runs().iter().map(|r| r.run_id).collect::<Vec<_>>(),
        [R3, R2]
    );
    assert!(log.snapshot(R1).is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn attaching_to_an_unknown_run_or_a_future_seq_is_a_clean_error() {
    let log = log_with_run();
    push_n(&log, R1, 1, 5);
    let (sink, seen) = recorder(&log, true);
    let e = log.attach(RunId(7), 0, sink.clone()).unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidInput);
    assert_eq!(e.params["field"], "runId");
    let e = log.attach(R1, 7, sink.clone()).unwrap_err();
    assert_eq!(e.params["field"], "sinceSeq");
    let e = log.attach(R1, u64::MAX, sink.clone()).unwrap_err();
    assert_eq!(e.params["field"], "sinceSeq");
    // The next seq itself is fine: nothing to backfill, live from here.
    log.attach(R1, 6, sink).unwrap();
    push_n(&log, R1, 6, 1);
    until("the live line", || !seen.lock().unwrap().is_empty()).await;
    assert_eq!(messages(&seen.lock().unwrap()), ["line 6"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn attach_backfills_then_follows_live_lines_in_order_and_ends_with_the_run() {
    let log = log_with_run();
    push_n(&log, R1, 1, 500);
    let (sink, seen) = recorder(&log, true);
    log.attach(R1, 0, sink).unwrap();
    until("the backfill", || walk(&seen.lock().unwrap(), 1).0 == 501).await;
    push_n(&log, R1, 501, 300);
    log.close_run(R1);
    until("the end", || ended(&seen)).await;
    let seen = seen.lock().unwrap();
    assert_eq!(walk(&seen, 1), (801, 0));
    assert_eq!(seen.last().unwrap().end, Some(LogEnd::Complete));
    assert!(seen.iter().all(|b| b.run_id == R1));
    let msgs = messages(&seen);
    assert_eq!(msgs.first().unwrap(), "line 1");
    assert_eq!(msgs.last().unwrap(), "line 800");
    let first = &seen[0].lines[0];
    assert_eq!(first.thread.as_deref(), Some("Render thread"));
    assert_eq!(first.level, Some(crate::dto::log::LogLevelDto::Info));
}

#[tokio::test(flavor = "multi_thread")]
async fn attaching_to_a_finished_run_sends_its_lines_and_the_end() {
    let log = log_with_run();
    push_n(&log, R1, 1, 3);
    log.close_run(R1);
    let (sink, seen) = recorder(&log, true);
    log.attach(R1, 0, sink).unwrap();
    until("the end", || ended(&seen)).await;
    {
        let seen = seen.lock().unwrap();
        assert_eq!(walk(&seen, 1), (4, 0));
        assert_eq!(seen.last().unwrap().end, Some(LogEnd::Complete));
    }
    // A run without any output ends with an empty batch.
    log.start_run(R2, "9.9", 1, None, 0);
    log.close_run(R2);
    let (sink, empty) = recorder(&log, true);
    log.attach(R2, 0, sink).unwrap();
    until("the end", || ended(&empty)).await;
    let empty = empty.lock().unwrap();
    assert_eq!(empty.len(), 1);
    assert!(empty[0].lines.is_empty());
    assert_eq!(empty[0].end, Some(LogEnd::Complete));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_run_that_leaves_the_buffer_ends_its_stream_as_expired() {
    let log = log_with_run();
    push_n(&log, R1, 1, 2);
    let (sink, seen) = recorder(&log, true);
    log.attach(R1, 0, sink).unwrap();
    until("the backfill", || walk(&seen.lock().unwrap(), 1).0 == 3).await;
    log.start_run(R2, "9.9", 2, None, 0);
    log.start_run(R3, "9.9", 3, None, 0);
    until("the end", || ended(&seen)).await;
    assert_eq!(
        seen.lock().unwrap().last().unwrap().end,
        Some(LogEnd::Expired)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_backfill_after_a_reload_continues_without_gap_or_duplicate() {
    let log = log_with_run();
    push_n(&log, R1, 1, 250);
    let (sink, before) = recorder(&log, true);
    log.attach(R1, 0, sink).unwrap();
    until("the backfill", || walk(&before.lock().unwrap(), 1).0 == 251).await;
    // The page reloads: its consumer is gone; lines go on meanwhile.
    log.page_reset();
    push_n(&log, R1, 251, 120);
    let last_seen = walk(&before.lock().unwrap(), 1).0;
    let (sink, after) = recorder(&log, true);
    log.attach(R1, last_seen, sink).unwrap();
    push_n(&log, R1, 371, 30);
    log.close_run(R1);
    until("the end", || ended(&after)).await;
    let mut all = before.lock().unwrap().clone();
    all.extend(after.lock().unwrap().iter().cloned());
    assert_eq!(walk(&all, 1), (401, 0));
}

/// Paused Tokio time: the interval is measured on the clock the flush task uses.
#[tokio::test(start_paused = true)]
async fn a_batch_that_is_not_full_waits_for_the_interval() {
    let log = log_with_run();
    let (sink, seen, times) = timed_recorder(&log, true);
    log.attach(R1, 0, sink).unwrap();
    // The first line after a quiet time goes out at once.
    push_n(&log, R1, 1, 1);
    until("the first batch", || seen.lock().unwrap().len() == 1).await;
    // A few more right after it wait for the interval.
    push_n(&log, R1, 2, 5);
    until("the second batch", || seen.lock().unwrap().len() == 2).await;
    let t = times.lock().unwrap().clone();
    assert!(t[1] - t[0] >= BATCH_INTERVAL, "{:?}", t[1] - t[0]);
    assert_eq!(seen.lock().unwrap()[1].lines.len(), 5);
    // 200 lines at once are due at once, however recent the last batch.
    let sent = seen.lock().unwrap().len();
    let before = Instant::now();
    push_n(&log, R1, 7, BATCH_LINES);
    until("all lines", || walk(&seen.lock().unwrap(), 1).0 == 207).await;
    let t = times.lock().unwrap().clone();
    assert!(t[sent] - before < BATCH_INTERVAL, "{:?}", t[sent] - before);
    assert!(seen
        .lock()
        .unwrap()
        .iter()
        .all(|b| b.lines.len() <= BATCH_LINES));
}

/// A batch cut by its size is followed at once by the next one (within the credits): a burst of
/// long lines is not paced at one batch per interval.
#[tokio::test(start_paused = true)]
async fn a_batch_cut_by_size_lets_the_next_one_go_at_once() {
    let log = log_with_run();
    let (sink, seen, times) = timed_recorder(&log, true);
    log.attach(R1, 0, sink).unwrap();
    push_n(&log, R1, 1, 1);
    until("the first batch", || seen.lock().unwrap().len() == 1).await;
    // Sixty lines of 1 KiB: far fewer than 200, far more than one batch holds.
    let kib = "k".repeat(1024);
    for _ in 0..60 {
        log.push(R1, line(&kib));
    }
    until("all lines", || walk(&seen.lock().unwrap(), 1).0 == 62).await;
    let t = times.lock().unwrap().clone();
    assert!(t.len() > 4, "{} batches", t.len());
    // At most the one interval after the first batch, then no more waiting.
    let last = *t.last().unwrap() - t[0];
    assert!(
        last < BATCH_INTERVAL * 2,
        "{last:?} for {} batches",
        t.len()
    );
}

/// Every batch is evaluated inline by Tauri, so nothing waits in Tauri's channel queue (which a
/// reloaded page would never fetch): a line too long for one batch comes in parts.
#[tokio::test(flavor = "multi_thread")]
async fn every_batch_is_sent_inline_and_a_long_line_comes_in_parts() {
    let log = log_with_run();
    let escapes: String = (0..3000)
        .map(|i| ['\u{1}', '"', '\\', '\u{e4}', '\n', 'x'][i % 6])
        .collect();
    let msgs: Vec<String> = vec![
        "short".into(),
        escapes,
        "y".repeat(512 * 1024),
        "after".into(),
        "\u{1f984}".repeat(5000),
        String::new(),
    ];
    for m in &msgs {
        log.push(R1, line(m));
    }
    log.close_run(R1);
    let (sink, seen) = recorder(&log, true);
    log.attach(R1, 0, sink).unwrap();
    until("the end", || ended(&seen)).await;
    let seen = seen.lock().unwrap();
    assert_eq!(walk(&seen, 1), (7, 0));
    assert_eq!(messages(&seen), msgs);
    let parts = seen
        .iter()
        .flat_map(|b| &b.lines)
        .filter(|l| l.seq == 3)
        .count();
    assert!(parts > 60, "{parts}");
    // The first part carries time, level and thread.
    let first = seen
        .iter()
        .flat_map(|b| &b.lines)
        .find(|l| l.seq == 3)
        .unwrap();
    assert_eq!(first.thread.as_deref(), Some("Render thread"));
}

#[test]
fn the_batch_budget_leaves_room_for_the_largest_envelope() {
    let empty = LogBatch {
        ack: (1 << 53) - 1,
        run_id: RunId(u64::MAX),
        dropped: u64::MAX,
        lines: Vec::new(),
        // A batch with lines can only end with `Complete`, the longer of the two.
        end: Some(LogEnd::Complete),
    };
    let envelope = serde_json::to_string(&empty).unwrap().len();
    assert!(envelope + BATCH_JSON < INLINE_JSON, "{envelope}");
}

/// `INLINE_JSON` is Tauri's `MAX_JSON_DIRECT_EXECUTE_THRESHOLD` (ipc/channel.rs) of the version
/// in use: a newer Tauri must be checked again before this test is changed.
#[test]
fn the_inline_limit_belongs_to_the_tauri_version_in_use() {
    let lock = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../Cargo.lock"))
        .unwrap()
        .replace("\r\n", "\n");
    assert!(
        lock.contains("name = \"tauri\"\nversion = \"2.12.7\""),
        "Tauri changed: check MAX_JSON_DIRECT_EXECUTE_THRESHOLD and INLINE_JSON"
    );
    assert_eq!(INLINE_JSON, 8192);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_consumer_that_does_not_acknowledge_gets_a_window_of_batches_then_a_drop_count() {
    let log = log_with_run();
    push_n(&log, R1, 1, 2000);
    let (sink, seen) = recorder(&log, false);
    log.attach(R1, 0, sink).unwrap();
    until("the window", || seen.lock().unwrap().len() == WINDOW).await;
    // Nothing more while nothing is acknowledged, however much comes.
    push_n(&log, R1, 2001, RUN_LINES + 2000);
    tokio::time::sleep(BATCH_INTERVAL * 3).await;
    assert_eq!(seen.lock().unwrap().len(), WINDOW);
    // Forged tokens release nothing.
    for token in [0, 1, 2, u64::MAX, (1 << 53) - 1] {
        log.ack(token);
    }
    tokio::time::sleep(BATCH_INTERVAL * 2).await;
    assert_eq!(seen.lock().unwrap().len(), WINDOW);
    assert!(lock(&log.inner).in_flight.len() == WINDOW);
    // The page catches up: the next batch says how many lines it missed.
    let tokens: Vec<u64> = seen.lock().unwrap().iter().map(|b| b.ack).collect();
    for t in tokens {
        log.ack(t);
    }
    until("the next batch", || seen.lock().unwrap().len() > WINDOW).await;
    let seen = seen.lock().unwrap();
    let after = &seen[WINDOW];
    let first = log.runs()[0].first_seq;
    assert_eq!(first, 4001);
    let reached = walk(&seen[..WINDOW], 1).0;
    assert_eq!(after.dropped, first - reached);
    assert_eq!(after.lines[0].seq, first);
    walk(&seen, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_attach_releases_the_old_channel_and_detach_is_idempotent() {
    let log = log_with_run();
    push_n(&log, R1, 1, 10);
    let marker = Arc::new(());
    let (inner, old) = recorder(&log, true);
    let held = marker.clone();
    let sink: LogSink = Arc::new(move |b| {
        let _ = &held;
        inner(b)
    });
    log.attach(R1, 0, sink).unwrap();
    until("the backfill", || !old.lock().unwrap().is_empty()).await;
    let (sink, new) = recorder(&log, true);
    log.attach(R1, 0, sink).unwrap();
    until("the old channel to be dropped", || {
        Arc::strong_count(&marker) == 1
    })
    .await;
    let before = old.lock().unwrap().len();
    push_n(&log, R1, 11, 5);
    until("the new consumer", || walk(&new.lock().unwrap(), 1).0 == 16).await;
    assert_eq!(old.lock().unwrap().len(), before);
    log.detach();
    log.detach();
    push_n(&log, R1, 16, 5);
    tokio::time::sleep(BATCH_INTERVAL * 2).await;
    assert_eq!(walk(&new.lock().unwrap(), 1).0, 16);
    assert!(lock(&log.inner).consumer.is_none());
}

/// Many attaches in a row (a page reloading without detaching): only the last one is served, the
/// others' channels are dropped, and what they never acknowledged stays within the window.
#[tokio::test(flavor = "multi_thread")]
async fn an_attach_storm_leaves_one_consumer_and_at_most_the_window_in_flight() {
    let log = log_with_run();
    push_n(&log, R1, 1, 1000);
    let marker = Arc::new(());
    for _ in 0..500 {
        log.attach(R1, 0, holding(&marker)).unwrap();
        tokio::task::yield_now().await;
    }
    log.detach();
    until("the storm's channels to be dropped", || {
        Arc::strong_count(&marker) == 1
    })
    .await;
    assert!(lock(&log.inner).in_flight.len() <= WINDOW);
    // The storm never acknowledged anything: a page load returns the credits.
    log.page_reset();
    assert!(lock(&log.inner).in_flight.is_empty());
    let (sink, last) = recorder(&log, true);
    log.attach(R1, 0, sink).unwrap();
    until("the backfill", || walk(&last.lock().unwrap(), 1).0 == 1001).await;
}

/// The hand-over race: lines are pushed from another thread while consumers attach, from the
/// oldest line and from a seq in the middle. Every line reaches the consumer exactly once, in
/// order, unless the stream says it was dropped.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn lines_pushed_during_attach_arrive_once_in_order() {
    const LINES: usize = 30_000;
    const ROUNDS: u64 = 10;
    let mut mid_stream = 0;
    for round in 0..ROUNDS {
        let log = log_with_run();
        let go = Arc::new(AtomicBool::new(false));
        let pusher = {
            let (log, go) = (log.clone(), go.clone());
            std::thread::spawn(move || {
                while !go.load(Ordering::SeqCst) {
                    std::hint::spin_loop();
                }
                for i in 1..=LINES {
                    log.push(R1, line(&format!("line {i}")));
                    if i % 1000 == 0 {
                        std::thread::yield_now();
                    }
                }
                log.close_run(R1);
            })
        };
        go.store(true, Ordering::SeqCst);
        // Attach at a different moment each round, while the pusher runs.
        let target = 1 + round * 2500;
        while log.runs()[0].next_seq < target {
            std::hint::spin_loop();
        }
        let since = if round % 2 == 0 {
            0
        } else {
            log.runs()[0].next_seq / 2
        };
        let from = since.max(1);
        let (sink, seen) = recorder(&log, true);
        log.attach(R1, since, sink).unwrap();
        // Read after the attach returned: lines were still to come.
        if log.runs()[0].next_seq < LINES as u64 + 1 {
            mid_stream += 1;
        }
        pusher.join().unwrap();
        until("the end", || ended(&seen)).await;
        let seen = seen.lock().unwrap();
        let (next, dropped) = walk(&seen, from);
        assert_eq!(next, LINES as u64 + 1, "round {round}");
        let received = lines_received(&seen);
        assert_eq!(received + dropped, LINES as u64 + 1 - from, "round {round}");
        assert_eq!(seen.last().unwrap().end, Some(LogEnd::Complete));
    }
    // The race was real: most attaches came while lines were still being pushed.
    assert!(mid_stream >= ROUNDS / 2, "{mid_stream} of {ROUNDS}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_page_load_and_a_destroyed_window_detach_the_consumer() {
    use tauri::test::{mock_builder, mock_context, noop_assets};
    use tauri::webview::PageLoadEvent;
    use tauri::{Manager, WindowEvent};
    let dir = tempfile::tempdir().unwrap();
    let state = crate::state::AppState::new(
        hengst_core::paths::Paths::new(dir.path()),
        hengst_core::launch::Endpoints::default(),
    );
    let app = mock_builder()
        .manage(state)
        .build(mock_context(noop_assets()))
        .unwrap();
    let log = app.state::<crate::state::AppState>().log.clone();
    log.start_run(R1, "9.9", 1, None, 0);
    for (i, reload) in [true, false].into_iter().enumerate() {
        let marker = Arc::new(());
        log.attach(R1, 0, holding(&marker)).unwrap();
        crate::startup::cancel_operations_on_page_load(app.handle(), PageLoadEvent::Finished);
        crate::startup::cancel_operations_on(app.handle(), &WindowEvent::Focused(true));
        assert!(lock(&log.inner).consumer.is_some(), "{i}");
        if reload {
            crate::startup::cancel_operations_on_page_load(app.handle(), PageLoadEvent::Started);
        } else {
            crate::startup::cancel_operations_on(app.handle(), &WindowEvent::Destroyed);
        }
        assert!(lock(&log.inner).consumer.is_none(), "{i}");
        until("the channel to be dropped", || {
            Arc::strong_count(&marker) == 1
        })
        .await;
    }
}

fn all_levels(text: &str) -> LogFilter {
    LogFilter {
        error: true,
        warning: true,
        info: true,
        debug: true,
        other: true,
        text: text.to_string(),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn copy_filters_by_level_group_and_text() {
    let log = log_with_run();
    for (level, msg) in [
        (Some(Level::Fatal), "fatal one"),
        (Some(Level::Error), "error two"),
        (Some(Level::Warn), "warn Three"),
        (Some(Level::Info), "info four"),
        (Some(Level::Debug), "debug five"),
        (Some(Level::Trace), "trace six"),
        (None, "plain seven"),
    ] {
        log.push(R1, leveled(level, msg));
    }
    let text = |f: LogFilter| {
        let log = log.clone();
        async move { log.copy(R1, f).await.unwrap() }
    };
    let errors = text(LogFilter {
        error: true,
        ..no_levels()
    })
    .await;
    assert_eq!(errors.lines, 2);
    assert!(errors.text.contains("fatal one") && errors.text.contains("error two"));
    let debug_other = text(LogFilter {
        debug: true,
        other: true,
        ..no_levels()
    })
    .await;
    assert_eq!(debug_other.lines, 3);
    // A plain line without time, thread and level is copied as its message.
    assert!(
        debug_other.text.ends_with("\nplain seven\n"),
        "{}",
        debug_other.text
    );
    let search = text(all_levels("THREE")).await;
    assert_eq!(search.lines, 1);
    assert!(
        search.text.contains("[Render thread/WARN]: warn Three"),
        "{}",
        search.text
    );
    // Regex characters are plain text.
    assert_eq!(text(all_levels(".*")).await.lines, 0);
    let none = text(no_levels()).await;
    assert_eq!((none.lines, none.omitted, none.text.as_str()), (0, 0, ""));
}

fn no_levels() -> LogFilter {
    LogFilter {
        error: false,
        warning: false,
        info: false,
        debug: false,
        other: false,
        text: String::new(),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn copy_refuses_a_long_search_text_and_an_unknown_run() {
    let log = log_with_run();
    let e = log
        .copy(R1, all_levels(&"a".repeat(FILTER_TEXT_CHARS + 1)))
        .await
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidInput);
    assert_eq!(e.params["field"], "text");
    assert!(log
        .copy(R1, all_levels(&"ä".repeat(FILTER_TEXT_CHARS)))
        .await
        .is_ok());
    let e = log.copy(RunId(9), all_levels("")).await.unwrap_err();
    assert_eq!(e.params["field"], "runId");
}

#[test]
fn copy_keeps_the_newest_lines_that_fit_and_counts_the_rest() {
    let log = log_with_run();
    for i in 0..10 {
        log.push(R1, leveled(None, &format!("{i:09}")));
    }
    let held = log.snapshot(R1).unwrap().0;
    // Ten bytes per line with its line break: room for three.
    let out = copy_text(&held, &all_levels(""), 0, 35);
    assert_eq!(out.text, "000000007\n000000008\n000000009\n");
    assert_eq!((out.lines, out.omitted), (3, 7));
    let all = copy_text(&held, &all_levels(""), 0, COPY_BYTES);
    assert_eq!((all.lines, all.omitted), (10, 0));
}

#[test]
fn copy_writes_local_time_like_the_launcher_log_file() {
    let log = log_with_run();
    log.push(R1, line("hello"));
    let held = log.snapshot(R1).unwrap().0;
    // 1795445575774 ms is 14:52:55 UTC; one hour east.
    let out = copy_text(&held, &all_levels(""), 3600, COPY_BYTES);
    assert_eq!(out.text, "[15:52:55] [Render thread/INFO]: hello\n");
}

/// The copy uses the offset of the run's start, as the launcher log file does, not the one at
/// the time of the copy.
#[tokio::test(flavor = "multi_thread")]
async fn copy_uses_the_utc_offset_of_the_run_start() {
    let log = Arc::new(LiveLog::new());
    log.start_run(R1, "9.9", 1, None, 2 * 3600);
    log.push(R1, line("hello"));
    let out = log.copy(R1, all_levels("")).await.unwrap();
    assert_eq!(out.text, "[16:52:55] [Render thread/INFO]: hello\n");
}

/// One copy at a time: a second one while the first runs is refused as `busy`.
#[tokio::test(flavor = "multi_thread")]
async fn a_second_copy_while_one_runs_is_refused() {
    let log = log_with_run();
    log.push(R1, line("hello"));
    let running = log.copy_gate.try_acquire().unwrap();
    let e = log.copy(R1, all_levels("")).await.unwrap_err();
    assert_eq!(e.code, ErrorCode::Busy);
    drop(running);
    assert_eq!(log.copy(R1, all_levels("")).await.unwrap().lines, 1);
}

/// `cut` is the core's flag, not a look at the text: chat that quotes the marker is not cut.
#[test]
fn the_cut_flag_comes_from_the_core_not_from_the_text() {
    let log = log_with_run();
    let quoted = format!("chat: {}", hengst_core::game::CUT_MARKER);
    log.push(R1, line(&quoted));
    log.push(
        R1,
        LogLine {
            cut: true,
            ..line("aaaa")
        },
    );
    let held = log.snapshot(R1).unwrap().0;
    let dtos: Vec<LogLineDto> = held.iter().map(|h| LogLineDto::from(&h.line)).collect();
    assert!(!dtos[0].cut);
    assert!(dtos[1].cut);
}

/// The run's game state for the run switcher: running until the backend saw the exit.
#[test]
fn a_run_carries_its_exit_once_it_ended() {
    let log = log_with_run();
    assert_eq!(log.runs()[0].state, LogRunStateDto::Running);
    log.run_ended(R1, 3, true);
    log.run_ended(R2, 1, false);
    assert_eq!(
        log.runs()[0].state,
        LogRunStateDto::Ended {
            code: 3,
            stop_requested: true
        }
    );
    let json = serde_json::to_value(&log.runs()[0]).unwrap();
    assert_eq!(
        json["state"],
        serde_json::json!({ "state": "ended", "code": 3, "stopRequested": true })
    );
}

/// A sink whose channel is gone gives its credit back and ends its consumer: five dead
/// channels in a row do not stall a healthy attach after them.
#[tokio::test(flavor = "multi_thread")]
async fn failed_sends_return_their_credit_and_end_the_consumer() {
    let log = log_with_run();
    push_n(&log, R1, 1, 1000);
    for _ in 0..5 {
        log.attach(R1, 0, Arc::new(|_| Err(()))).unwrap();
        until("the dead consumer to end", || {
            lock(&log.inner).consumer.is_none()
        })
        .await;
        assert!(lock(&log.inner).in_flight.is_empty());
    }
    let (sink, seen) = recorder(&log, true);
    log.attach(R1, 0, sink).unwrap();
    until("the backfill", || walk(&seen.lock().unwrap(), 1).0 == 1001).await;
}

/// All credits are out (the page stopped reading) and the run leaves the buffer: the end still
/// arrives, because a batch without lines needs no credit.
#[tokio::test(flavor = "multi_thread")]
async fn an_expiry_reaches_a_consumer_without_credits() {
    let log = log_with_run();
    push_n(&log, R1, 1, 2000);
    let (sink, seen) = recorder(&log, false);
    log.attach(R1, 0, sink).unwrap();
    until("the window", || seen.lock().unwrap().len() == WINDOW).await;
    assert_eq!(lock(&log.inner).in_flight.len(), WINDOW);
    log.start_run(R2, "9.9", 2, None, 0);
    log.start_run(R3, "9.9", 3, None, 0);
    until("the end", || ended(&seen)).await;
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), WINDOW + 1);
    assert_eq!(seen[WINDOW].end, Some(LogEnd::Expired));
    assert!(seen[WINDOW].lines.is_empty());
}

#[test]
fn the_log_file_stays_in_rust_and_runs_carry_no_path() {
    let log = Arc::new(LiveLog::new());
    let file = Path::new("C:\\data\\logs\\game\\9.9-20261126-101010.log");
    log.start_run(R1, "9.9", 1, Some(file), 0);
    assert_eq!(log.log_file(R1).as_deref(), Some(file));
    assert_eq!(log.log_file(R2), None);
    let json = serde_json::to_string(&log.runs()).unwrap();
    assert!(!json.contains("logs") && !json.contains("data"), "{json}");
}

/// Not a check: how long the drain path takes per line (`cargo test -- --ignored push_cost
/// --nocapture`).
#[test]
#[ignore]
fn push_cost() {
    let log = log_with_run();
    let lines: Vec<LogLine> = (0..200_000)
        .map(|i| {
            line(&format!(
                "[12:00:00] [Render thread/INFO]: a typical line of the game {i}"
            ))
        })
        .collect();
    let start = Instant::now();
    for l in lines {
        log.push(R1, l);
    }
    let per = start.elapsed() / 200_000;
    println!("push: {per:?} per line");
}
