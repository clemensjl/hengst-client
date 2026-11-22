import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it } from "vitest";
import type { LoginEvent } from "./bindings/LoginEvent";
import type { LoginPagesDto } from "./bindings/LoginPagesDto";
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

  const pages: LoginPagesDto = {
    successTitle: "Done",
    successBody: "You can return to the launcher.",
    failureTitle: "Not done",
    failureBody: "Go back to the launcher.",
  };

  it("loginStart passes the channel and the pages and returns the op", async () => {
    let seen: [string, string[]] | undefined;
    mockIPC((cmd, args) => {
      const a = args as { channel: { onmessage?: (e: LoginEvent) => void } };
      seen = [cmd, Object.keys(a)];
      a.channel.onmessage?.({ event: "started", data: { op: 7 } });
      return 7;
    });
    const events: LoginEvent[] = [];
    const op = await api.loginStart("browser", pages, (e) => events.push(e));
    expect(seen).toEqual(["login_start", ["method", "pages", "channel"]]);
    expect(op).toBe(7);
    expect(events[0]).toEqual({ event: "started", data: { op: 7 } });
  });

  it("accounts commands use camelCase args", async () => {
    const seen: [string, unknown][] = [];
    mockIPC((cmd, args) => {
      seen.push([cmd, args]);
      return cmd === "login_cancel" ? true : undefined;
    });
    await api.accountSelect("abc");
    await api.accountRemove("abc");
    expect(await api.loginCancel(3)).toBe(true);
    await api.loginReopenBrowser(3);
    await api.accountsList();
    expect(seen).toEqual([
      ["account_select", { id: "abc" }],
      ["account_remove", { id: "abc" }],
      ["login_cancel", { op: 3 }],
      ["login_reopen_browser", { op: 3 }],
      ["accounts_list", {}],
    ]);
  });

  it("a failed sign-in command rejects with the typed error", async () => {
    mockIPC(() => {
      throw { code: "invalidInput", params: { field: "pages" }, action: null, detail: "x" };
    });
    await expect(api.loginStart("deviceCode", pages, () => {})).rejects.toMatchObject({
      dto: { code: "invalidInput" },
    });
  });
});
