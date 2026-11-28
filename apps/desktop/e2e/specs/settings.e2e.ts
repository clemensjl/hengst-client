import { readFileSync } from "node:fs";
import { join } from "node:path";
import { byRole, cspViolations, focusedName, restartApp } from "../support/app.ts";
import { openView } from "../support/game.ts";

// One app for the whole file, in this order: the memory test runs in English; the next test
// switches to German and the dark theme and restarts the app on the same data folder.

const settingsFile = () =>
  JSON.parse(readFileSync(join(process.env.HENGST_E2E_HOME!, "settings.json"), "utf8")) as {
    language?: string;
    theme?: string;
    memoryMb?: number;
  };

/** The help, warning or error line under the memory field (its description). */
function memoryLine(): Promise<string> {
  return browser.execute(() => {
    const field = document.activeElement as HTMLElement | null;
    const id = field?.getAttribute("aria-describedby") ?? "";
    return (document.getElementById(id)?.textContent ?? "").trim();
  });
}

describe("settings", () => {
  it("checks the memory value typed by keyboard and saves a valid one", async () => {
    await openView("Settings");
    const field = await byRole("textbox", "Memory");
    await field.click();
    expect(await focusedName()).toBe("Memory");
    await browser.keys(["Control", "a"]);
    await browser.keys("100");
    // Checked when the field is left (or on Enter), not while typing the first time.
    await browser.keys("Enter");
    await expect(field).toHaveAttribute("aria-invalid", "true");
    expect(await memoryLine()).toBe("Choose a value between 512 and 65,536 MB.");
    // After the first error the check follows every keystroke.
    await browser.keys(["Control", "a"]);
    await browser.keys("3072");
    await expect(field).not.toHaveAttribute("aria-invalid", "true");
    await browser.keys("Enter");
    await browser.waitUntil(() => settingsFile().memoryMb === 3072, {
      timeoutMsg: "3072 was not saved",
    });
    // Escape puts the saved value back.
    await browser.keys(["Control", "a"]);
    await browser.keys("abc");
    await browser.keys("Escape");
    await expect(field).toHaveValue("3072");
    expect(await memoryLine()).not.toContain("Choose a value");
  });

  it("switches language and theme and keeps both after a restart", async () => {
    await openView("Settings");
    await (await byRole("radio", "Deutsch")).click();
    await expect($("header h1")).toHaveText("Einstellungen");
    expect(await browser.execute(() => document.documentElement.lang)).toBe("de");
    await (await byRole("radio", "Dunkel")).click();
    await browser.waitUntil(() =>
      browser.execute(() => document.documentElement.classList.contains("dark")),
    );
    await browser.waitUntil(() => {
      const s = settingsFile();
      return s.language === "de" && s.theme === "dark";
    });

    await restartApp();
    // The start page in German, dark from the first paint on.
    await expect($('nav button[aria-label="Spielen"]')).toBeDisplayed();
    expect(await browser.execute(() => document.documentElement.classList.contains("dark"))).toBe(
      true,
    );
    await openView("Einstellungen");
    await expect(await byRole("radio", "Deutsch")).toHaveAttribute("aria-checked", "true");
    await expect(await byRole("radio", "Dunkel")).toHaveAttribute("aria-checked", "true");
    await expect(await byRole("textbox", "Arbeitsspeicher")).toHaveValue("3072");
  });

  it("logged no Content Security Policy violation", async () => {
    expect(await cspViolations()).toEqual([]);
  });
});
