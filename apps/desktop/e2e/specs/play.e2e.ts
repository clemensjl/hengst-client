import {
  alertDialog,
  byRole,
  copiedTexts,
  cspViolations,
  dialog,
  documentText,
  interceptClipboard,
  setScript,
} from "../support/app.ts";
import {
  caption,
  gameLogs,
  leakedPieces,
  logArea,
  logText,
  mainButton,
  openView,
  progressSeen,
  START,
  waitForLogText,
  waitReady,
  waitRunning,
  watchProgress,
} from "../support/game.ts";

// One app for the whole file, in this order: the first test needs the downloads still ahead
// (a fresh data folder), so its preparation lasts long enough to cancel. Every Mojang answer of
// the mock waits 400 ms. The instance is "E2E Game" (9.9, no loader) with fake-java as its Java;
// the account is E2EPlayer, signed in through the mock, so the game's arguments carry its
// Minecraft token. fake-java's `variants` prints that token in every shape a log might hold it.

/** The Minecraft token and the refresh token the mock hands to E2EPlayer (`PLAYER_A`). */
const SECRETS = ["test-mc-token-a", "test-refresh-a"];

const leaks = (text: string) => SECRETS.flatMap((s) => leakedPieces(text, s));

describe("playing", () => {
  before(async () => {
    // Sign in E2EPlayer: the header's account menu ("Choose account": only the development
    // account is listed and it is not chosen).
    await (await byRole("button", "Choose account", undefined, "header")).click();
    await $('[role="menu"]').waitForDisplayed();
    await $('[role="menuitem"]*=Add account').click();
    await expect(dialog()).toBeDisplayed();
    await dialog().$("aria/Sign in with Microsoft").click();
    await $("aria/Account: E2EPlayer").waitForDisplayed({ timeout: 30_000 });
    await expect(dialog()).not.toBeExisting();
    await mainButton("Play");
  });

  afterEach(async () => {
    for (let i = 0; i < 3 && (await alertDialog().isExisting()); i++) await browser.keys("Escape");
  });

  it("cancels during the preparation, and a double click never undoes its first click", async () => {
    setScript(["out must not run", "exit 0"]);
    await (await mainButton("Play")).click();
    // Preparing: the button turns into Cancel. One click cancels; the second click of a double
    // click lands on "Play" within 600 ms of its change and is ignored, so no new start begins.
    // WebDriver's `doubleClick` sends both clicks before the cancel is answered, so it would never
    // reach the guard: the second click is sent as soon as the label reads "Play" again.
    const cancel = await mainButton("Cancel", 10_000);
    await cancel.click();
    // Polled every 50 ms, so the click comes well inside the 600 ms.
    const again = await mainButton("Play", 15_000, 50);
    await again.click();
    await waitReady(15_000);
    await browser.pause(1500);
    await mainButton("Play");
    await expect($('[data-slot="launch-progress"]')).not.toBeExisting();
    // Nothing started: no run in the log view, no game log file.
    await openView("Log");
    await expect($("main")).toHaveText(expect.stringContaining("No log yet"));
    expect(gameLogs()).toEqual([]);
    await openView("Play");

    // Well past 600 ms the button takes clicks again: this start goes through.
    // A double click on Play starts once and does not cancel: the second click hits "Cancel"
    // within 600 ms and is ignored. (If WebDriver's two clicks both land before the button has
    // changed, the second one is a second start, which the app answers "busy": that would show
    // an error panel, checked below.)
    setScript(["out started after a double click", "exit 0"]);
    await watchProgress();
    await (await mainButton("Play")).doubleClick();
    await browser.pause(1000);
    await expect($("main")).not.toHaveText(
      expect.stringContaining("Another action is in progress"),
    );
    // The preparation went on to a game that printed its line and ended with 0 (no exit panel).
    await browser.waitUntil(
      () => gameLogs().some((log) => log.includes("started after a double click")),
      { ...START, timeoutMsg: "the game of the double-clicked Play never ran" },
    );
    await waitReady();
    await expect($("main")).not.toHaveText(expect.stringContaining("closed unexpectedly"));
    // The progress panel was shown, with at least one named phase.
    const seen = await progressSeen();
    console.log(`progress phases seen: ${seen.phases.join(" | ")}`);
    expect(seen.panel).toBe(true);
    expect(seen.phases.length).toBeGreaterThan(0);
  });

  it("runs, shows its log without the token anywhere, copies it redacted and stops", async () => {
    setScript(["out fake game started", "args", "variants", "window", "out fake game closed"]);
    await (await mainButton("Play")).click();
    await waitRunning();
    await expect(caption()).toHaveText(expect.stringMatching(/^Running since \S/));
    await mainButton("Stop");

    await (await mainButton("Show log")).click();
    await expect($("header h1")).toHaveText("Log");
    await waitForLogText("fake game started");
    await waitForLogText("ARG --accessToken");
    // The token was printed by fake-java and replaced by the core: nowhere in the page, not in
    // the drawn rows, attributes, hidden text or the title.
    await waitForLogText("<redacted>");
    expect(leaks(await documentText())).toEqual([]);

    // The search runs over every line the view holds, drawn or not.
    const search = await byRole("textbox", "Search log");
    await search.setValue("accessToken");
    await expect($("#log-search-hits")).toHaveText(expect.stringMatching(/^[1-9][\d,]* match/));
    await search.setValue("test-mc");
    await expect($("#log-search-hits")).toHaveText("0 matches");
    await search.setValue("token-a");
    await expect($("#log-search-hits")).toHaveText("0 matches");
    await browser.keys("Escape");

    // The app's own copy, read back from a recorder in place of the clipboard.
    expect(await interceptClipboard()).toBe(true);
    await (await byRole("button", "Copy")).click();
    await browser.waitUntil(async () => (await copiedTexts()).length === 1, {
      timeoutMsg: "the app copied nothing",
    });
    const [copied] = await copiedTexts();
    expect(copied).toContain("fake game started");
    expect(copied).toContain("ARG --accessToken");
    expect(copied).toContain("<redacted>");
    expect(leaks(copied)).toEqual([]);

    // Stop: the game's window is asked to close, the run ends as stopped on request.
    await openView("Play");
    await (await mainButton("Stop")).click();
    await waitReady(30_000);
    await openView("Log");
    await waitForLogText("fake game closed");
    await waitForLogText("The game was stopped on request.");
    expect(leaks(await documentText())).toEqual([]);
    // The log file on disk is redacted too.
    for (const log of gameLogs()) expect(leaks(log)).toEqual([]);
    await openView("Play");
  });

  it("shows the unexpected-exit panel for a non-zero exit code, with its log", async () => {
    setScript(["out about to fail", "exit 3"]);
    await (await mainButton("Play")).click();
    await expect($("main")).toHaveText(expect.stringContaining("The game closed unexpectedly"), {
      wait: START.timeout,
    });
    await (await mainButton("Show log")).click();
    await expect($("header h1")).toHaveText("Log");
    await waitForLogText("about to fail");
    await waitForLogText("The game exited with code 3.");
    await expect(logArea()).toBeDisplayed();
    expect(await logText()).not.toContain("fake game started");

    await openView("Play");
    await (await mainButton("Dismiss")).click();
    await expect($("main")).not.toHaveText(expect.stringContaining("closed unexpectedly"));
    await mainButton("Play");
  });

  it("logged no Content Security Policy violation", async () => {
    expect(await cspViolations()).toEqual([]);
  });
});
