import { readdirSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import {
  CSS,
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
import { buttonVariants } from "@/components/ui/button";
import { classConflicts, classLiterals } from "./classConflicts";
import { cnClassLists } from "./cnCalls";
import { ALLOWED_SAMPLES, BANNED_CLASSES, bannedIn } from "./gatePatterns";
import { handWritten } from "./sourceFiles";

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

  it("Tailwind does not scan the tests and gate helpers, whose banned class strings would become utilities", () => {
    for (const source of [
      "../**/*.test.{ts,tsx}",
      "./gatePatterns.ts",
      "./classConflicts.ts",
      "./sourceFiles.ts",
      "../test/**",
    ]) {
      expect(CSS).toContain(`@source not "${source}";`);
    }
  });

  it("Tailwind's default color palette is reset before our colors, so bg-white and bg-gray-* do not exist", () => {
    const inline = themeInline();
    const reset = inline.search(/--color-\*:\s*initial;/);
    expect(reset, "--color-*: initial in @theme inline").toBeGreaterThanOrEqual(0);
    expect(reset).toBeLessThan(inline.indexOf("--color-background:"));
    // The reset comes first, or it would also remove the tokens declared before it.
    expect(inline.slice(0, reset)).not.toMatch(/--color-[a-z]/);
  });

  it("a menu item highlighted by keyboard shows the focus outline inside the item (DESIGN.md 9)", () => {
    const start = CSS.indexOf("@utility menu-item");
    const utility = CSS.slice(start, CSS.indexOf("@utility", start + 1));
    expect(utility).toMatch(/\[data-highlighted\][^}]*background-color:\s*var\(--hover\)/);
    expect(utility).toMatch(/:focus-visible\s*\{[^}]*outline-offset:\s*-2px/);
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

  it("spacing, radius and motion tokens are declared on :root with the values of DESIGN.md sections 4, 5 and 8", () => {
    const section = (heading: string, prefix: RegExp) =>
      tableRows(heading).filter((r) => prefix.test(r[0]));
    // The exact counts of the tables: 7 spacing steps, 3 radii, 7 motion tokens (one curve, six
    // durations). A row added to DESIGN.md without a token, or a parser that misses one, fails here.
    const spacing = section("## 4. Spacing", /^`--space-/);
    const radius = section("## 5. Shape, borders, elevation", /^`--radius-/);
    const motion = section("## 8. Motion", /^`--(dur|ease)-/);
    expect([spacing.length, radius.length, motion.length]).toEqual([7, 3, 7]);
    const rows = [...spacing, ...radius, ...motion];
    for (const [name, value] of rows) {
      const token = firstToken(name)!;
      const doc = value.replace(/`/g, "");
      const unit = doc.match(/^([\d.]+) ?(px|ms)/);
      const want = unit ? unit[1] + unit[2] : doc;
      expect(normalize(cssVar(":root", token) ?? "MISSING"), token).toBe(normalize(want));
    }
  });
});

describe("generated components pass the gate of DESIGN.md 2.4", () => {
  const walk = (dir: string): string[] =>
    readdirSync(dir).flatMap((f) => {
      const full = path.join(dir, f);
      return statSync(full).isDirectory() ? walk(full) : [full];
    });
  const files = walk(UI_DIR)
    .filter((f) => f.endsWith(".tsx"))
    .map((f) => ({
      name: path.relative(UI_DIR, f).split(path.sep).join("/"),
      text: readFileSync(f, "utf8"),
    }));
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
        "alert-dialog.tsx",
        "button.tsx",
        "collapsible.tsx",
        "dialog.tsx",
        "dropdown-menu.tsx",
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
    for (const { name, text } of files) {
      expect(bannedIn(text), name).toEqual([]);
    }
  });

  // Each banned pattern must flag its own sample, and the patterns must not be so wide that they
  // flag what the design allows: a pattern that silently stops matching fails here.
  it("every banned pattern flags its sample and none flags an allowed class", () => {
    expect(BANNED_CLASSES.length).toBeGreaterThan(25);
    for (const b of BANNED_CLASSES) {
      expect(b.pattern.test(b.sample), `${b.why}: ${b.sample}`).toBe(true);
    }
    for (const sample of ALLOWED_SAMPLES) {
      expect(bannedIn(sample), sample).toEqual([]);
    }
    // The holes the re-review found, as classes in a list.
    for (const hole of [
      "shadow-inner",
      "inset-shadow-sm",
      "drop-shadow-lg",
      "rounded-t-full",
      "rounded-[8px]",
      "bg-(--primary)",
      "bg-[#fff]/50",
      "hover:text-[#000]",
    ]) {
      expect(bannedIn(`flex ${hole} p-2`), hole).not.toEqual([]);
    }
  });

  it("no component relies on a later class beating an earlier one", () => {
    // `cn` only joins classes: every class list of the generated components, and every button
    // variant with every size, must set each property once per variant.
    for (const { name, text } of files) {
      for (const list of [...classLiterals(text), ...cnClassLists(text)]) {
        expect(classConflicts(list), `${name}: ${list}`).toEqual([]);
      }
    }
    for (const variant of ["primary", "secondary", "ghost", "destructive", "link"] as const) {
      for (const size of ["default", "icon", "lg"] as const) {
        const list = buttonVariants({ variant, size });
        expect(classConflicts(list), `${variant} ${size}: ${list}`).toEqual([]);
      }
    }
  });

  it("no hand-written component relies on a later class beating an earlier one", () => {
    // Everything outside components/ui that builds class lists (cn calls and plain strings):
    // Badge, Segments, Sidebar, Shell, Boot and the feature views.
    expect(handWritten.length).toBeGreaterThan(10);
    // The scanner must really find cn calls, or the check below would pass on nothing.
    expect(handWritten.flatMap(({ text }) => cnClassLists(text)).length).toBeGreaterThan(5);
    // Also across the arguments of one cn(...) call (cnCalls.ts): `cn("px-4", "px-2")` is a conflict.
    for (const { file, text } of handWritten) {
      for (const list of [...classLiterals(text), ...cnClassLists(text)]) {
        expect(classConflicts(list), `${file}: ${list}`).toEqual([]);
      }
    }
  });

  it("the conflict check catches the conflicts it is for", () => {
    const conflicting = [
      "px-4 px-0",
      "text-label text-small",
      "text-label text-[13px]",
      "text-[13px] text-sm",
      "text-body text-label",
      "border border-transparent border-input",
      "bg-surface bg-[#fff]",
      "bg-[#fff] bg-hover",
      "flex block",
      "items-center items-start",
      "justify-between justify-center",
      "overflow-hidden overflow-auto",
      "overflow-x-hidden overflow-x-auto",
      "leading-5 leading-6",
      "tracking-tight tracking-wide",
      "duration-100 duration-200",
      "ring-1 ring-2",
      "ring-input ring-focus",
      "outline-2 outline-1",
      "outline-none outline-dashed",
      "outline-focus outline-input",
      "rounded-md rounded-lg",
      "font-semibold font-normal",
    ];
    for (const list of conflicting) expect(classConflicts(list), list).not.toEqual([]);
    const fine = [
      "px-4 hover:px-0",
      "text-label text-foreground",
      "text-[13px] text-foreground",
      "text-[#123456] text-sm",
      "border border-b",
      "border-x border-b border-input",
      "p-4 px-2 pt-1",
      "rounded-md rounded-t-lg",
      "text-body font-semibold",
      "text-label leading-5",
      "flex items-center justify-between gap-2",
      "overflow-x-hidden overflow-y-auto",
      "ring-1 ring-input",
      "outline-2 outline-focus outline-solid",
      "hidden wide:flex",
    ];
    for (const list of fine) expect(classConflicts(list), list).toEqual([]);
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
