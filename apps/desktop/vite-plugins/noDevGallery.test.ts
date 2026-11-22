import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { findGalleryMarkers, noDevGallery } from "./noDevGallery";

describe("noDevGallery", () => {
  it("finds the page name, the hash and the sample text in any output", () => {
    expect(findGalleryMarkers({ "a.js": "var x=1;" })).toEqual([]);
    expect(findGalleryMarkers({ "a.js": 'location.hash==="#error-gallery"' })).toEqual([
      "a.js: error-gallery",
    ]);
    expect(
      findGalleryMarkers({
        "a.js": "function ErrorGallery(){}",
        "b.html": "gallery-sample-detail",
      }),
    ).toEqual(["a.js: ErrorGallery", "b.html: gallery-sample-detail"]);
  });

  it("finds the sign-in gallery too: its name, its hash and its sample text", () => {
    expect(findGalleryMarkers({ "a.js": 'location.hash.startsWith("#sign-in-gallery")' })).toEqual([
      "a.js: sign-in-gallery",
    ]);
    expect(findGalleryMarkers({ "a.js": "function SignInGallery(){}" })).toEqual([
      "a.js: SignInGallery",
    ]);
    expect(findGalleryMarkers({ "b.js": 'detail:"gallery sample: x"' })).toEqual([
      "b.js: gallery sample:",
    ]);
  });

  it("fails the build, naming the file and the marker", () => {
    const plugin = noDevGallery();
    const errors: string[] = [];
    const context = {
      error: (message: string) => {
        errors.push(message);
        throw new Error(message);
      },
    };
    const generate = plugin.generateBundle as unknown as (
      this: typeof context,
      options: object,
      bundle: object,
    ) => void;
    const clean = {
      "a.js": { type: "chunk", code: "ok" },
      "a.css": { type: "asset", source: "b" },
    };
    expect(() => generate.call(context, {}, clean)).not.toThrow();
    const dirty = { "a.js": { type: "chunk", code: 'import("./ErrorGallery")' } };
    expect(() => generate.call(context, {}, dirty)).toThrow(/a\.js: ErrorGallery/);
    expect(errors).toHaveLength(1);
  });

  it("is registered for production builds only and in vite.config.ts", () => {
    expect(noDevGallery().apply).toBe("build");
    const config = readFileSync(path.resolve(import.meta.dirname, "../vite.config.ts"), "utf8");
    expect(config).toMatch(/plugins: \[[^\]]*noDevGallery\(\)/);
  });
});
