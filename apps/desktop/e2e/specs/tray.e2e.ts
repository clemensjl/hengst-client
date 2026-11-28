import {
  alertDialog,
  alive,
  appPid,
  appRequest,
  cspViolations,
  dialogName,
  focusedName,
  hasVisibleWindow,
  setScript,
} from "../support/app.ts";
import { caption, gameLogs, mainButton, openView, waitReady } from "../support/game.ts";

// One app for the whole file, in this order; the last test quits it. "Minimize to the
// notification area when the game starts" is on (the app's default). The tray menu is a native
// menu and the close button is not part of the page, so their actions are asked for through the
// e2e build's request file (`appRequest`, e2e_hook.rs), which calls the same handlers the menu
// and the close request call. NOT covered here: the tray icon itself, its tooltip, the menu as
// drawn, where Windows puts the icon, focus after showing, and "Quit" without a running game
// (it ends the app at once; unit tests in tray.rs cover the decision).

/** fake-java with a hidden window that ends the game when asked to close. */
const GAME = ["out tray game started", "window", "out tray game closed"];

let pid = 0;

const windowShown = () => hasVisibleWindow(pid);

async function waitWindow(shown: boolean, timeoutMsg: string) {
  await browser.waitUntil(async () => (await windowShown()) === shown, {
    timeout: 15_000,
    timeoutMsg,
  });
}

/** The page answers although its window may be hidden: the caption says a game runs. */
const pageSaysRunning = () =>
  browser.waitUntil(
    async () =>
      (
        await browser.execute(
          () => document.querySelector('[data-slot="play-caption"]')?.textContent ?? "",
        )
      ).includes("Running since"),
    { timeout: 60_000, timeoutMsg: "the game never ran" },
  );

describe("tray and quit", () => {
  before(async () => {
    pid = await appPid();
    expect(await windowShown()).toBe(true);
  });

  it("hides the window when the game starts and shows it from the tray", async () => {
    setScript(GAME);
    await (await mainButton("Play")).click();
    await pageSaysRunning();
    await waitWindow(false, "the window stayed visible after the game started");
    expect(alive(pid)).toBe(true);
    appRequest("tray-open");
    await waitWindow(true, "tray Open did not show the window");
    await expect(caption()).toHaveText(expect.stringContaining("Running since"));
  });

  it("hides into the tray on close while the game runs, and the app keeps running", async () => {
    appRequest("close");
    await waitWindow(false, "closing did not hide the window");
    await browser.pause(1000);
    expect(alive(pid)).toBe(true);
    await pageSaysRunning();
    appRequest("tray-open");
    await waitWindow(true, "tray Open did not show the window");
  });

  it("stops the game from the tray; the window comes back", async () => {
    appRequest("tray-stop");
    await waitReady(30_000);
    expect(await windowShown()).toBe(true);
    // Asked to close, not killed: fake-java printed the line after its window closed.
    const last = gameLogs().at(-1) ?? "";
    expect(last).toContain("CLOSE REQUESTED");
    expect(last).toContain("tray game closed");
  });

  it("logged no Content Security Policy violation", async () => {
    expect(await cspViolations()).toEqual([]);
  });

  it("asks before quitting while a game runs; Keep playing keeps both, a double click quits once", async () => {
    await openView("Play");
    setScript(GAME);
    await (await mainButton("Play")).click();
    await pageSaysRunning();
    await waitWindow(false, "the window stayed visible after the game started");

    // Quit from the tray shows the window and the question, the safe button focused.
    appRequest("tray-quit");
    await waitWindow(true, "tray Quit did not show the window");
    await expect(alertDialog()).toBeDisplayed();
    expect(await dialogName()).toBe("Stop the game and quit Hengst Client?");
    expect(await focusedName()).toBe("Keep playing");
    await alertDialog().$("button=Keep playing").click();
    await expect(alertDialog()).not.toBeExisting();
    await browser.pause(1000);
    expect(alive(pid)).toBe(true);
    await expect(caption()).toHaveText(expect.stringContaining("Running since"));

    // Again, and this time "Stop game and quit" by double click: the second click must not turn
    // into a force stop (the button ignores clicks for 600 ms after it changes).
    appRequest("tray-quit");
    await expect(alertDialog()).toBeDisplayed();
    const runsBefore = gameLogs().length;
    await alertDialog().$("button=Stop game and quit").doubleClick();
    // The app quits once the game has exited; the session ends with it.
    await browser.waitUntil(async () => !alive(pid), {
      timeout: 30_000,
      timeoutMsg: "the app did not quit after the game stopped",
    });
    const logs = gameLogs();
    expect(logs.length).toBe(runsBefore);
    const last = logs.at(-1) ?? "";
    expect(last).toContain("CLOSE REQUESTED");
    expect(last).toContain("tray game closed");
  });
});
