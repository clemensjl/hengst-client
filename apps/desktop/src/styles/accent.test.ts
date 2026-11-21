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

const rel = (f: string) => path.relative(SRC, f).replace(/\\/g, "/");

// Hand-written code: not components/ui (generated and gated in tokens.test.ts), no tests, no styles.
const scanned = files(SRC)
  .filter((f) => /\.(ts|tsx)$/.test(f))
  .filter((f) => !rel(f).startsWith("components/ui/") && !rel(f).startsWith("styles/"))
  .filter((f) => !/\.test\.(ts|tsx)$/.test(f))
  .map((f) => ({ file: rel(f), text: readFileSync(f, "utf8") }));

// The accent serves the primary action (Button variant "primary", no own bg-primary), the active
// navigation icon and the focus ring (the --focus token of index.css) and nothing else
// (DESIGN.md 2.2). Where a raw use may appear:
const allowed: Record<string, string[]> = {
  "app/Sidebar.tsx": ["text-primary"], // the active navigation icon
};

describe("accent", () => {
  it("scans the hand-written sources", () => {
    expect(scanned.length).toBeGreaterThan(5);
  });

  it("is not reachable through shadcn's hover role: no bg-accent, text-accent, border-accent", () => {
    for (const { file, text } of scanned) {
      expect(/\b(?:bg|text|border)-accent(?![-\w])/.test(text), file).toBe(false);
    }
  });

  it("is used only for the primary action, the active nav icon and the focus ring", () => {
    const raw =
      /\b(?:[a-z-]+:)*(?:bg|text|border|ring)-primary(?:-foreground)?(?![\w-])|var\(--accent\)|accent-hover|accent-pressed/g;
    for (const { file, text } of scanned) {
      for (const [hit] of text.matchAll(raw)) {
        expect(allowed[file] ?? [], `${file}: ${hit}`).toContain(hit);
      }
    }
  });
});
