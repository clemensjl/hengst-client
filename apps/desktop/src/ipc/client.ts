import { Channel, invoke } from "@tauri-apps/api/core";
import type { AccountsDto } from "./bindings/AccountsDto";
import type { AppInfo } from "./bindings/AppInfo";
import type { CreateInstanceRequest } from "./bindings/CreateInstanceRequest";
import type { ErrorDto } from "./bindings/ErrorDto";
import type { GameStateDto } from "./bindings/GameStateDto";
import type { GameVersionsDto } from "./bindings/GameVersionsDto";
import type { InstanceDto } from "./bindings/InstanceDto";
import type { InstancesDto } from "./bindings/InstancesDto";
import type { Language } from "./bindings/Language";
import type { LoaderInfoDto } from "./bindings/LoaderInfoDto";
import type { LoaderKindDto } from "./bindings/LoaderKindDto";
import type { LoaderVersionsDto } from "./bindings/LoaderVersionsDto";
import type { LoginEvent } from "./bindings/LoginEvent";
import type { LoginMethod } from "./bindings/LoginMethod";
import type { OpenTarget } from "./bindings/OpenTarget";
import type { PlayEvent } from "./bindings/PlayEvent";
import type { SettingsDto } from "./bindings/SettingsDto";
import type { SettingsPatch } from "./bindings/SettingsPatch";
import type { StopResult } from "./bindings/StopResult";
import type { UrlTarget } from "./bindings/UrlTarget";
import type { VersionFilter } from "./bindings/VersionFilter";

export class IpcError extends Error {
  constructor(readonly dto: ErrorDto) {
    super(dto.detail || dto.code);
    this.name = "IpcError";
  }
}

function isErrorDto(value: unknown): value is ErrorDto {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as ErrorDto).code === "string" &&
    typeof (value as ErrorDto).params === "object" &&
    typeof (value as ErrorDto).detail === "string"
  );
}

/** The longest detail a non-DTO rejection keeps. */
const DETAIL_LIMIT = 1000;

/**
 * Makes the text of a rejection that did not come from the backend's own errors safe for the
 * "Details" disclosure, as the backend does with its own (`error.rs`): URL query parts, the value
 * of an `Authorization` header and bearer tokens are removed, every absolute Windows path (drive,
 * UNC or `\\?\`) is cut to its last component, and the text is bounded.
 */
function redactDetail(text: string): string {
  const out = text
    .replace(/(https?:\/\/[^\s?)\]"'>]*)\?[^\s)\]"'>]*/g, "$1")
    .replace(/(authorization[:=])[^\n\r;]*/gi, "$1 ***")
    .replace(/(bearer\s+)([^\s;,)"']+)/gi, (all, prefix: string, word: string) =>
      // The word "bearer" in prose ("the bearer of ...") has no token after it.
      word.length >= 16 || /[\d.\-_/+=]/.test(word) ? `${prefix}***` : all,
    );
  const stripped = stripPaths(out);
  return stripped.length > DETAIL_LIMIT ? `${stripped.slice(0, DETAIL_LIMIT - 1)}…` : stripped;
}

/**
 * As `strip_paths` in `error.rs`: a path runs to `: `, a line break, the end of the text, or the
 * quote that opened it when no letter or digit follows that quote (`O'Brien` stays inside).
 */
function stripPaths(text: string): string {
  const start = /(?<![A-Za-z0-9])(?:[A-Za-z]:[\\/]|\\{2,}(?=[^\s"'\\]))/g;
  let out = "";
  let from = 0;
  for (let m = start.exec(text); m; m = start.exec(text)) {
    const i = m.index;
    const before = text[i - 1];
    const opener = before === '"' || before === "'" ? before : null;
    let end = i + m[0].length;
    while (end < text.length) {
      const c = text[end];
      const next = text[end + 1] ?? "";
      if (c === "\n" || c === "\r" || (c === ":" && next === " ")) break;
      if (c === opener && !/[\p{L}\p{N}]/u.test(next)) break;
      end++;
    }
    const last = text.slice(i, end).split(/[\\/]/).filter(Boolean).pop() ?? "";
    out += `${text.slice(from, i)}...\\${last}`;
    from = end;
    start.lastIndex = end;
  }
  return out + text.slice(from);
}

/** Every rejection of `invoke` becomes an `IpcError`; a non-DTO value is wrapped as `internal`. */
export async function guard<T>(call: Promise<T>): Promise<T> {
  try {
    return await call;
  } catch (value) {
    if (isErrorDto(value)) throw new IpcError(value);
    throw new IpcError({
      code: "internal",
      params: {},
      action: null,
      detail: redactDetail(String(value)),
    });
  }
}

/** The backend wants every `Patch` field; a change names only what it changes. */
const KEEP: Pick<SettingsPatch, "language" | "javaOverride"> = {
  language: { op: "keep" },
  javaOverride: { op: "keep" },
};

export const api = {
  appInfo: (): Promise<AppInfo> => guard(invoke<AppInfo>("app_info")),
  gameVersions: (filter: VersionFilter): Promise<GameVersionsDto> =>
    guard(invoke<GameVersionsDto>("game_versions", { filter })),
  /** Opens a fixed page in the browser. The address is chosen in Rust; only the name travels. */
  openUrl: (target: UrlTarget): Promise<void> => guard(invoke<void>("open_url", { target })),
  settingsGet: (): Promise<SettingsDto> => guard(invoke<SettingsDto>("settings_get")),
  settingsUpdate: (change: Partial<SettingsPatch>): Promise<SettingsDto> =>
    guard(invoke<SettingsDto>("settings_update", { patch: { ...KEEP, ...change } })),
  accountsList: (): Promise<AccountsDto> => guard(invoke<AccountsDto>("accounts_list")),
  /** `id` is the id of an entry of `accountsList`. */
  accountSelect: (id: string): Promise<void> => guard(invoke<void>("account_select", { id })),
  accountRemove: (id: string): Promise<void> => guard(invoke<void>("account_remove", { id })),
  /**
   * Starts a sign-in; `onEvent` gets that sign-in's events and nobody else's. The first event is
   * `started` with the operation id, which can arrive before this resolves. A sign-in that runs
   * is cancelled by a new one. `language` is the language the app shows: it only picks which of
   * the two fixed page texts the browser shows afterwards (the texts stay in Rust).
   */
  loginStart: (
    method: LoginMethod,
    language: Language,
    onEvent: (e: LoginEvent) => void,
  ): Promise<number> => {
    const channel = new Channel<LoginEvent>();
    channel.onmessage = onEvent;
    return guard(invoke<number>("login_start", { method, language, channel }));
  },
  /** `true` if the sign-in was still running; its channel then gets `cancelled`. */
  loginCancel: (op: number): Promise<boolean> => guard(invoke<boolean>("login_cancel", { op })),
  /** Opens the sign-in page again; the address never reaches the web view. */
  loginReopenBrowser: (op: number): Promise<void> =>
    guard(invoke<void>("login_reopen_browser", { op })),
  /** The loaders in the order the choice shows them, with their display names. */
  loaderKinds: (): Promise<LoaderInfoDto[]> => guard(invoke<LoaderInfoDto[]>("loader_kinds")),
  /** One request to the loader service; `recommended` is the build the core would pick. */
  loaderVersions: (kind: LoaderKindDto, game: string): Promise<LoaderVersionsDto> =>
    guard(invoke<LoaderVersionsDto>("loader_versions", { kind, game })),
  /** The instances, newest played first, and whether every folder could be read. */
  instancesList: (): Promise<InstancesDto> => guard(invoke<InstancesDto>("instances_list")),
  /** Sends slugs and versions only, never a display name. Selects the new instance. */
  instanceCreate: (req: CreateInstanceRequest): Promise<InstanceDto> =>
    guard(invoke<InstanceDto>("instance_create", { req })),
  /** `id` is the id of an entry of `instancesList`. */
  instanceSelect: (id: string): Promise<void> => guard(invoke<void>("instance_select", { id })),
  /** Moves the folder to the Recycle Bin; never deletes for good. */
  instanceDelete: (id: string): Promise<void> => guard(invoke<void>("instance_delete", { id })),
  /** Opens the data folder or the folder of an instance; the path is made in Rust. */
  openLocation: (target: OpenTarget): Promise<void> =>
    guard(invoke<void>("open_location", { target })),
  /**
   * Starts the instance; `onEvent` gets this start's events, `preparing` with the operation id
   * first (it can arrive before this resolves), then progress and one of `started`, `failed`,
   * `cancelled`. Only the id travels: account, Java and arguments are chosen in Rust. Running,
   * stopping and the exit arrive as `gameState` app events. `busy` while a game is prepared or runs.
   */
  play: (instanceId: string, onEvent: (e: PlayEvent) => void): Promise<number> => {
    const channel = new Channel<PlayEvent>();
    channel.onmessage = onEvent;
    return guard(invoke<number>("play", { instanceId, channel }));
  },
  /** `true` if the start was still being prepared; its channel then gets `cancelled`. */
  playCancel: (op: number): Promise<boolean> => guard(invoke<boolean>("play_cancel", { op })),
  /** Asks the game to close; `noWindow` means nothing was sent (offer the force stop). */
  gameStop: (): Promise<StopResult> => guard(invoke<StopResult>("game_stop")),
  /** Ends the game at once; `false` when none runs. */
  gameForceStop: (): Promise<boolean> => guard(invoke<boolean>("game_force_stop")),
  /** The game's state now, as the last `gameState` event carried it. */
  gameState: (): Promise<GameStateDto> => guard(invoke<GameStateDto>("game_state")),
  /** Quits the app; `false` (and nothing happens) while a game still runs. */
  appQuit: (): Promise<boolean> => guard(invoke<boolean>("app_quit")),
};
