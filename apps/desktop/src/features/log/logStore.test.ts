import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { LogBatch } from "@/ipc/bindings/LogBatch";
import { batch, line } from "@/test/logFixtures";
import {
  ALL_LEVELS,
  attachTo,
  detach,
  EMPTY_FEED,
  followAction,
  frontShift,
  hitIndex,
  hitPrefix,
  hitRanges,
  ingest,
  initialIndex,
  locateHit,
  resetLog,
  searchPattern,
  useLog,
  visibleRows,
  type LineRow,
  type Row,
} from "./logStore";

const messages = (rows: readonly Row[]) =>
  rows.map((r) => (r.kind === "line" ? r.message : `gap ${r.count}${r.older ? " older" : ""}`));

describe("ingest", () => {
  it("joins the parts of one seq, also across batches, and shows a line only once it is whole", () => {
    let feed = ingest(EMPTY_FEED, [
      batch([line(1), line(2, "ab", { more: true, level: "warn", thread: "Render" })]),
    ]);
    expect(messages(feed.rows)).toEqual(["line 1"]);
    expect(feed.lastSeq).toBe(1);
    feed = ingest(feed, [
      batch([line(2, "cd", { part: 1, more: true, level: null, thread: null })]),
    ]);
    feed = ingest(feed, [batch([line(2, "ef", { part: 2, level: null, thread: null }), line(3)])]);
    expect(messages(feed.rows)).toEqual(["line 1", "abcdef", "line 3"]);
    const joined = feed.rows[1] as LineRow;
    // Only part 0 carries level and thread.
    expect([joined.level, joined.thread, joined.group]).toEqual(["warn", "Render", "warning"]);
    expect(feed.lastSeq).toBe(3);
  });

  it("a part that does not continue the open line is ignored, a cut part marks the line", () => {
    const feed = ingest(EMPTY_FEED, [
      batch([
        line(4, "orphan", { part: 2 }),
        line(5, "a", { more: true }),
        line(5, "b", { part: 1, cut: true }),
      ]),
    ]);
    expect(messages(feed.rows)).toEqual(["ab"]);
    expect((feed.rows[0] as LineRow).cut).toBe(true);
  });

  it("keeps the order by seq without duplicates", () => {
    const feed = ingest(EMPTY_FEED, [
      batch([line(1), line(2)]),
      batch([line(2), line(1), line(3)]),
    ]);
    expect(messages(feed.rows)).toEqual(["line 1", "line 2", "line 3"]);
  });

  it("shows dropped lines as a gap before the batch's lines, also in a batch without lines", () => {
    let feed = ingest(EMPTY_FEED, [batch([line(5)], { dropped: 4 })]);
    expect(messages(feed.rows)).toEqual(["gap 4", "line 5"]);
    feed = ingest(feed, [batch([], { dropped: 2 }), batch([], { dropped: 1 })]);
    feed = ingest(feed, [batch([line(9)])]);
    // Two gaps in a row are one note; the ord keeps rows in order.
    expect(messages(feed.rows)).toEqual(["gap 4", "line 5", "gap 3", "line 9"]);
    const ords = feed.rows.map((r) => r.ord);
    expect([...ords].sort((a, b) => a - b)).toEqual(ords);
  });

  it("holds at most the limit; the oldest go and are counted in one leading note", () => {
    let feed = ingest(EMPTY_FEED, [batch([line(1), line(2)], { dropped: 0 })], 3);
    feed = ingest(feed, [batch([line(5)], { dropped: 2 })], 3);
    feed = ingest(feed, [batch([line(6), line(7)])], 3);
    // 1, 2 and the gap of 2 before 5 are gone: 4 lines in the note.
    expect(messages(feed.rows)).toEqual(["gap 4 older", "line 5", "line 6", "line 7"]);
    expect(feed.lines).toBe(3);
    feed = ingest(feed, [batch([line(8)])], 3);
    expect(messages(feed.rows)).toEqual(["gap 5 older", "line 6", "line 7", "line 8"]);
    const ords = feed.rows.map((r) => r.ord);
    expect(new Set(ords).size).toBe(ords.length);
    expect([...ords].sort((a, b) => a - b)).toEqual(ords);
  });

  it("10,000 lines stay 10,000 when more come", () => {
    const lines = Array.from({ length: 10_050 }, (_, i) => line(i + 1));
    const feed = ingest(EMPTY_FEED, [batch(lines)]);
    expect(feed.lines).toBe(10_000);
    expect(feed.rows).toHaveLength(10_001);
    expect(messages(feed.rows.slice(0, 2))).toEqual(["gap 50 older", "line 51"]);
  });
});

describe("filter and search", () => {
  const rows = ingest(EMPTY_FEED, [
    batch([
      line(1, "Error: Boom", { level: "error" }),
      line(2, "warn x", { level: "warn" }),
      line(3, "boom boom", { level: "debug" }),
      line(4, "plain (a+b)", { level: null }),
    ]),
    batch([], { dropped: 1 }),
  ]).rows;

  it("lets the chosen levels through and always shows gaps", () => {
    const shown = visibleRows(rows, { ...ALL_LEVELS, debug: false, other: false });
    expect(messages(shown)).toEqual(["Error: Boom", "warn x", "gap 1"]);
    expect(visibleRows(rows, ALL_LEVELS)).toBe(rows);
  });

  it("finds plain text ignoring case, pattern characters stay literal", () => {
    expect(hitRanges("a+b A+B", searchPattern("a+b")!)).toEqual([
      [0, 3],
      [4, 7],
    ]);
    expect(hitPrefix(rows, searchPattern("(a+b)"))[rows.length]).toBe(1);
    expect(hitPrefix(rows, searchPattern(""))[rows.length]).toBe(0);
  });

  it("counts every match and steps through them with wrap-around", () => {
    const prefix = hitPrefix(rows, searchPattern("BOOM"));
    expect(prefix[rows.length]).toBe(3);
    expect(locateHit(prefix, 0)).toEqual({ index: 0, nth: 0 });
    expect(locateHit(prefix, 2)).toEqual({ index: 2, nth: 1 });
    // The current match is kept by row and occurrence; a row that lost its match falls forward.
    expect(hitIndex(rows, prefix, { ord: rows[2].ord, nth: 1 })).toBe(2);
    expect(hitIndex(rows, prefix, { ord: rows[1].ord, nth: 0 })).toBe(1);
    expect(hitIndex(rows, prefix, { ord: 999, nth: 0 })).toBe(0);
    expect(hitIndex(rows, prefix, null)).toBe(0);
  });

  it("a current match whose row left the front falls forward to the first remaining match", () => {
    let feed = ingest(EMPTY_FEED, [batch([line(1, "boom"), line(2, "x"), line(3, "boom")])], 3);
    const current = { ord: feed.rows[0].ord, nth: 0 };
    feed = ingest(feed, [batch([line(4, "boom")])], 3);
    // Now: older note, line 2, line 3 ("boom"), line 4 ("boom").
    const prefix = hitPrefix(feed.rows, searchPattern("boom"));
    expect(prefix[feed.rows.length]).toBe(2);
    const k = hitIndex(feed.rows, prefix, current)!;
    expect(feed.rows[locateHit(prefix, k).index]).toMatchObject({ seq: 3 });
  });
});

describe("list position", () => {
  const rows = (seqs: number[], older = 0) =>
    ingest(EMPTY_FEED, [
      batch(
        seqs.map((n) => line(n)),
        { dropped: older },
      ),
    ]).rows;

  it("frontShift counts the rows that left the front, also behind a new leading note", () => {
    const prev = ingest(EMPTY_FEED, [batch([1, 2, 3, 4].map((n) => line(n)))], 3).rows;
    // [older 1, line 2, line 3, line 4]
    const next = ingest(EMPTY_FEED, [batch([1, 2, 3, 4, 5].map((n) => line(n)))], 3).rows;
    // [older 2, line 3, line 4, line 5]: the note is new, line 3 moved from 2 to 1.
    expect(frontShift(prev, next)).toBe(1);
    expect(frontShift(rows([1, 2]), rows([1, 2, 3]))).toBe(0);
    expect(frontShift(rows([1, 2]), rows([8, 9]))).toBeNull();
  });

  it("follows the end after every change while following, also when the count stays the same", () => {
    const at = 1000;
    // At the limit a batch adds as many rows as leave the front: same length, new rows.
    expect(followAction(rows([2, 3]), true, null, at)).toEqual({ kind: "end" });
    expect(followAction(rows([2, 3]), false, null, at)).toBeNull();
  });

  it("keeps scrolling to a match by its row while lines leave the front, for a while", () => {
    const before = rows([1, 2, 3, 4]);
    const target = { ord: before[2].ord, until: 2000 };
    const after = rows([3, 4, 5]);
    expect(followAction(after, false, target, 1500)).toEqual({ kind: "index", index: 0 });
    expect(followAction(after, false, target, 2500)).toBeNull();
    expect(followAction(rows([4, 5]), false, target, 1500)).toBeNull();
  });

  it("a list that starts over (filter change) opens at the end or at the line it showed", () => {
    const list = rows([1, 2, 3, 4]);
    expect(initialIndex(list, true, list[2].ord)).toEqual({ index: "LAST", align: "end" });
    expect(initialIndex(list, false, list[2].ord)).toBe(2);
    // The line it showed is filtered out now: the next one after it.
    expect(initialIndex(list, false, list[2].ord - 1)).toBe(2);
    expect(initialIndex(list, false, null)).toEqual({ index: "LAST", align: "end" });
  });
});

describe("the stream", () => {
  let channels: { onmessage?: (b: LogBatch) => void }[];
  let calls: [string, unknown][];
  let frames: FrameRequestCallback[];
  let attachGate: Promise<void> | null;
  let attachFails: unknown;

  beforeEach(() => {
    resetLog();
    channels = [];
    calls = [];
    frames = [];
    attachGate = null;
    attachFails = null;
    vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => frames.push(cb));
    vi.stubGlobal("cancelAnimationFrame", () => {});
    mockIPC(async (cmd, args) => {
      const a = args as Record<string, unknown>;
      if (cmd === "log_attach") {
        calls.push([cmd, { runId: a.runId, sinceSeq: a.sinceSeq }]);
        channels.push(a.channel as never);
        if (attachGate) await attachGate;
        if (attachFails) throw attachFails;
        return null;
      }
      calls.push([cmd, a]);
      if (cmd === "log_runs") return [];
      return null;
    });
  });
  afterEach(() => {
    resetLog();
    clearMocks();
    vi.unstubAllGlobals();
  });

  const send = (b: LogBatch, n = channels.length - 1) => channels[n].onmessage?.(b);
  const runFrame = () => frames.splice(0).forEach((f) => f(0));
  const settle = () => new Promise((r) => setTimeout(r, 0));

  it("is set up before log_attach, so a batch that comes before its answer counts", async () => {
    let release!: () => void;
    attachGate = new Promise((r) => (release = r));
    attachTo(7);
    expect(useLog.getState().attach).toEqual({ status: "attaching", runId: 7 });
    await settle();
    send(batch([line(1)]));
    runFrame();
    expect(messages(useLog.getState().feed.rows)).toEqual(["line 1"]);
    release();
    await settle();
    expect(useLog.getState().attach.status).toBe("live");
  });

  it("many batches within one frame make one store commit", async () => {
    attachTo(7);
    await settle();
    let commits = 0;
    const off = useLog.subscribe(() => (commits += 1));
    for (let i = 1; i <= 300; i++) send(batch([line(i)]));
    expect(commits).toBe(0);
    runFrame();
    off();
    expect(commits).toBe(1);
    expect(useLog.getState().feed.lines).toBe(300);
  });

  it("without frames (a hidden window) a timer applies the queue", async () => {
    vi.useFakeTimers();
    try {
      attachTo(7);
      // The attach goes out after the call before it has settled.
      await vi.advanceTimersByTimeAsync(1);
      send(batch([line(1)]));
      expect(useLog.getState().feed.lines).toBe(0);
      vi.advanceTimersByTime(300);
      expect(useLog.getState().feed.lines).toBe(1);
    } finally {
      vi.useRealTimers();
    }
  });

  it("drops batches of an earlier channel and of another run", async () => {
    attachTo(7);
    await settle();
    attachTo(7);
    await settle();
    send(batch([line(1, "old channel")]), 0);
    send(batch([line(1, "other run")], { runId: 8 }), 1);
    send(batch([line(1, "current")]), 1);
    runFrame();
    expect(messages(useLog.getState().feed.rows)).toEqual(["current"]);
  });

  it("attaching again continues after the last whole line and throws a half-built one away", async () => {
    attachTo(7);
    await settle();
    send(batch([line(1), line(2), line(3, "hea", { more: true })]));
    runFrame();
    detach();
    attachTo(7);
    await settle();
    expect(calls.filter((c) => c[0] === "log_attach").map((c) => c[1])).toEqual([
      { runId: 7, sinceSeq: 0 },
      { runId: 7, sinceSeq: 3 },
    ]);
    // The backend sends the line from its first part again: it is whole, not just its tail.
    send(batch([line(3, "hea", { more: true }), line(3, "d", { part: 1 })]));
    runFrame();
    expect(messages(useLog.getState().feed.rows)).toEqual(["line 1", "line 2", "head"]);
  });

  it("another run starts empty and from the oldest line", async () => {
    attachTo(7);
    await settle();
    send(batch([line(1)]));
    runFrame();
    attachTo(8);
    await settle();
    expect(useLog.getState().feed.rows).toEqual([]);
    expect(calls.at(-1)).toEqual(["log_attach", { runId: 8, sinceSeq: 0 }]);
  });

  it("detach waits for the attach it follows and is skipped when a newer attach came (StrictMode)", async () => {
    let release!: () => void;
    attachGate = new Promise((r) => (release = r));
    attachTo(7);
    detach();
    attachTo(7);
    release();
    await settle();
    await settle();
    // Effect, cleanup, effect: only the last one reaches the backend.
    expect(calls.map((c) => c[0])).toEqual(["log_attach"]);
    detach();
    await settle();
    await settle();
    expect(calls.map((c) => c[0])).toEqual(["log_attach", "log_detach"]);
  });

  it("complete ends the stream and reloads the runs for the exit state", async () => {
    attachTo(7);
    await settle();
    send(batch([line(1)], { end: "complete" }));
    runFrame();
    expect(useLog.getState().attach).toEqual({ status: "ended", runId: 7 });
    await settle();
    expect(calls.some((c) => c[0] === "log_runs")).toBe(true);
  });

  it("expired makes the log unavailable and lets its lines go, an open line too", async () => {
    attachTo(7);
    await settle();
    send(batch([line(1), line(2, "half", { more: true })]));
    send(batch([], { end: "expired" }));
    runFrame();
    expect(useLog.getState().attach).toEqual({ status: "gone", runId: 7 });
    expect(useLog.getState().feed).toEqual(EMPTY_FEED);
  });

  it("attaching again after gone starts empty from the oldest line", async () => {
    attachTo(7);
    await settle();
    send(batch([line(1), line(2)]));
    send(batch([], { end: "expired" }));
    runFrame();
    attachTo(7);
    await settle();
    expect(calls.filter((c) => c[0] === "log_attach").at(-1)).toEqual([
      "log_attach",
      { runId: 7, sinceSeq: 0 },
    ]);
    expect(useLog.getState().feed.rows).toEqual([]);
  });

  it("a frame and the timer both pending apply the queue once", async () => {
    vi.useFakeTimers();
    try {
      attachTo(7);
      // The attach goes out after the call before it has settled.
      await vi.advanceTimersByTimeAsync(1);
      let commits = 0;
      const off = useLog.subscribe((s, before) => {
        if (s.feed !== before.feed) commits += 1;
      });
      send(batch([line(1)]));
      runFrame();
      vi.advanceTimersByTime(300);
      off();
      expect(commits).toBe(1);
      expect(useLog.getState().feed.lines).toBe(1);
    } finally {
      vi.useRealTimers();
    }
  });

  it("a detach after a rejected attach still reaches the backend", async () => {
    attachFails = { code: "internal", params: {}, action: null, detail: "y" };
    attachTo(7);
    await settle();
    detach();
    await settle();
    expect(calls.map((c) => c[0])).toEqual(["log_attach", "log_detach"]);
  });

  it("another run resets what belonged to the last one: copy result, open error, selection", async () => {
    attachTo(7);
    await settle();
    useLog.setState({
      copy: { status: "failed", error: null },
      omitted: 4,
      openError: { code: "openFailed", params: {}, action: null, detail: "" },
      allSelected: true,
    });
    attachTo(8);
    const s = useLog.getState();
    expect([s.copy, s.omitted, s.openError, s.allSelected]).toEqual([
      { status: "idle" },
      0,
      null,
      false,
    ]);
  });

  it("an invalidInput on runId means the log is gone, another error is a failure", async () => {
    attachFails = { code: "invalidInput", params: { field: "runId" }, action: null, detail: "x" };
    attachTo(7);
    await settle();
    expect(useLog.getState().attach).toEqual({ status: "gone", runId: 7 });
    attachFails = { code: "internal", params: {}, action: null, detail: "y" };
    attachTo(8);
    await settle();
    expect(useLog.getState().attach).toMatchObject({ status: "failed", runId: 8 });
  });

  it("a refused attach that means gone lets the lines go, as expired does", async () => {
    attachTo(7);
    await settle();
    send(batch([line(1), line(2, "half", { more: true })]));
    runFrame();
    expect(useLog.getState().feed.lines).toBe(1);
    // The same run again (the view came back), and now the backend no longer has it.
    attachFails = { code: "invalidInput", params: { field: "runId" }, action: null, detail: "x" };
    attachTo(7);
    await settle();
    expect(useLog.getState().attach).toEqual({ status: "gone", runId: 7 });
    expect(useLog.getState().feed).toEqual(EMPTY_FEED);
  });

  it("an attach that never answers fails after 10 s with a retry, and the queue goes on", async () => {
    vi.useFakeTimers();
    try {
      attachGate = new Promise(() => {});
      attachTo(7);
      await vi.advanceTimersByTimeAsync(9_999);
      expect(useLog.getState().attach).toEqual({ status: "attaching", runId: 7 });
      await vi.advanceTimersByTimeAsync(1);
      expect(useLog.getState().attach).toMatchObject({ status: "failed", runId: 7 });
      // Try again: the next call is not stuck behind the first.
      attachGate = null;
      attachTo(7);
      await vi.advanceTimersByTimeAsync(1);
      expect(calls.filter((c) => c[0] === "log_attach")).toHaveLength(2);
      expect(useLog.getState().attach).toEqual({ status: "live", runId: 7 });
    } finally {
      vi.useRealTimers();
    }
  });

  it("late answers resend the newest wish once, not once per late answer", async () => {
    vi.useFakeTimers();
    try {
      const detaches = () => calls.filter((c) => c[0] === "log_detach").length;
      let first!: () => void;
      let second!: () => void;
      attachGate = new Promise((r) => (first = r));
      attachTo(7);
      await vi.advanceTimersByTimeAsync(10_000);
      // The retry times out too: the view shows the failure, no stream is wanted right now.
      attachGate = new Promise((r) => (second = r));
      attachTo(7);
      await vi.advanceTimersByTimeAsync(10_000);
      expect(useLog.getState().attach).toMatchObject({ status: "failed", runId: 7 });
      attachGate = null;
      first();
      await vi.advanceTimersByTimeAsync(1);
      expect(detaches()).toBe(1);
      // A second late answer for the same wish sends nothing more.
      second();
      await vi.advanceTimersByTimeAsync(1);
      expect(detaches()).toBe(1);
      expect(calls.filter((c) => c[0] === "log_attach")).toHaveLength(2);
    } finally {
      vi.useRealTimers();
    }
  });

  it("a resend for the run that is live keeps it live (no flicker back to attaching)", async () => {
    vi.useFakeTimers();
    try {
      let late!: () => void;
      attachGate = new Promise((r) => (late = r));
      attachTo(7);
      await vi.advanceTimersByTimeAsync(10_000);
      attachGate = null;
      attachTo(7);
      await vi.advanceTimersByTimeAsync(1);
      expect(useLog.getState().attach).toEqual({ status: "live", runId: 7 });
      const seen: string[] = [];
      const off = useLog.subscribe((s) => seen.push(s.attach.status));
      let resent!: () => void;
      attachGate = new Promise((r) => (resent = r));
      late();
      await vi.advanceTimersByTimeAsync(1);
      expect(calls.filter((c) => c[0] === "log_attach")).toHaveLength(3);
      expect(useLog.getState().attach).toEqual({ status: "live", runId: 7 });
      resent();
      await vi.advanceTimersByTimeAsync(1);
      off();
      expect(seen).not.toContain("attaching");
      expect(useLog.getState().attach).toEqual({ status: "live", runId: 7 });
    } finally {
      vi.useRealTimers();
    }
  });

  it("a timed-out attach that answers after the retry is followed by the newest call again", async () => {
    vi.useFakeTimers();
    try {
      let release!: () => void;
      attachGate = new Promise((r) => (release = r));
      attachTo(7);
      await vi.advanceTimersByTimeAsync(10_000);
      expect(useLog.getState().attach).toMatchObject({ status: "failed", runId: 7 });
      attachGate = null;
      attachTo(7);
      await vi.advanceTimersByTimeAsync(1);
      expect(useLog.getState().attach).toEqual({ status: "live", runId: 7 });
      // The first call answers late: in the backend it finished last and feeds its old channel.
      // The view sends its attach once more, so the newest call is the last one done.
      release();
      await vi.advanceTimersByTimeAsync(1);
      expect(calls.filter((c) => c[0] === "log_attach")).toHaveLength(3);
      expect(useLog.getState().attach).toEqual({ status: "live", runId: 7 });
      // Without a stream wanted, a late answer is followed by a detach instead.
      let late!: () => void;
      attachGate = new Promise((r) => (late = r));
      attachTo(8);
      await vi.advanceTimersByTimeAsync(10_000);
      detach();
      await vi.advanceTimersByTimeAsync(1);
      const before = calls.filter((c) => c[0] === "log_detach").length;
      late();
      await vi.advanceTimersByTimeAsync(1);
      expect(calls.filter((c) => c[0] === "log_detach").length).toBe(before + 1);
    } finally {
      vi.useRealTimers();
    }
  });
});

describe("copy and open", () => {
  afterEach(() => {
    resetLog();
    clearMocks();
  });

  it("copies the filtered lines once at a time and says how many were left out", async () => {
    let release!: () => void;
    const gate = new Promise<void>((r) => (release = r));
    const filters: unknown[] = [];
    mockIPC(async (cmd, args) => {
      if (cmd !== "log_copy") return null;
      filters.push((args as { filter: unknown }).filter);
      await gate;
      return { text: "a\nb\n", lines: 2, omitted: 5 };
    });
    useLog.getState().setLevel("debug", false);
    useLog.getState().setQuery("ignored by copy");
    const written: string[] = [];
    const write = async (t: string) => void written.push(t);
    const first = useLog.getState().copyLines(7, write);
    const second = useLog.getState().copyLines(7, write);
    release();
    await Promise.all([first, second]);
    expect(filters).toEqual([
      { error: true, warning: true, info: true, debug: false, other: true, text: "" },
    ]);
    expect(written).toEqual(["a\nb\n"]);
    expect(useLog.getState().copy).toEqual({ status: "copied" });
    expect(useLog.getState().omitted).toBe(5);
  });

  it("Copied goes back after 2 s also when lines were left out; the note stays until the next copy or filter change", async () => {
    vi.useFakeTimers();
    try {
      let omitted = 5;
      mockIPC(() => ({ text: "a", lines: 1, omitted }));
      await useLog.getState().copyLines(7, async () => {});
      expect(useLog.getState().copy.status).toBe("copied");
      vi.advanceTimersByTime(2000);
      expect(useLog.getState().copy.status).toBe("idle");
      expect(useLog.getState().omitted).toBe(5);
      omitted = 0;
      await useLog.getState().copyLines(7, async () => {});
      expect(useLog.getState().omitted).toBe(0);
      omitted = 2;
      await useLog.getState().copyLines(7, async () => {});
      useLog.getState().setLevel("info", false);
      expect(useLog.getState().omitted).toBe(0);
    } finally {
      vi.useRealTimers();
    }
  });

  it("a filter change ends a select-all and the omitted note, an open-file error stays", () => {
    const openError = { code: "openFailed" as const, params: {}, action: null, detail: "" };
    useLog.setState({ allSelected: true, omitted: 3, openError });
    useLog.getState().setLevel("debug", false);
    expect([useLog.getState().allSelected, useLog.getState().omitted]).toEqual([false, 0]);
    expect(useLog.getState().openError).toBe(openError);
  });

  it("a refused clipboard or a failed copy says so", async () => {
    mockIPC(() => ({ text: "x", lines: 1, omitted: 0 }));
    await useLog.getState().copyLines(7, () => Promise.reject(new Error("denied")));
    expect(useLog.getState().copy).toEqual({ status: "failed", error: null });
    mockIPC(() => {
      throw { code: "busy", params: {}, action: null, detail: "" };
    });
    await useLog.getState().copyLines(7, async () => {});
    expect(useLog.getState().copy).toMatchObject({ status: "failed", error: { code: "busy" } });
  });

  it("opens the run's log file by its id and keeps the error", async () => {
    const targets: unknown[] = [];
    mockIPC((_cmd, args) => {
      targets.push((args as { target: unknown }).target);
      throw { code: "openFailed", params: {}, action: null, detail: "" };
    });
    await useLog.getState().openFile(7);
    expect(targets).toEqual([{ kind: "gameLog", runId: 7 }]);
    expect(useLog.getState().openError?.code).toBe("openFailed");
  });

  it("a double click on Open log file opens one viewer", async () => {
    let release!: () => void;
    const gate = new Promise<void>((r) => (release = r));
    let opens = 0;
    mockIPC(async (cmd) => {
      if (cmd !== "open_location") return null;
      opens += 1;
      await gate;
      return null;
    });
    const first = useLog.getState().openFile(7);
    const second = useLog.getState().openFile(7);
    release();
    await Promise.all([first, second]);
    expect(opens).toBe(1);
    // Done: the next click opens again.
    await useLog.getState().openFile(7);
    expect(opens).toBe(2);
  });
});

describe("attach order and the chosen run", () => {
  // The backend runs `log_attach` on a multi-thread runtime: of two calls in flight, the one it
  // finishes last is the consumer. This mock makes that explicit: a call takes effect when it is
  // settled, and `settleAll` settles whatever is in flight newest first (the worst order).
  let inFlight: {
    runId: number;
    channel: { onmessage?: (b: LogBatch) => void };
    done: () => void;
  }[];
  let consumer: { runId: number; channel: { onmessage?: (b: LogBatch) => void } } | null;
  let sent: string[];
  let frames: FrameRequestCallback[];

  beforeEach(() => {
    resetLog();
    inFlight = [];
    consumer = null;
    sent = [];
    frames = [];
    vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => frames.push(cb));
    vi.stubGlobal("cancelAnimationFrame", () => {});
    mockIPC((cmd, args) => {
      const a = args as Record<string, unknown>;
      sent.push(cmd === "log_attach" ? `attach ${a.runId as number}` : cmd);
      if (cmd === "log_attach") {
        const call = { runId: a.runId as number, channel: a.channel as never };
        return new Promise<null>((resolve) =>
          inFlight.push({
            ...call,
            done: () => {
              consumer = call;
              resolve(null);
            },
          }),
        );
      }
      if (cmd === "log_detach") consumer = null;
      if (cmd === "log_runs") return [{ runId: 4 }];
      return null;
    });
  });
  afterEach(() => {
    resetLog();
    clearMocks();
    vi.unstubAllGlobals();
  });

  const tick = () => new Promise((r) => setTimeout(r, 0));
  async function settleAll() {
    for (let round = 0; round < 10; round++) {
      await tick();
      if (inFlight.length === 0) continue;
      for (const c of inFlight.splice(0).reverse()) c.done();
    }
  }
  /** The backend sends a line to whoever it holds as the consumer. */
  const stream = (seq: number) =>
    consumer?.channel.onmessage?.(
      batch([line(seq, `run ${consumer.runId}`)], { runId: consumer.runId }),
    );

  it("a second attach waits for the first, so the backend ends on the newest channel", async () => {
    attachTo(7);
    await tick();
    attachTo(8);
    await settleAll();
    expect(consumer?.runId).toBe(8);
    stream(1);
    frames.splice(0).forEach((f) => f(0));
    expect(messages(useLog.getState().feed.rows)).toEqual(["run 8"]);
  });

  it("A, then B, then A again ends on A with A's lines", async () => {
    attachTo(7);
    await tick();
    attachTo(8);
    attachTo(7);
    await settleAll();
    expect(sent).toEqual(["attach 7", "attach 7"]);
    expect(consumer?.runId).toBe(7);
    stream(1);
    frames.splice(0).forEach((f) => f(0));
    expect(messages(useLog.getState().feed.rows)).toEqual(["run 7"]);
  });

  it("StrictMode's effect, cleanup, effect sends one attach and leaves it in place", async () => {
    attachTo(7);
    detach();
    attachTo(7);
    await settleAll();
    expect(sent).toEqual(["attach 7"]);
    expect(consumer?.runId).toBe(7);
  });

  it("a detach waits for the attach before it", async () => {
    attachTo(7);
    detach();
    await tick();
    attachTo(7);
    await tick();
    detach();
    await settleAll();
    expect(consumer).toBeNull();
  });

  it("the run chosen before the run list is there becomes 'the newest' when it is the newest", async () => {
    useLog.getState().choose(4);
    expect(useLog.getState().chosen).toBe(4);
    await useLog.getState().loadRuns();
    expect(useLog.getState().chosen).toBeNull();
  });
});
