import {
  byRole,
  cspViolations,
  dialog,
  dialogName,
  setScript,
  touchGameFile,
} from "../support/app.ts";
import {
  logArea,
  logText,
  mainButton,
  openView,
  waitForLogText,
  waitReady,
  waitRunning,
} from "../support/game.ts";

// One app for the whole file, in this order. The development account plays "E2E Game" with
// fake-java; each run waits for a file in the game folder before it ends, so the tests decide
// when. "Open log file" is never pressed: it would open a viewer window.

const BURST = 5000;

/** The burst numbers of the drawn rows, top to bottom. */
function drawnBurstNumbers(): Promise<number[]> {
  return browser.execute(() => {
    const area = document.querySelector('[role="region"][aria-label="Log lines"]');
    return [...(area?.querySelectorAll("[data-seq]") ?? [])]
      .map((row) => /burst-line (\d+) /.exec(row.textContent ?? "")?.[1])
      .filter((n): n is string => n !== undefined)
      .map(Number);
  });
}

/** The text without bidi isolates (U+2066 to U+2069). */
const plainText = (text: string) => text.replace(/[⁦-⁩]/g, "");

/** The one displayed option of the open select whose text, without isolates, starts with `text`. */
async function option(text: string): Promise<WebdriverIO.Element> {
  let found: unknown = null;
  await browser.waitUntil(
    async () => {
      found = await browser.execute((wanted: string) => {
        const hits = [...document.querySelectorAll('[role="option"]')].filter(
          (el) =>
            el.getClientRects().length > 0 &&
            (el.textContent ?? "").replace(/[⁦-⁩]/g, "").trim().startsWith(wanted),
        );
        return hits.length === 1 ? hits[0] : null;
      }, text);
      return found !== null;
    },
    { timeoutMsg: `no single option "${text}"` },
  );
  return await $(found as Parameters<typeof $>[0]);
}

/** The log area's height and scroll position are the same twice, 500 ms apart. */
async function listSettled() {
  const position = () =>
    browser.execute(() => {
      const a = document.querySelector('[role="region"][aria-label="Log lines"]');
      return a ? `${a.scrollHeight}:${a.scrollTop}` : "";
    });
  let last = await position();
  await browser.waitUntil(
    async () => {
      await browser.pause(500);
      const now = await position();
      const still = now === last;
      last = now;
      return still;
    },
    { timeout: 15_000, timeoutMsg: "the log list never held still" },
  );
}

describe("the live log in WebView2", () => {
  after(() => {
    // Whatever failed: no fake game keeps waiting (fake-java also ends itself after 120 s).
    touchGameFile("release-1");
    touchGameFile("release-2");
  });

  it("keeps up with a burst: every line, in order, without a gap note", async () => {
    setScript([
      // Time to open the log view first: the burst must arrive live, not as the backlog of an
      // attach after it.
      "out waiting for the view",
      "sleep 4000",
      "xml INFO|Render thread|info-line before the burst",
      "xml WARN|Render thread|warn-line before the burst",
      "xml ERROR|main|error-line before the burst",
      `burst ${BURST} 200`,
      "sleep 300",
      "out after the burst",
      "wait release-1",
      "out first run released",
    ]);
    await openView("Play");
    await (await mainButton("Play")).click();
    await waitRunning();
    await (await mainButton("Show log")).click();
    await expect($("header h1")).toHaveText("Log");
    await waitForLogText("waiting for the view");
    expect(await logText()).not.toContain("burst-line");
    // Following: the view stays at the end while the lines arrive.
    await waitForLogText("after the burst", 60_000);
    const drawn = await drawnBurstNumbers();
    expect(drawn.length).toBeGreaterThan(0);
    for (let i = 1; i < drawn.length; i++) expect(drawn[i]).toBe(drawn[i - 1] + 1);
    expect(drawn[drawn.length - 1]).toBe(BURST);
    // No line went missing on the way to this view: the search counts the view's own lines.
    const search = await byRole("textbox", "Search log");
    await search.setValue("burst-line");
    await expect($("#log-search-hits")).toHaveText(`${BURST.toLocaleString("en")} matches`, {
      wait: 10_000,
    });
    await browser.keys("Escape");
    expect(await logText()).not.toMatch(/lines? skipped/);
  });

  it("stops following when moved up and jumps back to the end", async () => {
    await logArea().click();
    await browser.keys("Home");
    const jump = await byRole("button", "Jump to end");
    await browser.waitUntil(async () => !(await logText()).includes("after the burst"), {
      timeoutMsg: "Home did not move the view away from the end",
    });
    await jump.click();
    await waitForLogText("after the burst");
    await expect($("button*=Jump to end")).not.toBeExisting();
  });

  it("filters by level", async () => {
    const levels = await byRole("button", "Levels: 5 of 5");
    await levels.click();
    await $('[role="menuitemcheckbox"]*=Info').click();
    await browser.keys("Escape");
    await byRole("button", "Levels: 4 of 5");
    // The closing menu lies over the log area: a click before it is gone lands on the menu.
    await $('[role="menu"]').waitForExist({ reverse: true });
    // A new filter starts the list over at the end (it was following); while the list still
    // measures its rows it keeps moving itself there and overtakes a Home, so wait until the
    // scroll position holds still.
    await waitForLogText("after the burst");
    await listSettled();
    await logArea().click();
    await browser.keys("Home");
    await waitForLogText("warn-line before the burst");
    await waitForLogText("error-line before the burst");
    expect(await logText()).not.toContain("info-line before the burst");
    // Back to all levels.
    await (await byRole("button", "Levels: 4 of 5")).click();
    await $('[role="menuitemcheckbox"]*=Info').click();
    await browser.keys("Escape");
    await byRole("button", "Levels: 5 of 5");
    await $('[role="menu"]').waitForExist({ reverse: true });
    await waitForLogText("info-line before the burst");
    // Back at the end, following again, for the next test's new lines.
    await (await byRole("button", "Jump to end")).click();
    await waitForLogText("after the burst");
    await expect($("button*=Jump to end")).not.toBeExisting();
  });

  it("switches between the current and the previous run", async () => {
    touchGameFile("release-1");
    await waitForLogText("first run released");
    await waitForLogText("The game closed.");
    setScript(["out the second run", "wait release-2"]);
    await openView("Play");
    await waitReady();
    await (await mainButton("Play")).click();
    await waitRunning();
    await openView("Log");
    // The view follows the newest run.
    await waitForLogText("the second run");
    // The run switcher is named by its visible value (aria-labelledby). The instance name in it
    // is wrapped in bidi isolates (`isolate`), which `plainText` drops.
    const runs = $('[role="combobox"][aria-labelledby="log-run-value"]');
    await browser.waitUntil(
      async () => plainText(await runs.getText()).includes("Current run: E2E Game"),
      { timeoutMsg: "the run switcher does not show the current run" },
    );
    await runs.click();
    await (await option("Previous run: E2E Game")).click();
    await waitForLogText("first run released");
    expect(await logText()).not.toContain("the second run");
    await runs.click();
    await (await option("Current run: E2E Game")).click();
    await waitForLogText("the second run");
    touchGameFile("release-2");
    await openView("Play");
    await waitReady();
  });

  it("runs with a frozen prototype and no CSP violation with log, settings and licences open", async () => {
    expect(await browser.execute(() => Object.isFrozen(Object.prototype))).toBe(true);
    await openView("Log");
    await expect(logArea()).toBeDisplayed();
    await openView("Settings");
    await (await byRole("button", "Third-party licenses")).click();
    await expect(dialog()).toBeDisplayed();
    expect(await dialogName()).toBe("Third-party licenses");
    await browser.keys("Escape");
    await expect(dialog()).not.toBeExisting();
    expect(await cspViolations()).toEqual([]);
  });
});
