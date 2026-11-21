import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it } from "vitest";
import { api } from "./client";

afterEach(() => clearMocks());

describe("ipc client", () => {
  it("calls game_versions with camelCase args", async () => {
    let seen: [string, unknown] | undefined;
    mockIPC((cmd, args) => {
      seen = [cmd, args];
      return { versions: [], latestRelease: "1.21.11", latestSnapshot: "x", fromCache: false };
    });
    const filter = { release: true, snapshot: false, beta: false, alpha: false };
    await api.gameVersions(filter);
    expect(seen).toEqual(["game_versions", { filter }]);
  });

  it("opens a page by target name only, never by URL", async () => {
    let seen: [string, unknown] | undefined;
    mockIPC((cmd, args) => {
      seen = [cmd, args];
    });
    await api.openUrl("buildHelp");
    expect(seen).toEqual(["open_url", { target: "buildHelp" }]);
  });

  it("turns a rejected ErrorDto into IpcError", async () => {
    mockIPC(() => {
      throw { code: "network", params: {}, action: { kind: "retry" }, detail: "x" };
    });
    await expect(api.appInfo()).rejects.toMatchObject({ dto: { code: "network" } });
  });

  it("wraps a non-DTO rejection as internal", async () => {
    mockIPC(() => {
      throw "boom";
    });
    await expect(api.appInfo()).rejects.toMatchObject({
      dto: { code: "internal", params: {}, action: null, detail: "boom" },
    });
  });
});
