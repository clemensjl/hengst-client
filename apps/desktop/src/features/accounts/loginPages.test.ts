import { describe, expect, it } from "vitest";
import de from "@/i18n/de.json";
import en from "@/i18n/en.json";
import i18n, { initI18n } from "@/i18n";
import { loginPages } from "./loginPages";

describe("login page texts", () => {
  it("come from the UI language and are complete", () => {
    initI18n("de", "de-AT");
    const german = loginPages(i18n.t);
    expect(german.successTitle).toBe(de["accounts.loginPage.successTitle"]);
    initI18n("en", "en-US");
    const english = loginPages(i18n.t);
    expect(english.failureBody).toBe(en["accounts.loginPage.failureBody"]);
    for (const text of [...Object.values(german), ...Object.values(english)]) {
      expect(text.trim()).not.toBe("");
      expect(text.length).toBeLessThanOrEqual(500);
    }
  });

  it("the success text sends the user back and does not claim a finished sign-in", () => {
    // The Xbox and game steps still follow; the page may only say that the user can return.
    expect(en["accounts.loginPage.successBody"]).toMatch(/return to Hengst Client/);
    expect(de["accounts.loginPage.successBody"]).toMatch(/zurückkehren/);
    expect(en["accounts.loginPage.successBody"]).not.toMatch(/signed in|you are in/i);
    expect(de["accounts.loginPage.successBody"]).not.toMatch(/angemeldet/i);
    expect(en["accounts.loginPage.successTitle"]).not.toMatch(/signed in/i);
  });
});
