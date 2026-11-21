import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import {
  CSS,
  DESIGN,
  UI_DIR,
  block,
  contrast,
  cssVar,
  declared,
  firstToken,
  normalize,
  tableRows,
  themeInline,
} from "./designDoc";

const THEMES = [":root", ".dark"] as const;

describe("tokens", () => {
  const rows21 = tableRows("### 2.1 Token table");

  it("index.css holds the token values of DESIGN.md 2.1", () => {
    expect(rows21.length).toBeGreaterThanOrEqual(30);
    for (const [name, light, dark] of rows21) {
      const token = firstToken(name);
      expect(token, name).toBeDefined();
      for (const [theme, doc] of [
        [":root", light],
        [".dark", dark],
      ] as const) {
        const want = doc.replace(/^`|`$/g, "");
        expect(normalize(cssVar(theme, token!) ?? "MISSING"), `${token} in ${theme}`).toBe(
          normalize(want),
        );
      }
    }
  });

  const requirement = (cell: string): number | undefined => {
    const m = cell.match(/^(4\.5|3)\b/);
    return m ? Number(m[1]) : undefined;
  };
  const rows23 = tableRows("### 2.3 Contrast (calculated, WCAG 2.x relative luminance)")
    .map((c) => ({
      fg: firstToken(c[0]),
      bg: firstToken(c[1]),
      listed: [Number(c[2]), Number(c[3])],
      need: requirement(c[4] ?? ""),
    }))
    .filter((r) => r.fg && r.bg && r.need !== undefined);

  it("reads at least 40 contrast rows from DESIGN.md 2.3", () => {
    expect(rows23.length).toBeGreaterThanOrEqual(40);
  });

  it.each(rows23.map((r) => [r.fg, r.bg, r] as const))(
    "%s on %s meets its requirement in both themes",
    (fg, bg, r) => {
      THEMES.forEach((theme, i) => {
        const ratio = contrast(cssVar(theme, fg!)!, cssVar(theme, bg!)!);
        expect(ratio, `${fg} on ${bg} in ${theme}`).toBeGreaterThanOrEqual(r.need!);
        // The table lists the ratio with two decimals; it must still describe the CSS values.
        expect(Math.abs(ratio - r.listed[i]), `${fg} on ${bg} in ${theme}: listed`).toBeLessThan(
          0.02,
        );
      });
    },
  );

  // Interactive controls get their boundary from --border-control (3:1); the two decorative
  // border tokens are exempt (DESIGN.md 2.1).
  const interactiveBorders = ["--border-control"];
  const decorativeBorders = ["--border", "--border-strong"];
  it.each(
    interactiveBorders.flatMap((b) => ["--bg", "--surface", "--surface-raised"].map((s) => [b, s])),
  )("%s against %s >= 3 in both themes", (border, surface) => {
    for (const theme of THEMES) {
      expect(contrast(cssVar(theme, border)!, cssVar(theme, surface)!)).toBeGreaterThanOrEqual(3);
    }
  });

  it("decorative borders are exempt, but shadcn --input points at --border-control", () => {
    expect(cssVar(":root", "--input")).toBe("var(--border-control)");
    for (const b of decorativeBorders) {
      expect(cssVar(":root", "--input")).not.toBe(`var(${b})`);
    }
  });

  it("each theme sets its color-scheme", () => {
    expect(block(CSS, ":root")).toMatch(/color-scheme:\s*light;/);
    expect(block(CSS, ".dark")).toMatch(/color-scheme:\s*dark;/);
  });

  it("the window background Rust sets at creation equals --bg of each theme", () => {
    const rust = readFileSync(
      path.resolve(import.meta.dirname, "../../src-tauri/src/startup.rs"),
      "utf8",
    );
    const rgb = (hex: string) =>
      [0, 2, 4].map((i) => parseInt(hex.slice(1 + i, 3 + i), 16)).join(", ");
    expect(rust).toContain(`BG_LIGHT: (u8, u8, u8) = (${rgb(cssVar(":root", "--bg")!)});`);
    expect(rust).toContain(`BG_DARK: (u8, u8, u8) = (${rgb(cssVar(".dark", "--bg")!)});`);
  });

  it("shadcn variables are declared on :root and point at our tokens (DESIGN.md 2.4)", () => {
    const rows = tableRows("### 2.4 Mapping onto shadcn/ui").filter((r) => r.length >= 3);
    expect(rows.length).toBeGreaterThanOrEqual(18);
    const root = block(CSS, ":root");
    const inline = themeInline();
    for (const [variable, value, utility] of rows) {
      const name = variable.match(/^`(--[a-z-]+)`$/)?.[1];
      if (name && value.startsWith("`var(")) {
        expect(declared(root, name), `:root ${name}`).toBe(value.replace(/`/g, ""));
      }
      const util = utility.match(/`(--color-[a-z-]+): (var\([^)]+\))`/);
      if (util) expect(declared(inline, util[1]), util[1]).toBe(util[2]);
    }
    // --accent stays the brand token; shadcn's hover role is only the utility.
    expect(declared(root, "--accent")).toMatch(/^#/);
    expect(declared(inline, "--color-accent")).toBe("var(--hover)");
    expect(declared(root, "--accent-foreground")).toBe("var(--text)");
    expect(declared(root, "--radius")).toBe("var(--radius-panel)");
    // The text colors live on :root, never in @theme (they would read as font sizes).
    for (const t of ["--text", "--text-muted", "--text-disabled"]) {
      expect(declared(root, t)).toBeDefined();
      expect(declared(inline, t)).toBeUndefined();
    }
  });

  it("@theme inline carries the radius, shadow, font, ease and breakpoint scale of DESIGN.md 2.4", () => {
    const inline = themeInline();
    const radius = (n: string) => declared(inline, `--radius-${n}`);
    expect(radius("sm")).toBe("var(--radius-control)");
    expect(radius("md")).toBe("var(--radius-control)");
    for (const n of ["lg", "xl", "2xl", "3xl", "4xl"])
      expect(radius(n)).toBe("var(--radius-panel)");
    expect(declared(inline, "--shadow-xs")).toBe("none");
    expect(declared(inline, "--shadow-sm")).toBe("none");
    for (const n of ["md", "lg", "xl"]) {
      expect(declared(inline, `--shadow-${n}`)).toBe("var(--elevation-1)");
    }
    for (const f of ["sans", "display", "mono"]) {
      expect(declared(inline, `--font-${f}`), f).toBeDefined();
    }
    expect(declared(inline, "--ease-out")).toBe("var(--ease-out)");
    expect(cssVar(":root", "--ease-out")).toBe("cubic-bezier(0.23, 1, 0.32, 1)");
    expect(inline).toMatch(/--breakpoint-\*:\s*initial;/);
    expect(declared(inline, "--breakpoint-medium")).toBe("880px");
    expect(declared(inline, "--breakpoint-wide")).toBe("1200px");
    expect(declared(inline, "--breakpoint-full")).toBe("1280px");
  });

  it("the type scale of DESIGN.md 2.5 is declared with its size, line height and weight", () => {
    const rows = tableRows("Font-size utilities (Tailwind v4").filter((r) =>
      r[1]?.startsWith("`--text-"),
    );
    expect(rows.length).toBe(11);
    const inline = themeInline();
    for (const [, variable, spec] of rows) {
      const name = variable.replace(/`/g, "");
      const [size, line, weight] = spec.match(/^([\d.]+rem) \/ ([\d.]+rem) \/ (\d+)/)!.slice(1);
      expect(declared(inline, name), name).toBe(size);
      expect(declared(inline, `${name}--line-height`), name).toBe(line);
      expect(declared(inline, `${name}--font-weight`), name).toBe(weight);
    }
  });

  it("spacing, radius and motion tokens of DESIGN.md sections 4, 5 and 8 are declared on :root", () => {
    const want: Record<string, string> = {
      "--space-1": "4px",
      "--space-2": "8px",
      "--space-3": "12px",
      "--space-4": "16px",
      "--space-5": "24px",
      "--space-6": "32px",
      "--space-7": "48px",
      "--radius-control": "6px",
      "--radius-panel": "10px",
      "--radius-pill": "999px",
      "--dur-press": "120ms",
      "--dur-popover-in": "160ms",
      "--dur-popover-out": "120ms",
      "--dur-dialog-in": "220ms",
      "--dur-dialog-out": "160ms",
      "--dur-progress": "200ms",
    };
    for (const [name, value] of Object.entries(want)) {
      expect(cssVar(":root", name), name).toBe(value);
      // The value is stated in the document too.
      expect(DESIGN, name).toContain(`\`${name}\``);
    }
  });
});

describe("generated components pass the gate of DESIGN.md 2.4", () => {
  const files = readdirSync(UI_DIR)
    .filter((f) => f.endsWith(".tsx"))
    .map((f) => ({ name: f, text: readFileSync(path.join(UI_DIR, f), "utf8") }));
  // Base UI sets these at runtime on its popups and panels.
  const runtimeVariables = new Set([
    "--transform-origin",
    "--anchor-width",
    "--anchor-height",
    "--available-width",
    "--available-height",
    "--positioner-width",
    "--positioner-height",
    "--collapsible-panel-height",
    "--collapsible-panel-width",
  ]);
  const declaredInCss = (name: string) => new RegExp(`(?:^|[;{\\s])${name}:`, "m").test(CSS);

  it("holds the expected set of components", () => {
    expect(files.map((f) => f.name).sort()).toEqual(
      [
        "button.tsx",
        "collapsible.tsx",
        "radio-group.tsx",
        "separator.tsx",
        "skeleton.tsx",
        "sonner.tsx",
        "tooltip.tsx",
      ].sort(),
    );
  });

  it("every CSS variable it uses is declared in index.css or set by Base UI at runtime", () => {
    for (const { name, text } of files) {
      const used = [...text.matchAll(/(?:var\(|\()(--[a-z0-9-]+)/g)].map((m) => m[1]);
      for (const v of used) {
        expect(declaredInCss(v) || runtimeVariables.has(v), `${name}: ${v}`).toBe(true);
      }
    }
  });

  it("none of the shadcn defaults the design rules out remains", () => {
    const banned = [
      "ring-ring/",
      "outline-ring/",
      "dark:",
      "shadow-xs",
      "shadow-sm",
      "rounded-2xl",
      "rounded-3xl",
      "rounded-4xl",
      "var(--accent)",
      "animate-pulse",
      "text-white",
      "bg-input/",
      "focus-visible:ring",
      "focus-visible:border-ring",
      "hover:bg-primary/",
      "hover:bg-destructive/",
      "font-medium",
    ];
    for (const { name, text } of files) {
      for (const b of banned) expect(text.includes(b), `${name}: ${b}`).toBe(false);
    }
  });

  const primaryUtility =
    /\b[a-z0-9:[\]-]*(?:bg|text|border|ring|accent|fill|stroke|outline|decoration|shadow|from|to|via)-primary(?:-foreground)?(?![\w-])/;

  it("primary utilities appear only in the primary variant of button.tsx", () => {
    for (const { name, text } of files) {
      const scanned =
        name === "button.tsx" ? text.replace(/primary:\s*\n?\s*"[^"]*"/, "primary: ") : text;
      expect(primaryUtility.test(scanned), name).toBe(false);
    }
    const button = files.find((f) => f.name === "button.tsx")!.text;
    expect(primaryUtility.test(button)).toBe(true);
  });

  it("button.tsx has exactly five variants and three sizes, default variant secondary", () => {
    const text = files.find((f) => f.name === "button.tsx")!.text;
    const keys = (from: string, to: string) =>
      text
        .slice(text.indexOf(from), text.indexOf(to))
        .split("\n")
        .map((l) => l.match(/^ {8}"?([a-z-]+)"?:/)?.[1])
        .filter(Boolean);
    expect(keys("variant: {", "size: {")).toEqual([
      "primary",
      "secondary",
      "ghost",
      "destructive",
      "link",
    ]);
    expect(keys("size: {", "defaultVariants")).toEqual(["default", "icon", "lg"]);
    expect(text).toMatch(/defaultVariants:\s*\{\s*variant: "secondary"/);
  });
});
