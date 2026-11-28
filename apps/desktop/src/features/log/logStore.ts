import { create } from "zustand";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";
import type { LogBatch } from "@/ipc/bindings/LogBatch";
import type { LogEnd } from "@/ipc/bindings/LogEnd";
import type { LogFilter } from "@/ipc/bindings/LogFilter";
import type { LogLevelDto } from "@/ipc/bindings/LogLevelDto";
import type { LogLineDto } from "@/ipc/bindings/LogLineDto";
import type { LogRunDto } from "@/ipc/bindings/LogRunDto";
import type { LogTimeDto } from "@/ipc/bindings/LogTimeDto";
import { api, IpcError } from "@/ipc/client";

/** The five level groups of the filter (DESIGN.md 10.13), in the order the menu lists them. */
export type LevelGroup = "error" | "warning" | "info" | "debug" | "other";
export const LEVEL_GROUPS: readonly LevelGroup[] = ["error", "warning", "info", "debug", "other"];
export type Levels = Record<LevelGroup, boolean>;
export const ALL_LEVELS: Levels = {
  error: true,
  warning: true,
  info: true,
  debug: true,
  other: true,
};

export function groupOf(level: LogLevelDto | null): LevelGroup {
  switch (level) {
    case "fatal":
    case "error":
      return "error";
    case "warn":
      return "warning";
    case "info":
      return "info";
    case "debug":
    case "trace":
      return "debug";
    default:
      return "other";
  }
}

/** The web view holds at most this many lines; the oldest go first (the backend holds as many). */
export const MAX_LINES = 10_000;

/**
 * One line as the view shows it, its parts joined. `ord` orders every row and never repeats:
 * a line has `2 * seq`, a gap before the line `seq` has `2 * seq - 1`.
 */
export interface LineRow {
  kind: "line";
  ord: number;
  seq: number;
  time: LogTimeDto | null;
  level: LogLevelDto | null;
  group: LevelGroup;
  thread: string | null;
  message: string;
  /** The backend shortened it, or its last parts never came (the run expired mid-line). */
  cut: boolean;
}

/**
 * Lines that are not shown: `older` counts what the view let go at its limit (always first),
 * otherwise lines that left the backend's buffer before they were sent (`LogBatch.dropped`).
 */
export interface GapRow {
  kind: "gap";
  ord: number;
  count: number;
  older: boolean;
}

export type Row = LineRow | GapRow;

/** A line whose parts are still coming (`more`). */
interface Partial {
  head: LogLineDto;
  parts: string[];
  cut: boolean;
}

export interface Feed {
  rows: readonly Row[];
  /** Line rows in `rows`. */
  lines: number;
  /** The last seq whose final part arrived: a new attach continues after it. */
  lastSeq: number;
  partial: Partial | null;
}

export const EMPTY_FEED: Feed = { rows: [], lines: 0, lastSeq: 0, partial: null };

const lineOf = (p: Partial, cut: boolean): LineRow => ({
  kind: "line",
  ord: 2 * p.head.seq,
  seq: p.head.seq,
  time: p.head.time,
  level: p.head.level,
  group: groupOf(p.head.level),
  thread: p.head.thread,
  message: p.parts.length === 1 ? p.parts[0] : p.parts.join(""),
  cut,
});

/**
 * Applies batches to a feed and returns the new one (the old one is left as it was). Parts of
 * one seq are joined; a line counts once its last part (`more` false) is there. Lines at or
 * below `lastSeq` are duplicates and ignored. `dropped` becomes a gap row before the batch's
 * lines (also when it has none). Beyond `limit` lines the oldest go, counted in one leading row.
 */
export function ingest(feed: Feed, batches: readonly LogBatch[], limit = MAX_LINES): Feed {
  const rows = feed.rows.slice();
  let { lines, lastSeq, partial } = feed;
  const gap = (count: number) => {
    const ord = 2 * (lastSeq + 1) - 1;
    const last = rows[rows.length - 1];
    if (last?.kind === "gap" && !last.older && last.ord === ord)
      rows[rows.length - 1] = { ...last, count: last.count + count };
    else rows.push({ kind: "gap", ord, count, older: false });
  };
  for (const b of batches) {
    if (b.dropped > 0) {
      // Parts are never dropped singly: nothing open can continue after a gap.
      partial = null;
      gap(b.dropped);
    }
    for (const l of b.lines) {
      if (l.seq <= lastSeq) continue;
      if (l.part === 0) partial = { head: l, parts: [l.message], cut: l.cut };
      else if (partial && partial.head.seq === l.seq && partial.parts.length === l.part)
        partial = { ...partial, parts: [...partial.parts, l.message], cut: partial.cut || l.cut };
      else continue;
      if (l.more) continue;
      rows.push(lineOf(partial, partial.cut));
      lines += 1;
      lastSeq = partial.head.seq;
      partial = null;
    }
  }
  return trim({ rows, lines, lastSeq, partial }, limit);
}

/** Lets the oldest lines go beyond `limit`; they and any gap among them join the leading row. */
function trim(feed: Feed, limit: number): Feed {
  if (feed.lines <= limit) return feed;
  const rows = feed.rows;
  let i = 0;
  let count = 0;
  let removed = 0;
  while (i < rows.length && feed.lines - removed > limit) {
    const r = rows[i];
    if (r.kind === "line") {
      removed += 1;
      count += 1;
    } else count += r.count;
    i += 1;
  }
  // A gap right behind the cut belongs to the lines that are gone as well.
  while (i < rows.length && rows[i].kind === "gap") count += (rows[i++] as GapRow).count;
  const first = rows[i];
  const lead: GapRow = { kind: "gap", ord: (first?.ord ?? 0) - 0.5, count, older: true };
  return { ...feed, rows: [lead, ...rows.slice(i)], lines: feed.lines - removed };
}

/** The rows the level filter lets through; gaps always show. */
export function visibleRows(rows: readonly Row[], levels: Levels): readonly Row[] {
  if (LEVEL_GROUPS.every((g) => levels[g])) return rows;
  return rows.filter((r) => r.kind === "gap" || levels[r.group]);
}

/** A plain, case-insensitive substring pattern (no pattern syntax reaches the regex). */
export function searchPattern(query: string): RegExp | null {
  if (query === "") return null;
  return new RegExp(query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "giu");
}

const hitCache = new WeakMap<LineRow, { source: string; count: number }>();

/** How often `pattern` occurs in the row's message (cached per row and pattern). */
export function hitCount(row: Row, pattern: RegExp): number {
  if (row.kind !== "line") return 0;
  const cached = hitCache.get(row);
  if (cached?.source === pattern.source) return cached.count;
  let count = 0;
  pattern.lastIndex = 0;
  while (pattern.exec(row.message)) count += 1;
  hitCache.set(row, { source: pattern.source, count });
  return count;
}

/** The ranges of the matches in `text`. */
export function hitRanges(text: string, pattern: RegExp): [number, number][] {
  const out: [number, number][] = [];
  pattern.lastIndex = 0;
  for (let m = pattern.exec(text); m; m = pattern.exec(text)) {
    out.push([m.index, m.index + m[0].length]);
  }
  return out;
}

/** `prefix[i]` = matches in the rows before `i`; `prefix[rows.length]` is the total. */
export function hitPrefix(rows: readonly Row[], pattern: RegExp | null): Int32Array {
  const prefix = new Int32Array(rows.length + 1);
  if (!pattern) return prefix;
  for (let i = 0; i < rows.length; i++) prefix[i + 1] = prefix[i] + hitCount(rows[i], pattern);
  return prefix;
}

/** The row and the occurrence within it of match `k` (0-based). */
export function locateHit(prefix: Int32Array, k: number): { index: number; nth: number } {
  let lo = 0;
  let hi = prefix.length - 2;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (prefix[mid] <= k) lo = mid;
    else hi = mid - 1;
  }
  return { index: lo, nth: k - prefix[lo] };
}

/** The current match as an index into all matches, from its row's `ord` and occurrence. */
export function hitIndex(
  rows: readonly Row[],
  prefix: Int32Array,
  current: { ord: number; nth: number } | null,
): number | null {
  const total = prefix[rows.length];
  if (total === 0) return null;
  if (!current) return 0;
  // The first row at or after the current one's place (rows are ordered by `ord`).
  let lo = 0;
  let hi = rows.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (rows[mid].ord < current.ord) lo = mid + 1;
    else hi = mid;
  }
  if (lo < rows.length && rows[lo].ord === current.ord) {
    const inRow = prefix[lo + 1] - prefix[lo];
    if (current.nth < inRow) return prefix[lo] + current.nth;
  }
  return prefix[lo] % total;
}

// ---- List position (used by the view's virtualized list) ------------------------------------

/** Where `ord` is in `rows` (ordered by `ord`), or -1. */
export function indexOfOrd(rows: readonly Row[], ord: number): number {
  let lo = 0;
  let hi = rows.length - 1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (rows[mid].ord === ord) return mid;
    if (rows[mid].ord < ord) lo = mid + 1;
    else hi = mid - 1;
  }
  return -1;
}

/**
 * How many rows left the front of `prev` to give `next`, found by the first rows of `next` that
 * `prev` also holds (the leading note is new each time the oldest lines go); `null` when none
 * of them is in `prev`. The list's `firstItemIndex` grows by this, so the viewport stays.
 */
export function frontShift(prev: readonly Row[], next: readonly Row[]): number | null {
  for (let i = 0; i < Math.min(3, next.length); i++) {
    const j = indexOfOrd(prev, next[i].ord);
    if (j >= 0) return j - i;
  }
  return null;
}

/** A match the list is scrolling to, by its row; kept up until `until` (ms). */
export interface ScrollTarget {
  ord: number;
  until: number;
}

/**
 * What the list does after its rows changed. Following: go to the end (at the limit the row count
 * stays the same, so the list's own follow, which reacts to a growing count, does not fire).
 * Scrolling to a match: go to its row's current index, because lines leaving the front move every
 * index while the list still retries the old one.
 */
export function followAction(
  rows: readonly Row[],
  following: boolean,
  target: ScrollTarget | null,
  now: number,
): { kind: "end" } | { kind: "index"; index: number } | null {
  if (following) return { kind: "end" };
  if (!target || now > target.until) return null;
  const index = indexOfOrd(rows, target.ord);
  return index >= 0 ? { kind: "index", index } : null;
}

/**
 * Where a list that starts over (a new level filter) opens: at the end while following, else at
 * the line it showed on top, or the first one after it that is still shown.
 */
export function initialIndex(
  rows: readonly Row[],
  following: boolean,
  topOrd: number | null,
): number | { index: "LAST"; align: "end" } {
  const end = { index: "LAST" as const, align: "end" as const };
  if (following || topOrd === null) return end;
  const i = rows.findIndex((r) => r.ord >= topOrd);
  return i < 0 ? end : i;
}

/** Text of an error that did not come from the backend. */
function errorOf(e: unknown): ErrorDto {
  if (e instanceof IpcError) return e.dto;
  return { code: "internal", params: {}, action: null, detail: String(e) };
}

export type AttachState =
  | { status: "idle" }
  | { status: "attaching"; runId: number }
  | { status: "live"; runId: number }
  /** `end: complete`: no more lines come. */
  | { status: "ended"; runId: number }
  /** The run left the buffer (`end: expired`, or `invalidInput` on `runId`). */
  | { status: "gone"; runId: number }
  | { status: "failed"; runId: number; error: ErrorDto };

export type CopyState =
  | { status: "idle" }
  | { status: "busy" }
  | { status: "copied" }
  | { status: "failed"; error: ErrorDto | null };

interface LogState {
  runs: { status: "idle" | "loading" | "ready" | "error"; data?: LogRunDto[]; error?: ErrorDto };
  /**
   * The run the user chose; `null`: the newest one, whichever it is (it follows a new start). A
   * choice made before the run list is there becomes `null` once the list shows it is the newest.
   */
  chosen: number | null;
  attach: AttachState;
  /** The run `feed` belongs to. */
  feedRun: number | null;
  feed: Feed;
  levels: Levels;
  query: string;
  /** The current match by its row and occurrence, so new lines do not move it. */
  current: { ord: number; nth: number } | null;
  copy: CopyState;
  /**
   * Lines the last copy left out at the 4 MiB limit. Shown until the next copy, a level change or
   * another run; "Kopiert" itself goes after 2 s.
   */
  omitted: number;
  openError: ErrorDto | null;
  /** Ctrl+A in the log area: every line of the current filter is selected (for Ctrl+C). */
  allSelected: boolean;
  loadRuns: () => Promise<void>;
  /** Shows `runId` (from the start page); the newest run means "always the newest". */
  choose: (runId: number | null) => void;
  setLevel: (group: LevelGroup, on: boolean) => void;
  showAllLevels: () => void;
  setQuery: (query: string) => void;
  setCurrent: (current: { ord: number; nth: number } | null) => void;
  setAllSelected: (on: boolean) => void;
  copyLines: (runId: number, write: (text: string) => Promise<void>) => Promise<void>;
  openFile: (runId: number) => Promise<void>;
  dismissOpenError: () => void;
}

const INITIAL = {
  runs: { status: "idle" as const },
  chosen: null,
  attach: { status: "idle" as const },
  feedRun: null,
  feed: EMPTY_FEED,
  levels: ALL_LEVELS,
  query: "",
  current: null,
  copy: { status: "idle" as const },
  omitted: 0,
  openError: null,
  allSelected: false,
};

let runsRequests = 0;
let copyTimer: ReturnType<typeof setTimeout> | undefined;
/** `openFile` is waiting for the backend. */
let opening = false;

/** The filter of `log_copy` for the level choice; search finds, it does not filter (10.13). */
export const copyFilter = (levels: Levels): LogFilter => ({ ...levels, text: "" });

export const useLog = create<LogState>()((set, get) => ({
  ...INITIAL,

  loadRuns: async () => {
    const mine = ++runsRequests;
    set((s) => ({ runs: s.runs.data ? s.runs : { status: "loading" } }));
    try {
      const data = await api.logRuns();
      if (mine !== runsRequests) return;
      set((s) => ({
        runs: { status: "ready", data },
        chosen: s.chosen !== null && s.chosen === data[0]?.runId ? null : s.chosen,
      }));
    } catch (e) {
      if (mine === runsRequests)
        set((s) => ({ runs: { ...s.runs, status: "error", error: errorOf(e) } }));
    }
  },

  choose: (runId) => {
    const newest = get().runs.data?.[0]?.runId;
    set({ chosen: runId === newest ? null : runId });
  },

  // A level change ends a select-all (it selected the old filter's lines) and the omitted note
  // (it was about the old filter); an open-file error is not about the filter and stays.
  setLevel: (group, on) =>
    set((s) => ({ levels: { ...s.levels, [group]: on }, allSelected: false, omitted: 0 })),
  showAllLevels: () => set({ levels: ALL_LEVELS, allSelected: false, omitted: 0 }),
  setQuery: (query) => set({ query, current: null }),
  setCurrent: (current) => set({ current }),
  setAllSelected: (on) => set({ allSelected: on }),

  copyLines: async (runId, write) => {
    // One copy at a time: the backend refuses a second one (`busy`), the button waits instead.
    if (get().copy.status === "busy") return;
    clearTimeout(copyTimer);
    set({ copy: { status: "busy" }, omitted: 0 });
    let text: string;
    let omitted: number;
    try {
      ({ text, omitted } = await api.logCopy(runId, copyFilter(get().levels)));
    } catch (e) {
      set({ copy: { status: "failed", error: errorOf(e) } });
      return;
    }
    try {
      await write(text);
    } catch {
      set({ copy: { status: "failed", error: null } });
      return;
    }
    set({ copy: { status: "copied" }, omitted });
    // "Kopiert" shows for 2 s, as "Code kopieren" (DESIGN.md 11.5); the omitted note stays.
    copyTimer = setTimeout(() => {
      if (get().copy.status === "copied") set({ copy: { status: "idle" } });
    }, 2000);
  },

  openFile: async (runId) => {
    // One open at a time: a double click must not start a second viewer.
    if (opening) return;
    opening = true;
    set({ openError: null });
    try {
      await api.openLocation({ kind: "gameLog", runId });
    } catch (e) {
      set({ openError: errorOf(e) });
    } finally {
      opening = false;
    }
  },
  dismissOpenError: () => set({ openError: null }),
}));

// ---- The stream ------------------------------------------------------------------------------

// Each attach gets a number; batches of an earlier channel (a replaced backend task can still
// deliver one) are dropped because their number is no longer the current one.
let generation = 0;
// Every `log_attach` and `log_detach` waits until the call before it has settled. The backend
// runs `log_attach` as an async command on a multi-thread runtime: two in flight can finish in
// either order, and the one that finishes last becomes the consumer. Chained, the last call sent
// is the last one done. A call that is no longer the newest by the time its turn comes is not
// sent at all (StrictMode's effect, cleanup, effect sends one attach). A call that does not settle
// within `CALL_TIMEOUT_MS` counts as failed, so it cannot hold up the calls after it for good.
// If it settles after all, it may have finished after a newer call in the backend (and an attach
// then feeds its old channel): `resend` sends the newest wish once more, so it is the last call
// done again. Once per wish (a wish is an `attachTo` or `detach` of the view): with a backend that
// stays slow, further late answers for the same wish send nothing.
let chain: Promise<unknown> = Promise.resolve();
let wish = 0;
let resentWish = -1;
const CALL_TIMEOUT_MS = 10_000;
function bounded<T>(call: Promise<T>): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    let late = false;
    const timer = setTimeout(() => {
      late = true;
      reject(new Error(`no answer from the backend within ${CALL_TIMEOUT_MS} ms`));
    }, CALL_TIMEOUT_MS);
    const settled = () => {
      clearTimeout(timer);
      if (late) resend();
    };
    call.then(
      (value) => {
        settled();
        resolve(value);
      },
      (e: unknown) => {
        settled();
        reject(e);
      },
    );
  });
}
/** A stream is wanted: attach to its run again; otherwise detach. */
function resend() {
  if (resentWish === wish) return;
  resentWish = wish;
  const a = useLog.getState().attach;
  if (a.status === "attaching" || a.status === "live") connect(a.runId, true);
  else disconnect();
}
function enqueue<T>(call: () => Promise<T> | undefined): Promise<T | undefined> {
  const next = chain.then(() => {
    const sent = call();
    return sent === undefined ? undefined : bounded(sent);
  });
  chain = next.catch(() => undefined);
  return next;
}
// Batches that arrived and are not applied yet. They are applied together once per frame, so
// many small batches cost one store update and one render (the ack went out on arrival).
let queue: LogBatch[] = [];
let frame: number | null = null;
let fallback: ReturnType<typeof setTimeout> | null = null;
// A hidden window gets no animation frames (the app sits in the tray while the game runs): the
// timer still applies the queue, so it stays bounded.
const FALLBACK_MS = 250;

function schedule() {
  if (frame === null && typeof requestAnimationFrame === "function")
    frame = requestAnimationFrame(flush);
  if (fallback === null) fallback = setTimeout(flush, FALLBACK_MS);
}

function cancelScheduled() {
  if (frame !== null && typeof cancelAnimationFrame === "function") cancelAnimationFrame(frame);
  if (fallback !== null) clearTimeout(fallback);
  frame = null;
  fallback = null;
}

/** Applies every queued batch in one store update. */
export function flush() {
  cancelScheduled();
  if (queue.length === 0) return;
  const batches = queue;
  queue = [];
  const s = useLog.getState();
  const attach = s.attach;
  if (attach.status === "idle") return;
  const feed = ingest(s.feed, batches);
  const end: LogEnd | null = batches.reduce<LogEnd | null>((e, b) => b.end ?? e, null);
  let next: AttachState = attach;
  if (end === "expired") {
    // DESIGN.md 11.9: the log area says the log is no longer available; what was shown goes (it
    // cannot be copied or opened any more either), an open line with it.
    useLog.setState({ feed: EMPTY_FEED, attach: { status: "gone", runId: attach.runId } });
    void s.loadRuns();
    return;
  } else if (end === "complete") next = { status: "ended", runId: attach.runId };
  else if (attach.status === "attaching") next = { status: "live", runId: attach.runId };
  useLog.setState({ feed, attach: next });
  // The end state (exit code) and what is still held come from the run list.
  if (end) void s.loadRuns();
}

/**
 * Attaches to `runId`. The view state is set up before `log_attach` is called, because batches
 * can arrive before its promise resolves. On the same run it continues after the last whole
 * line (a half-built one is thrown away and comes again); another run starts empty.
 */
export function attachTo(runId: number): void {
  wish += 1;
  connect(runId, false);
}

/** `attachTo` without a new wish; `resent`: a resend, which keeps a live stream of the run live. */
function connect(runId: number, resent: boolean): void {
  const mine = ++generation;
  queue = [];
  cancelScheduled();
  const s = useLog.getState();
  const same = s.feedRun === runId && s.attach.status !== "gone";
  const feed = same ? { ...s.feed, partial: null } : EMPTY_FEED;
  const keepLive = resent && s.attach.status === "live" && s.attach.runId === runId;
  useLog.setState({
    feed,
    feedRun: runId,
    attach: keepLive ? s.attach : { status: "attaching", runId },
    // What belonged to the last run's lines goes with them.
    ...(same
      ? {}
      : {
          current: null,
          allSelected: false,
          copy: { status: "idle" as const },
          omitted: 0,
          openError: null,
        }),
  });
  const since = feed.lastSeq === 0 ? 0 : feed.lastSeq + 1;
  const call = enqueue(() => {
    if (mine !== generation) return undefined;
    return api.logAttach(runId, since, (b) => {
      // Hand off and return: no work here beyond the checks.
      if (mine !== generation || b.runId !== runId) return;
      queue.push(b);
      schedule();
    });
  });
  call.then(
    () => {
      // A run without lines yet sends nothing: the answer alone says the stream is there.
      const a = useLog.getState().attach;
      if (mine === generation && a.status === "attaching")
        useLog.setState({ attach: { status: "live", runId } });
    },
    (e: unknown) => {
      if (mine !== generation) return;
      const error = errorOf(e);
      const gone = error.code === "invalidInput" && error.params.field === "runId";
      // Gone: the lines go with the log, as for `expired` (DESIGN.md 11.9).
      useLog.setState(
        gone
          ? { feed: EMPTY_FEED, attach: { status: "gone", runId } }
          : { attach: { status: "failed", runId, error } },
      );
    },
  );
}

/** Stops the stream: batches still on their way are dropped, the backend stops sending. */
export function detach(): void {
  wish += 1;
  disconnect();
}

/** `detach` without a new wish. */
function disconnect(): void {
  const mine = ++generation;
  queue = [];
  cancelScheduled();
  useLog.setState((s) =>
    s.attach.status === "attaching" || s.attach.status === "live"
      ? { attach: { status: "idle" } }
      : {},
  );
  // A newer attach replaces the old one in the backend by itself.
  void enqueue(() => (mine === generation ? api.logDetach() : undefined)).catch(() => undefined);
}

/** Back to the start (tests). */
export function resetLog() {
  generation += 1;
  queue = [];
  cancelScheduled();
  chain = Promise.resolve();
  wish = 0;
  resentWish = -1;
  clearTimeout(copyTimer);
  opening = false;
  useLog.setState(INITIAL);
}
