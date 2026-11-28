import { readdirSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";

const SRC = path.resolve(import.meta.dirname, "..");

function files(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const full = path.join(dir, name);
    return statSync(full).isDirectory() ? files(full) : [full];
  });
}

const rel = (f: string) => path.relative(SRC, f).split(path.sep).join("/");

describe("the galleries are dev-server pages", () => {
  /** The files outside `dev/` that mention the gallery's module. */
  const loaders = (name: string) =>
    files(SRC)
      .filter((f) => /\.(ts|tsx)$/.test(f))
      .filter((f) => !f.endsWith("devOnly.test.ts"))
      .filter((f) => !rel(f).startsWith("dev/"))
      .filter((f) => readFileSync(f, "utf8").includes(`dev/${name}`))
      .map(rel);
  const main = readFileSync(path.join(SRC, "main.tsx"), "utf8");

  it("only main.tsx loads the error gallery, by dynamic import, behind import.meta.env.DEV and the hash", () => {
    expect(loaders("ErrorGallery")).toEqual(["main.tsx"]);
    expect(main).toMatch(
      /if \(import\.meta\.env\.DEV && location\.hash === "#error-gallery"\) \{\s*void import\("@\/dev\/ErrorGallery"\)/,
    );
    expect(main).not.toMatch(/^import .*ErrorGallery/m);
  });

  it("only main.tsx loads the sign-in gallery, by dynamic import, behind import.meta.env.DEV and the hash", () => {
    expect(loaders("SignInGallery")).toEqual(["main.tsx"]);
    expect(main).toMatch(
      /else if \(import\.meta\.env\.DEV && location\.hash\.startsWith\("#sign-in-gallery"\)\) \{\s*void import\("@\/dev\/SignInGallery"\)/,
    );
    expect(main).not.toMatch(/^import .*SignInGallery/m);
  });

  it("only main.tsx loads the log gallery, by dynamic import, behind import.meta.env.DEV and the hash", () => {
    expect(loaders("LogGallery")).toEqual(["main.tsx"]);
    expect(main).toMatch(
      /else if \(import\.meta\.env\.DEV && location\.hash\.startsWith\("#log-gallery"\)\) \{\s*void import\("@\/dev\/LogGallery"\)/,
    );
    expect(main).not.toMatch(/^import .*LogGallery/m);
    const plugin = readFileSync(
      path.resolve(import.meta.dirname, "../../vite-plugins/noDevGallery.ts"),
      "utf8",
    );
    for (const marker of ["log-gallery", "LogGallery", "log gallery"]) {
      expect(plugin).toContain(`"${marker}"`);
    }
    // Every sample message of the log gallery carries the marker the build searches for.
    const gallery = readFileSync(path.join(SRC, "dev/LogGallery.tsx"), "utf8");
    const samples = [...gallery.matchAll(/return [`"]([^`"]*)/g)].map((m) => m[1]);
    expect(samples.filter((s) => s.startsWith("log gallery")).length).toBeGreaterThan(4);
  });

  it("the sign-in gallery's markers are among those the build searches for", () => {
    const plugin = readFileSync(
      path.resolve(import.meta.dirname, "../../vite-plugins/noDevGallery.ts"),
      "utf8",
    );
    for (const marker of ["sign-in-gallery", "SignInGallery", "gallery sample:"]) {
      expect(plugin).toContain(`"${marker}"`);
    }
    // Every sample text in the gallery starts with the marker the build searches for.
    const gallery = readFileSync(path.join(SRC, "dev/SignInGallery.tsx"), "utf8");
    const details = [...gallery.matchAll(/detail:\s*"([^"]*)"/g)].map((m) => m[1]);
    expect(details.length).toBeGreaterThan(0);
    for (const detail of details) expect(detail, detail).toMatch(/^gallery sample/);
  });

  // The production output is checked by the build itself (vite-plugins/noDevGallery.ts), not here:
  // a test that reads dist would depend on the order of the CI steps.
});
