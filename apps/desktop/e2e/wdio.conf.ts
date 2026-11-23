import { spawn, type ChildProcess } from "node:child_process";
import { existsSync, rmSync, writeFileSync } from "node:fs";
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
import {
  abortMarker,
  guardFailure,
  listenerIsOurs,
  listenerPids,
  portInUse,
} from "./support/guard.ts";

const DRIVER_PORT = 4444;
const NATIVE_DRIVER = resolve(import.meta.dirname, ".driver/msedgedriver.exe");

/** Per worker (one spec file): its data folder, its mock and its tauri-driver. */
let home: string | undefined;
let mock: Mock | undefined;
let tauriDriver: ChildProcess | undefined;
let driverError: Error | undefined;

/**
 * Waits until our tauri-driver listens on `port`, and proves the listener is that process: the
 * owning PID of every listener on the port must be `tauriDriver.pid`. A foreign listener (one that
 * took the port after the free-port check, or that binds beside ours) fails this instead of
 * getting our sessions. This holds at the moment of the check, right before the session is
 * created; the data-folder guard in `before` stays the check that the app really runs on ours.
 */
async function waitForOurListener(port: number, ms: number): Promise<void> {
  const until = Date.now() + ms;
  for (;;) {
    if (!driverRunning()) throw new Error(`it is not running (${driverError ?? "it exited"})`);
    const pids = await listenerPids(port);
    if (listenerIsOurs(pids, tauriDriver?.pid)) return;
    if (pids.some((pid) => pid !== tauriDriver?.pid)) {
      throw new Error(
        `port ${port} is held by process ${pids.join(", ")}, not by our tauri-driver`,
      );
    }
    if (Date.now() > until) throw new Error(`nothing listens on port ${port}`);
    await new Promise((done) => setTimeout(done, 100));
  }
}

const driverRunning = () =>
  !!tauriDriver && tauriDriver.exitCode === null && tauriDriver.signalCode === null && !driverError;

/**
 * Stops the whole run. WDIO only logs errors thrown in worker hooks and carries on, so this ends
 * the worker itself: its exit code 1 fails the run (the launcher keeps the first non-zero worker
 * exit code), and the marker makes every later worker of this launcher exit before it starts
 * anything. Everything this worker started is ended by PID and its data folder removed.
 */
async function abortRun(reason: string, withSession: boolean): Promise<never> {
  console.error(`e2e run stopped: ${reason}`);
  try {
    writeFileSync(abortMarker(process.ppid), reason);
  } catch {
    // The exit code below still fails the run.
  }
  // Only a driver that still runs: the PID of one that exited may belong to another process now.
  if (tauriDriver?.pid && tauriDriver.exitCode === null && tauriDriver.signalCode === null)
    killTree(tauriDriver.pid);
  if (withSession) await browser.deleteSession().catch(() => {});
  if (mock && mock.process.exitCode === null && mock.process.signalCode === null) {
    mock.process.kill();
  }
  if (home) removeHome(home);
  process.exit(1);
}

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

  // Launcher, once, before any worker: a driver that is already there (started by hand, or left
  // over) would get our sessions and start the app without our data folder.
  onPrepare: async () => {
    rmSync(abortMarker(process.pid), { force: true });
    if (await portInUse(DRIVER_PORT)) {
      console.error(
        `e2e run stopped: something already listens on port ${DRIVER_PORT}. Close that ` +
          "WebDriver (tauri-driver) first.",
      );
      process.exit(1);
    }
  },

  onComplete: () => {
    rmSync(abortMarker(process.pid), { force: true });
  },

  // Runs in the worker of each spec file, before its session (and with it the app) starts.
  beforeSession: async (_config, _capabilities, specs) => {
    if (existsSync(abortMarker(process.ppid))) {
      console.error("e2e run stopped: an earlier spec file stopped the run.");
      process.exit(1);
    }
    // The previous worker's driver is gone; anything on the port now is foreign.
    if (await portInUse(DRIVER_PORT)) {
      await abortRun(`something already listens on port ${DRIVER_PORT}`, false);
    }
    const setup = setupFor(specs[0] ?? "");
    home = makeHome(setup.seed);
    process.env.HENGST_E2E_HOME = home;
    try {
      mock = await startMock(setup.scenario);
    } catch (e) {
      await abortRun(`hengst-mock did not start: ${String(e)}`, false);
      return;
    }
    // The app gets its data folder and the mock from this environment: msedgedriver hands
    // `tauri:options.args` to WebView2, not to the app, so `--home` cannot be passed that way.
    driverError = undefined;
    tauriDriver = spawn("tauri-driver", ["--native-driver", NATIVE_DRIVER], {
      env: { ...process.env, HENGST_HOME: home, HENGST_E2E_BASE_URL: mock.url },
      stdio: ["ignore", "inherit", "inherit"],
      windowsHide: true,
    });
    tauriDriver.once("error", (e) => {
      driverError = e;
    });
    try {
      await waitForOurListener(DRIVER_PORT, 15_000);
    } catch (e) {
      await abortRun(`tauri-driver did not start: ${String(e)}`, false);
    }
  },

  // The app must run on the temp folder behind our driver: anything else, including a failed
  // check, stops the run before a single click.
  before: async () => {
    let failure: string | null;
    try {
      const info = await browser.execute(async () => {
        const internals = (
          window as unknown as {
            __TAURI_INTERNALS__: { invoke: (cmd: string) => Promise<{ dataDir: string }> };
          }
        ).__TAURI_INTERNALS__;
        return internals.invoke("app_info");
      });
      failure = guardFailure({ home, dataDir: info?.dataDir, driverRunning: driverRunning() });
    } catch (e) {
      failure = `the data folder could not be checked: ${String(e)}`;
    }
    if (failure) await abortRun(failure, true);
  },

  // A failed test notes whether the app window had the focus: another window taking the
  // foreground closes menus and popups.
  afterTest: async (_test, _context, { passed }) => {
    if (passed) return;
    const focused = await browser.execute(() => document.hasFocus()).catch(() => "unknown");
    console.log(`after the failed test: document.hasFocus() = ${String(focused)}`);
  },

  afterSession: async () => {
    if (tauriDriver?.pid && tauriDriver.exitCode === null && tauriDriver.signalCode === null)
      killTree(tauriDriver.pid);
    tauriDriver = undefined;
    if (mock) await stopMock(mock);
    mock = undefined;
    if (home) removeHome(home);
    home = undefined;
  },
};
