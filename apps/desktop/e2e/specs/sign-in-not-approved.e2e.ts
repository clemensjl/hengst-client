import { byRole, cspViolations, dialog, dialogName } from "../support/app.ts";

// The mock answers the Minecraft login with 403: Mojang has not approved the app (scenario
// `not-approved`).

describe("sign-in before Mojang's approval", () => {
  it("shows_not_approved_explanation", async () => {
    await $("aria/Choose account").click();
    await $('[role="menuitem"]*=Add account').click();
    expect(await dialogName()).toBe("Add account");
    await dialog().$("aria/Sign in with Microsoft").click();

    await byRole("heading", "Sign-in is not available yet", 30_000);
    await expect(dialog()).toHaveText(
      expect.stringContaining("Mojang has not approved Hengst Client's Microsoft sign-in yet."),
    );
    await expect(dialog()).toHaveText(
      expect.stringContaining("Development builds can use the offline development account."),
    );
    await dialog().$("button=Close").click();
    await expect(dialog()).not.toBeExisting();

    // No account was added: the header still asks to choose, the menu lists only "Dev".
    await $("aria/Choose account").click();
    const items = await $$('[role="menuitemradio"]');
    expect(items).toHaveLength(1);
    await expect(items[0]).toHaveText(expect.stringContaining("Dev"));
    await browser.keys("Escape");
  });

  it("logged no Content Security Policy violation", async () => {
    expect(await cspViolations()).toEqual([]);
  });
});
