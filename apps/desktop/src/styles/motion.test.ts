import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { CSS } from "./designDoc";

const read = (rel: string) => readFileSync(path.resolve(import.meta.dirname, "..", rel), "utf8");

// DESIGN.md 8: the active navigation item and settings changes never animate, and nothing
// animates while the last input was the keyboard.
describe("motion", () => {
  it("the navigation items and the segments carry no transition", () => {
    for (const file of ["app/Sidebar.tsx", "components/Segments.tsx"]) {
      expect(read(file), file).not.toMatch(/transition|duration-|animate-/);
    }
  });

  it("nothing transitions or animates under data-input=keyboard", () => {
    const rule = CSS.match(
      /html\[data-input="keyboard"\] \*[^{]*\{[^}]*transition:\s*none\s*!important;[^}]*animation:\s*none\s*!important/,
    );
    expect(rule).not.toBeNull();
  });

  // DESIGN.md 10.15: the spinner is a progress indicator, not a transition, so it keeps turning
  // when a keyboard-triggered request runs; under reduced motion it is hidden.
  it("the spinner utility is exempt from the keyboard rule and turns linearly at 800 ms", () => {
    const rule = CSS.match(/html\[data-input="keyboard"\] \*:not\(([^)]*)\)/);
    expect(rule?.[1]).toContain(".spinner");
    const spinner = CSS.match(/@utility spinner \{([\s\S]*?)\n\}\n/)?.[1] ?? "";
    expect(spinner).toMatch(/animation:\s*spinner-turn 800ms linear infinite/);
    expect(spinner).toMatch(/@media \(prefers-reduced-motion: reduce\)\s*\{\s*display:\s*none/);
    expect(CSS).toMatch(/@keyframes spinner-turn \{[^}]*\}[^}]*\}/);
  });
});
