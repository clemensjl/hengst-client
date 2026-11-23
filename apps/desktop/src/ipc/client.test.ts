import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it } from "vitest";
import type { LoginEvent } from "./bindings/LoginEvent";
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

  it("a non-DTO rejection reaches the detail bounded and without tokens, queries or user folders", async () => {
    const text =
      "failed GET https://api.example/v1?access_token=SECRET-1&state=Q with Authorization: Bearer SECRET-2; " +
      "bearer SECRET-3.abc at C:\\Users\\someone\\AppData\\x.json: denied; share \\\\fileserver\\share\\someone\\y " +
      "x".repeat(5000);
    mockIPC(() => {
      throw new Error(text);
    });
    const detail = await api.appInfo().then(
      () => "",
      (e: { dto: { detail: string } }) => e.dto.detail,
    );
    for (const banned of ["SECRET", "state=Q", "someone", "Users", "fileserver"]) {
      expect(detail).not.toContain(banned);
    }
    expect(detail).toContain("https://api.example/v1");
    // A long text without a path is cut.
    mockIPC(() => {
      throw "y".repeat(5000);
    });
    const long = await api.appInfo().then(
      () => "",
      (e: { dto: { detail: string } }) => e.dto.detail,
    );
    expect(long.length).toBeLessThanOrEqual(1000);
    expect(long.startsWith("yyyy")).toBe(true);
  });

  it.each([
    [
      "could not read C:\\Users\\O'Brien\\AppData\\settings.json: access denied",
      "could not read ...\\settings.json: access denied",
    ],
    [
      "could not read 'C:\\Users\\O'Brien\\AppData\\settings.json': access denied",
      "could not read '...\\settings.json': access denied",
    ],
    [
      'could not read "C:\\Users\\O\'Brien\\AppData\\settings.json": access denied',
      'could not read "...\\settings.json": access denied',
    ],
  ])("an apostrophe in a user name does not end the path: %s", async (text, want) => {
    mockIPC(() => {
      throw text;
    });
    const detail = await api.appInfo().then(
      () => "",
      (e: { dto: { detail: string } }) => e.dto.detail,
    );
    expect(detail).toBe(want);
  });

  it("loginStart passes the method, the language and the channel, no texts, and returns the op", async () => {
    let seen: [string, string[]] | undefined;
    mockIPC((cmd, args) => {
      const a = args as { channel: { onmessage?: (e: LoginEvent) => void } };
      seen = [cmd, Object.keys(a)];
      a.channel.onmessage?.({ event: "started", data: { op: 7 } });
      return 7;
    });
    const events: LoginEvent[] = [];
    const op = await api.loginStart("browser", "de", (e) => events.push(e));
    expect(seen).toEqual(["login_start", ["method", "language", "channel"]]);
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
      throw { code: "invalidInput", params: { field: "op" }, action: null, detail: "x" };
    });
    await expect(api.loginStart("deviceCode", "en", () => {})).rejects.toMatchObject({
      dto: { code: "invalidInput" },
    });
  });

  it("instance commands use camelCase args and send only slugs, ids and targets", async () => {
    const seen: [string, unknown][] = [];
    mockIPC((cmd, args) => {
      seen.push([cmd, args]);
      return cmd === "loader_kinds" || cmd === "instances_list" ? [] : undefined;
    });
    await api.loaderKinds();
    await api.loaderVersions("legacy-fabric", "1.8.9");
    await api.instancesList();
    const req = {
      name: "1.8.9 Legacy Fabric",
      game: "1.8.9",
      loader: { kind: "legacy-fabric", version: "0.15.1" },
    } as const;
    await api.instanceCreate(req);
    await api.instanceSelect("1.8.9");
    await api.instanceDelete("1.8.9");
    await api.openLocation({ kind: "instance", id: "1.8.9" });
    await api.openLocation({ kind: "dataDir" });
    expect(seen).toEqual([
      ["loader_kinds", {}],
      ["loader_versions", { kind: "legacy-fabric", game: "1.8.9" }],
      ["instances_list", {}],
      ["instance_create", { req }],
      ["instance_select", { id: "1.8.9" }],
      ["instance_delete", { id: "1.8.9" }],
      ["open_location", { target: { kind: "instance", id: "1.8.9" } }],
      ["open_location", { target: { kind: "dataDir" } }],
    ]);
  });

  it("a Recycle Bin failure rejects with the typed error and its id", async () => {
    mockIPC(() => {
      throw { code: "recycleBinFailed", params: { id: "9.9" }, action: null, detail: "x" };
    });
    await expect(api.instanceDelete("9.9")).rejects.toMatchObject({
      dto: { code: "recycleBinFailed", params: { id: "9.9" } },
    });
  });
});
