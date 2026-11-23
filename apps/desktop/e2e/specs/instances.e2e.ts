import { readdirSync } from "node:fs";
import { join } from "node:path";
import {
  alertDialog,
  cspViolations,
  dialog,
  dialogName,
  focusedName,
  focusIndicatorShown,
} from "../support/app.ts";

// One app for the whole file, in this order (Cancel keeps the draft of the create dialog, so the
// test that leaves a draft comes last). The mock lists the game versions 9.9 (release), 9.9-snap,
// b9.9 and a9.9; Fabric has 0.99.0 (stable) and 0.100.0-beta.1 for 9.9 and no build for b9.9.

const instanceFolders = () => {
  try {
    return readdirSync(join(process.env.HENGST_E2E_HOME!, "instances"));
  } catch {
    return [];
  }
};

/** The game version picker: the first combobox (the loader version select is the second). */
const versionTrigger = () => dialog().$$('[role="combobox"]')[0];

async function openCreate() {
  await $("button=Create instance").click();
  await expect(dialog()).toBeDisplayed();
  expect(await dialogName()).toBe("Create instance");
  await expect(versionTrigger()).not.toHaveText("Loading versions…");
}

async function chooseVersion(id: string) {
  await versionTrigger().click();
  const option = $(`[role="option"]*=${id}`);
  await option.waitForDisplayed();
  await option.click();
  await expect(versionTrigger()).toHaveText(id);
}

async function chooseLoader(name: string) {
  await dialog().$(`[role="radio"]=${name}`).click();
}

/** Top and height of the elements whose own text is exactly one of `texts`, inside `scope`. */
async function rectsByText(scope: ReturnType<typeof $>, texts: string[]) {
  const root = await scope.getElement();
  return browser.execute(
    (root, wanted) => {
      const out: Record<string, { top: number; height: number }> = {};
      for (const el of root.querySelectorAll("span")) {
        const own = el.textContent?.trim() ?? "";
        if (wanted.includes(own) && !(own in out)) {
          const r = el.getBoundingClientRect();
          out[own] = { top: r.top, height: r.height };
        }
      }
      return out;
    },
    root as unknown as HTMLElement,
    texts,
  );
}

describe("instances", () => {
  // A failed test must not leave its dialog over the next one.
  afterEach(async () => {
    for (let i = 0; i < 3 && (await dialog().isExisting()); i++) await browser.keys("Escape");
  });

  it("creates, lists and deletes an instance", async () => {
    await expect($("h2=No instances yet")).toBeDisplayed();
    await openCreate();
    // The latest release is preselected.
    await expect(versionTrigger()).toHaveText("9.9");
    await chooseVersion("9.9");
    await chooseLoader("Fabric");

    const loaderVersion = dialog().$('[aria-haspopup="listbox"]*=0.99.0');
    await loaderVersion.waitForDisplayed();
    await loaderVersion.click();
    const recommended = $('[role="option"]*=0.99.0');
    await expect(recommended).toHaveText(expect.stringContaining("Recommended"));
    await browser.keys("Escape");
    await expect(dialog()).toBeDisplayed();

    await expect(dialog().$("aria/Name")).toHaveValue("9.9 Fabric");
    await dialog().$("button=Create instance").click();
    await expect(dialog()).not.toBeExisting();
    const actions = $("aria/Actions for 9.9 Fabric");
    await expect(actions).toBeDisplayed();
    const folders = instanceFolders();
    expect(folders).toHaveLength(1);

    await actions.click();
    await $('[role="menuitem"][aria-label="Delete"]').click();
    await expect(alertDialog()).toBeDisplayed();
    expect(await dialogName()).toBe("Delete 9.9 Fabric?");
    await alertDialog().$("button=Move to Recycle Bin").click();
    await expect($("h2=No instances yet")).toBeDisplayed({ wait: 30_000 });
    expect(instanceFolders()).toEqual([]);
    // The folder this run leaves in the user's Recycle Bin.
    console.log(`moved to the Recycle Bin: instance folder "${folders[0]}"`);
  });

  it("walks the create dialog by keyboard", async () => {
    await openCreate();
    // First focus on the version trigger; the loader radio group is one stop; the close button is
    // last; Tab wraps inside the dialog; every stop shows the focus indicator.
    expect(await focusedName()).toBe("Game version 9.9");
    const expected = [
      "Releases",
      "Snapshots",
      "Beta",
      "Alpha",
      "No loader",
      "Name",
      "Cancel",
      "Create instance",
      "Close",
      "Game version 9.9",
    ];
    const order: string[] = [];
    for (let i = 0; i < expected.length; i++) {
      await browser.keys("Tab");
      order.push(await focusedName());
      expect(await focusIndicatorShown()).toBe(true);
    }
    expect(order).toEqual(expected);
    // Backwards too; the name field is three stops before the close button.
    for (const name of ["Close", "Create instance", "Cancel", "Name"]) {
      await browser.keys(["Shift", "Tab"]);
      expect(await focusedName()).toBe(name);
    }
    // Escape leaves the dialog (on the close button it would first close its tooltip).
    await browser.keys("Escape");
    await expect(dialog()).not.toBeExisting();
    expect(await focusedName()).toBe("Create instance");
  });

  it("switches to the narrow layout below 880 CSS px", async () => {
    await openCreate();
    const name = dialog().$("aria/Name");
    await name.setValue("Narrow check");
    await dialog().$("button=Create instance").click();
    await expect(dialog()).not.toBeExisting();
    const row = $("button*=Narrow check");
    await expect(row).toBeDisplayed();
    const header = $("button=Create instance");
    const { width: startWidth, height: startHeight } = await browser.getWindowSize();

    try {
      // Regular grid: one line, 48 px rows, version beside the name, labeled header button.
      expect(await browser.execute(() => window.innerWidth)).toBeGreaterThanOrEqual(880);
      expect((await row.getSize()).height).toBe(48);
      let rects = await rectsByText(row, ["Narrow check", "9.9"]);
      expect(Math.abs(rects["9.9"].top - rects["Narrow check"].top)).toBeLessThan(8);
      expect((await header.getSize()).width).toBeGreaterThan(80);

      await browser.setWindowSize(860, startHeight);
      await browser.waitUntil(async () => (await browser.execute(() => window.innerWidth)) < 880);
      // Narrow grid: 64 px rows, version on its own line under the name, icon-only header button
      // that keeps its name.
      await browser.waitUntil(async () => (await row.getSize()).height === 64);
      rects = await rectsByText(row, ["Narrow check", "9.9"]);
      expect(rects["9.9"].top).toBeGreaterThan(
        rects["Narrow check"].top + rects["Narrow check"].height - 1,
      );
      const icon = $('[aria-label="Create instance"]');
      await expect(icon).toBeDisplayed();
      expect((await icon.getSize()).width).toBeLessThanOrEqual(48);
      await expect($("aria/Actions for Narrow check")).toBeDisplayed();
      expect(
        await browser.execute(() => document.documentElement.scrollWidth <= window.innerWidth),
      ).toBe(true);
    } finally {
      await browser.setWindowSize(startWidth, startHeight);
    }
    await browser.waitUntil(async () => (await row.getSize()).height === 48);
  });

  it("explains a loader that has no builds for the version", async () => {
    await openCreate();
    // The checkbox is named by the label around it.
    await dialog().$("label=Beta").click();
    await expect(dialog().$("label=Beta").$('[role="checkbox"]')).toHaveAttribute(
      "aria-checked",
      "true",
    );
    await chooseVersion("b9.9");
    await chooseLoader("Fabric");
    await expect(dialog().$("span=No Fabric version for b9.9")).toBeDisplayed();
    await expect(dialog()).toHaveText(
      expect.stringContaining("Pick another game version or another loader."),
    );
    const create = dialog().$("button=Create instance");
    await expect(create).toHaveAttribute("aria-disabled", "true");
    // The reason is the button's description.
    const describedBy = await create.getAttribute("aria-describedby");
    const reason = await browser.execute(
      (id) => document.getElementById(id)?.textContent ?? "",
      describedBy ?? "",
    );
    expect(reason).toContain("No Fabric version for b9.9");
    await create.click();
    await expect(dialog()).toBeDisplayed();
    await dialog().$("button=Cancel").click();
    await expect(dialog()).not.toBeExisting();
  });

  it("logged no Content Security Policy violation", async () => {
    expect(await cspViolations()).toEqual([]);
  });
});
