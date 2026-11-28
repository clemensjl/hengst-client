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
import { batch, line } from "@/test/logFixtures";
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
    /** Held until resolved: `log_attach` answers only then. */
    attachGate: null as Promise<void> | null,
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
      if (b.attachGate) await b.attachGate;
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

/** Every control of the toolbar is aria-disabled and described by the visible `reason`. */
function expectInertToolbar(reason: string) {
  const controls = [
    screen.getByLabelText("Search log"),
    screen.getByRole("button", { name: "Previous match" }),
    screen.getByRole("button", { name: "Next match" }),
    screen.getByRole("button", { name: /^Levels: / }),
    screen.getByRole("button", { name: "Copy" }),
    screen.getByRole("button", { name: "Open log file" }),
  ];
  for (const c of controls) {
    expect(c.getAttribute("aria-disabled"), c.textContent ?? "").toBe("true");
    const ids = (c.getAttribute("aria-describedby") ?? "").split(" ");
    const texts = ids.map((id) => document.getElementById(id)?.textContent);
    expect(texts, c.textContent ?? "").toContain(reason);
  }
}

describe("log view", () => {
  it("without a run: the empty state, Open Play, and a toolbar that is inert with its reason", async () => {
    const b = logBackend([]);
    view();
    expect(await screen.findByText("No log yet")).toBeTruthy();
    expect(screen.getByText("Start an instance and its output appears here.")).toBeTruthy();
    expectInertToolbar("No log yet");
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
    // The text of every cell is inside its own <bdi>, so an unterminated U+202E cannot run past
    // it: not into the neighbouring cells, not into the cut hint after the message.
    const [time, level, thread, message] = [...row.children];
    for (const cell of [time, level, thread, message])
      expect(cell.querySelector("bdi")).not.toBeNull();
    expect(thread.querySelector("bdi")!.textContent).toBe("<b>t</b>\u202E");
    expect(message.querySelector("bdi")!.textContent).toBe(printable(evil));
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
    const hint = within(row as HTMLElement).getByText("Line shortened");
    // The hint is outside the message's isolate, so the message's own direction cannot reach it.
    expect(hint.closest("bdi")).toBeNull();
    expect(row.children[3].querySelector("bdi")!.textContent).toBe("first second");
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

  it("a long thread name shows the whole name in a tooltip, a short one has none", async () => {
    const { b } = await attached();
    const wide = vi.spyOn(HTMLElement.prototype, "scrollWidth", "get").mockImplementation(function (
      this: HTMLElement,
    ) {
      return this.textContent === "Netty Server IO #12 with a long name" ? 400 : 0;
    });
    try {
      act(() =>
        b.send(
          batch([
            line(1, "a", { thread: "Netty Server IO #12 with a long name" }),
            line(2, "b", { thread: "main" }),
          ]),
        ),
      );
      frame();
      const cell = (seq: number) =>
        area().querySelector(`[data-seq="${seq}"]`)!.children[2] as HTMLElement;
      expect(cell(1).getAttribute("title")).toBeNull();
      expect(cell(2).getAttribute("title")).toBeNull();
      fireEvent.pointerEnter(cell(1).firstElementChild!);
      fireEvent.mouseEnter(cell(1).firstElementChild!);
      await waitFor(() =>
        expect(document.querySelector("[data-slot=tooltip-content]")?.textContent).toBe(
          "Netty Server IO #12 with a long name",
        ),
      );
      expect(cell(2).querySelector("[data-slot=tooltip-trigger]")).toBeNull();
    } finally {
      wide.mockRestore();
    }
  });

  it("while attaching the toolbar is inert and says why; Copy does nothing", async () => {
    const b = logBackend([run(7)]);
    let release!: () => void;
    b.attachGate = new Promise((r) => (release = r));
    view();
    expect(await screen.findByText("Loading the log…")).toBeTruthy();
    expectInertToolbar("Loading the log…");
    fireEvent.click(screen.getByRole("button", { name: "Copy" }));
    expect(b.of("log_copy")).toEqual([]);
    act(() => release());
    await waitFor(() => expect(screen.queryByText("Loading the log…")).toBeNull());
    expect(screen.getByLabelText("Search log").getAttribute("aria-disabled")).toBeNull();
  });

  it("Escape in the log area ends a select-all", async () => {
    const { b } = await attached();
    act(() => b.send(batch([line(1)])));
    frame();
    fireEvent.keyDown(area(), { key: "a", ctrlKey: true });
    expect(area().querySelector(".bg-selection")).not.toBeNull();
    fireEvent.keyDown(area(), { key: "Escape" });
    expect(area().querySelector(".bg-selection")).toBeNull();
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
    expect(screen.getByRole("button", { name: "Copied" })).toBeTruthy();
    const note = await screen.findByText(/^The oldest 3 lines are missing because at most 4/);
    // The unit keeps its no-break space (DESIGN.md 3.3).
    expect(note.textContent).toBe(
      "The oldest 3 lines are missing because at most 4\u00a0MiB are copied.",
    );
    // "Copied" goes back after 2 s; the note stays.
    await waitFor(() => expect(screen.getByRole("button", { name: "Copy" })).toBeTruthy(), {
      timeout: 3000,
    });
    expect(screen.getByText(/^The oldest 3 lines are missing/)).toBeTruthy();
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
    // Its lines go with it (DESIGN.md 11.9): the block takes the log area.
    expect(document.querySelector("[data-seq]")).toBeNull();
    expectInertToolbar("Log no longer available");
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
    // WCAG 2.5.3: the accessible name is the visible value, not a separate "Run" label.
    const trigger = await screen.findByRole("combobox", {
      name: /^Current run: ⁨Überleben⁩, since /,
    });
    expect(trigger.getAttribute("aria-label")).toBeNull();
    expect(trigger.textContent).toMatch(/^Current run: ⁨Überleben⁩, since /);
  });

  it("follows the newest run when a second game starts and no run was chosen", async () => {
    const { b } = await attached([run(7)]);
    b.runs = [run(8), run(7, { state: { state: "ended", code: 0, stopRequested: false } })];
    act(() =>
      useApp.setState({
        game: {
          status: "ready",
          data: { state: "running", data: { instance: "survival", runId: 8, since: Date.now() } },
        },
      }),
    );
    await waitFor(() => expect(b.of("log_attach").at(-1)?.runId).toBe(8));
    expect(useLog.getState().chosen).toBeNull();
  });

  it.each([
    [-1073741819, "The game exited with code 0xC0000005."],
    [-1, "The game exited with code -1."],
    [3, "The game exited with code 3."],
  ])("the end of a run with code %i reads %s", async (code, text) => {
    const { b } = await attached();
    b.runs = [run(7, { closed: true, state: { state: "ended", code, stopRequested: false } })];
    act(() => b.send(batch([line(1)], { end: "complete" })));
    frame();
    expect(await screen.findByText(text)).toBeTruthy();
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
