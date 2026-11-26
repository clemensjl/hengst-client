import { describe, expect, it } from "vitest";
import de from "./de.json";
import en from "./en.json";
import { pickLanguage } from "./index";

const flat = (o: Record<string, string>) => Object.entries(o);
const params = (s: string) => [...s.matchAll(/\{\{\s*([\w.]+)[^}]*\}\}/g)].map((m) => m[1]).sort();
const PREFIXES = [
  ...["nav.", "settings.", "about.", "errors.", "accounts.", "signIn.", "instances.", "common."],
  "play.",
  "tray.",
];
const both = [...flat(de), ...flat(en)];

describe("translations", () => {
  it("de and en have the same keys", () => {
    expect(Object.keys(de).sort()).toEqual(Object.keys(en).sort());
  });

  it("keys are flat and use the namespace prefixes", () => {
    for (const [k, v] of both) {
      expect(typeof v, k).toBe("string");
      expect(
        PREFIXES.some((p) => k.startsWith(p)),
        k,
      ).toBe(true);
    }
  });

  it("no translation is empty", () => {
    for (const [k, v] of both) expect(v.trim(), k).not.toBe("");
  });

  it("interpolation params match", () => {
    for (const [k, v] of flat(de)) {
      expect(params(v), k).toEqual(params((en as Record<string, string>)[k]));
    }
  });

  it("picks German for de-AT and English otherwise", () => {
    expect(pickLanguage(null, "de-AT")).toBe("de");
    expect(pickLanguage(null, "fr-FR")).toBe("en");
    expect(pickLanguage("en", "de-DE")).toBe("en");
    expect(pickLanguage("de", "en-US")).toBe("de");
  });

  it("en.json uses US spelling", () => {
    const uk =
      /\b(colour|behaviour|centre|licence|organis\w*|cancelled|favourite|grey|catalogue|programme|initialis\w*|recognis\w*|customis\w*)\b/i;
    for (const [k, v] of flat(en)) expect(uk.test(v), `${k}: ${v}`).toBe(false);
  });

  it("no string has an exclamation mark or an emoji", () => {
    for (const [k, v] of both) {
      expect(v.includes("!"), k).toBe(false);
      expect(/\p{Extended_Pictographic}/u.test(v), k).toBe(false);
    }
  });

  it("no dash is used as a separator", () => {
    for (const [k, v] of both) expect(/ [-–—] /.test(v), `${k}: ${v}`).toBe(false);
  });

  it("German never addresses the user formally", () => {
    for (const [k, v] of flat(de)) expect(/\bSie\b|\bIhr\w*\b/.test(v), `${k}: ${v}`).toBe(false);
  });

  it("the legal line is the same in both languages and the game name stays out of everything else", () => {
    expect(de["about.legal"]).toBe("Not an official Minecraft product.");
    expect(en["about.legal"]).toBe("Not an official Minecraft product.");
    for (const [k, v] of both) {
      // The factual references to the game's name (DESIGN.md 12, 11.5): the product the account
      // must own and the name of Mojang's profile page.
      if (!["about.legal", "errors.gameNotOwned.title", "errors.noProfile.body"].includes(k)) {
        expect(v, k).not.toMatch(/minecraft/i);
      }
    }
  });
});
