import { alertDialog, cspViolations, focusedName } from "../support/app.ts";

// The data folder starts with two instances the app cannot read as its own: "from-the-future"
// (format 99, saved by a newer build) and "damaged" (no valid instance.json). Both are listed by
// their folder name.

/** Opens the actions menu of a row by keyboard and moves down to "Delete". */
async function highlightDelete(row: string) {
  const actions = $(`aria/Actions for ${row}`);
  await actions.waitForDisplayed();
  // A focus tooltip of the row above may cover the button.
  await browser.keys("Escape");
  await actions.click();
  await $('[role="menu"]').waitForDisplayed();
  await browser.keys("Escape");
  await expect($('[role="menu"]')).not.toBeDisplayed();
  expect(await focusedName()).toBe(`Actions for ${row}`);
  await browser.keys("Enter");
  await $('[role="menu"]').waitForDisplayed();
  // First item "Open folder", then "Delete"; a disabled item stays reachable by arrow key.
  await browser.waitUntil(async () => (await focusedName()) === "Open folder");
  await browser.keys("ArrowDown");
  await browser.waitUntil(async () => (await focusedName()) === "Delete");
}

describe("instances this build cannot read", () => {
  it("offers Delete on a too-new row only as a disabled item with its reason", async () => {
    await expect($("aria/Actions for from-the-future")).toBeDisplayed();
    await expect($("button*=from-the-future")).toHaveText(expect.stringContaining("Newer format"));

    await highlightDelete("from-the-future");
    const reason = await browser.execute(() => {
      const el = document.activeElement!;
      const ids = el.getAttribute("aria-describedby") ?? "";
      return {
        disabled: el.getAttribute("aria-disabled"),
        description: ids
          .split(/\s+/)
          .map((id) => document.getElementById(id)?.textContent ?? "")
          .join(" ")
          .trim(),
      };
    });
    expect(reason).toEqual({
      disabled: "true",
      description: "Saved by a newer version of Hengst Client. You can delete it only in Explorer.",
    });
    // Enter on the disabled item asks nothing.
    await browser.keys("Enter");
    await browser.pause(300);
    await expect(alertDialog()).not.toBeExisting();
    await browser.keys("Escape");
    await expect($('[role="menu"]')).not.toBeDisplayed();
  });

  it("keeps Delete usable on an unreadable row", async () => {
    await expect($("button*=damaged")).toHaveText(expect.stringContaining("Unreadable"));
    await highlightDelete("damaged");
    expect(await browser.execute(() => document.activeElement!.getAttribute("aria-disabled"))).toBe(
      null,
    );
    // Opens the confirmation; "Keep" leaves the folder where it is.
    await browser.keys("Enter");
    await expect(alertDialog()).toBeDisplayed();
    await alertDialog().$("button=Keep").click();
    await expect(alertDialog()).not.toBeExisting();
  });

  it("logged no Content Security Policy violation", async () => {
    expect(await cspViolations()).toEqual([]);
  });
});
