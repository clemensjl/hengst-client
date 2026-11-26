//! The live log on its own: buffer limits, runs, batches, flow control and copying.

use super::*;
use hengst_core::gamelog::{LogTime, Stream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Weak;

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
    log.start_run(R1, "9.9", 1000, None);
    log
}

fn push_n(log: &LiveLog, run: RunId, from: usize, n: usize) {
    for i in from..from + n {
        log.push(run, line(&format!("line {i}")));
    }
}

type Batches = Arc<Mutex<Vec<LogBatch>>>;

/// A consumer like the web view: it records every batch and, with `ack`, acknowledges it.
fn recorder(log: &Arc<LiveLog>, ack: bool) -> (EventSink<LogBatch>, Batches) {
    let seen: Batches = Arc::default();
    let (kept, weak): (Batches, Weak<LiveLog>) = (seen.clone(), Arc::downgrade(log));
    let sink: EventSink<LogBatch> = Arc::new(move |b: LogBatch| {
        let token = b.ack;
        kept.lock().unwrap().push(b);
        if let (true, Some(log)) = (ack, weak.upgrade()) {
            log.ack(token);
        }
    });
    (sink, seen)
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

/// Checks the stream from `from` on: every line has the next seq, gaps only where `dropped` says
/// so, nothing after the end. Returns the seq after the last line and the dropped total.
fn walk(seen: &[LogBatch], from: u64) -> (u64, u64) {
    let (mut next, mut dropped) = (from, 0);
    for (i, b) in seen.iter().enumerate() {
        assert!(b.lines.len() <= BATCH_LINES, "batch {i} too long");
        next += b.dropped;
        dropped += b.dropped;
        for l in &b.lines {
            assert_eq!(l.seq, next, "batch {i}: gap or duplicate");
            next += 1;
        }
        if b.end.is_some() {
            assert_eq!(i, seen.len() - 1, "a batch after the end");
        }
    }
    (next, dropped)
}

fn messages(seen: &[LogBatch]) -> Vec<String> {
    seen.iter()
        .flat_map(|b| b.lines.iter().map(|l| l.message.clone()))
        .collect()
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
    let held = log.snapshot(R1).unwrap();
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
    let held = log.snapshot(R1).unwrap();
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
    assert_eq!(log.snapshot(R1).unwrap().len(), 1);
}

#[test]
fn only_two_runs_are_kept_and_late_lines_go_to_their_own_run() {
    let log = log_with_run();
    push_n(&log, R1, 1, 3);
    log.start_run(R2, "1.21.11", 2000, None);
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
    assert_eq!(log.snapshot(R1).unwrap()[3].line.message, "late");
    log.start_run(R3, "9.9", 3000, None);
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
    log.start_run(R2, "9.9", 1, None);
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
    log.start_run(R2, "9.9", 2, None);
    log.start_run(R3, "9.9", 3, None);
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

#[tokio::test(flavor = "multi_thread")]
async fn batches_hold_at_most_200_lines_and_wait_for_the_interval() {
    let log = log_with_run();
    let (sink, seen) = recorder(&log, true);
    log.attach(R1, 0, sink).unwrap();
    // The first line after a quiet time goes out at once.
    push_n(&log, R1, 1, 1);
    until("the first batch", || seen.lock().unwrap().len() == 1).await;
    let first = Instant::now();
    // A few more right after it wait for the interval.
    push_n(&log, R1, 2, 5);
    until("the second batch", || seen.lock().unwrap().len() == 2).await;
    assert!(first.elapsed() >= BATCH_INTERVAL - Duration::from_millis(40));
    assert_eq!(seen.lock().unwrap()[1].lines.len(), 5);
    // Many lines at once go out in full batches without waiting.
    push_n(&log, R1, 7, 450);
    until("all lines", || walk(&seen.lock().unwrap(), 1).0 == 457).await;
    let seen = seen.lock().unwrap();
    assert!(seen.iter().all(|b| b.lines.len() <= BATCH_LINES));
    assert!(seen.iter().any(|b| b.lines.len() == BATCH_LINES));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_batch_holds_at_most_256_kib_of_lines() {
    let log = log_with_run();
    let text = "z".repeat(100 * 1024);
    for _ in 0..6 {
        log.push(R1, line(&text));
    }
    log.close_run(R1);
    let (sink, seen) = recorder(&log, true);
    log.attach(R1, 0, sink).unwrap();
    until("the end", || ended(&seen)).await;
    let seen = seen.lock().unwrap();
    assert_eq!(walk(&seen, 1), (7, 0));
    assert!(
        seen.iter().all(|b| b.lines.len() <= 2),
        "{:?}",
        seen.iter().map(|b| b.lines.len()).collect::<Vec<_>>()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_consumer_that_does_not_acknowledge_gets_four_batches_then_a_drop_count() {
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
    assert_eq!(after.dropped, first - (WINDOW * BATCH_LINES) as u64 - 1);
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
    let sink: EventSink<LogBatch> = Arc::new(move |b| {
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
        let held = marker.clone();
        log.attach(
            R1,
            0,
            Arc::new(move |_| {
                let _ = &held;
            }),
        )
        .unwrap();
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
        let now = log.runs()[0].clone();
        let since = if round % 2 == 0 { 0 } else { now.next_seq / 2 };
        let from = since.max(1);
        let (sink, seen) = recorder(&log, true);
        log.attach(R1, since, sink).unwrap();
        if !now.closed {
            mid_stream += 1;
        }
        pusher.join().unwrap();
        until("the end", || ended(&seen)).await;
        let seen = seen.lock().unwrap();
        let (next, dropped) = walk(&seen, from);
        assert_eq!(next, LINES as u64 + 1, "round {round}");
        let received: u64 = seen.iter().map(|b| b.lines.len() as u64).sum();
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
    log.start_run(R1, "9.9", 1, None);
    for (i, reload) in [true, false].into_iter().enumerate() {
        let marker = Arc::new(());
        let held = marker.clone();
        log.attach(
            R1,
            0,
            Arc::new(move |_| {
                let _ = &held;
            }),
        )
        .unwrap();
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
    let held = log.snapshot(R1).unwrap();
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
    let held = log.snapshot(R1).unwrap();
    // 1795445575774 ms is 14:52:55 UTC; one hour east.
    let out = copy_text(&held, &all_levels(""), 3600, COPY_BYTES);
    assert_eq!(out.text, "[15:52:55] [Render thread/INFO]: hello\n");
}

#[test]
fn a_cut_line_is_marked() {
    let log = log_with_run();
    log.push(
        R1,
        line(&format!(
            "{}{}",
            "a".repeat(10),
            hengst_core::game::CUT_MARKER
        )),
    );
    log.push(R1, line("whole"));
    let held = log.snapshot(R1).unwrap();
    let dtos: Vec<LogLineDto> = held.iter().map(|h| LogLineDto::from(&h.line)).collect();
    assert!(dtos[0].cut);
    assert!(!dtos[1].cut);
}

#[test]
fn the_log_file_stays_in_rust_and_runs_carry_no_path() {
    let log = Arc::new(LiveLog::new());
    let file = Path::new("C:\\data\\logs\\game\\9.9-20261126-101010.log");
    log.start_run(R1, "9.9", 1, Some(file));
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
