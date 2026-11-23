import { cspViolations, dialog, dialogName } from "../support/app.ts";

// The mock declines every sign-in (scenario `declined`): the dialog ends in the error panel.

describe("the production page in the real WebView2", () => {
  it("runs with a frozen prototype and without CSP violations", async () => {
    expect(await browser.execute(() => Object.isFrozen(Object.prototype))).toBe(true);

    // Account menu.
    await $("aria/Choose account").click();
    await expect($('[role="menu"]')).toBeDisplayed();
    // Sign-in dialog, ending in the error panel with its details opened.
    await $('[role="menuitem"]*=Add account').click();
    expect(await dialogName()).toBe("Add account");
    await dialog().$("aria/Sign in with Microsoft").click();
    await expect(dialog()).toHaveText(expect.stringContaining("Sign-in declined"), {
      wait: 30_000,
    });
    const details = dialog().$("button[aria-expanded]*=Details");
    await details.click();
    await expect(details).toHaveAttribute("aria-expanded", "true");
    await expect(dialog().$('[role="region"][aria-label="Details"]')).toBeDisplayed();
    // "Copy" is not pressed: the clipboard is the machine's own.
    await dialog().$("button=Close").click();
    await expect(dialog()).not.toBeExisting();

    expect(await cspViolations()).toEqual([]);
  });
});
