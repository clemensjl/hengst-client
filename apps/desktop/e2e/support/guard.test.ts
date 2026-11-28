// @vitest-environment node
import { createServer, type Server } from "node:net";
import { afterEach, describe, expect, it } from "vitest";
import {
  abortMarker,
  guardFailure,
  listenerIsOurs,
  listenerPids,
  listenerQuery,
  parseListenerPids,
  portInUse,
} from "./guard.ts";

let server: Server | undefined;
afterEach(async () => {
  await new Promise<void>((done) => (server ? server.close(() => done()) : done()));
  server = undefined;
});

function listen(): Promise<number> {
  server = createServer();
  return new Promise((done) =>
    server!.listen(0, "127.0.0.1", () => {
      const address = server!.address();
      done(typeof address === "object" && address ? address.port : 0);
    }),
  );
}

describe("portInUse", () => {
  it("sees a listener on the port", async () => {
    const port = await listen();
    expect(await portInUse(port)).toBe(true);
  });

  it("sees a free port as free", async () => {
    const port = await listen();
    await new Promise<void>((done) => server!.close(() => done()));
    server = undefined;
    expect(await portInUse(port)).toBe(false);
  });
});

describe("listenerPids", () => {
  it("asks for the listeners of exactly that port and reads one PID per line", async () => {
    const asked: string[] = [];
    const pids = await listenerPids(4444, async (command) => {
      asked.push(command);
      return "1234\r\n1234\r\n";
    });
    expect(pids).toEqual([1234, 1234]);
    expect(asked).toHaveLength(1);
    expect(asked[0]).toContain("Get-NetTCPConnection -State Listen -LocalPort 4444 ");
  });

  it("is empty when the query prints nothing (nothing listens)", async () => {
    expect(await listenerPids(4444, async () => "")).toEqual([]);
    expect(await listenerPids(4444, async () => "\r\n")).toEqual([]);
  });

  it("a failing query is an error, never 'nothing listens'", async () => {
    await expect(
      listenerPids(4444, () => Promise.reject(new Error("exit code 2"))),
    ).rejects.toThrow("exit code 2");
  });

  it("refuses a port that is not one", () => {
    expect(() => listenerQuery(0)).toThrow();
    expect(() => listenerQuery(65536)).toThrow();
    expect(() => listenerQuery(4444.5)).toThrow();
    expect(listenerQuery(1)).toContain("-LocalPort 1 ");
  });

  it("keeps only whole numbers from the output", () => {
    expect(parseListenerPids(" 12\n\nwarning\n34 ")).toEqual([12, 34]);
  });

  // The one real query: a fresh powershell.exe plus the NetTCPIP (CIM) module needs seconds on a
  // busy machine (more than vitest's 5 s default on the CI runner), so it gets its own budget. The
  // port is an ephemeral one of this test process, never a fixed one.
  it(
    "names this test process for a port it listens on (real Get-NetTCPConnection)",
    { timeout: 30_000 },
    async () => {
      const port = await listen();
      expect(await listenerPids(port)).toEqual([process.pid]);
    },
  );
});

describe("listenerIsOurs", () => {
  it("only when every listener on the port is our driver", () => {
    expect(listenerIsOurs([42], 42)).toBe(true);
    expect(listenerIsOurs([42, 42], 42)).toBe(true);
    expect(listenerIsOurs([], 42)).toBe(false);
    expect(listenerIsOurs([7], 42)).toBe(false);
    // A second process bound to the same port next to ours.
    expect(listenerIsOurs([42, 7], 42)).toBe(false);
    expect(listenerIsOurs([42], undefined)).toBe(false);
  });
});

describe("guardFailure", () => {
  const home = "C:\\Temp\\hengst-e2e-abc";

  it("passes only when our driver runs and the app uses the e2e folder", () => {
    expect(guardFailure({ home, dataDir: home, driverRunning: true, e2eBuild: true })).toBeNull();
    // Same folder, other spelling.
    expect(
      guardFailure({
        home,
        dataDir: "c:\\temp\\HENGST-E2E-ABC\\",
        driverRunning: true,
        e2eBuild: true,
      }),
    ).toBeNull();
  });

  it("fails when the app runs on another folder", () => {
    expect(
      guardFailure({
        home,
        dataDir: "D:\\Roaming\\HengstClient",
        driverRunning: true,
        e2eBuild: true,
      }),
    ).toMatch(/not on the e2e data folder/);
    // A folder below or beside the e2e folder is another folder.
    expect(
      guardFailure({ home, dataDir: `${home}\\sub`, driverRunning: true, e2eBuild: true }),
    ).not.toBeNull();
    expect(
      guardFailure({ home, dataDir: `${home}x`, driverRunning: true, e2eBuild: true }),
    ).not.toBeNull();
  });

  it("fails without an answer, without a folder or without our driver", () => {
    expect(guardFailure({ home, dataDir: undefined, driverRunning: true, e2eBuild: true })).toMatch(
      /no data folder/,
    );
    expect(guardFailure({ home, dataDir: "", driverRunning: true, e2eBuild: true })).toMatch(
      /no data folder/,
    );
    expect(
      guardFailure({ home: undefined, dataDir: home, driverRunning: true, e2eBuild: true }),
    ).toMatch(/no e2e data folder/);
    expect(guardFailure({ home, dataDir: home, driverRunning: false, e2eBuild: true })).toMatch(
      /tauri-driver/,
    );
  });

  it("fails for a build without the e2e feature, even on the right folder", () => {
    // A plain debug build honours HENGST_HOME too, but talks to Microsoft and Mojang.
    for (const e2eBuild of [false, undefined]) {
      expect(guardFailure({ home, dataDir: home, driverRunning: true, e2eBuild })).toMatch(
        /not an e2e build/,
      );
    }
  });
});

describe("abortMarker", () => {
  it("is one file per launcher process", () => {
    expect(abortMarker(10)).not.toBe(abortMarker(11));
    expect(abortMarker(10)).toMatch(/hengst-e2e-abort-10$/);
  });
});
