import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";

const SRC = path.resolve(import.meta.dirname, "..");
const DIST = path.resolve(SRC, "../dist");

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

  // Runs against the output of `pnpm build`; without one (a plain test run) there is nothing to check.
  it.skipIf(!existsSync(path.join(DIST, "assets")))(
    "a production build contains neither the page nor its sample texts",
    () => {
      const built = files(path.join(DIST, "assets"))
        .filter((f) => /\.(js|css|html)$/.test(f))
        .map((f) => readFileSync(f, "utf8"))
        .join("\n");
      expect(built.length).toBeGreaterThan(1000);
      for (const marker of ["error-gallery", "ErrorGallery", "gallery-sample-detail"]) {
        expect(built.includes(marker), marker).toBe(false);
      }
    },
  );
});
