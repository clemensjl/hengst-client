import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import {
  assertTransformedOnce,
  removeSonnerRuntimeStyle,
  sonnerStaticStyle,
} from "./sonnerStaticStyle";

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

  it("a production build fails unless Sonner's module was transformed exactly once", () => {
    expect(() => assertTransformedOnce(0)).toThrow(/renamed/);
    expect(() => assertTransformedOnce(2)).toThrow(/2 times/);
    expect(() => assertTransformedOnce(1)).not.toThrow();
    // Through the plugin's own hooks: build mode, nothing transformed, buildEnd throws.
    const plugin = sonnerStaticStyle() as unknown as {
      configResolved: (c: { command: string }) => void;
      transform: (code: string, id: string) => unknown;
      buildEnd: (e?: Error) => void;
    };
    plugin.configResolved({ command: "build" });
    expect(() => plugin.buildEnd()).toThrow();
    expect(plugin.transform("x", "/other/module.js")).toBeNull();
    plugin.transform(sonner, "C:\\app\\node_modules\\sonner\\dist\\index.mjs");
    expect(() => plugin.buildEnd()).not.toThrow();
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
