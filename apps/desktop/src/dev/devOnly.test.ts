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

describe("the error gallery is a dev-server page", () => {
  it("only main.tsx loads it, by dynamic import, behind import.meta.env.DEV and the hash", () => {
    const loaders = files(SRC)
      .filter((f) => /\.(ts|tsx)$/.test(f))
      .filter((f) => !f.endsWith("devOnly.test.ts"))
      .filter((f) => !path.relative(SRC, f).replace(/\\/g, "/").startsWith("dev/"))
      .filter((f) => /dev\/ErrorGallery/.test(readFileSync(f, "utf8")))
      .map((f) => path.relative(SRC, f).replace(/\\/g, "/"));
    expect(loaders).toEqual(["main.tsx"]);
    const main = readFileSync(path.join(SRC, "main.tsx"), "utf8");
    expect(main).toMatch(
      /if \(import\.meta\.env\.DEV && location\.hash === "#error-gallery"\) \{\s*void import\("@\/dev\/ErrorGallery"\)/,
    );
    expect(main).not.toMatch(/^import .*ErrorGallery/m);
  });

  // The production output is checked by the build itself (vite-plugins/noDevGallery.ts), not here:
  // a test that reads dist would depend on the order of the CI steps.
});
