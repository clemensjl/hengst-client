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
    name: /^(Play|Stop|Cancel|Starting…|Stopping…|Create instance|Sign in|Sign in again|Choose instance)$/,
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
    expect(screen.getByText("Launching game")).toBeTruthy();
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
    expect(screen.getByText("Alt cannot be started.")).toBeTruthy();
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
