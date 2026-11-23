import { createConnection } from "node:net";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

/**
 * Something accepts connections on `127.0.0.1:<port>`. Before our tauri-driver starts, that is a
 * foreign driver: a session sent there would start an app without our data folder.
 */
export function portInUse(port: number): Promise<boolean> {
  return new Promise((done) => {
    const socket = createConnection({ host: "127.0.0.1", port }, () => {
      socket.end();
      done(true);
    });
    socket.once("error", () => {
      socket.destroy();
      done(false);
    });
  });
}

export interface GuardInput {
  /** The temp data folder of this spec file. */
  home: string | undefined;
  /** What the app reports as its data folder (`app_info`). */
  dataDir: string | undefined;
  /** The tauri-driver this worker started is still running. */
  driverRunning: boolean;
}

const normalize = (p: string) =>
  resolve(p)
    .replace(/[\\/]+$/, "")
    .toLowerCase();

/** Why the run must stop, or `null` when the app runs on the e2e data folder behind our driver. */
export function guardFailure({ home, dataDir, driverRunning }: GuardInput): string | null {
  if (!driverRunning) return "our tauri-driver is not running (it exited or did not start)";
  if (!home) return "there is no e2e data folder";
  if (!dataDir) return "the app reported no data folder";
  if (normalize(dataDir) !== normalize(home)) {
    return `the app runs on ${dataDir}, not on the e2e data folder`;
  }
  return null;
}

/**
 * Written by a worker that stopped the run; every later worker of the same launcher (its parent
 * process) finds it and exits before it starts anything.
 */
export function abortMarker(launcherPid: number): string {
  return join(tmpdir(), `hengst-e2e-abort-${launcherPid}`);
}
