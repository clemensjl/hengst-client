import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { byRole } from "./app.ts";

/** Generous: a preparation against the mock with delayed answers takes a few seconds. */
export const START = { timeout: 60_000 };

export type ViewName = "Play" | "Instances" | "Log" | "Settings";

/** Opens a view from the sidebar (its buttons are named by `aria-label`). */
export async function openView(name: ViewName | string) {
  await $(`nav button[aria-label="${name}"]`).click();
  await expect($("header h1")).toHaveText(name);
}

/** A button of the view (not the sidebar's or the header's button of the same name). */
export const mainButton = (name: string, timeout?: number, interval?: number) =>
  byRole("button", name, timeout, "main", interval);

/** The line above the Play button: "Running since …" while a game runs. */
export const caption = () => $('[data-slot="play-caption"]');

export async function waitRunning(timeout = START.timeout) {
  await expect(caption()).toHaveText(expect.stringContaining("Running since"), { wait: timeout });
}

/** The Play button reads "Play" again: no start, no game. */
export async function waitReady(timeout = START.timeout) {
  await mainButton("Play", timeout);
  await expect(caption()).not.toHaveText(expect.stringContaining("Running since"));
}

/**
 * Records every phase line the progress panel shows from now on (it may change faster than a
 * WebDriver poll). Read with `progressSeen`.
 */
export function watchProgress(): Promise<void> {
  return browser.execute(() => {
    const w = window as unknown as { e2eProgress: string[]; e2ePanel: boolean };
    w.e2eProgress = [];
    w.e2ePanel = false;
    const look = () => {
      const panel = document.querySelector('[data-slot="launch-progress"]');
      if (!panel) return;
      w.e2ePanel = true;
      const phase = panel.querySelector("p")?.textContent?.trim();
      if (phase && w.e2eProgress[w.e2eProgress.length - 1] !== phase) w.e2eProgress.push(phase);
    };
    new MutationObserver(look).observe(document.body, {
      subtree: true,
      childList: true,
      characterData: true,
    });
  });
}

export function progressSeen(): Promise<{ panel: boolean; phases: string[] }> {
  return browser.execute(() => {
    const w = window as unknown as { e2eProgress?: string[]; e2ePanel?: boolean };
    return { panel: w.e2ePanel ?? false, phases: w.e2eProgress ?? [] };
  });
}

/** The log area of the log view. */
export const logArea = () => $('[role="region"][aria-label="Log lines"]');

/** The rendered text of the log area (only the rows the list has drawn, plus its end note). */
export function logText(): Promise<string> {
  return browser.execute(
    () => document.querySelector('[role="region"][aria-label="Log lines"]')?.textContent ?? "",
  );
}

export async function waitForLogText(text: string, timeout = 30_000) {
  await browser.waitUntil(async () => (await logText()).includes(text), {
    timeout,
    timeoutMsg: `the log area never showed "${text}"`,
  });
}

/** Every 8-character piece of `secret`: none may appear (the core's own leak check). */
export function leakedPieces(text: string, secret: string): string[] {
  const pieces: string[] = [];
  for (let i = 0; i + 8 <= secret.length; i++) {
    const piece = secret.slice(i, i + 8);
    if (text.includes(piece)) pieces.push(piece);
  }
  return pieces;
}

/** The game logs the app wrote into the data folder (`logs/game`), newest last. */
export function gameLogs(): string[] {
  const dir = join(process.env.HENGST_E2E_HOME!, "logs", "game");
  let names: string[];
  try {
    names = readdirSync(dir).filter((n) => n.endsWith(".log"));
  } catch {
    return [];
  }
  return names
    .map((n) => join(dir, n))
    .sort((a, b) => statSync(a).mtimeMs - statSync(b).mtimeMs)
    .map((file) => readFileSync(file, "utf8"));
}
