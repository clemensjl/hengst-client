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
export function listenerPids(
  port: number,
  run: (command: string) => Promise<string> = runPowerShell,
): Promise<number[]> {
  return run(listenerQuery(port)).then(parseListenerPids);
}

/**
 * The PowerShell command behind `listenerPids`. "No such connection" is the answer "nothing
 * listens" (category ObjectNotFound); any other error exits with 2 and fails the query.
 */
export function listenerQuery(port: number): string {
  if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error(`bad port ${port}`);
  return (
    `$c = @(Get-NetTCPConnection -State Listen -LocalPort ${port}` +
    " -ErrorAction SilentlyContinue -ErrorVariable e);" +
    " if ($e | Where-Object { $_.CategoryInfo.Category -ne 'ObjectNotFound' }) { exit 2 };" +
    " $c | ForEach-Object { $_.OwningProcess }; exit 0"
  );
}

/** One PID per line of the query's output; anything else in it is ignored. */
export function parseListenerPids(stdout: string): number[] {
  return stdout
    .split(/\s+/)
    .filter((word) => /^\d+$/.test(word))
    .map(Number);
}

/**
 * Runs a command in a fresh Windows PowerShell and resolves with its output; a non-zero exit is
 * an error. A cold start plus the NetTCPIP (CIM) module takes seconds on a busy machine.
 */
export function runPowerShell(command: string): Promise<string> {
  return new Promise((done, fail) => {
    execFile(
      "powershell.exe",
      ["-NoProfile", "-NonInteractive", "-Command", command],
      { windowsHide: true, timeout: 15_000 },
      (error, stdout) => (error ? fail(error) : done(stdout)),
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
  /** What the app reports as `e2eBuild` (`app_info`): built with the `e2e` feature. */
  e2eBuild: boolean | undefined;
}

const normalize = (p: string) =>
  resolve(p)
    .replace(/[\\/]+$/, "")
    .toLowerCase();

/** Why the run must stop, or `null` when the app runs on the e2e data folder behind our driver. */
export function guardFailure({
  home,
  dataDir,
  driverRunning,
  e2eBuild,
}: GuardInput): string | null {
  if (!driverRunning) return "our tauri-driver is not running (it exited or did not start)";
  if (!home) return "there is no e2e data folder";
  if (!dataDir) return "the app reported no data folder";
  if (normalize(dataDir) !== normalize(home)) {
    return `the app runs on ${dataDir}, not on the e2e data folder`;
  }
  // A plain debug build honours HENGST_HOME as well, but signs in with Microsoft and downloads
  // from Mojang: a game spec would reach the real services.
  if (e2eBuild !== true) return "the app is not an e2e build (build it with --features e2e)";
  return null;
}

/**
 * Written by a worker that stopped the run; every later worker of the same launcher (its parent
 * process) finds it and exits before it starts anything.
 */
export function abortMarker(launcherPid: number): string {
  return join(tmpdir(), `hengst-e2e-abort-${launcherPid}`);
}
