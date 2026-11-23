import { describe, expect, it } from "vitest";
import type { GameStateDto } from "@/ipc/bindings/GameStateDto";
import { instance } from "@/test/instancesBackend";
import { playButtonState, type PlayInput } from "./playState";

const idle: GameStateDto = { state: "idle", data: { lastExit: null } };
const preparing: GameStateDto = { state: "preparing", data: { instance: "a" } };
const running: GameStateDto = { state: "running", data: { instance: "a", runId: 1, since: 5 } };
const stopping: GameStateDto = { state: "stopping", data: { instance: "a", runId: 1 } };

const a = instance("a", { selected: true });
const broken = instance("x", { status: "broken", name: "x" });
const tooNew = instance("n", { status: "tooNew", name: "n" });

const ready: PlayInput = {
  game: idle,
  launch: null,
  instances: [a],
  account: { kind: "ok" },
};
const kind = (over: Partial<PlayInput>) => playButtonState({ ...ready, ...over }).kind;

// DESIGN.md 10.2, first match wins. One case per row, then states that hold at the same time.
describe("play_button_state_precedence", () => {
  it("row 1: stopping", () => expect(kind({ game: stopping })).toBe("stopping"));
  it("row 2: running, with the start time", () => {
    expect(playButtonState({ ...ready, game: running })).toEqual({ kind: "running", since: 5 });
  });
  it("row 3: starting until the start has its operation id", () => {
    expect(kind({ launch: { op: null, cancelRefused: false } })).toBe("starting");
  });
  it("row 3: preparing once the operation id is known", () => {
    expect(playButtonState({ ...ready, launch: { op: 4, cancelRefused: false } })).toEqual({
      kind: "preparing",
      op: 4,
    });
  });
  it("row 4: no instance at all, and only unreadable or newer-format ones", () => {
    expect(playButtonState({ ...ready, instances: [] })).toEqual({
      kind: "noInstance",
      unreadable: false,
    });
    expect(playButtonState({ ...ready, instances: [broken, tooNew] })).toEqual({
      kind: "noInstance",
      unreadable: true,
    });
  });
  it("row 5: the selected instance is unreadable and a readable one exists", () => {
    const list = [{ ...broken, selected: true }, instance("b")];
    expect(playButtonState({ ...ready, instances: list })).toEqual({
      kind: "chooseInstance",
      name: "x",
    });
    const newer = [{ ...tooNew, selected: true }, instance("b")];
    expect(kind({ instances: newer })).toBe("chooseInstance");
  });
  it("row 6: no account", () => expect(kind({ account: { kind: "none" } })).toBe("signIn"));
  it("row 7: the account has to sign in again", () => {
    const account = { kind: "signInAgain" as const, id: "u", name: "Mira_K" };
    expect(playButtonState({ ...ready, account })).toEqual(account);
  });
  it("row 8: ready, with the instance to start", () => {
    expect(playButtonState(ready)).toEqual({ kind: "ready", instance: "a" });
    // No selection: the fallback of Task 11 (first readable in list order).
    const list = [broken, instance("b"), instance("c")];
    expect(playButtonState({ ...ready, instances: list })).toEqual({
      kind: "ready",
      instance: "b",
    });
  });

  it("a preparing game without a start of this page (after a reload) is starting, not cancelable", () => {
    expect(kind({ game: preparing })).toBe("starting");
  });
  it("a start whose cancel came too late stays starting", () => {
    expect(kind({ game: preparing, launch: { op: 4, cancelRefused: true } })).toBe("starting");
  });

  it("overlaps: the first matching row wins", () => {
    const signInAgain = { kind: "signInAgain" as const, id: "u", name: "M" };
    const cases: [Partial<PlayInput>, string][] = [
      // The game state beats everything below it, whatever the page's own start says.
      [{ game: stopping, launch: { op: 4, cancelRefused: false } }, "stopping"],
      [{ game: running, launch: { op: null, cancelRefused: false } }, "running"],
      [{ game: stopping, instances: [] }, "stopping"],
      [{ game: running, account: { kind: "none" } }, "running"],
      // A start in progress beats a missing account or instance (they may change meanwhile).
      [{ launch: { op: 4, cancelRefused: false }, account: { kind: "none" } }, "preparing"],
      [{ launch: { op: null, cancelRefused: false }, instances: [] }, "starting"],
      [{ game: preparing, account: signInAgain }, "starting"],
      // Instances before accounts (DESIGN.md 11.0: instance first).
      [{ instances: [], account: { kind: "none" } }, "noInstance"],
      [
        { instances: [{ ...broken, selected: true }, instance("b")], account: { kind: "none" } },
        "chooseInstance",
      ],
      [{ account: { kind: "none" } }, "signIn"],
      [{ instances: [broken], account: signInAgain }, "noInstance"],
    ];
    for (const [over, want] of cases) expect(kind(over), JSON.stringify(over)).toBe(want);
  });

  it("an unknown game state (not loaded) counts as idle", () => {
    expect(kind({ game: null })).toBe("ready");
  });
});
