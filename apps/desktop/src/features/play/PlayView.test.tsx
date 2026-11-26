import { clearMocks } from "@tauri-apps/api/mocks";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { initI18n } from "@/i18n";
import type { GameStateDto } from "@/ipc/bindings/GameStateDto";
import type { LastExit } from "@/ipc/bindings/LastExit";
import { useApp } from "@/stores/app";
import { account } from "@/test/fakeBackend";
import { instance } from "@/test/instancesBackend";
import { IDLE, mira, playBackend } from "@/test/playBackend";
import { AccountChip } from "@/features/accounts/AccountChip";
import { PlayView } from "./PlayView";

const initial = useApp.getState();
const running = (runId = 1, since = Date.UTC(2026, 10, 23, 13, 5)): GameStateDto => ({
  state: "running",
  data: { instance: "survival", runId, since },
});
const stopping = (runId = 1): GameStateDto => ({
  state: "stopping",
  data: { instance: "survival", runId },
});
const preparing: GameStateDto = { state: "preparing", data: { instance: "survival" } };
const exited = (code: number, stopRequested: boolean, runId = 1): GameStateDto => ({
  state: "idle",
  data: { lastExit: { instance: "survival", runId, code, stopRequested } satisfies LastExit },
});

beforeEach(() => {
  initI18n("en", "en-US");
  useApp.setState(initial, true);
  Element.prototype.scrollIntoView = vi.fn();
});
afterEach(async () => {
  vi.useRealTimers();
  cleanup();
  clearMocks();
  await new Promise((r) => setTimeout(r, 30));
});

/** The start page as the app mounts it, with instances, accounts and game state loaded. */
async function setup(options: Parameters<typeof playBackend>[0] = {}) {
  const b = playBackend(options);
  await useApp.getState().startEventLoop();
  await useApp.getState().loadAccounts();
  render(
    <TooltipProvider delay={0}>
      <PlayView />
    </TooltipProvider>,
  );
  await waitFor(() => expect(document.querySelector('[aria-busy="true"]')).toBeNull());
  await waitFor(() => expect(document.querySelector("[data-slot=button]")).not.toBeNull());
  return b;
}

const playButton = () =>
  screen.getByRole("button", {
    name: /^(Play|Stop|Cancel|Starting…|Stopping…|Create instance|Sign in|Sign in again|Choose instance|Choose account)$/,
  });
const flush = () => act(async () => {});

describe("start page", () => {
  it("shows the instance with its meta line and plays it", async () => {
    const b = await setup({
      instances: [
        instance("survival", {
          name: "Überleben 1.21",
          selected: true,
          loader: { kind: "fabric", displayName: "Fabric", version: "0.17.3" },
        }),
      ],
    });
    expect(screen.getByRole("heading", { name: "Überleben 1.21" })).toBeTruthy();
    expect(screen.getByText("1.21.11")).toBeTruthy();
    expect(screen.getByText("Fabric 0.17.3")).toBeTruthy();
    expect(screen.getByText("Never played")).toBeTruthy();
    // One instance: no switcher.
    expect(screen.queryByRole("button", { name: "Switch instance" })).toBeNull();
    fireEvent.click(playButton());
    await flush();
    expect(b.of("play")[0].instanceId).toBe("survival");
  });

  it("the button is never disabled", async () => {
    await setup({ game: stopping() });
    expect(playButton().hasAttribute("disabled")).toBe(false);
  });

  it("double click starts once", async () => {
    const b = await setup();
    const button = playButton();
    fireEvent.click(button);
    fireEvent.click(button);
    await flush();
    expect(b.count("play")).toBe(1);
  });

  it("starting_is_inert", async () => {
    const b = await setup();
    fireEvent.click(playButton());
    await flush();
    const button = playButton();
    expect(button.textContent).toBe("Starting…");
    expect(button.getAttribute("aria-disabled")).toBe("true");
    const reason = document.getElementById(button.getAttribute("aria-describedby")!)!;
    expect(reason.textContent).toMatch(/cannot be canceled right now/);
    // Inert: no hover fill, no pressed fill, no press scale.
    expect(button.className).toContain("aria-disabled:hover:bg-primary");
    expect(button.className).toContain("aria-disabled:active:bg-primary");
    expect(button.className).not.toContain("play-press");
    fireEvent.click(button);
    await flush();
    expect(b.count("play")).toBe(1);
    expect(b.count("play_cancel")).toBe(0);
    // The progress area is there from the first moment, at its fixed height.
    expect(document.querySelector('[data-slot="launch-progress"]')?.className).toContain("h-40");
  });

  it("cancel_ignores_clicks_for_600_ms_after_the_change", async () => {
    const b = await setup();
    vi.useFakeTimers({ toFake: ["Date", "setTimeout", "clearTimeout"] });
    fireEvent.click(playButton());
    await flush();
    act(() => b.play({ event: "preparing", data: { op: 1 } }));
    expect(playButton().textContent).toBe("Cancel");
    act(() => vi.advanceTimersByTime(300));
    fireEvent.click(playButton());
    await flush();
    expect(b.count("play_cancel")).toBe(0);
    act(() => vi.advanceTimersByTime(299));
    fireEvent.click(playButton());
    await flush();
    expect(b.count("play_cancel")).toBe(0);
    act(() => vi.advanceTimersByTime(1));
    fireEvent.click(playButton());
    await flush();
    expect(b.of("play_cancel")).toEqual([{ op: 1 }]);
    act(() => b.play({ event: "cancelled" }));
    expect(playButton().textContent).toBe("Play");
  });

  it("a cancel that came too late keeps showing the start, never 'cancelled'", async () => {
    const b = await setup();
    vi.useFakeTimers({ toFake: ["Date", "setTimeout", "clearTimeout"] });
    fireEvent.click(playButton());
    await flush();
    act(() => b.play({ event: "preparing", data: { op: 1 } }));
    b.cancelAnswer = false;
    act(() => vi.advanceTimersByTime(600));
    fireEvent.click(playButton());
    await flush();
    expect(playButton().textContent).toBe("Starting…");
    act(() => b.state(running()));
    act(() => b.play({ event: "started", data: { runId: 1 } }));
    expect(playButton().textContent).toBe("Stop");
  });

  it.each([
    ["started first", true],
    ["running first", false],
  ])("ends in running whatever the order of the two channels (%s)", async (_, startedFirst) => {
    const b = await setup();
    fireEvent.click(playButton());
    await flush();
    act(() => b.play({ event: "preparing", data: { op: 1 } }));
    act(() => b.state(preparing));
    if (startedFirst) {
      act(() => b.play({ event: "started", data: { runId: 1 } }));
      // Started, but the game state is not running yet: still starting, no progress gone.
      expect(playButton().textContent).toBe("Starting…");
      act(() => b.state(running()));
    } else {
      act(() => b.state(running()));
      expect(playButton().textContent).toBe("Stop");
      act(() => b.play({ event: "started", data: { runId: 1 } }));
    }
    expect(playButton().textContent).toBe("Stop");
    expect(screen.getByText(/^Running since /)).toBeTruthy();
    expect(document.querySelector('[data-slot="launch-progress"]')).toBeNull();
    // And back to Play when it ends normally.
    act(() => b.state(exited(0, false)));
    expect(playButton().textContent).toBe("Play");
  });

  it("shows the phase, the file count and the installer step, and opens the reported log", async () => {
    const b = await setup();
    fireEvent.click(playButton());
    await flush();
    act(() => b.play({ event: "preparing", data: { op: 1 } }));
    act(() => b.play({ event: "downloading", data: { what: "gameFiles", count: 4118 } }));
    expect(screen.getByText("Downloading files")).toBeTruthy();
    expect(screen.getByText("4,118 files")).toBeTruthy();
    expect(screen.queryByRole("progressbar")).toBeNull();
    act(() => b.play({ event: "installLog", data: { path: "logs/loader-install-forge-x.log" } }));
    // A step name from remote metadata with markup and a bidi override stays plain text.
    const hostile = "<img src=x onerror=alert(1)>‮exe.gol";
    act(() => b.play({ event: "installStep", data: { index: 3, total: 6, name: hostile } }));
    expect(screen.getByText("Installing")).toBeTruthy();
    expect(screen.getByText("Step 3 of 6:")).toBeTruthy();
    const name = screen.getByText(hostile);
    expect(name.tagName).toBe("BDI");
    expect(document.querySelector("img")).toBeNull();
    const bar = screen.getByRole("progressbar");
    expect(bar.getAttribute("aria-valuenow")).toBe("2");
    expect(bar.getAttribute("aria-valuemax")).toBe("6");
    fireEvent.click(screen.getByRole("button", { name: "Open install log" }));
    await flush();
    expect(b.of("open_location")).toEqual([
      { target: { kind: "logFile", path: "logs/loader-install-forge-x.log" } },
    ]);
  });

  it("a failed install offers the reported log in its panel", async () => {
    const b = await setup();
    fireEvent.click(playButton());
    await flush();
    act(() => b.play({ event: "preparing", data: { op: 1 } }));
    act(() => b.play({ event: "installLog", data: { path: "logs/i.log" } }));
    act(() =>
      b.play({
        event: "failed",
        data: { error: { code: "installStepFailed", params: {}, action: null, detail: "d" } },
      }),
    );
    const panel = screen.getByRole("alert");
    expect(within(panel).getByText("An install step failed")).toBeTruthy();
    fireEvent.click(within(panel).getByRole("button", { name: "Open install log" }));
    await flush();
    expect(b.of("open_location")).toEqual([{ target: { kind: "logFile", path: "logs/i.log" } }]);
    fireEvent.click(within(panel).getByRole("button", { name: "Try again" }));
    await flush();
    expect(b.count("play")).toBe(2);
  });

  it("offline_with_missing_files_shows_no_connection", async () => {
    const b = await setup();
    fireEvent.click(playButton());
    await flush();
    act(() => b.play({ event: "preparing", data: { op: 1 } }));
    act(() =>
      b.play({
        event: "failed",
        data: { error: { code: "offline", params: {}, action: { kind: "retry" }, detail: "d" } },
      }),
    );
    const panel = screen.getByRole("alert");
    expect(within(panel).getByText("No connection")).toBeTruthy();
    expect(
      within(panel).getByText(
        "Files for this version are missing. Check your internet connection and try again.",
      ),
    ).toBeTruthy();
    expect(within(panel).getByRole("button", { name: "Try again" })).toBeTruthy();
    expect(document.querySelector('[data-slot="launch-progress"]')).toBeNull();
    expect(playButton().textContent).toBe("Play");
  });

  it("exit_code_zero_returns_silently_and_nonzero_shows_the_panel", async () => {
    const b = await setup({ game: running() });
    act(() => b.state(exited(0, false)));
    expect(screen.queryByRole("alert")).toBeNull();
    expect(playButton().textContent).toBe("Play");
    act(() => b.state(running(2)));
    act(() => b.state(exited(3, false, 2)));
    const panel = screen.getByRole("alert");
    expect(within(panel).getByText("The game closed unexpectedly")).toBeTruthy();
    expect(within(panel).getByText("The log shows what happened.")).toBeTruthy();
    // No detail text: no empty "Details".
    expect(within(panel).queryByRole("button", { name: "Details" })).toBeNull();
    fireEvent.click(within(panel).getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByRole("alert")).toBeNull();
    // An unknown code (-1) without a requested stop is unexpected too.
    act(() => b.state(running(3)));
    act(() => b.state(exited(-1, false, 3)));
    expect(screen.getByRole("alert")).toBeTruthy();
  });

  it("stop_requested_with_exit_code_1_shows_no_error_panel", async () => {
    const b = await setup({ game: running() });
    act(() => b.state(stopping()));
    act(() => b.state(exited(1, true)));
    expect(screen.queryByRole("alert")).toBeNull();
    expect(playButton().textContent).toBe("Play");
  });

  it("force_stop_appears_after_10_seconds_in_stopping", async () => {
    const b = await setup({ game: running() });
    vi.useFakeTimers({ toFake: ["Date", "setTimeout", "clearTimeout"] });
    act(() => b.state(stopping()));
    const button = playButton();
    expect(button.textContent).toBe("Stopping…");
    expect(button.getAttribute("aria-disabled")).toBe("true");
    act(() => vi.advanceTimersByTime(9_999));
    expect(screen.queryByRole("button", { name: "Force stop" })).toBeNull();
    act(() => vi.advanceTimersByTime(1));
    const force = screen.getByRole("button", { name: "Force stop" });
    expect(force.getAttribute("data-variant")).toBe("destructive");
    // It stays as long as the state lasts.
    act(() => vi.advanceTimersByTime(60_000));
    fireEvent.click(screen.getByRole("button", { name: "Force stop" }));
    await flush();
    expect(b.count("game_force_stop")).toBe(1);
    act(() => b.state(exited(1, true)));
    expect(screen.queryByRole("button", { name: "Force stop" })).toBeNull();
  });

  it("stop with no window says so and offers the force stop at once", async () => {
    const b = await setup({ game: running() });
    b.stopAnswer = "noWindow";
    fireEvent.click(playButton());
    await flush();
    expect(b.count("game_stop")).toBe(1);
    expect(screen.getByRole("status").textContent).toBe(
      "The game has no window yet and was not asked to close.",
    );
    expect(playButton().textContent).toBe("Stop");
    expect(screen.getByRole("button", { name: "Force stop" })).toBeTruthy();
  });

  it("a double click on Stop asks the game once", async () => {
    const b = await setup({ game: running() });
    fireEvent.click(playButton());
    fireEvent.click(playButton());
    await flush();
    expect(b.count("game_stop")).toBe(1);
  });
});

describe("after a reload the page rebuilds from game_state()", () => {
  it("preparing: starting, with the progress area and nothing to cancel", async () => {
    const b = await setup({ game: preparing });
    expect(playButton().textContent).toBe("Starting…");
    expect(screen.getByText("Preparing")).toBeTruthy();
    fireEvent.click(playButton());
    await flush();
    expect(b.count("play_cancel") + b.count("play")).toBe(0);
  });

  it("running: Stop with the start time", async () => {
    initI18n("de", "de-AT");
    await setup({ game: running(1, new Date(2026, 10, 23, 14, 5).getTime()) });
    expect(screen.getByRole("button", { name: "Beenden" })).toBeTruthy();
    expect(screen.getByText("Läuft seit 14:05")).toBeTruthy();
  });

  it("stopping: the 10 s start again from the reload", async () => {
    vi.useFakeTimers({ toFake: ["Date", "setTimeout", "clearTimeout"], shouldAdvanceTime: true });
    await setup({ game: stopping() });
    expect(playButton().textContent).toBe("Stopping…");
    expect(screen.queryByRole("button", { name: "Force stop" })).toBeNull();
    act(() => vi.advanceTimersByTime(10_000));
    expect(screen.getByRole("button", { name: "Force stop" })).toBeTruthy();
  });

  it("idle with an unexpected last exit: the panel; with a requested one: nothing", async () => {
    await setup({ game: exited(3, false) });
    expect(screen.getByText("The game closed unexpectedly")).toBeTruthy();
    cleanup();
    useApp.setState(initial, true);
    await setup({ game: exited(1, true) });
    expect(screen.queryByRole("alert")).toBeNull();
    expect(playButton().textContent).toBe("Play");
  });

  it("the running instance is shown even if another one is selected", async () => {
    await setup({
      instances: [
        instance("survival", { name: "Überleben" }),
        instance("other", { name: "Andere", selected: true }),
      ],
      game: running(),
    });
    expect(screen.getByRole("heading", { name: "Überleben" })).toBeTruthy();
    expect(useApp.getState().activeInstance).toBe("survival");
  });
});

describe("states without a game", () => {
  it("no instance: the empty state and Create instance", async () => {
    await setup({ instances: [] });
    expect(screen.getByRole("heading", { name: "No instances yet" })).toBeTruthy();
    expect(
      screen.getByText("Create an instance, then sign in with Microsoft to play."),
    ).toBeTruthy();
    fireEvent.click(playButton());
    expect(playButton().textContent).toBe("Create instance");
    expect(useApp.getState().createOpen).toBe(true);
  });

  it("only unreadable instances: Create instance with the caption", async () => {
    await setup({ instances: [instance("x", { status: "broken" })] });
    expect(playButton().textContent).toBe("Create instance");
    expect(screen.getByText("No instance can be started.")).toBeTruthy();
  });

  it("selected instance unreadable: Choose instance opens the switcher", async () => {
    await setup({
      instances: [instance("x", { status: "tooNew", name: "Alt", selected: true }), instance("b")],
    });
    expect(screen.getByText("\u2068Alt\u2069 cannot be started.")).toBeTruthy();
    fireEvent.click(playButton());
    expect(await screen.findByRole("menu")).toBeTruthy();
    expect(screen.getByRole("menuitemradio", { name: /Alt/ }).getAttribute("aria-disabled")).toBe(
      "true",
    );
  });

  it("switching the instance selects it", async () => {
    const b = await setup({
      instances: [instance("a", { name: "Eins", selected: true }), instance("b", { name: "Zwei" })],
    });
    fireEvent.click(screen.getByRole("button", { name: "Switch instance" }));
    fireEvent.click(await screen.findByRole("menuitemradio", { name: "Zwei" }));
    await waitFor(() => expect(b.of("instance_select")).toEqual([{ id: "b" }]));
    expect(await screen.findByRole("heading", { name: "Zwei" })).toBeTruthy();
  });

  it("no account: Sign in opens the sign-in path from the button", async () => {
    await setup({ accounts: { accounts: [], canSignIn: true } });
    expect(screen.getByRole("heading", { name: "Überleben 1.21" })).toBeTruthy();
    fireEvent.click(playButton());
    expect(useApp.getState().accountDialog).toMatchObject({ kind: "signIn", resign: null });
  });

  it("no client id: the no-client-id dialog instead of the sign-in dialog", async () => {
    await setup({ accounts: { accounts: [], canSignIn: false } });
    fireEvent.click(playButton());
    expect(useApp.getState().accountDialog).toEqual({ kind: "noClientId" });
  });

  it("an account that has to sign in again: Sign in again for that account", async () => {
    await setup({
      accounts: {
        accounts: [account("u", "Mira_K", { selected: true, signInRequired: true })],
        canSignIn: true,
      },
    });
    fireEvent.click(playButton());
    expect(useApp.getState().accountDialog).toMatchObject({
      kind: "signIn",
      resign: { id: "u", name: "Mira_K" },
    });
  });

  it("instances that cannot be loaded: the panel with Try again", async () => {
    const b = playBackend();
    b.listFails = { code: "fileSystem", params: {}, action: null, detail: "d" };
    await useApp.getState().loadAccounts();
    render(<PlayView />);
    const panel = await screen.findByRole("alert");
    b.listFails = null;
    fireEvent.click(within(panel).getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("button", { name: "Play" })).toBeTruthy();
  });

  it("the selected account of the store is what decides", async () => {
    await setup({ accounts: { accounts: [mira], canSignIn: true } });
    expect(playButton().textContent).toBe("Play");
  });

  it("shows the state of the game when it is loaded after the page", async () => {
    const b = await setup({ game: IDLE });
    act(() => b.state(running()));
    expect(playButton().textContent).toBe("Stop");
  });
});

describe("review fixes", () => {
  const LONG = "A".repeat(64);
  const LONG_SPACES = "Wort ".repeat(13).trim().slice(0, 64);

  it.each([LONG, LONG_SPACES])(
    "a 64-character name stays on the caption line (%#)",
    async (name) => {
      await setup({
        instances: [instance("x", { status: "broken", name, selected: true }), instance("b")],
      });
      const text = screen.getByText(/cannot be started\.$/);
      // One truncated line, the full sentence in its title.
      expect(text.className).toContain("truncate");
      expect(text.className).toContain("min-w-0");
      expect(text.getAttribute("title")).toBe(text.textContent);
      expect(text.textContent).toContain(name);
    },
  );

  it.each([
    ["\u202Eevil", "\u2068evil\u2069 cannot be started."],
    // A closing isolate in the name would end the isolation early: controls are dropped first.
    ["\u2069\u202Eevil", "\u2068evil\u2069 cannot be started."],
  ])("bidi controls in a name cannot reorder the caption sentence (%#)", async (name, want) => {
    await setup({
      instances: [instance("x", { status: "broken", name, selected: true }), instance("b")],
    });
    expect(screen.getByText(/cannot be started\.$/).textContent).toBe(want);
  });

  it("the progress panel cannot grow past its 160 px and a zero step total draws no bar", async () => {
    const b = await setup();
    fireEvent.click(playButton());
    await flush();
    act(() => b.play({ event: "preparing", data: { op: 1 } }));
    act(() => b.play({ event: "installStep", data: { index: 1, total: 0, name: "x" } }));
    const panel = document.querySelector<HTMLElement>('[data-slot="launch-progress"]')!;
    expect(panel.className).toContain("h-40");
    expect(panel.className).toContain("overflow-hidden");
    expect(screen.queryByRole("progressbar")).toBeNull();
    expect(panel.innerHTML).not.toContain("NaN");
    // Log link, two notices and a failed open: the failure takes a notice's place.
    act(() => b.play({ event: "installLog", data: { path: "logs/i.log" } }));
    act(() => b.play({ event: "notice", data: { message: "first" } }));
    act(() => b.play({ event: "notice", data: { message: "second" } }));
    b.openFails = { code: "openFailed", params: {}, action: null, detail: "d" };
    fireEvent.click(screen.getByRole("button", { name: "Open install log" }));
    await flush();
    const bottom = panel.querySelector('[data-slot="launch-notes"]')!;
    expect(bottom.children).toHaveLength(2);
    expect(bottom.textContent).toContain("The install log could not be opened.");
    expect(bottom.textContent).toContain("second");
    expect(bottom.textContent).not.toContain("first");
  });

  it("a warm start without any phase from the core says Preparing, never an empty panel", async () => {
    const b = await setup();
    fireEvent.click(playButton());
    await flush();
    act(() => b.play({ event: "preparing", data: { op: 1 } }));
    const panel = document.querySelector<HTMLElement>('[data-slot="launch-progress"]')!;
    expect(panel.querySelector("[aria-live]")!.textContent).toBe("Preparing");
  });

  it("after the last download batch the line returns to Preparing, not between batches", async () => {
    const b = await setup();
    vi.useFakeTimers({ toFake: ["Date", "setTimeout", "clearTimeout"] });
    fireEvent.click(playButton());
    await flush();
    act(() => b.play({ event: "preparing", data: { op: 1 } }));
    const line = () =>
      document.querySelector('[data-slot="launch-progress"] [aria-live]')!.textContent;
    act(() => b.play({ event: "downloading", data: { what: "gameFiles", count: 30 } }));
    act(() => b.play({ event: "downloaded", data: { what: "gameFiles" } }));
    // The next batch follows at once: the line stays.
    act(() => vi.advanceTimersByTime(200));
    act(() => b.play({ event: "downloading", data: { what: "assets", count: 4118 } }));
    expect(line()).toBe("Downloading files");
    act(() => b.play({ event: "downloaded", data: { what: "assets" } }));
    act(() => vi.advanceTimersByTime(999));
    expect(line()).toBe("Downloading files");
    // Nothing follows for a second: the downloads are over.
    act(() => vi.advanceTimersByTime(1));
    expect(line()).toBe("Preparing");
  });

  it("a double click on Cancel never starts the game again", async () => {
    const b = await setup();
    vi.useFakeTimers({ toFake: ["Date", "setTimeout", "clearTimeout"] });
    fireEvent.click(playButton());
    await flush();
    act(() => b.play({ event: "preparing", data: { op: 1 } }));
    act(() => vi.advanceTimersByTime(600));
    // First click of a double click on Cancel; cancelled arrives before the second one.
    fireEvent.click(playButton());
    await flush();
    act(() => b.play({ event: "cancelled" }));
    expect(playButton().textContent).toBe("Play");
    act(() => vi.advanceTimersByTime(150));
    fireEvent.click(playButton());
    await flush();
    expect(b.count("play")).toBe(1);
    // After the guard Play works again.
    act(() => vi.advanceTimersByTime(450));
    fireEvent.click(playButton());
    await flush();
    expect(b.count("play")).toBe(2);
  });

  it("a double click on Stop when the game exits first never starts it again", async () => {
    const b = await setup({ game: running() });
    fireEvent.click(playButton());
    await flush();
    act(() => b.state(exited(0, true)));
    expect(playButton().textContent).toBe("Play");
    fireEvent.click(playButton());
    await flush();
    expect(b.count("play")).toBe(0);
  });

  it("a second cancel while the first is on its way is not sent", async () => {
    const b = await setup();
    await useApp.getState().startGame("survival");
    act(() => b.play({ event: "preparing", data: { op: 1 } }));
    await Promise.all([useApp.getState().cancelStart(), useApp.getState().cancelStart()]);
    expect(b.count("play_cancel")).toBe(1);
  });

  it("the 600 ms guard is not armed when the button turns into Play", async () => {
    const b = await setup({ accounts: { accounts: [], canSignIn: true } });
    vi.useFakeTimers({ toFake: ["Date", "setTimeout", "clearTimeout"] });
    expect(playButton().textContent).toBe("Sign in");
    // Signed in meanwhile: the button turns into Play and works at once.
    b.accounts = { accounts: [mira], canSignIn: true };
    await act(() => useApp.getState().loadAccounts());
    expect(playButton().textContent).toBe("Play");
    fireEvent.click(playButton());
    await flush();
    expect(b.count("play")).toBe(1);
  });

  it("the 600 ms guard is armed when the button turns into Stop", async () => {
    const b = await setup();
    vi.useFakeTimers({ toFake: ["Date", "setTimeout", "clearTimeout"] });
    act(() => b.state(running()));
    fireEvent.click(playButton());
    await flush();
    expect(b.count("game_stop")).toBe(0);
    act(() => vi.advanceTimersByTime(600));
    fireEvent.click(playButton());
    await flush();
    expect(b.count("game_stop")).toBe(1);
  });

  it("running and the end of the game are announced once in the caption region", async () => {
    const b = await setup();
    const region = document.querySelector<HTMLElement>('[data-slot="play-caption"]')!;
    expect(region.getAttribute("aria-live")).toBe("polite");
    expect(region.textContent).toBe("");
    act(() => b.state(running()));
    expect(region.textContent).toMatch(/^Running since /);
    act(() => b.state(stopping()));
    act(() => b.state(exited(0, true)));
    expect(region.textContent).toBe("The game has ended.");
    // The same element all along: a region that exists before its text.
    expect(document.querySelector('[data-slot="play-caption"]')).toBe(region);
  });

  it("Choose instance: picking the readable instance in the switcher selects it", async () => {
    const b = await setup({
      instances: [
        instance("x", { status: "broken", name: "Alt", selected: true }),
        instance("b", { name: "Zwei" }),
      ],
    });
    fireEvent.click(playButton());
    // The check stays with what is selected (the unreadable one), not with the fallback.
    const zwei = await screen.findByRole("menuitemradio", { name: "Zwei" });
    expect(zwei.getAttribute("aria-checked")).toBe("false");
    expect(screen.getByRole("menuitemradio", { name: /Alt/ }).getAttribute("aria-checked")).toBe(
      "true",
    );
    fireEvent.click(zwei);
    await waitFor(() => expect(b.of("instance_select")).toEqual([{ id: "b" }]));
  });

  it("accounts but none selected: Choose account opens the header chip's menu", async () => {
    playBackend({ accounts: { accounts: [account("u", "Mira_K")], canSignIn: true } });
    await useApp.getState().startEventLoop();
    await useApp.getState().loadAccounts();
    render(
      <TooltipProvider delay={0}>
        <AccountChip />
        <PlayView />
      </TooltipProvider>,
    );
    const button = await screen.findByRole("button", { name: "Choose account", hidden: false });
    const play = screen
      .getAllByRole("button", { name: "Choose account" })
      .find((b) => b.getAttribute("data-variant") === "primary")!;
    expect(play).toBeTruthy();
    expect(button).toBeTruthy();
    // The chip's own trigger: the "Choose account" button that is not the Play button.
    const chip = screen.getAllByRole("button", { name: "Choose account" }).find((b) => b !== play)!;
    expect(chip).toBeTruthy();
    // A whole pointer press on the Play button, as a mouse makes it.
    fireEvent.pointerDown(play, { pointerType: "mouse", button: 0 });
    fireEvent.mouseDown(play, { button: 0 });
    fireEvent.pointerUp(play, { pointerType: "mouse", button: 0 });
    fireEvent.mouseUp(play, { button: 0 });
    fireEvent.click(play);
    const menu = await screen.findByRole("menu");
    expect(within(menu).getByRole("menuitemradio", { name: /Mira_K/ })).toBeTruthy();
    // Not closed again by the press that opened it.
    await act(() => new Promise((r) => setTimeout(r, 50)));
    expect(screen.getByRole("menu")).toBe(menu);
    expect(useApp.getState().accountDialog).toBeNull();
    // Escape closes it; focus goes to the chip, the menu's own trigger (DESIGN.md 11.4).
    fireEvent.keyDown(menu, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    await waitFor(() => expect(document.activeElement).toBe(chip));
  });

  it("the install log is offered once, whether the backend or the page names it", async () => {
    for (const action of [{ kind: "openLog" as const, path: "logs/from-backend.log" }, null]) {
      const b = await setup();
      fireEvent.click(playButton());
      await flush();
      act(() => b.play({ event: "preparing", data: { op: 1 } }));
      act(() => b.play({ event: "installLog", data: { path: "logs/from-backend.log" } }));
      act(() =>
        b.play({
          event: "failed",
          data: { error: { code: "installStepFailed", params: {}, action, detail: "d" } },
        }),
      );
      expect(screen.getAllByRole("button", { name: "Open install log" })).toHaveLength(1);
      fireEvent.click(screen.getByRole("button", { name: "Open install log" }));
      await flush();
      expect(b.of("open_location")).toEqual([
        { target: { kind: "logFile", path: "logs/from-backend.log" } },
      ]);
      cleanup();
      clearMocks();
      useApp.setState(initial, true);
    }
  });
});
