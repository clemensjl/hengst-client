// Test helper: a fake backend for the start page, on top of `mockIPC`. It answers the instance,
// account and game commands, keeps the channel of every `play` and the app event channel, and
// lets a test send events on either, in any order.
import { mockIPC } from "@tauri-apps/api/mocks";
import type { AccountsDto } from "@/ipc/bindings/AccountsDto";
import type { AppEvent } from "@/ipc/bindings/AppEvent";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";
import type { GameStateDto } from "@/ipc/bindings/GameStateDto";
import type { InstanceDto } from "@/ipc/bindings/InstanceDto";
import type { PlayEvent } from "@/ipc/bindings/PlayEvent";
import type { StopResult } from "@/ipc/bindings/StopResult";
import { account } from "./fakeBackend";
import { instance, type Call } from "./instancesBackend";

interface Channel<T> {
  onmessage?: (e: T) => void;
}

export const IDLE: GameStateDto = { state: "idle", data: { lastExit: null } };
export const mira = account("a", "Mira_K", { selected: true });

export function playBackend(
  options: {
    instances?: InstanceDto[];
    accounts?: AccountsDto;
    game?: GameStateDto;
  } = {},
) {
  const calls: Call[] = [];
  const plays: Channel<PlayEvent>[] = [];
  let events: Channel<AppEvent> | null = null;
  let nextOp = 1;
  const b = {
    calls,
    instances: options.instances ?? [
      instance("survival", { name: "Überleben 1.21", selected: true }),
    ],
    accounts: options.accounts ?? { accounts: [mira], canSignIn: true },
    game: options.game ?? IDLE,
    /** Held until resolved: `game_state` answers only then (with the state at that moment). */
    gameGate: null as Promise<void> | null,
    gameFails: null as ErrorDto | null,
    listFails: null as ErrorDto | null,
    playFails: null as ErrorDto | null,
    cancelAnswer: true,
    stopAnswer: "asked" as StopResult,
    /** Held until resolved: `game_stop` answers only then. */
    stopGate: null as Promise<void> | null,
    forceAnswer: true,
    stopFails: null as ErrorDto | null,
    forceFails: null as ErrorDto | null,
    quitAnswer: true,
    quitFails: null as ErrorDto | null,
    openFails: null as ErrorDto | null,
    count: (cmd: string) => calls.filter((c) => c.cmd === cmd).length,
    of: (cmd: string) => calls.filter((c) => c.cmd === cmd).map((c) => c.args),
    /** Sends an event on the channel of the n-th `play` (default: the last). */
    play: (event: PlayEvent, n = plays.length - 1) => plays[n].onmessage?.(event),
    /** Sets the game state and sends it as an app event. */
    state: (state: GameStateDto) => {
      b.game = state;
      events?.onmessage?.({ event: "gameState", data: { state } });
    },
    app: (event: AppEvent) => events?.onmessage?.(event),
  };
  mockIPC(async (cmd, payload) => {
    const args = (payload ?? {}) as Record<string, unknown>;
    calls.push({ cmd, args });
    switch (cmd) {
      case "subscribe_events":
        events = args.channel as Channel<AppEvent>;
        return;
      case "instances_list":
        if (b.listFails) throw b.listFails;
        return { instances: b.instances, incomplete: false };
      case "accounts_list":
        return b.accounts;
      case "game_state":
        if (b.gameGate) await b.gameGate;
        if (b.gameFails) throw b.gameFails;
        return b.game;
      case "play":
        if (b.playFails) throw b.playFails;
        plays.push(args.channel as Channel<PlayEvent>);
        return nextOp++;
      case "play_cancel":
        return b.cancelAnswer;
      case "game_stop":
        if (b.stopGate) await b.stopGate;
        if (b.stopFails) throw b.stopFails;
        return b.stopAnswer;
      case "game_force_stop":
        if (b.forceFails) throw b.forceFails;
        return b.forceAnswer;
      case "app_quit":
        if (b.quitFails) throw b.quitFails;
        return b.quitAnswer;
      case "instance_select":
        b.instances = b.instances.map((i) => ({ ...i, selected: i.id === args.id }));
        return;
      case "open_location":
        if (b.openFails) throw b.openFails;
        return;
      default:
        return;
    }
  });
  return b;
}
