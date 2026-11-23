import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { createInterface } from "node:readline";

/** The workspace's debug binaries: `pnpm e2e` does not build them. */
const TARGET = resolve(import.meta.dirname, "../../../../target/debug");
export const APP_EXE = join(TARGET, "hengst-desktop.exe");
export const MOCK_EXE = join(TARGET, "hengst-mock.exe");

/** What a spec file needs before its app starts. */
export interface SpecSetup {
  /** `HENGST_MOCK_SCENARIO` of its `hengst-mock` (names in `AuthScenario::from_name`). */
  scenario: string;
  /** Writes extra files into the fresh data folder before the app starts. */
  seed?: (home: string) => void;
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
export async function startMock(scenario: string): Promise<Mock> {
  const child = spawn(MOCK_EXE, [], {
    env: { ...process.env, HENGST_MOCK_SCENARIO: scenario },
    stdio: ["pipe", "pipe", "inherit"],
    windowsHide: true,
  });
  const lines = createInterface({ input: child.stdout! });
  const url = await new Promise<string>((resolveUrl, reject) => {
    const timer = setTimeout(() => reject(new Error("hengst-mock did not start in 10 s")), 10_000);
    child.once("error", reject);
    child.once("exit", (code) => reject(new Error(`hengst-mock exited with ${code}`)));
    lines.once("line", (line) => {
      clearTimeout(timer);
      resolveUrl((JSON.parse(line) as { url: string }).url);
    });
  });
  return { url, process: child };
}

/** Closing stdin ends the mock; the PID it was started with is the fallback. */
export async function stopMock(mock: Mock) {
  const child = mock.process;
  if (child.exitCode !== null) return;
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

/** The accessible name of the open dialog (its `aria-labelledby` title); "" without one. */
export function dialogName(): Promise<string> {
  return browser.execute(() => {
    const el = document.querySelector('[role="dialog"][aria-labelledby], [role="alertdialog"]');
    const ids = el?.getAttribute("aria-labelledby") ?? "";
    return ids
      .split(/\s+/)
      .map((id) => (document.getElementById(id)?.textContent ?? "").trim())
      .join(" ")
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

/** The focused element shows a focus indicator: an outline or a ring (box-shadow). */
export function focusIndicatorShown(): Promise<boolean> {
  return browser.execute(() => {
    const el = document.activeElement;
    if (!el || el === document.body) return false;
    const style = getComputedStyle(el);
    const outline = style.outlineStyle !== "none" && parseFloat(style.outlineWidth) > 0;
    return outline || style.boxShadow !== "none";
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
