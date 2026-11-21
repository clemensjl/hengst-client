import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { removeSonnerRuntimeStyle } from "./sonnerStaticStyle";

const sonner = readFileSync(
  path.resolve(import.meta.dirname, "../node_modules/sonner/dist/index.mjs"),
  "utf8",
);
const conf = JSON.parse(
  readFileSync(path.resolve(import.meta.dirname, "../src-tauri/tauri.conf.json"), "utf8"),
);

describe("Sonner under the CSP", () => {
  it("loses its run-time style insertion and keeps the rest", () => {
    const out = removeSonnerRuntimeStyle(sonner);
    // Only the helper's own definition of __insertCSS stays, no call with CSS text.
    expect(out).not.toMatch(/__insertCSS\("/);
    expect(out.length).toBeLessThan(sonner.length - 10000);
    expect(out).toContain("createElement");
  });

  it("fails loudly when the insertion looks different", () => {
    expect(() => removeSonnerRuntimeStyle("export const x = 1;")).toThrow(/CSP/);
  });

  it("the CSP allows style elements from 'self' only", () => {
    for (const key of ["csp", "devCsp"]) {
      const csp: string = conf.app.security[key];
      const styles = csp
        .split(";")
        .map((d) => d.trim())
        .find((d) => d.startsWith("style-src "));
      expect(styles).toBe("style-src 'self'");
    }
  });
});
