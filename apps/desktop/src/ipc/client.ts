import { invoke } from "@tauri-apps/api/core";
import type { AppInfo } from "./bindings/AppInfo";
import type { ErrorDto } from "./bindings/ErrorDto";
import type { GameVersionsDto } from "./bindings/GameVersionsDto";
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

export const api = {
  appInfo: (): Promise<AppInfo> => guard(invoke<AppInfo>("app_info")),
  gameVersions: (filter: VersionFilter): Promise<GameVersionsDto> =>
    guard(invoke<GameVersionsDto>("game_versions", { filter })),
};
