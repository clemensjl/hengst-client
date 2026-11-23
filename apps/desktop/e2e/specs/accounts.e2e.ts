import { alertDialog, cspViolations, dialog, dialogName, focusedName } from "../support/app.ts";

// One app for the whole file, in this order; the later tests select their starting account
// themselves, so a failure of one test does not fail the next.
// The mock answers every sign-in request after a second (scenario `slow:1000`). Browser sign-ins
// hand out E2EPlayer, SecondPlayer, E2EPlayer, ...; the first code sign-in is E2EPlayer.

const SIGN_IN = { timeout: 30_000 };

const chip = (name: string) => $(`aria/Account: ${name}`);

async function openMenu(trigger: string) {
  await $(`aria/${trigger}`).click();
  await $('[role="menu"]').waitForDisplayed();
}

/** Opens the menu of the chip, whichever account it shows. */
async function openAccountMenu() {
  await $('[aria-haspopup="menu"][aria-label^="Account: "]').click();
  await $('[role="menu"]').waitForDisplayed();
}

/** Makes `name` the selected account, from whatever state the earlier tests left. */
async function selectAccount(name: string) {
  await openAccountMenu();
  await $(`[role="menuitemradio"]*=${name}`).click();
  await chip(name).waitForDisplayed();
}

async function startBrowserSignIn() {
  await $('[role="menuitem"]*=Add account').click();
  await expect(dialog()).toBeDisplayed();
  expect(await dialogName()).toBe("Add account");
  await dialog().$("aria/Sign in with Microsoft").click();
}

describe("accounts", () => {
  // A failed test must not leave a confirmation or a menu over the next one ("Keep" on Escape).
  afterEach(async () => {
    for (let i = 0; i < 3 && (await alertDialog().isExisting()); i++) await browser.keys("Escape");
    if (await $('[role="menu"]').isExisting()) await browser.keys("Escape");
  });

  it("offers the development account in development builds", async () => {
    // Development builds always list the offline account, so the empty header shows "Choose
    // account" (no account selected) instead of the plain "Sign in" button.
    await expect($("aria/Choose account")).toBeDisplayed();
    await openMenu("Choose account");
    const items = await $$('[role="menuitemradio"]');
    expect(items).toHaveLength(1);
    await expect(items[0]).toHaveText(expect.stringContaining("Dev"));
    await expect(items[0]).toHaveText(expect.stringContaining("Development build"));
    await browser.keys("Escape");
    await expect($('[role="menu"]')).not.toBeDisplayed();
  });

  it("signs in two accounts, switches and signs out", async () => {
    await openMenu("Choose account");
    await startBrowserSignIn();
    await chip("E2EPlayer").waitForDisplayed(SIGN_IN);

    await openMenu("Account: E2EPlayer");
    await startBrowserSignIn();
    await chip("SecondPlayer").waitForDisplayed(SIGN_IN);

    await openMenu("Account: SecondPlayer");
    await $('[role="menuitemradio"]*=E2EPlayer').click();
    await chip("E2EPlayer").waitForDisplayed();

    await openMenu("Account: E2EPlayer");
    // By keyboard: "Sign out" is the last item, ArrowRight opens its submenu on the first account.
    await browser.keys("ArrowUp");
    expect(await focusedName()).toBe("Sign out");
    await browser.keys("ArrowRight");
    await browser.waitUntil(async () => (await focusedName()) === "E2EPlayer");
    await browser.keys("Enter");
    await expect(alertDialog()).toBeDisplayed();
    expect(await dialogName()).toBe("Sign out E2EPlayer?");
    await alertDialog().$("button=Sign out").click();
    await chip("SecondPlayer").waitForDisplayed();
    await openMenu("Account: SecondPlayer");
    const names = await $$('[role="menuitemradio"]').map((i) => i.getText());
    expect(names.some((n) => n.includes("E2EPlayer"))).toBe(false);
    await browser.keys("Escape");
  });

  it("uses the code flow on request", async () => {
    await selectAccount("SecondPlayer");
    await openAccountMenu();
    await $('[role="menuitem"]*=Add account').click();
    await dialog().$("button=Use a code instead").click();
    await expect(dialog()).toHaveText(
      expect.stringContaining("Open microsoft.com/link and enter this code."),
    );
    await expect(dialog()).toHaveText(expect.stringContaining("TEST-0001"));
    await expect(dialog().$("button=Copy code")).toBeDisplayed();
    await chip("E2EPlayer").waitForDisplayed(SIGN_IN);
    await expect(dialog()).not.toBeExisting();
  });

  it("keeps the sign-in dialog open while the account is being saved", async () => {
    await selectAccount("SecondPlayer");
    await openAccountMenu();
    await startBrowserSignIn();
    await dialog().$('[role="status"]*=Signing in').waitForDisplayed(SIGN_IN);
    const close = dialog().$('[aria-label="Close"]');
    await expect(close).toHaveAttribute("aria-disabled", "true");
    await close.click();
    await browser.keys("Escape");
    await browser.pause(300);
    // Still open and still completing: neither the close button nor Escape left it.
    await expect(dialog().$('[role="status"]*=Signing in')).toBeDisplayed();
    await chip("E2EPlayer").waitForDisplayed(SIGN_IN);
    await expect(dialog()).not.toBeExisting();
  });

  it("logged no Content Security Policy violation", async () => {
    expect(await cspViolations()).toEqual([]);
  });
});
