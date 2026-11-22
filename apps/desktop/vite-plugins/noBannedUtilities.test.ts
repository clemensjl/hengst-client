import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import {
  BANNED_UTILITY_SAMPLES,
  findBannedUtilities,
  noBannedUtilities,
} from "./noBannedUtilities";

describe("noBannedUtilities", () => {
  it("flags every spelling of a default-palette utility in a style sheet (self-test)", () => {
    expect(BANNED_UTILITY_SAMPLES.length).toBeGreaterThan(8);
    for (const sample of BANNED_UTILITY_SAMPLES) {
      expect(findBannedUtilities({ "a.css": `${sample} { color: red }` }), sample).toHaveLength(1);
    }
  });

  it("lets the design's own utilities through", () => {
    const css = [
      ".bg-surface{a:b}",
      ".text-muted-foreground{a:b}",
      ".bg-transparent{a:b}",
      ".border-input{a:b}",
      String.raw`.hover\:bg-hover:hover{a:b}`,
      ".text-black-ish{a:b}",
      ".bg-background{a:b}",
      ".text-white-space{a:b}",
    ].join("\n");
    expect(findBannedUtilities({ "a.css": css })).toEqual([]);
  });

  it("names the file and the utility", () => {
    expect(findBannedUtilities({ "x.css": ".bg-gray-100{a:b}" })).toEqual(["x.css: .bg-gray-100"]);
  });

  it("does not look into scripts (their class strings are not utilities)", () => {
    expect(findBannedUtilities({ "a.js": 'const x = ".bg-white"' })).toEqual([]);
  });

  it("fails the build, naming the file and the utility", () => {
    const plugin = noBannedUtilities();
    const context = {
      error: (message: string) => {
        throw new Error(message);
      },
    };
    const generate = plugin.generateBundle as unknown as (
      this: typeof context,
      options: object,
      bundle: object,
    ) => void;
    expect(() =>
      generate.call(context, {}, { "a.css": { type: "asset", source: ".bg-surface{a:b}" } }),
    ).not.toThrow();
    expect(() =>
      generate.call(context, {}, { "a.css": { type: "asset", source: ".text-black{a:b}" } }),
    ).toThrow(/a\.css: \.text-black/);
  });

  it("is registered for production builds only and in vite.config.ts", () => {
    expect(noBannedUtilities().apply).toBe("build");
    const config = readFileSync(path.resolve(import.meta.dirname, "../vite.config.ts"), "utf8");
    expect(config).toMatch(/plugins: \[[^\]]*noBannedUtilities\(\)/);
  });
});
