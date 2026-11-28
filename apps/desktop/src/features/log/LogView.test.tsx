import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { StrictMode } from "react";
import { VirtuosoMockContext } from "react-virtuoso";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { initI18n } from "@/i18n";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";
import type { LogBatch } from "@/ipc/bindings/LogBatch";
import type { LogRunDto } from "@/ipc/bindings/LogRunDto";
import { useApp } from "@/stores/app";
import { instance } from "@/test/instancesBackend";
import { IDLE } from "@/test/playBackend";
import { resetLog, useLog } from "./logStore";
import { batch, line } from "./logStore.test";
import { LogView, printable } from "./LogView";

const initial = useApp.getState();

const run = (runId: number, extra: Partial<LogRunDto> = {}): LogRunDto => ({
  runId,
  instance: "survival",
  since: Date.now(),
  firstSeq: 1,
  nextSeq: 1,
  closed: false,
  state: { state: "running" },
  ...extra,
});

function logBackend(runs: LogRunDto[]) {
  const calls: [string, Record<string, unknown>][] = [];
  const channels: { onmessage?: (b: LogBatch) => void }[] = [];
  const b = {
    calls,
    runs,
    attachFails: null as ErrorDto | null,
    copyAnswer: { text: "copied text\n", lines: 1, omitted: 0 },
    openFails: null as ErrorDto | null,
    of: (cmd: string) => calls.filter((c) => c[0] === cmd).map((c) => c[1]),
    send: (batch: LogBatch) => channels[channels.length - 1].onmessage?.(batch),
  };
  mockIPC(async (cmd, payload) => {
    const args = (payload ?? {}) as Record<string, unknown>;
    if (cmd === "log_attach") {
      calls.push([cmd, { runId: args.runId, sinceSeq: args.sinceSeq }]);
      channels.push(args.channel as never);
      if (b.attachFails) throw b.attachFails;
      return null;
    }
    calls.push([cmd, args]);
    switch (cmd) {
      case "log_runs":
        return b.runs;
      case "log_copy":
        return b.copyAnswer;
      case "open_location":
        if (b.openFails) throw b.openFails;
        return null;
      case "instances_list":
        return { instances: [instance("survival", { name: "Überleben" })], incomplete: false };
      case "game_state":
        return IDLE;
      default:
        return null;
    }
  });
  return b;
}

let frames: FrameRequestCallback[] = [];
/** Runs animation frames until none is asked for (Virtuoso positions itself over several). */
const drain = () => {
  for (let i = 0; i < 20 && frames.length > 0; i++) frames.splice(0).forEach((f) => f(0));
};
/**
 * Runs animation frames until none is asked for. Virtuoso positions itself over several frames
 * and then waits for the scroll its positioning causes, which jsdom does not report: the test
 * sends that scroll event.
 */
const frame = () => {
  // React renders what a frame changed when `act` ends, so each round is its own `act`.
  for (let round = 0; round < 3; round++) {
    act(() => {
      drain();
      const scroller = document.querySelector("[data-virtuoso-scroller]");
      if (scroller) fireEvent.scroll(scroller);
    });
  }
};
let clipboard: string[];

beforeEach(() => {
  initI18n("en", "en-US");
  useApp.setState(initial, true);
  resetLog();
  frames = [];
  vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => frames.push(cb));
  vi.stubGlobal("cancelAnimationFrame", () => {});
  // A fake clipboard: the real one is never touched.
  clipboard = [];
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText: async (t: string) => void clipboard.push(t) },
  });
});
afterEach(() => {
  cleanup();
  clearMocks();
  resetLog();
  vi.unstubAllGlobals();
});

function view(strict = false) {
  const tree = (
    <VirtuosoMockContext.Provider value={{ viewportHeight: 400, itemHeight: 20 }}>
      <TooltipProvider delay={0}>
        <LogView />
      </TooltipProvider>
    </VirtuosoMockContext.Provider>
  );
  return render(strict ? <StrictMode>{tree}</StrictMode> : tree);
}

async function attached(runs = [run(7)], strict = false) {
  const b = logBackend(runs);
  const r = view(strict);
  await waitFor(() => expect(b.of("log_attach").length).toBeGreaterThan(0));
  await act(async () => {});
  return { b, ...r };
}

const area = () => screen.getByRole("region", { name: "Log lines" });

describe("log view", () => {
  it("without a run: the empty state, Open Play, and an inert toolbar", async () => {
    const b = logBackend([]);
    view();
    expect(await screen.findByText("No log yet")).toBeTruthy();
    expect(screen.getByText("Start an instance and its output appears here.")).toBeTruthy();
    expect(screen.getByLabelText("Search log").closest("[inert]")).not.toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Open Play" }));
    expect(useApp.getState().view).toBe("play");
    expect(b.of("log_attach")).toEqual([]);
  });

  it("shows lines in the grid with time, level word and thread; a run without lines says so", async () => {
    const { b } = await attached();
    expect(screen.getByText("No output yet")).toBeTruthy();
    act(() =>
      b.send(
        batch([
          line(1, "Setting user: Mira_K", {
            time: { kind: "clock", hour: 14, minute: 5, second: 9 },
            level: "warn",
            thread: "Render thread",
          }),
        ]),
      ),
    );
    frame();
    const row = area().querySelector('[data-seq="1"]')!;
    expect(row.textContent).toContain("14:05:09");
    expect(row.textContent).toContain("WARN");
    expect(row.textContent).toContain("Render thread");
    expect(row.textContent).toContain("Setting user: Mira_K");
    expect(row.querySelector(".text-log-warn")).not.toBeNull();
  });

  it("renders attacker text strictly as text", async () => {
    const { b } = await attached();
    const evil =
      '<img src=x onerror="alert(1)"> https://example.com \u001b[31mred\u001b[0m **bold** ‮evil\u0007';
    act(() => b.send(batch([line(1, evil, { thread: "<b>t</b>‮" })])));
    frame();
    const row = area().querySelector('[data-seq="1"]')!;
    expect(row.querySelector("img, a, b, strong")).toBeNull();
    expect(row.textContent).toContain('<img src=x onerror="alert(1)">');
    expect(row.textContent).toContain("**bold**");
    // ESC and BEL are shown as their pictures, the escape sequence is not interpreted.
    expect(row.textContent).toContain("␛[31mred");
    expect(row.textContent).toContain("␇");
    // Every cell is its own bidi isolate.
    for (const cell of row.children) expect(cell.className).toContain("[unicode-bidi:isolate]");
    expect(printable("a\tb\nc\r\u0085")).toBe("a\tb\nc␍�");
  });

  it("joins parts and shows gaps, the cut hint and the limit note", async () => {
    const { b } = await attached();
    act(() => {
      b.send(batch([line(3, "first ", { more: true })], { dropped: 2 }));
      b.send(batch([line(3, "second", { part: 1, cut: true })]));
    });
    frame();
    expect(screen.getByText("2 lines skipped")).toBeTruthy();
    const row = area().querySelector('[data-seq="3"]')!;
    expect(row.textContent).toContain("first second");
    expect(within(row as HTMLElement).getByText("Line shortened")).toBeTruthy();
  });

  it("filters by level: the count in the label, all hidden, show all levels", async () => {
    const { b } = await attached();
    act(() => b.send(batch([line(1, "an info", { level: "info" })])));
    frame();
    fireEvent.click(screen.getByRole("button", { name: "Levels: 5 of 5" }));
    fireEvent.click(await screen.findByRole("menuitemcheckbox", { name: "Info" }));
    expect(screen.getByRole("button", { name: "Levels: 4 of 5" })).toBeTruthy();
    expect(screen.getByText("All lines are hidden.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Show all levels" }));
    frame();
    expect(screen.getByText("an info")).toBeTruthy();
  });

  it("search marks matches with text nodes, counts them and steps with wrap-around", async () => {
    const { b } = await attached();
    act(() => b.send(batch([line(1, "Boom <b>boom</b>"), line(2, "quiet"), line(3, "BOOM")])));
    frame();
    const field = screen.getByLabelText("Search log");
    fireEvent.change(field, { target: { value: "boom" } });
    expect(screen.getByText("3 matches")).toBeTruthy();
    const marks = () => [...area().querySelectorAll("mark")];
    expect(marks().map((m) => m.textContent)).toEqual(["Boom", "boom", "BOOM"]);
    const current = () => marks().findIndex((m) => m.className.includes("outline"));
    expect(current()).toBe(-1);
    fireEvent.keyDown(field, { key: "Enter" });
    expect(current()).toBe(0);
    fireEvent.keyDown(field, { key: "Enter", shiftKey: true });
    expect(current()).toBe(2);
    fireEvent.click(screen.getByRole("button", { name: "Next match" }));
    expect(current()).toBe(0);
    // Escape clears the search.
    fireEvent.keyDown(field, { key: "Escape" });
    expect((field as HTMLInputElement).value).toBe("");
    expect(marks()).toEqual([]);
  });

  it("Ctrl+F focuses the search field", async () => {
    await attached();
    fireEvent.keyDown(document.body, { key: "f", ctrlKey: true });
    expect(document.activeElement).toBe(screen.getByLabelText("Search log"));
  });

  it("following stops only when the user moves up; Jump to end resumes it", async () => {
    const { b } = await attached();
    act(() => b.send(batch(Array.from({ length: 60 }, (_, i) => line(i + 1)))));
    frame();
    const jump = () => screen.queryByRole("button", { name: "Jump to end" });
    // New lines alone never stop following.
    act(() => b.send(batch([line(61)])));
    frame();
    expect(jump()).toBeNull();
    fireEvent.wheel(area(), { deltaY: 120 });
    expect(jump()).toBeNull();
    fireEvent.wheel(area(), { deltaY: -120 });
    expect(jump()).not.toBeNull();
    fireEvent.keyDown(area(), { key: "End" });
    expect(jump()).not.toBeNull();
    fireEvent.click(jump()!);
    expect(jump()).toBeNull();
    fireEvent.keyDown(area(), { key: "PageUp" });
    expect(jump()).not.toBeNull();
  });

  it("the log area is focusable and not a live region", async () => {
    const { b } = await attached();
    act(() => b.send(batch([line(1)])));
    frame();
    expect(area().tabIndex).toBe(0);
    expect(area().getAttribute("aria-live")).toBeNull();
    expect(area().closest("[aria-live]")).toBeNull();
  });

  it("copy writes what the backend made to the clipboard and says when lines are left out", async () => {
    const { b } = await attached();
    b.copyAnswer = { text: "a\n", lines: 1, omitted: 3 };
    fireEvent.click(screen.getByRole("button", { name: "Copy" }));
    await waitFor(() => expect(clipboard).toEqual(["a\n"]));
    expect(b.of("log_copy")[0]).toEqual({
      runId: 7,
      filter: { error: true, warning: true, info: true, debug: true, other: true, text: "" },
    });
    const note = await screen.findByText(/^The oldest 3 lines are missing because at most 4/);
    // The unit keeps its no-break space (DESIGN.md 3.3).
    expect(note.textContent).toBe(
      "The oldest 3 lines are missing because at most 4 MB are copied.",
    );
  });

  it("Ctrl+A in the log area then Ctrl+C copies every line of the filter", async () => {
    const { b } = await attached();
    act(() => b.send(batch([line(1)])));
    frame();
    fireEvent.keyDown(area(), { key: "a", ctrlKey: true });
    expect(useLog.getState().allSelected).toBe(true);
    expect(area().querySelector(".bg-selection")).not.toBeNull();
    fireEvent.keyDown(area(), { key: "c", ctrlKey: true });
    await waitFor(() => expect(clipboard).toEqual(["copied text\n"]));
    fireEvent.pointerDown(area());
    expect(useLog.getState().allSelected).toBe(false);
  });

  it("opens the run's log file; a failure shows its panel", async () => {
    const { b } = await attached();
    b.openFails = { code: "openFailed", params: {}, action: null, detail: "" };
    fireEvent.click(screen.getByRole("button", { name: "Open log file" }));
    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(b.of("open_location")).toEqual([{ target: { kind: "gameLog", runId: 7 } }]);
  });

  it("the end of the run shows its exit from the run list", async () => {
    const { b } = await attached();
    b.runs = [run(7, { closed: true, state: { state: "ended", code: 3, stopRequested: false } })];
    act(() => b.send(batch([line(1)], { end: "complete" })));
    frame();
    expect(await screen.findByText("The game exited with code 3.")).toBeTruthy();
  });

  it("an expired run or an unknown run id: log no longer available", async () => {
    const { b } = await attached();
    act(() => b.send(batch([], { end: "expired" })));
    frame();
    expect(screen.getByText("Log no longer available")).toBeTruthy();
    cleanup();
    resetLog();
    const b2 = logBackend([run(9)]);
    b2.attachFails = { code: "invalidInput", params: { field: "runId" }, action: null, detail: "" };
    view();
    expect(await screen.findByText("Log no longer available")).toBeTruthy();
  });

  it("a failed attach shows the panel and Try again attaches again", async () => {
    const b = logBackend([run(7)]);
    b.attachFails = { code: "internal", params: {}, action: null, detail: "x" };
    view();
    fireEvent.click(await screen.findByRole("button", { name: "Try again" }));
    await waitFor(() => expect(b.of("log_attach")).toHaveLength(2));
  });

  it("the run switcher names the current run with its instance and start", async () => {
    await attached([run(8), run(7, { state: { state: "ended", code: 0, stopRequested: false } })]);
    const trigger = screen.getByRole("combobox", { name: "Run" });
    await waitFor(() => expect(trigger.textContent).toMatch(/^Current run: ⁨Überleben⁩, since /));
  });

  it("leaving detaches; coming back continues after the last whole line", async () => {
    const { b, unmount } = await attached();
    act(() => b.send(batch([line(1), line(2)])));
    frame();
    unmount();
    await waitFor(() => expect(b.of("log_detach")).toHaveLength(1));
    view();
    await waitFor(() => expect(b.of("log_attach")).toHaveLength(2));
    expect(b.of("log_attach")[1]).toEqual({ runId: 7, sinceSeq: 3 });
  });

  it("under StrictMode attach and detach stay balanced: one stream stays", async () => {
    const { b } = await attached([run(7)], true);
    await act(async () => {});
    const order = b.calls.map((c) => c[0]).filter((c) => c === "log_attach" || c === "log_detach");
    expect(order.at(-1)).toBe("log_attach");
    act(() => b.send(batch([line(1, "once")])));
    frame();
    expect(screen.getAllByText("once")).toHaveLength(1);
  });
});
