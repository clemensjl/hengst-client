import { clearMocks } from "@tauri-apps/api/mocks";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { initI18n } from "@/i18n";
import type { GameStateDto } from "@/ipc/bindings/GameStateDto";
import { useApp } from "@/stores/app";
import { playBackend } from "@/test/playBackend";
import { QuitDialogHost } from "./QuitDialog";

const initial = useApp.getState();
const running: GameStateDto = {
  state: "running",
  data: { instance: "survival", runId: 1, since: 1 },
};
const stopping: GameStateDto = { state: "stopping", data: { instance: "survival", runId: 1 } };
const exited = (stopRequested: boolean): GameStateDto => ({
  state: "idle",
  data: { lastExit: { instance: "survival", runId: 1, code: 0, stopRequested } },
});

beforeEach(() => {
  initI18n("en", "en-US");
  useApp.setState(initial, true);
  Element.prototype.scrollIntoView = vi.fn();
});
afterEach(async () => {
  delete (Element.prototype as { getAnimations?: unknown }).getAnimations;
  vi.useRealTimers();
  cleanup();
  clearMocks();
  await new Promise((r) => setTimeout(r, 30));
});

const flush = () => act(async () => {});

/**
 * Holds every exit (and open) animation until the returned function is called, as a real exit
 * animation keeps the closing dialog mounted for about 150 ms. jsdom has no animations at all.
 */
function holdAnimations(): () => void {
  let finish: () => void = () => {};
  const finished = new Promise<void>((r) => (finish = r));
  const animation = { finished, effect: { getTiming: () => ({ duration: 150, iterations: 1 }) } };
  Object.defineProperty(Element.prototype, "getAnimations", {
    configurable: true,
    value: () => [animation],
  });
  return () => {
    // Base UI may look for replacement animations after these end: there are none.
    Object.defineProperty(Element.prototype, "getAnimations", {
      configurable: true,
      value: () => [],
    });
    finish();
  };
}

/** The app with a running game, after the tray asked to quit. */
async function setup(game: GameStateDto = running) {
  const b = playBackend({ game });
  await useApp.getState().startEventLoop();
  await useApp.getState().loadGame();
  render(
    <TooltipProvider delay={0}>
      <QuitDialogHost />
    </TooltipProvider>,
  );
  act(() => b.app({ event: "quitRequested" }));
  await screen.findByRole("alertdialog");
  return b;
}

const dialog = () => screen.queryByRole("alertdialog");
const keep = () => screen.getByRole("button", { name: "Keep playing" });
const destructive = () =>
  screen.getByRole("button", {
    name: /^(Stop game and quit|Stopping…|Force stop and quit)$/,
  });

describe("quit while a game runs", () => {
  it("asks the concrete question, with Keep playing focused first", async () => {
    await setup();
    expect(screen.getByRole("alertdialog", { name: "Stop the game and quit Hengst Client?" }));
    expect(
      screen.getByText("The game is stopped. Progress since the game last saved may be lost."),
    ).toBeTruthy();
    await waitFor(() => expect(document.activeElement).toBe(keep()));
    expect(destructive().textContent).toBe("Stop game and quit");
  });

  it("keep playing closes it and touches nothing", async () => {
    const b = await setup();
    fireEvent.click(keep());
    await waitFor(() => expect(dialog()).toBeNull());
    expect(b.count("game_stop")).toBe(0);
    expect(b.count("app_quit")).toBe(0);
  });

  it("a game that exits while the dialog closes after keep playing does not quit", async () => {
    const b = await setup();
    const finish = holdAnimations();
    fireEvent.click(keep());
    await flush();
    // The close has not completed: the dialog is still mounted for its exit animation.
    expect(dialog()).not.toBeNull();
    act(() => b.state(exited(false)));
    await flush();
    finish();
    await waitFor(() => expect(dialog()).toBeNull());
    await flush();
    expect(b.count("app_quit")).toBe(0);
  });

  it("a game that exits long after keep playing does not quit", async () => {
    const b = await setup();
    fireEvent.click(keep());
    await waitFor(() => expect(dialog()).toBeNull());
    act(() => b.state(exited(false)));
    await flush();
    expect(b.count("app_quit")).toBe(0);
  });

  it("a tray quit right after keep playing opens the dialog again", async () => {
    const b = await setup();
    const finish = holdAnimations();
    fireEvent.click(keep());
    await flush();
    // Still closing when the next request comes.
    act(() => b.app({ event: "quitRequested" }));
    await flush();
    finish();
    await flush();
    // The new question stays open after the old one has finished closing.
    await new Promise((r) => setTimeout(r, 100));
    expect(screen.getByRole("alertdialog")).toBeTruthy();
    expect(useApp.getState().quitAsked).not.toBeNull();
  });

  it("while it quits, the reason says so", async () => {
    const b = await setup();
    b.stopAnswer = "notRunning";
    let release: () => void = () => {};
    b.quitGate = new Promise<void>((r) => (release = r));
    fireEvent.click(destructive());
    await flush();
    const reason = screen.getByText("Hengst Client is quitting.");
    expect(keep().getAttribute("aria-describedby")).toBe(reason.id);
    release();
  });

  it("escape closes it while nothing runs", async () => {
    const b = await setup();
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    await waitFor(() => expect(dialog()).toBeNull());
    expect(b.count("app_quit")).toBe(0);
  });

  it("stopping shows the loading state, keeps the dialog and quits once the game has exited", async () => {
    const b = await setup();
    fireEvent.click(destructive());
    await flush();
    expect(b.count("game_stop")).toBe(1);
    expect(destructive().textContent).toBe("Stopping…");
    expect(destructive().getAttribute("aria-disabled")).toBe("true");
    expect(keep().getAttribute("aria-disabled")).toBe("true");
    // Neither Escape nor Keep playing closes it while the game is being stopped.
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    fireEvent.click(keep());
    await flush();
    expect(dialog()).not.toBeNull();
    act(() => b.state(stopping));
    await flush();
    expect(b.count("app_quit")).toBe(0);
    act(() => b.state(exited(true)));
    await waitFor(() => expect(b.count("app_quit")).toBe(1));
  });

  it("a double click stops once", async () => {
    const b = await setup();
    fireEvent.click(destructive());
    fireEvent.click(destructive());
    await flush();
    expect(b.count("game_stop")).toBe(1);
  });

  it("after 10 s without an exit it says the game is not responding and offers the force stop", async () => {
    const b = await setup();
    vi.useFakeTimers({ toFake: ["Date", "setTimeout", "clearTimeout"] });
    fireEvent.click(destructive());
    await flush();
    act(() => b.state(stopping));
    act(() => vi.advanceTimersByTime(9_999));
    expect(screen.queryByText("The game is not responding")).toBeNull();
    expect(destructive().textContent).toBe("Stopping…");
    act(() => vi.advanceTimersByTime(1));
    expect(screen.getByRole("alert").textContent).toContain("The game is not responding");
    expect(destructive().textContent).toBe("Force stop and quit");
    expect(destructive().getAttribute("aria-disabled")).toBeNull();
    expect(keep().getAttribute("aria-disabled")).toBeNull();
    // Past the click guard of the changed button.
    act(() => vi.advanceTimersByTime(600));
    fireEvent.click(destructive());
    await flush();
    expect(b.count("game_force_stop")).toBe(1);
    expect(destructive().textContent).toBe("Stopping…");
    act(() => b.state(exited(true)));
    await flush();
    expect(b.count("app_quit")).toBe(1);
  });

  it("a game without a window is not claimed to be asked: the force stop is offered at once", async () => {
    const b = await setup();
    b.stopAnswer = "noWindow";
    fireEvent.click(destructive());
    await flush();
    expect(screen.getByText("The game has no window yet and was not asked to close.")).toBeTruthy();
    expect(screen.queryByText("The game is not responding")).toBeNull();
    expect(destructive().textContent).toBe("Force stop and quit");
    expect(keep().getAttribute("aria-disabled")).toBeNull();
    // Past the click guard of the changed button.
    await act(() => new Promise((r) => setTimeout(r, 650)));
    fireEvent.click(destructive());
    await flush();
    expect(b.count("game_force_stop")).toBe(1);
    act(() => b.state(exited(true)));
    await waitFor(() => expect(b.count("app_quit")).toBe(1));
  });

  it("a game that is already gone quits at once", async () => {
    const b = await setup();
    b.stopAnswer = "notRunning";
    fireEvent.click(destructive());
    await waitFor(() => expect(b.count("app_quit")).toBe(1));
  });

  it("a game that ends by itself while the dialog asks quits the app (the user chose to quit)", async () => {
    const b = await setup();
    act(() => b.state(exited(false)));
    await waitFor(() => expect(b.count("app_quit")).toBe(1));
  });

  it("a double click does not run into the force stop when the game has no window", async () => {
    const b = await setup();
    vi.useFakeTimers({ toFake: ["Date", "setTimeout", "clearTimeout"] });
    b.stopAnswer = "noWindow";
    fireEvent.click(destructive());
    await flush();
    expect(destructive().textContent).toBe("Force stop and quit");
    // The second click of the double click lands on the changed button.
    fireEvent.click(destructive());
    await flush();
    expect(b.count("game_force_stop")).toBe(0);
    act(() => vi.advanceTimersByTime(600));
    fireEvent.click(destructive());
    await flush();
    expect(b.count("game_force_stop")).toBe(1);
  });

  it("the switch to the force stop after 10 s takes no click for 600 ms", async () => {
    const b = await setup();
    vi.useFakeTimers({ toFake: ["Date", "setTimeout", "clearTimeout"] });
    fireEvent.click(destructive());
    await flush();
    act(() => vi.advanceTimersByTime(10_000));
    expect(destructive().textContent).toBe("Force stop and quit");
    fireEvent.click(destructive());
    await flush();
    expect(b.count("game_force_stop")).toBe(0);
    act(() => vi.advanceTimersByTime(599));
    fireEvent.click(destructive());
    await flush();
    expect(b.count("game_force_stop")).toBe(0);
    act(() => vi.advanceTimersByTime(1));
    fireEvent.click(destructive());
    await flush();
    expect(b.count("game_force_stop")).toBe(1);
  });

  it("while it stops, Keep playing says why it does nothing", async () => {
    await setup();
    fireEvent.click(destructive());
    await flush();
    const reason = screen.getByText(
      "Keep playing works again once the game has closed or stops responding.",
    );
    expect(keep().getAttribute("aria-describedby")).toBe(reason.id);
    expect(reason.id).not.toBe("");
  });

  it("a force stop that never ends unlocks after 10 s, and a later tray quit opens the dialog again", async () => {
    const b = await setup();
    vi.useFakeTimers({ toFake: ["Date", "setTimeout", "clearTimeout"] });
    b.stopAnswer = "noWindow";
    fireEvent.click(destructive());
    await flush();
    act(() => vi.advanceTimersByTime(600));
    fireEvent.click(destructive());
    await flush();
    expect(b.count("game_force_stop")).toBe(1);
    expect(keep().getAttribute("aria-disabled")).toBe("true");
    act(() => vi.advanceTimersByTime(10_000));
    expect(keep().getAttribute("aria-disabled")).toBeNull();
    expect(screen.getByRole("alert").textContent).toContain("The game is not responding");
    vi.useRealTimers();
    fireEvent.click(keep());
    await waitFor(() => expect(dialog()).toBeNull());
    act(() => b.app({ event: "quitRequested" }));
    expect(await screen.findByRole("alertdialog")).toBeTruthy();
  });

  it("a failed quit after the game is gone keeps the dialog with the error and a retry", async () => {
    const b = await setup();
    b.quitFails = { code: "internal", params: {}, action: null, detail: "x" };
    fireEvent.click(destructive());
    await flush();
    act(() => b.state(exited(true)));
    await waitFor(() => expect(b.count("app_quit")).toBe(1));
    await flush();
    expect(dialog()).not.toBeNull();
    expect(screen.getByRole("alert")).toBeTruthy();
    b.quitFails = null;
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(b.count("app_quit")).toBe(2));
  });

  it("a failed stop shows the error in the dialog and lets the user try again or keep playing", async () => {
    const b = await setup();
    b.stopFails = { code: "internal", params: {}, action: null, detail: "x" };
    fireEvent.click(destructive());
    await flush();
    expect(screen.getByRole("alert")).toBeTruthy();
    expect(destructive().textContent).toBe("Stop game and quit");
    expect(destructive().getAttribute("aria-disabled")).toBeNull();
    expect(keep().getAttribute("aria-disabled")).toBeNull();
    expect(b.count("app_quit")).toBe(0);
  });

  it("a second quit request while it is open keeps the one dialog", async () => {
    const b = await setup();
    act(() => b.app({ event: "quitRequested" }));
    await flush();
    expect(screen.getAllByRole("alertdialog")).toHaveLength(1);
  });
});

describe("stop from the tray without a window", () => {
  it("marks the run so the start page offers the force stop", async () => {
    const b = playBackend({ game: running });
    await useApp.getState().startEventLoop();
    await useApp.getState().loadGame();
    act(() => b.app({ event: "stopNoWindow", data: { runId: 1 } }));
    expect(useApp.getState().stopNoWindow).toBe(1);
    // A run that is no longer the running one is not marked.
    act(() => b.app({ event: "stopNoWindow", data: { runId: 7 } }));
    expect(useApp.getState().stopNoWindow).toBe(1);
  });
});
