import { execFile } from "node:child_process";
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

/**
 * The processes that listen on TCP `<port>` (any local address), from `Get-NetTCPConnection`
 * (its output does not depend on the Windows display language, unlike `netstat`). Empty when
 * nothing listens; a failing query is an error, never "nothing".
 */
export function listenerPids(port: number): Promise<number[]> {
  // "No such connection" is the answer "nothing listens" (category ObjectNotFound); any other
  // error fails the query.
  const command =
    `$c = @(Get-NetTCPConnection -State Listen -LocalPort ${Math.trunc(port)}` +
    " -ErrorAction SilentlyContinue -ErrorVariable e);" +
    " if ($e | Where-Object { $_.CategoryInfo.Category -ne 'ObjectNotFound' }) { exit 2 };" +
    " $c | ForEach-Object { $_.OwningProcess }; exit 0";
  return new Promise((done, fail) => {
    execFile(
      "powershell.exe",
      ["-NoProfile", "-NonInteractive", "-Command", command],
      { windowsHide: true, timeout: 15_000 },
      (error, stdout) => {
        if (error) return fail(error);
        done(
          stdout
            .split(/\s+/)
            .filter(Boolean)
            .map(Number)
            .filter((pid) => Number.isInteger(pid)),
        );
      },
    );
  });
}

/**
 * The port is held by our driver alone: at least one listener, and every listener (Windows lets
 * a second process bind the same port beside it) is the process we started.
 */
export function listenerIsOurs(pids: number[], ours: number | undefined): boolean {
  return ours !== undefined && pids.length > 0 && pids.every((pid) => pid === ours);
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
