// Test helper: the hand-written source files that build class names (not components/ui, which is
// generated and gated in tokens.test.ts; no tests or test support, no styles, no dev-only pages).
import { readdirSync, readFileSync, statSync } from "node:fs";
import path from "node:path";

export const SRC = path.resolve(import.meta.dirname, "..");

function files(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const full = path.join(dir, name);
    return statSync(full).isDirectory() ? files(full) : [full];
  });
}

const rel = (f: string) => path.relative(SRC, f).replace(/\\/g, "/");

export const handWritten = files(SRC)
  .filter((f) => /\.(ts|tsx)$/.test(f))
  .filter(
    (f) =>
      !rel(f).startsWith("components/ui/") &&
      !rel(f).startsWith("styles/") &&
      // Test support (fake backend, harness): class names there are assertions, not styling.
      !rel(f).startsWith("test/"),
  )
  .filter((f) => !/\.test\.(ts|tsx)$/.test(f))
  .map((f) => ({ file: rel(f), text: readFileSync(f, "utf8") }));
