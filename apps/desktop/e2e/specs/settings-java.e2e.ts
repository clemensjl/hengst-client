import { byRole, cspViolations } from "../support/app.ts";
import { mainButton, openView } from "../support/game.ts";

// The settings file was saved with a Java on a network share (\\192.0.2.1\share\java.exe, a
// documentation address nobody answers on). The app refuses it from its text alone, before any
// file-system call, so nothing here may wait for a network time-out. The development account and
// "E2E Game" make Play reachable. "Choose Java…" is never pressed: it opens a native dialog.

/** Far below any SMB or network time-out (those take 20 s and more). */
const FAST_MS = 3000;

describe("a stored Java path on a network share", () => {
  it("shows the Java setting as unusable", async () => {
    await openView("Settings");
    // The Java setting sits in the collapsed "Advanced" section.
    await (await byRole("button", "Advanced")).click();
    await expect($("main")).toHaveText(
      expect.stringContaining("Choose a Java file on a local drive."),
      { wait: FAST_MS },
    );
    await expect(await byRole("button", "Reset")).toBeDisplayed();
  });

  it("refuses Play at once and offers the settings", async () => {
    await openView("Play");
    const play = await mainButton("Play");
    const started = Date.now();
    await play.click();
    await expect($("main")).toHaveText(expect.stringContaining("Java could not be used"), {
      wait: FAST_MS,
    });
    const took = Date.now() - started;
    console.log(`Play refused after ${took} ms`);
    expect(took).toBeLessThan(FAST_MS);
    // Nothing was prepared.
    await expect($('[data-slot="launch-progress"]')).not.toBeExisting();
    await (await mainButton("Open settings")).click();
    await expect($("header h1")).toHaveText("Settings");
  });

  it("logged no Content Security Policy violation", async () => {
    expect(await cspViolations()).toEqual([]);
  });
});
