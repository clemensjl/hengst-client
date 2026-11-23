import { clearMocks } from "@tauri-apps/api/mocks";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { GameStateDto } from "@/ipc/bindings/GameStateDto";
import { useApp } from "@/stores/app";
import { deferred } from "@/test/instancesBackend";
import { IDLE, playBackend } from "@/test/playBackend";

const initial = useApp.getState();
const running = (runId = 1): GameStateDto => ({
  state: "running",
  data: { instance: "survival", runId, since: 1_000 },
});
const preparing: GameStateDto = { state: "preparing", data: { instance: "survival" } };
const flush = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => useApp.setState(initial, true));
afterEach(() => clearMocks());

async function started() {
  const b = playBackend();
  await useApp.getState().startEventLoop();
  return b;
}

describe("game state in the store", () => {
  it("an event that arrives while game_state() is on its way wins over its older answer", async () => {
    const b = await started();
    const gate = deferred();
    b.gameGate = gate.promise;
    const load = useApp.getState().loadGame();
    // A newer event arrives before the answer...
    b.state(running());
    // ...which still carries what the backend had when it was asked.
    b.game = IDLE;
    gate.resolve();
    await load;
    expect(useApp.getState().game.data).toEqual(running());
    expect(useApp.getState().activeInstance).toBe("survival");
  });

  it("of two loads only the newer answer counts", async () => {
    const b = await started();
    const gate = deferred();
    b.gameGate = gate.promise;
    b.game = preparing;
    const first = useApp.getState().loadGame();
    b.gameGate = null;
    b.game = running();
    await useApp.getState().loadGame();
    b.game = preparing;
    gate.resolve();
    await first;
    expect(useApp.getState().game.data).toEqual(running());
  });

  it("activeInstance follows the game state and is cleared by idle", async () => {
    const b = await started();
    b.state(preparing);
    expect(useApp.getState().activeInstance).toBe("survival");
    b.state({ state: "stopping", data: { instance: "survival", runId: 1 } });
    expect(useApp.getState().activeInstance).toBe("survival");
    b.state(IDLE);
    expect(useApp.getState().activeInstance).toBeNull();
  });
});

describe("the page's own start", () => {
  it("double start runs play once", async () => {
    const b = await started();
    void useApp.getState().startGame("survival");
    void useApp.getState().startGame("survival");
    await flush();
    expect(b.count("play")).toBe(1);
  });

  it("ends when started arrives before the running state", async () => {
    const b = await started();
    await useApp.getState().startGame("survival");
    b.play({ event: "preparing", data: { op: 1 } });
    b.play({ event: "started", data: { runId: 1 } });
    expect(useApp.getState().launch?.progress.outcome?.kind).toBe("started");
    b.state(running(1));
    expect(useApp.getState().launch).toBeNull();
  });

  it("ends when the running state arrives before started", async () => {
    const b = await started();
    await useApp.getState().startGame("survival");
    b.play({ event: "preparing", data: { op: 1 } });
    b.state(running(1));
    b.play({ event: "started", data: { runId: 1 } });
    expect(useApp.getState().launch).toBeNull();
  });

  it("ends when the run is already over before started arrives", async () => {
    const b = await started();
    await useApp.getState().startGame("survival");
    b.play({ event: "preparing", data: { op: 1 } });
    b.state({
      state: "idle",
      data: { lastExit: { instance: "survival", runId: 1, code: 0, stopRequested: false } },
    });
    b.play({ event: "started", data: { runId: 1 } });
    expect(useApp.getState().launch).toBeNull();
  });

  it("a failure keeps the error and the install log until the next start", async () => {
    const b = await started();
    await useApp.getState().startGame("survival");
    b.play({ event: "installLog", data: { path: "logs/x.log" } });
    const error = { code: "installFailed" as const, params: {}, action: null, detail: "d" };
    b.play({ event: "failed", data: { error } });
    expect(useApp.getState().launch).toBeNull();
    expect(useApp.getState().launchError).toEqual({ error, installLog: "logs/x.log" });
    await useApp.getState().startGame("survival");
    expect(useApp.getState().launchError).toBeNull();
  });

  it("a refused play call is the start's error", async () => {
    const b = await started();
    b.playFails = { code: "busy", params: {}, action: null, detail: "d" };
    await useApp.getState().startGame("survival");
    expect(useApp.getState().launch).toBeNull();
    expect(useApp.getState().launchError?.error.code).toBe("busy");
  });

  it("a cancel that came too late keeps the start going", async () => {
    const b = await started();
    await useApp.getState().startGame("survival");
    b.play({ event: "preparing", data: { op: 1 } });
    b.cancelAnswer = false;
    await useApp.getState().cancelStart();
    expect(b.of("play_cancel")).toEqual([{ op: 1 }]);
    expect(useApp.getState().launch?.cancelRefused).toBe(true);
    // A second cancel is not sent: it cannot succeed any more.
    await useApp.getState().cancelStart();
    expect(b.count("play_cancel")).toBe(1);
  });

  it("events of an earlier start are ignored", async () => {
    const b = await started();
    await useApp.getState().startGame("survival");
    b.play({ event: "cancelled" }, 0);
    await useApp.getState().startGame("survival");
    b.play({ event: "downloading", data: { what: "assets", count: 9 } }, 0);
    expect(useApp.getState().launch?.progress.files).toBeNull();
  });
});

describe("stopping", () => {
  it("a second stop while the first is on its way is not sent", async () => {
    const b = await started();
    b.state(running(1));
    const gate = deferred();
    b.stopGate = gate.promise;
    const first = useApp.getState().stopGame();
    void useApp.getState().stopGame();
    gate.resolve();
    await first;
    expect(b.count("game_stop")).toBe(1);
  });

  it("noWindow is kept for that run only", async () => {
    const b = await started();
    b.state(running(1));
    b.stopAnswer = "noWindow";
    await useApp.getState().stopGame();
    expect(useApp.getState().stopNoWindow).toBe(1);
    b.state({ state: "stopping", data: { instance: "survival", runId: 1 } });
    expect(useApp.getState().stopNoWindow).toBeNull();
  });

  it("the stopping time is taken once per run, also when the state is loaded after a reload", async () => {
    const b = playBackend({
      game: { state: "stopping", data: { instance: "survival", runId: 2 } },
    });
    await useApp.getState().startEventLoop();
    await useApp.getState().loadGame();
    const seen = useApp.getState().stoppingSeen;
    expect(seen?.runId).toBe(2);
    b.state({ state: "stopping", data: { instance: "survival", runId: 2 } });
    expect(useApp.getState().stoppingSeen).toBe(seen);
    b.state(IDLE);
    expect(useApp.getState().stoppingSeen).toBeNull();
  });
});
