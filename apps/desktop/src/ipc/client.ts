import { Channel, invoke } from "@tauri-apps/api/core";
import type { AccountsDto } from "./bindings/AccountsDto";
import type { AppInfo } from "./bindings/AppInfo";
import type { ErrorDto } from "./bindings/ErrorDto";
import type { GameVersionsDto } from "./bindings/GameVersionsDto";
import type { Language } from "./bindings/Language";
import type { LoginEvent } from "./bindings/LoginEvent";
import type { LoginMethod } from "./bindings/LoginMethod";
import type { SettingsDto } from "./bindings/SettingsDto";
import type { SettingsPatch } from "./bindings/SettingsPatch";
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

/** Every rejection of `invoke` becomes an `IpcError`; a non-DTO value is wrapped as `internal`. */
export async function guard<T>(call: Promise<T>): Promise<T> {
  try {
    return await call;
  } catch (value) {
    if (isErrorDto(value)) throw new IpcError(value);
    throw new IpcError({ code: "internal", params: {}, action: null, detail: String(value) });
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
};
