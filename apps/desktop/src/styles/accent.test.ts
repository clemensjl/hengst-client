import { describe, expect, it } from "vitest";
import { handWritten as scanned } from "./sourceFiles";

// The accent serves the primary action (Button variant "primary", no own bg-primary), the active
// navigation icon and the focus ring (the --focus token of index.css) and nothing else
// (DESIGN.md 2.2). Every raw use is allowed per file AND per line: the line must match the
// pattern given for it, so the same class on another element of the file fails.
const allowed: Record<string, { hit: string; line: RegExp; why: string }[]> = {
  "app/Sidebar.tsx": [
    { hit: "text-primary", line: /active \? "text-primary"/, why: "the active navigation icon" },
  ],
};

const RAW_ACCENT =
  /\b(?:[a-z0-9-]+:)*(?:bg|text|border|ring|accent|fill|stroke|outline|decoration|shadow|caret|from|to|via|divide)-(?:primary|\(--(?:primary|accent)[^)]*\)|\[var\(--(?:primary|accent)[^\]]*\)\])(?:-foreground)?(?![\w-])|var\(--accent\)|var\(--primary\)|accent-hover|accent-pressed/g;

/** Every raw accent use of a source text with the line it sits on. */
export function accentHits(text: string): { hit: string; line: string }[] {
  return text
    .split("\n")
    .flatMap((line) => [...line.matchAll(RAW_ACCENT)].map((m) => ({ hit: m[0], line })));
}

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
    for (const { file, text } of scanned) {
      for (const { hit, line } of accentHits(text)) {
        const rule = (allowed[file] ?? []).find((a) => a.hit === hit && a.line.test(line));
        expect(rule, `${file}: ${hit} in: ${line.trim()}`).toBeDefined();
      }
    }
  });

  it("every allowance is still needed (a stale entry would hide a later use)", () => {
    for (const [file, rules] of Object.entries(allowed)) {
      const source = scanned.find((s) => s.file === file);
      expect(source, file).toBeDefined();
      for (const rule of rules) {
        const used = accentHits(source!.text).some(
          (h) => h.hit === rule.hit && rule.line.test(h.line),
        );
        expect(used, `${file}: ${rule.why}`).toBe(true);
      }
    }
  });

  it("the detector flags every spelling of an accent use (self-test)", () => {
    for (const sample of [
      "bg-primary",
      "hover:text-primary",
      "ring-primary-foreground",
      "bg-(--primary)",
      "text-(--accent)",
      "bg-[var(--primary)]",
      "border-primary",
      "var(--accent)",
      "var(--primary)",
      "hover:bg-accent-hover",
      "active:bg-accent-pressed",
    ]) {
      expect(accentHits(`<i className="${sample}" />`), sample).not.toEqual([]);
    }
    for (const fine of [
      "bg-hover",
      "text-muted-foreground",
      "bg-(--hover)",
      "ring-focus",
      "bg-selected",
    ]) {
      expect(accentHits(`<i className="${fine}" />`), fine).toEqual([]);
    }
    // The Sidebar allowance applies to the active icon only, not to a second element.
    const second = 'className={cn("size-4", "text-primary")}';
    const rule = allowed["app/Sidebar.tsx"][0];
    expect(rule.line.test(second)).toBe(false);
  });
});
