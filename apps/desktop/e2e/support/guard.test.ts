// @vitest-environment node
import { createServer, type Server } from "node:net";
import { afterEach, describe, expect, it } from "vitest";
import { abortMarker, guardFailure, portInUse } from "./guard.ts";

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

describe("guardFailure", () => {
  const home = "C:\\Temp\\hengst-e2e-abc";

  it("passes only when our driver runs and the app uses the e2e folder", () => {
    expect(guardFailure({ home, dataDir: home, driverRunning: true })).toBeNull();
    // Same folder, other spelling.
    expect(
      guardFailure({ home, dataDir: "c:\\temp\\HENGST-E2E-ABC\\", driverRunning: true }),
    ).toBeNull();
  });

  it("fails when the app runs on another folder", () => {
    expect(
      guardFailure({
        home,
        dataDir: "D:\\Roaming\\HengstClient",
        driverRunning: true,
      }),
    ).toMatch(/not on the e2e data folder/);
    // A folder below or beside the e2e folder is another folder.
    expect(guardFailure({ home, dataDir: `${home}\\sub`, driverRunning: true })).not.toBeNull();
    expect(guardFailure({ home, dataDir: `${home}x`, driverRunning: true })).not.toBeNull();
  });

  it("fails without an answer, without a folder or without our driver", () => {
    expect(guardFailure({ home, dataDir: undefined, driverRunning: true })).toMatch(
      /no data folder/,
    );
    expect(guardFailure({ home, dataDir: "", driverRunning: true })).toMatch(/no data folder/);
    expect(guardFailure({ home: undefined, dataDir: home, driverRunning: true })).toMatch(
      /no e2e data folder/,
    );
    expect(guardFailure({ home, dataDir: home, driverRunning: false })).toMatch(/tauri-driver/);
  });
});

describe("abortMarker", () => {
  it("is one file per launcher process", () => {
    expect(abortMarker(10)).not.toBe(abortMarker(11));
    expect(abortMarker(10)).toMatch(/hengst-e2e-abort-10$/);
  });
});
