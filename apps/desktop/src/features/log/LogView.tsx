import { ArrowDownToLineIcon, CircleAlertIcon } from "lucide-react";
import {
  forwardRef,
  memo,
  useEffect,
  useImperativeHandle,
  useDeferredValue,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type Ref,
} from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import {
  Virtuoso,
  type ContextProp,
  type ScrollerProps,
  type VirtuosoHandle,
} from "react-virtuoso";
import { Delayed } from "@/components/Delayed";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectGhostTrigger,
  SelectItem,
  SelectItemText,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { ErrorPanel } from "@/features/errors/ErrorPanel";
import type { GameStateDto } from "@/ipc/bindings/GameStateDto";
import type { LogRunDto } from "@/ipc/bindings/LogRunDto";
import type { LogTimeDto } from "@/ipc/bindings/LogTimeDto";
import { isolate } from "@/lib/bidi";
import { useTruncated } from "@/lib/useTruncated";
import { cn } from "@/lib/utils";
import { useApp } from "@/stores/app";
import { LogToolbar } from "./LogToolbar";
import {
  attachTo,
  detach,
  followAction,
  frontShift,
  hitIndex,
  initialIndex,
  hitPrefix,
  hitRanges,
  locateHit,
  searchPattern,
  useLog,
  visibleRows,
  type LevelGroup,
  type Row,
  type ScrollTarget,
} from "./logStore";

// ---- Text ------------------------------------------------------------------------------------

/**
 * Control characters other than tab and line break are shown as their Control Pictures
 * (U+2400 ...), C1 controls as U+FFFD: one UTF-16 unit for one, so match ranges stay valid. The
 * text stays text; nothing in it is interpreted (no markup, links or ANSI colors).
 */
export function printable(text: string): string {
  return text.replace(/[\u0000-\u0008\u000B-\u001F\u007F-\u009F]/g, (c) => {
    const code = c.charCodeAt(0);
    if (code < 0x20) return String.fromCharCode(0x2400 + code);
    return code === 0x7f ? "␡" : "�";
  });
}

const LEVEL_COLOR: Record<LevelGroup, string> = {
  error: "text-log-error",
  warning: "text-log-warn",
  info: "text-log-info",
  other: "text-log-info",
  debug: "text-log-debug",
};

type ClockFormat = (t: LogTimeDto | null) => string;

function useClock(language: string): ClockFormat {
  return useMemo(() => {
    const f = new Intl.DateTimeFormat(language, {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      hourCycle: "h23",
    });
    const two = (n: number) => String(n).padStart(2, "0");
    return (t) => {
      if (!t) return "";
      // `unixMillis` is UTC: shown in local time. `clock` already is local (DESIGN.md 3.3).
      if (t.kind === "unixMillis") return f.format(new Date(t.ms));
      return `${two(t.hour)}:${two(t.minute)}:${two(t.second)}`;
    };
  }, [language]);
}

/** The message with its matches as `<mark>` elements, built from text nodes only. */
function Message({
  text,
  pattern,
  current,
}: {
  text: string;
  pattern: RegExp | null;
  current: number | null;
}) {
  const shown = printable(text);
  if (!pattern) return shown;
  const ranges = hitRanges(text, pattern);
  if (ranges.length === 0) return shown;
  const out: React.ReactNode[] = [];
  let from = 0;
  ranges.forEach(([a, b], i) => {
    if (a > from) out.push(shown.slice(from, a));
    out.push(
      <mark key={a} className={i === current ? "outline outline-foreground" : undefined}>
        {shown.slice(a, b)}
      </mark>,
    );
    from = b;
  });
  if (from < shown.length) out.push(shown.slice(from));
  return out;
}

interface RowProps {
  row: Row;
  pattern: RegExp | null;
  /** The occurrence in this row that is the current match. */
  current: number | null;
  selected: boolean;
  clock: ClockFormat;
}

/**
 * The thread name, cut with an ellipsis at 16ch; a cut name shows in full in a tooltip
 * (DESIGN.md 10.15), as text inside its own isolate.
 */
function Thread({ name, muted }: { name: string; muted: string }) {
  const ref = useRef<HTMLSpanElement>(null);
  const text = printable(name);
  const truncated = useTruncated(ref, text);
  const cut = (
    <span ref={ref} className="block truncate">
      <bdi>{text}</bdi>
    </span>
  );
  return (
    <span className={cn("min-w-0", muted)}>
      {truncated ? (
        <Tooltip>
          <TooltipTrigger render={cut} />
          <TooltipContent>
            <bdi>{text}</bdi>
          </TooltipContent>
        </Tooltip>
      ) : (
        cut
      )}
    </span>
  );
}

/**
 * DESIGN.md 10.13 line grid: time 8ch | level 5ch | thread 16ch | message. The text of every cell
 * is its own `<bdi>`, so bidi controls in it (an unterminated U+202E) cannot reorder anything
 * outside it, not even the cut hint in the same cell; the row clips what draws outside it
 * (stacked marks). Continuation lines of a message hang 2ch in.
 */
const LogRow = memo(function LogRow({ row, pattern, current, selected, clock }: RowProps) {
  const { t } = useTranslation();
  if (row.kind === "gap") {
    return (
      <div className="overflow-hidden px-3 font-mono text-mono text-muted-foreground">
        {t(row.older ? "log.older" : "log.gap", { count: row.count })}
      </div>
    );
  }
  const color = selected ? "text-foreground" : LEVEL_COLOR[row.group];
  const muted = selected ? "text-foreground" : "text-muted-foreground";
  return (
    <div
      data-seq={row.seq}
      className={cn(
        "grid grid-cols-[8ch_5ch_16ch_minmax(0,1fr)] gap-x-2 overflow-hidden px-3 font-mono text-mono",
        selected && "bg-selection",
      )}
    >
      <span className={cn("tabular-nums", muted)}>
        <bdi>{clock(row.time)}</bdi>
      </span>
      <span className={color}>
        <bdi>{row.level?.toUpperCase() ?? ""}</bdi>
      </span>
      {row.thread === null ? <span /> : <Thread name={row.thread} muted={muted} />}
      <span
        className={cn("-indent-[2ch] pl-[2ch] whitespace-pre-wrap [overflow-wrap:anywhere]", color)}
      >
        <bdi>
          <Message text={row.message} pattern={pattern} current={current} />
        </bdi>
        {row.cut && (
          <span className="ml-2 font-sans text-caption whitespace-normal text-muted-foreground">
            {t("log.cut")}
          </span>
        )}
      </span>
    </div>
  );
});

// ---- The list --------------------------------------------------------------------------------

interface ListContext {
  label: string;
  onKeyDown: (e: KeyboardEvent<HTMLDivElement>) => void;
  onMouseDown: () => void;
  /** The user moves the view up (wheel, keys, dragging the scrollbar): following stops. */
  onUserUp: () => void;
  end: string | null;
}

const UP_KEYS = new Set(["ArrowUp", "PageUp", "Home"]);

/**
 * The log area is one focusable region; it is not a live region (nothing is announced). Only
 * what the user does stops following: new lines below or old ones leaving above never do.
 */
const Scroller = forwardRef<HTMLDivElement, ScrollerProps & ContextProp<ListContext>>(
  function Scroller({ context, ...props }, ref) {
    const drag = useRef<{ top: number } | null>(null);
    return (
      <div
        {...props}
        ref={ref}
        tabIndex={0}
        role="region"
        aria-label={context.label}
        // The native menu (copy, select all) stays in the log area (DESIGN.md Appendix A).
        data-native-menu=""
        onKeyDown={(e) => {
          if (UP_KEYS.has(e.key)) context.onUserUp();
          context.onKeyDown(e);
        }}
        onWheel={(e) => {
          if (e.deltaY < 0) context.onUserUp();
        }}
        onPointerDown={(e) => {
          // A press on the scrollbar (or a drag selection) may move the view up.
          drag.current = { top: e.currentTarget.scrollTop };
          context.onMouseDown();
        }}
        onPointerUp={() => (drag.current = null)}
        onPointerCancel={() => (drag.current = null)}
        onScroll={(e) => {
          if (drag.current && e.currentTarget.scrollTop < drag.current.top) context.onUserUp();
        }}
      />
    );
  },
);

function Footer({ context }: ContextProp<ListContext>) {
  return context.end ? (
    <p className="px-3 py-2 text-caption text-muted-foreground">{context.end}</p>
  ) : null;
}

const COMPONENTS = { Scroller, Footer };

/** How long a scroll to a match is kept up while rows change. */
const TARGET_MS = 1500;

export interface ListHandle {
  scrollTo: (index: number) => void;
}

interface ListProps {
  rows: readonly Row[];
  /** A new run or a new level filter: the list starts over (at the end, or at the same line). */
  resetKey: string;
  pattern: RegExp | null;
  current: { ord: number; nth: number } | null;
  allSelected: boolean;
  end: string | null;
  onKeyDown: (e: KeyboardEvent<HTMLDivElement>) => void;
  onMouseDown: () => void;
  handle: Ref<ListHandle>;
}

function LogList(props: ListProps) {
  const { rows, resetKey, pattern, current, allSelected, end, handle } = props;
  const { t, i18n } = useTranslation();
  const clock = useClock(i18n.language);
  const virtuoso = useRef<VirtuosoHandle>(null);
  // Follows the end until the user moves up, and again once the view is back at the end or the
  // user jumps there; a list that starts over keeps the choice.
  const [following, setFollowing] = useState(true);
  const followingRef = useRef(true);
  const follow = (on: boolean) => {
    followingRef.current = on;
    setFollowing(on);
  };
  const topOrd = useRef<number | null>(null);

  // `firstItemIndex` grows by the rows that left the front, so Virtuoso keeps the viewport when
  // the oldest lines go; rows that cannot be matched up start the list over.
  const shift = useRef({ rows, first: 0, key: resetKey, mount: 0 });
  let s = shift.current;
  if (s.key !== resetKey) s = { rows, first: 0, key: resetKey, mount: s.mount + 1 };
  else if (s.rows !== rows) {
    const d = s.rows.length === 0 ? 0 : frontShift(s.rows, rows);
    s =
      d === null || d < 0
        ? { rows, first: 0, key: resetKey, mount: s.mount + 1 }
        : { ...s, rows, first: s.first + d };
  }
  shift.current = s;
  const latest = useRef(s);
  latest.current = s;

  useImperativeHandle(handle, () => ({
    scrollTo: (index) => {
      // Going to a match is moving away from the end.
      follow(false);
      target.current = { ord: latest.current.rows[index].ord, until: Date.now() + TARGET_MS };
      virtuoso.current?.scrollToIndex({ index, align: "center" });
    },
  }));

  // At the limit every batch adds as many rows as leave the front, so the count stays the same
  // and Virtuoso's own `followOutput` (it reacts to a growing count) does not fire: while
  // following, the view goes to the end after every change of the rows.
  // A match being scrolled to keeps its row, not its index: lines leaving the front move every
  // index, and Virtuoso retries a scroll with the index it was given while rows still change.
  const target = useRef<ScrollTarget | null>(null);
  useEffect(() => {
    const action = followAction(rows, followingRef.current, target.current, Date.now());
    if (action?.kind === "end") virtuoso.current?.scrollTo({ top: Number.MAX_SAFE_INTEGER });
    if (action?.kind === "index")
      virtuoso.current?.scrollToIndex({ index: action.index, align: "center" });
  }, [rows]);

  // Read only when the list (re)mounts: `rows` is left out on purpose.
  const initial = useMemo(
    () => initialIndex(rows, followingRef.current, topOrd.current),
    [s.mount, resetKey],
  );

  const context = useMemo<ListContext>(
    () => ({
      label: t("log.area"),
      onKeyDown: props.onKeyDown,
      onMouseDown: props.onMouseDown,
      onUserUp: () => {
        // The user's own move wins over a match still being scrolled to.
        target.current = null;
        follow(false);
      },
      end,
    }),
    [t, props.onKeyDown, props.onMouseDown, end],
  );

  return (
    <>
      <div className="absolute inset-0">
        <Virtuoso<Row, ListContext>
          key={`${s.mount}:${resetKey}`}
          ref={virtuoso}
          data={rows as Row[]}
          firstItemIndex={s.first}
          initialTopMostItemIndex={initial}
          computeItemKey={(_, row) => row.ord}
          itemContent={(_, row) => (
            <LogRow
              row={row}
              pattern={pattern}
              current={current && current.ord === row.ord ? current.nth : null}
              selected={allSelected && row.kind === "line"}
              clock={clock}
            />
          )}
          followOutput={() => (followingRef.current ? "auto" : false)}
          atBottomThreshold={8}
          atBottomStateChange={(bottom) => {
            if (bottom) follow(true);
          }}
          rangeChanged={(range) => {
            const { rows: r, first } = latest.current;
            topOrd.current = r[Math.max(0, range.startIndex - first)]?.ord ?? null;
          }}
          increaseViewportBy={200}
          context={context}
          components={COMPONENTS}
        />
      </div>
      {!following && (
        // Back at the end following resumes and the button goes (no animation, DESIGN.md 10.13).
        <Button
          variant="secondary"
          className="absolute right-3 bottom-3"
          onClick={() => {
            target.current = null;
            follow(true);
            virtuoso.current?.scrollToIndex({ index: "LAST", align: "end" });
          }}
        >
          <ArrowDownToLineIcon aria-hidden="true" />
          {t("log.jumpToEnd")}
        </Button>
      )}
    </>
  );
}

// ---- Runs ------------------------------------------------------------------------------------

/** The game state as a key that changes when a run starts or ends (the run list follows). */
const gameKeyOf = (g: GameStateDto | undefined): string => {
  if (!g) return "";
  if (g.state === "idle") return `idle:${g.data.lastExit?.runId ?? ""}`;
  if (g.state === "preparing") return "preparing";
  return `${g.state}:${g.data.runId}`;
};

function startText(since: number, language: string): string {
  const date = new Date(since);
  const today = new Date().toDateString() === date.toDateString();
  return new Intl.DateTimeFormat(
    language,
    today ? { timeStyle: "short" } : { dateStyle: "medium", timeStyle: "short" },
  ).format(date);
}

/**
 * DESIGN.md 10.13 run switcher: the newest run while it runs is "Aktueller Lauf", the newest
 * ended one "Letzter Lauf", one before that "Vorheriger Lauf".
 */
function runLabel(
  run: LogRunDto,
  index: number,
  list: LogRunDto[],
  name: (id: string) => string,
  t: TFunction,
  language: string,
): string {
  const values = { instance: isolate(name(run.instance)), time: startText(run.since, language) };
  if (index === 0 && run.state.state === "running") return t("log.run.current", values);
  const previous = index === 0 || list[0].state.state === "running";
  return t(previous ? "log.run.previous" : "log.run.earlier", values);
}

function endText(run: LogRunDto | undefined, t: TFunction): string {
  if (!run || run.state.state === "running") return t("log.end.output");
  if (run.state.stopRequested) return t("log.end.stopped");
  if (run.state.code === 0) return t("log.end.closed");
  return t("log.end.code", { code: exitCodeText(run.state.code) });
}

/**
 * DESIGN.md 10.13: a Windows status code (negative as a signed 32-bit number) in its usual unsigned
 * hex form, "0xC0000005"; -1 ("unknown") and non-negative codes as they are.
 */
export function exitCodeText(code: number): string {
  if (code >= 0 || code === -1) return String(code);
  return `0x${(code >>> 0).toString(16).toUpperCase().padStart(8, "0")}`;
}

// ---- The view --------------------------------------------------------------------------------

const SKELETON_WIDTHS = [
  "w-[62%]",
  "w-[40%]",
  "w-[85%]",
  "w-[54%]",
  "w-[90%]",
  "w-[47%]",
  "w-[73%]",
  "w-[66%]",
];

/** DESIGN.md 11.9 loading: eight skeleton lines of varying width, after 200 ms. */
function LineSkeleton() {
  return (
    <Delayed>
      <div aria-busy="true" className="flex flex-col gap-2 p-3">
        {SKELETON_WIDTHS.map((w) => (
          <Skeleton key={w} className={cn("h-3", w)} />
        ))}
      </div>
    </Delayed>
  );
}

const LOADING_ID = "log-loading";
const EMPTY_TITLE_ID = "log-empty-title";
const GONE_TITLE_ID = "log-gone-title";
const RUN_VALUE_ID = "log-run-value";

/** DESIGN.md 10.13 and 11.9: toolbar, run switcher and the virtualized log area. */
export function LogView() {
  const { t, i18n } = useTranslation();
  const runs = useLog((s) => s.runs);
  const chosen = useLog((s) => s.chosen);
  const attach = useLog((s) => s.attach);
  const feed = useLog((s) => s.feed);
  const feedRun = useLog((s) => s.feedRun);
  const levels = useLog((s) => s.levels);
  const query = useLog((s) => s.query);
  const current = useLog((s) => s.current);
  const copy = useLog((s) => s.copy);
  const omitted = useLog((s) => s.omitted);
  const openError = useLog((s) => s.openError);
  const allSelected = useLog((s) => s.allSelected);
  const gameKey = useApp((s) => gameKeyOf(s.game.data));
  const instances = useApp((s) => s.instances.data);
  const log = useLog.getState;
  const searchRef = useRef<HTMLInputElement>(null);
  const list = useRef<ListHandle>(null);

  // The run list follows every start and end of a game.
  useEffect(() => {
    void log().loadRuns();
  }, [gameKey, log]);
  useEffect(() => {
    const app = useApp.getState();
    if (app.instances.status === "idle") void app.loadInstances();
    if (app.game.status === "idle") void app.loadGame();
  }, []);

  const runList = runs.data;
  const runId = chosen ?? runList?.[0]?.runId ?? null;
  // One attach per shown run, detached when the run changes or the view goes (also balanced
  // under StrictMode's second effect run).
  useEffect(() => {
    if (runId === null) return;
    attachTo(runId);
    return () => detach();
  }, [runId]);

  // Ctrl+F focuses the search field. The release build cancels the browser's own find bar
  // (webviewDefaults.ts, capture phase); this listener only moves the focus.
  useEffect(() => {
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (!e.ctrlKey || e.altKey || e.metaKey || e.key.toLowerCase() !== "f") return;
      e.preventDefault();
      searchRef.current?.focus();
      searchRef.current?.select();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  const rows = feedRun === runId ? feed.rows : [];
  const visible = useMemo(() => visibleRows(rows, levels), [rows, levels]);
  // The field shows every keystroke at once; counting and marking follow when React has time
  // (with 10,000 lines that can be a frame later).
  const deferredQuery = useDeferredValue(query);
  const pattern = useMemo(() => searchPattern(deferredQuery), [deferredQuery]);
  const prefix = useMemo(() => hitPrefix(visible, pattern), [visible, pattern]);
  const hits = pattern ? prefix[visible.length] : null;
  const currentIndex = current && hits ? hitIndex(visible, prefix, current) : null;
  const currentHit = useMemo(() => {
    if (currentIndex === null) return null;
    const at = locateHit(prefix, currentIndex);
    return { ord: visible[at.index].ord, nth: at.nth };
  }, [currentIndex, prefix, visible]);

  const step = (direction: 1 | -1) => {
    if (!hits) return;
    const k =
      currentIndex === null
        ? direction === 1
          ? 0
          : hits - 1
        : (currentIndex + direction + hits) % hits;
    const at = locateHit(prefix, k);
    log().setCurrent({ ord: visible[at.index].ord, nth: at.nth });
    list.current?.scrollTo(at.index);
  };

  const write = (text: string) => navigator.clipboard.writeText(text);
  const copyLines = () => {
    if (runId !== null) void log().copyLines(runId, write);
  };

  // Ctrl+A selects every line of the filter (not only the rendered ones); Ctrl+C then copies
  // them through the backend, as "Kopieren" (DESIGN.md 11.9).
  const onAreaKey = useMemo(
    () => (e: KeyboardEvent<HTMLDivElement>) => {
      const mod = e.ctrlKey && !e.altKey && !e.metaKey;
      const key = e.key.toLowerCase();
      if (mod && key === "a") {
        e.preventDefault();
        window.getSelection()?.removeAllRanges();
        log().setAllSelected(true);
      } else if (mod && key === "c" && log().allSelected) {
        e.preventDefault();
        const run = log().attach;
        if (run.status !== "idle") void log().copyLines(run.runId, write);
      } else if (e.key === "Escape") log().setAllSelected(false);
    },
    [log],
  );
  const onAreaMouseDown = useMemo(() => () => log().setAllSelected(false), [log]);

  const name = (id: string) => instances?.find((i) => i.id === id)?.name ?? id;
  const selectedRun = runList?.find((r) => r.runId === runId);
  const ended = attach.status === "ended" && attach.runId === runId;
  const end = ended ? endText(selectedRun, t) : null;
  const levelsKey = Object.values(levels).join("");
  // DESIGN.md 11.9 and 10.0: the toolbar stays visible but does nothing until there are lines
  // to act on; each control names the visible reason (the loading line or the block's title).
  const loading = !runList
    ? runs.status !== "error"
    : runId !== null && attach.status === "attaching";
  const disabledReason =
    runList && runId === null
      ? EMPTY_TITLE_ID
      : attach.status === "gone" && attach.runId === runId
        ? GONE_TITLE_ID
        : loading
          ? LOADING_ID
          : null;

  let area: React.ReactNode;
  if (!runList) {
    area =
      runs.status === "error" && runs.error ? (
        <div className="p-4">
          <ErrorPanel error={runs.error} onRetry={() => log().loadRuns()} />
        </div>
      ) : (
        <LineSkeleton />
      );
  } else if (runId === null) {
    area = (
      <div className="flex max-w-120 flex-col items-start p-4">
        <h2 id={EMPTY_TITLE_ID} className="text-heading">
          {t("log.empty.title")}
        </h2>
        <p className="mt-2 text-body">{t("log.empty.body")}</p>
        <Button
          variant="primary"
          className="mt-4"
          onClick={() => useApp.getState().setView("play")}
        >
          {t("log.empty.openPlay")}
        </Button>
      </div>
    );
  } else if (attach.status === "gone" && attach.runId === runId) {
    area = (
      <div className="flex max-w-120 flex-col items-start p-4">
        <h2 id={GONE_TITLE_ID} className="text-heading">
          {t("log.gone.title")}
        </h2>
        <p className="mt-2 text-body">{t("log.gone.body")}</p>
      </div>
    );
  } else if (attach.status === "failed" && attach.runId === runId) {
    area = (
      <div className="max-w-160 p-4">
        <ErrorPanel error={attach.error} onRetry={() => attachTo(runId)} />
      </div>
    );
  } else if (rows.length === 0) {
    area =
      attach.status === "attaching" ? (
        <LineSkeleton />
      ) : (
        <div className="p-4">
          <p className="text-caption text-muted-foreground">{t("log.noOutput")}</p>
          {end && <p className="mt-2 text-caption text-muted-foreground">{end}</p>}
        </div>
      );
  } else if (visible.length === 0) {
    area = (
      <div className="flex flex-col items-start gap-2 p-4">
        <p className="text-small">{t("log.allHidden")}</p>
        <Button variant="ghost" className="-ml-4" onClick={() => log().showAllLevels()}>
          {t("log.showAllLevels")}
        </Button>
      </div>
    );
  } else {
    area = (
      <LogList
        rows={visible}
        resetKey={`${runId}:${levelsKey}`}
        pattern={pattern}
        current={currentHit}
        allSelected={allSelected}
        end={end}
        onKeyDown={onAreaKey}
        onMouseDown={onAreaMouseDown}
        handle={list}
      />
    );
  }

  return (
    <div className="flex flex-1 flex-col">
      <div className="mb-4">
        <LogToolbar
          disabledBy={disabledReason}
          query={query}
          onQuery={(q) => log().setQuery(q)}
          hits={hits}
          onStep={step}
          levels={levels}
          onLevel={(g, on) => log().setLevel(g, on)}
          copy={copy}
          onCopy={copyLines}
          onOpenFile={() => {
            if (runId !== null) void log().openFile(runId);
          }}
          searchRef={searchRef}
        />
        {loading && (
          <p id={LOADING_ID} className="mt-2 text-caption text-muted-foreground">
            {t("log.loading")}
          </p>
        )}
        {copy.status === "failed" && (
          <p className="mt-2 flex items-center gap-2 text-small text-danger">
            <CircleAlertIcon className="size-4 shrink-0" aria-hidden="true" />
            {t("log.copyFailed")}
          </p>
        )}
        {omitted > 0 && (
          <p className="mt-2 text-caption text-muted-foreground">
            {t("log.copyOmitted", { count: omitted })}
          </p>
        )}
        {openError && (
          <div className="mt-2 max-w-160">
            <ErrorPanel
              error={openError}
              dismiss={{ label: t("common.close"), onClick: () => log().dismissOpenError() }}
            />
          </div>
        )}
      </div>
      {runList && runList.length > 0 && (
        <Select<number>
          items={runList.map((r, i) => ({
            value: r.runId,
            label: runLabel(r, i, runList, name, t, i18n.language),
          }))}
          value={selectedRun ? selectedRun.runId : null}
          onValueChange={(v) => {
            if (v !== null) log().choose(v);
          }}
        >
          {/* Named by its visible value (WCAG 2.5.3), not by a hidden label. */}
          <SelectGhostTrigger aria-labelledby={RUN_VALUE_ID} className="-ml-3 mb-2 self-start">
            <SelectValue id={RUN_VALUE_ID} placeholder={t("log.run.choose")} />
          </SelectGhostTrigger>
          <SelectContent>
            {runList.map((r, i) => (
              <SelectItem key={r.runId} value={r.runId}>
                <SelectItemText className="min-w-0 truncate">
                  {runLabel(r, i, runList, name, t, i18n.language)}
                </SelectItemText>
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      )}
      <div className="relative min-h-60 flex-1 rounded-lg border bg-surface">{area}</div>
    </div>
  );
}
