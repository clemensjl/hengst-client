import { existsSync } from "node:fs";
import { join } from "node:path";
import {
  alertDialog,
  alive,
  appPid,
  dialogName,
  GAME_NAME,
  gameDir,
  killOne,
  ourProcesses,
  restartApp,
  setScript,
  touchGameFile,
} from "../support/app.ts";
import {
  GUARD_WAIT_MS,
  mainButton,
  openView,
  waitReady,
  waitRunning,
} from "../support/game.ts";

// One app session after another on the same data folder, in this order. The game runs in a job
// of its own that outlives the launcher, and holds the instance's run lock. The test ends only
// the app's own process (by its PID, not its tree), so the game survives; the next app must then
// refuse Play and Delete for that instance until the game has exited.

const java = () => join(process.env.HENGST_E2E_HOME!, "jdk", "bin", "java.exe");

let game = 0;

async function deleteInstance() {
  await openView("Instances");
  await $(`aria/Actions for ${GAME_NAME}`).click();
  await $('[role="menuitem"][aria-label="Delete"]').click();
  await expect(alertDialog()).toBeDisplayed();
  expect(await dialogName()).toBe(`Delete ${GAME_NAME}?`);
  await alertDialog().$("button=Move to Recycle Bin").click();
}

describe("a game that outlives the app", () => {
  after(() => {
    // Whatever failed: the surviving game ends (fake-java also ends itself after 120 s).
    if (existsSync(gameDir())) touchGameFile("release");
  });

  it("keeps running when the app is killed", async () => {
    setScript(["out held", "wait release", "out released"]);
    await (await mainButton("Play")).click();
    await waitRunning();
    const games = await ourProcesses(java());
    expect(games).toHaveLength(1);
    game = games[0];
    const app = await appPid();
    killOne(app);
    await browser.waitUntil(async () => !alive(app), { timeoutMsg: "the app did not end" });
    await browser.pause(1000);
    expect(alive(game)).toBe(true);
  });

  it("refuses Play and Delete in the next app while that game runs", async () => {
    await restartApp();
    // The new app does not know the game as its own; the run lock answers.
    await (await mainButton("Play")).click();
    await expect($("main")).toHaveText(expect.stringContaining("Currently running"), {
      wait: 15_000,
    });
    await expect($("main")).toHaveText(expect.stringContaining("Stop the game first."));
    await mainButton("Play");

    await deleteInstance();
    await expect(alertDialog()).toHaveText(expect.stringContaining("Currently running"), {
      wait: 15_000,
    });
    expect(existsSync(gameDir())).toBe(true);
    await browser.keys("Escape");
    await expect(alertDialog()).not.toBeExisting();
    expect(alive(game)).toBe(true);
  });

  it("allows both once the game has exited", async () => {
    touchGameFile("release");
    await browser.waitUntil(async () => !alive(game), {
      timeout: 15_000,
      timeoutMsg: "the released game did not exit",
    });
    setScript(["out second start", "window", "out closed"]);
    await openView("Play");
    await (await mainButton("Play")).click();
    await waitRunning();
    // The change into "Stop" arms the 600 ms click guard (DESIGN.md 10.0).
    await browser.pause(GUARD_WAIT_MS);
    await (await mainButton("Stop")).click();
    await waitReady(30_000);

    await deleteInstance();
    await browser.waitUntil(async () => !existsSync(gameDir()), {
      timeout: 30_000,
      timeoutMsg: "the instance folder is still there",
    });
    // The folder this run leaves in the user's Recycle Bin.
    console.log(`moved to the Recycle Bin: instance folder "${gameDir()}"`);
  });
});
