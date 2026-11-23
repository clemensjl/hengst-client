import { spawn, type ChildProcess } from "node:child_process";
import { createConnection } from "node:net";
import { resolve } from "node:path";
import {
  APP_EXE,
  killTree,
  makeHome,
  removeHome,
  setupFor,
  startMock,
  stopMock,
  type Mock,
} from "./support/app.ts";

const DRIVER_PORT = 4444;
const NATIVE_DRIVER = resolve(import.meta.dirname, ".driver/msedgedriver.exe");

/** Per worker (one spec file): its data folder, its mock and its tauri-driver. */
let home: string | undefined;
let mock: Mock | undefined;
let tauriDriver: ChildProcess | undefined;

function waitForPort(port: number, ms: number): Promise<void> {
  const until = Date.now() + ms;
  return new Promise((done, fail) => {
    const attempt = () => {
      const socket = createConnection({ host: "127.0.0.1", port }, () => {
        socket.end();
        done();
      });
      socket.once("error", () => {
        socket.destroy();
        if (Date.now() > until) fail(new Error(`nothing listens on port ${port}`));
        else setTimeout(attempt, 100);
      });
    };
    attempt();
  });
}

const normalize = (p: string) => resolve(p).toLowerCase();

export const config: WebdriverIO.Config = {
  runner: "local",
  hostname: "127.0.0.1",
  port: DRIVER_PORT,
  specs: ["./specs/**/*.e2e.ts"],
  maxInstances: 1,
  capabilities: [
    {
      // tauri-driver speaks classic WebDriver only.
      "wdio:enforceWebDriverClassic": true,
      "tauri:options": { application: APP_EXE },
    } as WebdriverIO.Capabilities,
  ],
  logLevel: "warn",
  framework: "mocha",
  reporters: ["spec"],
  mochaOpts: { ui: "bdd", timeout: 60_000 },
  waitforTimeout: 10_000,

  // Runs in the worker of each spec file, before its session (and with it the app) starts.
  beforeSession: async (_config, _capabilities, specs) => {
    const setup = setupFor(specs[0] ?? "");
    home = makeHome(setup.seed);
    mock = await startMock(setup.scenario);
    process.env.HENGST_E2E_HOME = home;
    // The app gets its data folder and the mock from this environment: msedgedriver hands
    // `tauri:options.args` to WebView2, not to the app, so `--home` cannot be passed that way.
    tauriDriver = spawn("tauri-driver", ["--native-driver", NATIVE_DRIVER], {
      env: { ...process.env, HENGST_HOME: home, HENGST_E2E_BASE_URL: mock.url },
      stdio: ["ignore", "inherit", "inherit"],
      windowsHide: true,
    });
    await waitForPort(DRIVER_PORT, 15_000);
  },

  // The app must run on the temp folder: anything else stops the run before a single click.
  before: async () => {
    const info = await browser.execute(async () => {
      const internals = (
        window as unknown as {
          __TAURI_INTERNALS__: { invoke: (cmd: string) => Promise<{ dataDir: string }> };
        }
      ).__TAURI_INTERNALS__;
      return internals.invoke("app_info");
    });
    if (!home || normalize(info.dataDir) !== normalize(home)) {
      if (tauriDriver?.pid) killTree(tauriDriver.pid);
      throw new Error(`the app does not run on the e2e data folder: ${info.dataDir}`);
    }
  },

  afterSession: async () => {
    if (tauriDriver?.pid && tauriDriver.exitCode === null) killTree(tauriDriver.pid);
    tauriDriver = undefined;
    if (mock) await stopMock(mock);
    mock = undefined;
    if (home) removeHome(home);
    home = undefined;
  },
};
