import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  realpathSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { runPowerShell } from "./guard.ts";
import { singleByRole } from "./roles.ts";

/** The cargo workspace root (where cargo is run from). */
const WORKSPACE = resolve(import.meta.dirname, "../../../..");

/**
 * Where cargo put the debug binaries: `CARGO_TARGET_DIR` when it is set, else the workspace's
 * `target/`. A relative `CARGO_TARGET_DIR` counts from the workspace root, where cargo is run,
 * not from the folder `pnpm` runs in.
 */
export function targetDebugDir(env: Record<string, string | undefined> = process.env): string {
  const dir = env.CARGO_TARGET_DIR?.trim();
  return resolve(WORKSPACE, dir || "target", "debug");
}

/** The workspace's debug binaries: `pnpm e2e` does not build them. */
const TARGET = targetDebugDir();
export const APP_EXE = join(TARGET, "hengst-desktop.exe");
export const MOCK_EXE = join(TARGET, "hengst-mock.exe");
/** The test kit's stand-in for java.exe (`cargo build -p hengst-testkit --bins`). */
export const FAKE_JAVA_EXE = join(TARGET, "fake-java.exe");

/** What a spec file needs before its app starts. */
export interface SpecSetup {
  /** `HENGST_MOCK_SCENARIO` of its `hengst-mock` (names in `AuthScenario::from_name`). */
  scenario: string;
  /** `HENGST_MOCK_MOJANG_DELAY_MS`: every Mojang answer waits this long (a slow preparation). */
  mojangDelayMs?: number;
  /** Writes extra files into the fresh data folder before the app starts. */
  seed?: (home: string) => void;
}

/** The instance the game specs play: version 9.9 of the mock, no loader. */
export const GAME_ID = "e2e-game";
export const GAME_NAME = "E2E Game";

export interface GameSeed {
  /** The development account is chosen (no sign-in needed, its token is "0"). */
  devAccount: boolean;
  /** "Minimize to the notification area when the game starts" (the app's default is on). */
  minimizeOnLaunch?: boolean;
  /** The stored Java path; default: a copy of fake-java as `<home>/jdk/bin/java.exe`. */
  java?: string;
}

/**
 * A data folder that can play: the instance `e2e-game` (9.9, selected) and fake-java as the Java
 * of the settings. What fake-java does is the instance's `fake-java.txt` (`setScript`). The copy
 * is named java.exe because the app accepts no other file name for Java.
 */
export function seedGame(seed: GameSeed): (home: string) => void {
  return (home) => {
    let java = seed.java;
    if (java === undefined) {
      const bin = join(home, "jdk", "bin");
      mkdirSync(bin, { recursive: true });
      java = join(bin, "java.exe");
      copyFileSync(FAKE_JAVA_EXE, java);
    }
    writeFileSync(
      join(home, "settings.json"),
      JSON.stringify({
        format: 1,
        language: "en",
        theme: "light",
        javaOverride: java,
        devAccount: seed.devAccount,
        minimizeOnLaunch: seed.minimizeOnLaunch ?? false,
        selectedInstance: GAME_ID,
      }),
    );
    const dir = join(home, "instances", GAME_ID);
    mkdirSync(dir, { recursive: true });
    writeFileSync(
      join(dir, "instance.json"),
      JSON.stringify({ format: 1, name: GAME_NAME, game: "9.9", loader: null, createdAt: 1 }),
    );
  };
}

/** The game folder of `e2e-game` in this spec file's data folder (fake-java's working folder). */
export const gameDir = () => join(process.env.HENGST_E2E_HOME!, "instances", GAME_ID);

/** What fake-java does at its next start (commands in `fake-java.rs`). */
export function setScript(lines: string[]) {
  writeFileSync(join(gameDir(), "fake-java.txt"), lines.join("\n") + "\n");
}

/** Creates `name` in the game folder: a running `wait <name>` goes on. */
export function touchGameFile(name: string) {
  writeFileSync(join(gameDir(), name), "");
}

/**
 * Asks the e2e build for what WebDriver cannot do (`e2e_hook.rs`): `close` (the window's close
 * request) or a tray menu item. Written through a rename, so the app never reads half a file.
 */
export function appRequest(request: "close" | "tray-open" | "tray-stop" | "tray-quit") {
  const home = process.env.HENGST_E2E_HOME!;
  const tmp = join(home, "e2e-request.tmp");
  writeFileSync(tmp, request);
  renameSync(tmp, join(home, "e2e-request.txt"));
}

/**
 * An instance saved by a newer build of the app ("Newer format", never deletable here) and an
 * unreadable one ("Unreadable", deletable).
 */
function seedTooNewInstance(home: string) {
  const write = (id: string, text: string) => {
    const dir = join(home, "instances", id);
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, "instance.json"), text);
  };
  write("from-the-future", JSON.stringify({ format: 99, name: "From the future" }));
  write("damaged", "{ not json");
}

/** Per spec file; files not named here run against the `ok` scenario. */
const SETUPS: Record<string, SpecSetup> = {
  // Every sign-in answer waits a second: the code view and "Signing in…" stay long enough to act on.
  "accounts.e2e.ts": { scenario: "slow:1000" },
  "sign-in-not-approved.e2e.ts": { scenario: "not-approved" },
  "sign-in-declined.e2e.ts": { scenario: "declined" },
  "instances.e2e.ts": { scenario: "ok" },
  "instances-too-new.e2e.ts": { scenario: "ok", seed: seedTooNewInstance },
  // Signs in E2EPlayer (a Microsoft account, so the token in the game's arguments is a real
  // secret to look for); every Mojang answer waits, so the preparation can be watched and cancelled.
  "play.e2e.ts": { scenario: "ok", mojangDelayMs: 400, seed: seedGame({ devAccount: false }) },
  "log.e2e.ts": { scenario: "ok", seed: seedGame({ devAccount: true }) },
  "tray.e2e.ts": { scenario: "ok", seed: seedGame({ devAccount: true, minimizeOnLaunch: true }) },
  "run-lock.e2e.ts": { scenario: "ok", seed: seedGame({ devAccount: true }) },
  // A stored Java path on a network share: refused before any file-system call.
  "settings-java.e2e.ts": {
    scenario: "ok",
    seed: seedGame({ devAccount: true, java: "\\\\192.0.2.1\\share\\java.exe" }),
  },
};

export function setupFor(specPath: string): SpecSetup {
  return SETUPS[basename(specPath)] ?? { scenario: "ok" };
}

/**
 * A fresh data folder in the temp directory with English and the light theme. The app is only
 * ever started on such a folder, never on the user's own.
 */
export function makeHome(seed?: (home: string) => void): string {
  const home = mkdtempSync(join(tmpdir(), "hengst-e2e-"));
  writeFileSync(
    join(home, "settings.json"),
    JSON.stringify({ format: 1, language: "en", theme: "light" }),
  );
  seed?.(home);
  return home;
}

export function removeHome(home: string) {
  try {
    rmSync(home, { recursive: true, force: true, maxRetries: 5, retryDelay: 500 });
  } catch (e) {
    console.warn(`could not remove the e2e data folder ${home}: ${String(e)}`);
  }
}

export interface Mock {
  url: string;
  process: ChildProcess;
}

/** Starts `hengst-mock` and waits for its first line `{"url": ...}`. */
export async function startMock(scenario: string, mojangDelayMs = 0): Promise<Mock> {
  const child = spawn(MOCK_EXE, [], {
    env: {
      ...process.env,
      HENGST_MOCK_SCENARIO: scenario,
      HENGST_MOCK_MOJANG_DELAY_MS: String(mojangDelayMs),
    },
    stdio: ["pipe", "pipe", "inherit"],
    windowsHide: true,
  });
  const lines = createInterface({ input: child.stdout! });
  try {
    const url = await new Promise<string>((resolveUrl, reject) => {
      const timer = setTimeout(
        () => reject(new Error("hengst-mock did not start in 10 s")),
        10_000,
      );
      const fail = (e: Error) => {
        clearTimeout(timer);
        reject(e);
      };
      child.once("error", fail);
      child.once("exit", (code) => fail(new Error(`hengst-mock exited with ${code}`)));
      lines.once("line", (line) => {
        clearTimeout(timer);
        try {
          resolveUrl((JSON.parse(line) as { url: string }).url);
        } catch {
          reject(new Error("hengst-mock's first line is not its address"));
        }
      });
    });
    return { url, process: child };
  } catch (e) {
    // Never leave a mock behind that nobody stops.
    if (child.exitCode === null && child.signalCode === null) child.kill();
    throw e;
  }
}

/** Closing stdin ends the mock; the PID it was started with is the fallback. */
export async function stopMock(mock: Mock) {
  const child = mock.process;
  if (child.exitCode !== null || child.signalCode !== null) return;
  const exited = new Promise<void>((done) => child.once("exit", () => done()));
  child.stdin?.end();
  const timeout = new Promise<"timeout">((done) => setTimeout(() => done("timeout"), 5_000));
  if ((await Promise.race([exited, timeout])) === "timeout") child.kill();
}

/**
 * The open modal dialog: the `dialog` with a name (the version picker's popup is a nameless
 * `dialog` too), or the `alertdialog`.
 */
export const dialog = () => $('[role="dialog"][aria-labelledby]');
export const alertDialog = () => $('[role="alertdialog"]');

/**
 * The accessible name of the open dialog (its `aria-labelledby` title); "" without one. Names in a
 * title are wrapped in bidi isolates (`lib/bidi.ts` `isolate`); those are dropped here, so specs
 * compare the plain text.
 */
export function dialogName(): Promise<string> {
  return browser.execute(() => {
    const el = document.querySelector('[role="dialog"][aria-labelledby], [role="alertdialog"]');
    const ids = el?.getAttribute("aria-labelledby") ?? "";
    return ids
      .split(/\s+/)
      .map((id) => (document.getElementById(id)?.textContent ?? "").trim())
      .join(" ")
      .replace(/[⁦-⁩]/g, "")
      .trim();
  });
}

/**
 * The name of the focused element as a screen reader would start to compute it: `aria-label`,
 * else the `aria-labelledby` texts, else its label, else its text.
 */
export function focusedName(): Promise<string> {
  return browser.execute(() => {
    const el = document.activeElement as HTMLElement | null;
    if (!el || el === document.body) return "";
    const text = (node: Element | null) => (node?.textContent ?? "").replace(/\s+/g, " ").trim();
    const label = el.getAttribute("aria-label");
    if (label) return label;
    const ids = el.getAttribute("aria-labelledby");
    if (ids) {
      return ids
        .split(/\s+/)
        .map((id) => text(document.getElementById(id)))
        .join(" ");
    }
    if (el.id) {
      const forLabel = document.querySelector(`label[for="${el.id}"]`);
      if (forLabel) return text(forLabel);
    }
    const wrapping = el.closest("label");
    return wrapping ? text(wrapping) : text(el);
  });
}

/**
 * The single displayed element with this role and accessible name (`singleByRole`), waited for;
 * with `within` only inside the first element that CSS selector matches. `interval` is the poll
 * interval in ms (WebdriverIO's default otherwise), short where a click must follow a change fast.
 */
export async function byRole(
  role: string,
  name: string,
  timeout?: number,
  within?: string,
  interval?: number,
): Promise<WebdriverIO.Element> {
  let found: unknown = null;
  await browser.waitUntil(
    async () => {
      found = await browser.execute(singleByRole, role, name, true, within);
      return found !== null;
    },
    {
      timeout,
      interval,
      timeoutMsg: `no single displayed ${role} named "${name}"${within ? ` in ${within}` : ""}`,
    },
  );
  // `execute` hands back a plain WebDriver element reference; `$` turns it into an element.
  return await $(found as Parameters<typeof $>[0]);
}

/**
 * The focused element matches `:focus-visible` and looks different from the same element without
 * focus (an unfocused, hidden copy next to it): its outline or its ring is the focus indicator,
 * not a shadow it always has.
 */
export function focusIndicatorShown(): Promise<boolean> {
  return browser.execute(() => {
    const el = document.activeElement as HTMLElement | null;
    if (!el || el === document.body || !el.matches(":focus-visible")) return false;
    const look = (s: CSSStyleDeclaration) =>
      `${s.outlineStyle} ${s.outlineWidth} ${s.outlineColor} | ${s.boxShadow}`;
    const focused = look(getComputedStyle(el));
    const copy = el.cloneNode(false) as HTMLElement;
    copy.removeAttribute("id");
    copy.setAttribute("aria-hidden", "true");
    copy.tabIndex = -1;
    copy.style.position = "absolute";
    copy.style.visibility = "hidden";
    copy.style.pointerEvents = "none";
    el.after(copy);
    const unfocused = look(getComputedStyle(copy));
    copy.remove();
    const style = getComputedStyle(el);
    const visibleOutline = style.outlineStyle !== "none" && parseFloat(style.outlineWidth) > 0;
    return focused !== unfocused && (visibleOutline || style.boxShadow !== "none");
  });
}

/** Browser log entries that report a Content Security Policy violation since the last read. */
export async function cspViolations(): Promise<string[]> {
  const logs = (await browser.getLogs("browser")) as { message: string }[];
  return logs.map((l) => l.message).filter((m) => /Content Security Policy/i.test(m));
}

/** Ends a process tree that this run started itself, by its PID. */
export function killTree(pid: number) {
  spawnSync("taskkill", ["/PID", String(pid), "/T", "/F"], { stdio: "ignore", windowsHide: true });
}

/** Ends one process this run started (not its children), by its PID. */
export function killOne(pid: number) {
  spawnSync("taskkill", ["/PID", String(pid), "/F"], { stdio: "ignore", windowsHide: true });
}

export interface ProcessRow {
  pid: number;
  parent: number;
  path: string;
}

/** One `pid<TAB>parent<TAB>path` line per process; anything else is skipped. */
export function parseProcessTable(stdout: string): ProcessRow[] {
  return stdout
    .split(/\r?\n/)
    .map((line) => line.split("\t"))
    .filter((cols) => cols.length === 3 && /^\d+$/.test(cols[0]) && /^\d+$/.test(cols[1]))
    .map(([pid, parent, path]) => ({ pid: Number(pid), parent: Number(parent), path }));
}

/**
 * The processes below `root` (children, their children, ...). Windows keeps a dead parent's PID
 * in its children, so a process whose parent died is no longer found here.
 */
export function descendantsOf(rows: ProcessRow[], root: number): ProcessRow[] {
  const found: ProcessRow[] = [];
  const seen = new Set([root]);
  for (let grew = true; grew;) {
    grew = false;
    for (const row of rows) {
      if (!seen.has(row.pid) && seen.has(row.parent)) {
        seen.add(row.pid);
        found.push(row);
        grew = true;
      }
    }
  }
  return found;
}

/** The long, final form of a path (the temp folder may be given in 8.3 short form). */
const finalPath = (p: string) => {
  try {
    return realpathSync.native(p).toLowerCase();
  } catch {
    return resolve(p).toLowerCase();
  }
};
const sameFile = (a: string, b: string) => finalPath(a) === finalPath(b);

async function processTable(): Promise<ProcessRow[]> {
  return parseProcessTable(
    await runPowerShell(
      "Get-CimInstance Win32_Process | ForEach-Object" +
        ' { "$($_.ProcessId)`t$($_.ParentProcessId)`t$($_.ExecutablePath)" }',
    ),
  );
}

/** The processes of `exe` that this worker's tauri-driver started (directly or not). */
export async function ourProcesses(exe: string): Promise<number[]> {
  const driver = Number(process.env.HENGST_E2E_DRIVER_PID);
  if (!driver) throw new Error("no tauri-driver PID in this worker");
  return descendantsOf(await processTable(), driver)
    .filter((row) => row.path && sameFile(row.path, exe))
    .map((row) => row.pid);
}

/** The app process behind the current session: exactly one, or an error. */
export async function appPid(): Promise<number> {
  const pids = await ourProcesses(APP_EXE);
  if (pids.length !== 1) throw new Error(`expected one app process, found ${pids.length}`);
  return pids[0];
}

/** The process exists (signal 0 only checks). */
export function alive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

/**
 * The process has a visible top-level window: `MainWindowHandle` counts only visible windows
 * without an owner, so the main window hidden into the tray reads as 0.
 */
export async function hasVisibleWindow(pid: number): Promise<boolean> {
  if (!Number.isInteger(pid) || pid <= 0) throw new Error(`bad pid ${pid}`);
  const out = await runPowerShell(`(Get-Process -Id ${pid} -ErrorAction Stop).MainWindowHandle`);
  return Number(out.trim()) !== 0;
}

/**
 * Replaces `navigator.clipboard.writeText` in the page by a recorder, so the app's own copy
 * command can be read back (`copiedTexts`) without touching the machine's clipboard. Only an own
 * property of this page's clipboard object is set (the frozen `Object.prototype` is untouched).
 * `false` when the page has no clipboard API (the app's copy would fail anyway).
 */
export function interceptClipboard(): Promise<boolean> {
  return browser.execute(() => {
    if (!navigator.clipboard) return false;
    const w = window as unknown as { e2eCopied: string[] };
    w.e2eCopied = [];
    Object.defineProperty(navigator.clipboard, "writeText", {
      configurable: true,
      value: (text: string) => {
        w.e2eCopied.push(text);
        return Promise.resolve();
      },
    });
    return true;
  });
}

/** What the app copied since `interceptClipboard`. */
export function copiedTexts(): Promise<string[]> {
  return browser.execute(() => (window as unknown as { e2eCopied?: string[] }).e2eCopied ?? []);
}

/** The whole document as text: rendered text, attribute values and the title. */
export function documentText(): Promise<string> {
  return browser.execute(() => {
    const parts = [document.title, document.documentElement.innerText];
    for (const el of document.querySelectorAll("*")) {
      for (const a of el.attributes) parts.push(a.value);
    }
    // Text in elements that do not render (hidden, display: none) is not in innerText.
    parts.push(document.documentElement.textContent ?? "");
    return parts.join("\n");
  });
}

/**
 * Starts the app again on the same data folder: a new WebDriver session (tauri-driver starts a new
 * app with the same environment). The data-folder guard runs again (`onReload` in wdio.conf.ts).
 */
export async function restartApp() {
  await browser.reloadSession();
}
