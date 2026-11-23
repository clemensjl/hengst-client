import type { GameStateDto } from "@/ipc/bindings/GameStateDto";
import type { InstanceDto } from "@/ipc/bindings/InstanceDto";
import { effectiveSelection } from "@/features/instances/selection";

/** What the Play button needs to know about the account the game would start with. */
export type PlayAccount =
  | { kind: "ok" }
  /** No account at all. */
  | { kind: "none" }
  /** Accounts exist, none is chosen (the header chip says "Konto wählen"). */
  | { kind: "choose" }
  | { kind: "signInAgain"; id: string; name: string };

export interface PlayInput {
  /** The backend's game state; `null` before it is known (counts as idle). */
  game: GameStateDto | null;
  /**
   * A start this page made that has not ended on its channel yet. `op` arrives with `preparing`;
   * `cancelRefused` is set when `play_cancel` came too late (the start goes on) and when the
   * start reported `started` but the game state does not say running yet.
   */
  launch: { op: number | null; cancelRefused: boolean } | null;
  instances: InstanceDto[];
  account: PlayAccount;
}

export type PlayButtonState =
  | { kind: "stopping" }
  | { kind: "running"; since: number }
  | { kind: "starting" }
  | { kind: "preparing"; op: number }
  | { kind: "noInstance"; unreadable: boolean }
  | { kind: "chooseInstance"; name: string }
  | { kind: "signIn" }
  | { kind: "chooseAccount" }
  | { kind: "signInAgain"; id: string; name: string }
  | { kind: "ready"; instance: string };

/**
 * DESIGN.md 10.2, first match wins. The game state comes first, then a start of this page, then
 * the instances, then the account. `PlayEvent` and the `gameState` event travel on different
 * channels, so either can be ahead: the game state decides once it says running or stopping, the
 * page's start decides while the game state does not.
 */
export function playButtonState({ game, launch, instances, account }: PlayInput): PlayButtonState {
  if (game?.state === "stopping") return { kind: "stopping" };
  if (game?.state === "running") return { kind: "running", since: game.data.since };
  if (launch) {
    if (launch.op === null || launch.cancelRefused) return { kind: "starting" };
    return { kind: "preparing", op: launch.op };
  }
  // Being prepared, but not by this page (a reload): the page has no operation id to cancel.
  if (game?.state === "preparing") return { kind: "starting" };
  const readable = instances.filter((i) => i.status === "ready");
  if (readable.length === 0) return { kind: "noInstance", unreadable: instances.length > 0 };
  const chosen = instances.find((i) => i.selected);
  if (chosen && chosen.status !== "ready") return { kind: "chooseInstance", name: chosen.name };
  if (account.kind === "none") return { kind: "signIn" };
  if (account.kind === "choose") return { kind: "chooseAccount" };
  if (account.kind === "signInAgain") return account;
  return { kind: "ready", instance: effectiveSelection(instances)! };
}
