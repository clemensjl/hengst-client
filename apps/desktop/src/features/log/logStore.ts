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

/**
 * The run expired while a line's parts were coming: what came is kept, marked as cut (the
 * nearest pattern of DESIGN.md 10.13: a line the backend shortened).
 */
export function closeOpenLine(feed: Feed): Feed {
  if (!feed.partial) return feed;
  const row = lineOf(feed.partial, true);
  return {
    rows: [...feed.rows, row],
    lines: feed.lines + 1,
    lastSeq: feed.partial.head.seq,
    partial: null,
  };
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
  | { status: "copied"; omitted: number }
  | { status: "failed"; error: ErrorDto | null };

interface LogState {
  runs: { status: "idle" | "loading" | "ready" | "error"; data?: LogRunDto[]; error?: ErrorDto };
  /** The run the user chose; `null`: always the newest one. */
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
  openError: null,
  allSelected: false,
};

let runsRequests = 0;
let copyTimer: ReturnType<typeof setTimeout> | undefined;

/** The filter of `log_copy` for the level choice; search finds, it does not filter (10.13). */
export const copyFilter = (levels: Levels): LogFilter => ({ ...levels, text: "" });

export const useLog = create<LogState>()((set, get) => ({
  ...INITIAL,

  loadRuns: async () => {
    const mine = ++runsRequests;
    set((s) => ({ runs: s.runs.data ? s.runs : { status: "loading" } }));
    try {
      const data = await api.logRuns();
      if (mine === runsRequests) set({ runs: { status: "ready", data } });
    } catch (e) {
      if (mine === runsRequests)
        set((s) => ({ runs: { ...s.runs, status: "error", error: errorOf(e) } }));
    }
  },

  choose: (runId) => {
    const newest = get().runs.data?.[0]?.runId;
    set({ chosen: runId === newest ? null : runId });
  },

  setLevel: (group, on) => set((s) => ({ levels: { ...s.levels, [group]: on } })),
  showAllLevels: () => set({ levels: ALL_LEVELS }),
  setQuery: (query) => set({ query, current: null }),
  setCurrent: (current) => set({ current }),
  setAllSelected: (on) => set({ allSelected: on }),

  copyLines: async (runId, write) => {
    // One copy at a time: the backend refuses a second one (`busy`), the button waits instead.
    if (get().copy.status === "busy") return;
    clearTimeout(copyTimer);
    set({ copy: { status: "busy" } });
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
    set({ copy: { status: "copied", omitted } });
    // "Kopiert" shows for 2 s, as "Code kopieren" (DESIGN.md 11.5); the omitted note stays.
    copyTimer = setTimeout(() => {
      const c = get().copy;
      if (c.status === "copied" && c.omitted === 0) set({ copy: { status: "idle" } });
    }, 2000);
  },

  openFile: async (runId) => {
    set({ openError: null });
    try {
      await api.openLocation({ kind: "gameLog", runId });
    } catch (e) {
      set({ openError: errorOf(e) });
    }
  },
  dismissOpenError: () => set({ openError: null }),
}));

// ---- The stream ------------------------------------------------------------------------------

// Each attach gets a number; batches of an earlier channel (a replaced backend task can still
// deliver one) are dropped because their number is no longer the current one.
let generation = 0;
// The attach a detach has to wait for: `log_attach` is async in the backend and `log_detach`
// is not, so a detach sent at once could overtake it and leave the attach running.
let pendingAttach: Promise<unknown> = Promise.resolve();
// Batches that arrived and are not applied yet. They are applied together once per frame, so
// many small batches cost one store update and one render (the ack went out on arrival).
let queue: LogBatch[] = [];
let frame: number | null = null;
let fallback: ReturnType<typeof setTimeout> | null = null;
// A hidden window gets no animation frames (the app sits in the tray while the game runs): the
// timer still applies the queue, so it stays bounded.
const FALLBACK_MS = 250;

/** Counts store commits of batches; for the test that many batches make one commit. */
export const flushes = { count: 0 };

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
  let feed = ingest(s.feed, batches);
  const end: LogEnd | null = batches.reduce<LogEnd | null>((e, b) => b.end ?? e, null);
  let next: AttachState = attach;
  if (end === "expired") {
    feed = closeOpenLine(feed);
    next = { status: "gone", runId: attach.runId };
  } else if (end === "complete") next = { status: "ended", runId: attach.runId };
  else if (attach.status === "attaching") next = { status: "live", runId: attach.runId };
  flushes.count += 1;
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
  const mine = ++generation;
  queue = [];
  cancelScheduled();
  const s = useLog.getState();
  const same = s.feedRun === runId && s.attach.status !== "gone";
  const feed = same ? { ...s.feed, partial: null } : EMPTY_FEED;
  useLog.setState({
    feed,
    feedRun: runId,
    attach: { status: "attaching", runId },
    ...(same ? {} : { current: null, allSelected: false }),
  });
  const since = feed.lastSeq === 0 ? 0 : feed.lastSeq + 1;
  const call = api.logAttach(runId, since, (b) => {
    // Hand off and return: no work here beyond the checks.
    if (mine !== generation || b.runId !== runId) return;
    queue.push(b);
    schedule();
  });
  pendingAttach = call.catch(() => undefined);
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
      useLog.setState({
        attach: gone ? { status: "gone", runId } : { status: "failed", runId, error },
      });
    },
  );
}

/** Stops the stream: batches still on their way are dropped, the backend stops sending. */
export function detach(): void {
  const mine = ++generation;
  queue = [];
  cancelScheduled();
  useLog.setState((s) =>
    s.attach.status === "attaching" || s.attach.status === "live"
      ? { attach: { status: "idle" } }
      : {},
  );
  void pendingAttach.then(() => {
    // A newer attach replaces the old one in the backend by itself.
    if (mine === generation) void api.logDetach().catch(() => undefined);
  });
}

/** Back to the start (tests). */
export function resetLog() {
  generation += 1;
  queue = [];
  cancelScheduled();
  pendingAttach = Promise.resolve();
  clearTimeout(copyTimer);
  flushes.count = 0;
  useLog.setState(INITIAL);
}
