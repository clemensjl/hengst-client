// Test helper: the class patterns the generated components must not contain (DESIGN.md 2.4, 2.2, 5).
// Every pattern carries a sample that it must flag, so a pattern that stops matching fails the
// self-test in tokens.test.ts instead of silently letting a default back in.

export interface Banned {
  /** What is ruled out and why, for the failure message. */
  why: string;
  pattern: RegExp;
  /** A class string the pattern must flag. */
  sample: string;
}

const literal = (s: string): RegExp => new RegExp(s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
const word = (s: string, why: string): Banned => ({ why, pattern: literal(s), sample: s });

/** The utilities that take a color; an alpha modifier or an arbitrary value on them is ruled out. */
const COLOR_UTILITY =
  "(?:bg|text|border|ring|outline|fill|stroke|from|to|via|decoration|shadow|divide|accent|caret)";
const VARIANTS = "(?:[a-z0-9-]+:)*";

export const BANNED_CLASSES: Banned[] = [
  word("ring-ring/", "shadcn's ring color with alpha"),
  word("outline-ring/", "shadcn's outline color with alpha"),
  word("dark:", "dark mode is the .dark token set, not per-class variants"),
  word("animate-pulse", "skeletons do not pulse"),
  word("text-white", "no pure white text"),
  word("bg-input/", "input fill with alpha"),
  word("focus-visible:ring", "focus is the outline of section 9"),
  word("focus-visible:border-ring", "focus is the outline of section 9"),
  word("hover:bg-primary/", "no accent hover with alpha"),
  word("hover:bg-destructive/", "no danger hover with alpha"),
  word("font-medium", "weights are 400 and 600"),
  word(
    "var(--accent)",
    "the accent only through the primary button, the nav icon and the focus ring",
  ),
  word(
    "var(--primary)",
    "the accent only through the primary button, the nav icon and the focus ring",
  ),
  {
    why: "arbitrary accent variable, for example bg-(--primary)",
    pattern: /\(--(?:primary|accent)(?:-[a-z]+)*\)/,
    sample: "bg-(--primary)",
  },
  // Shadows: only the elevation token (shadow-md, shadow-lg, shadow-xl all map to it, DESIGN.md 5).
  word("shadow-xs", "no shadow outside the elevation token"),
  word("shadow-sm", "no shadow outside the elevation token"),
  word("shadow-2xs", "no shadow outside the elevation token"),
  word("shadow-2xl", "no shadow outside the elevation token"),
  {
    why: "Tailwind's default shadow",
    pattern: /(?:^|[\s"'`:])shadow(?![-\w])/,
    sample: "rounded-md shadow",
  },
  {
    why: "inner shadow",
    pattern: /(?:^|[\s"'`:])(?:inset-shadow|shadow-inner)(?![\w])/,
    sample: "p-2 shadow-inner",
  },
  { why: "inset shadow utility", pattern: /\binset-shadow-/, sample: "inset-shadow-sm" },
  { why: "drop shadow utility", pattern: /\bdrop-shadow(?:-|\b)/, sample: "drop-shadow-md" },
  { why: "arbitrary shadow", pattern: /\bshadow-\[/, sample: "shadow-[0_1px_2px_#000]" },
  // Radii: control (6), panel (10) and the pill; every other step or value is ruled out.
  {
    why: "radius steps outside the two radii (xs, 2xl to 4xl, full), on any corner or side",
    pattern: /\brounded(?:-(?:[trblse]|tl|tr|bl|br|ss|se|es|ee))?-(?:xs|2xl|3xl|4xl|full)\b/,
    sample: "rounded-t-full",
  },
  { why: "full radius", pattern: /\brounded-full\b/, sample: "rounded-full" },
  { why: "xs radius", pattern: /\brounded-xs\b/, sample: "rounded-xs" },
  {
    why: "arbitrary radius, for example rounded-[8px]",
    pattern: /\brounded(?:-[a-z]{1,2})?-(?:\[|\()/,
    sample: "rounded-[8px]",
  },
  // Colors: no alpha modifier and no arbitrary literal color (tokens only).
  {
    why: "alpha modifier on a named color",
    pattern: new RegExp(`\\b${VARIANTS}${COLOR_UTILITY}-[a-z0-9-]+\\/(?:\\d|\\[)`),
    sample: "bg-surface/50",
  },
  {
    why: "alpha modifier on an arbitrary color or variable",
    pattern: new RegExp(
      `\\b${VARIANTS}${COLOR_UTILITY}-(?:\\[[^\\]]+\\]|\\([^)]+\\))\\/(?:\\d|\\[)`,
    ),
    sample: "bg-[#fff]/50",
  },
  {
    why: "arbitrary color literal",
    pattern: new RegExp(
      `\\b${VARIANTS}${COLOR_UTILITY}-\\[(?:#|rgb|hsl|oklch|oklab|lab|lch|color-mix|color:)`,
      "i",
    ),
    sample: "text-[#123456]",
  },
];

/** Every banned pattern a text matches, as `why`. */
export function bannedIn(text: string): string[] {
  return BANNED_CLASSES.filter((b) => b.pattern.test(text)).map((b) => b.why);
}

/** Classes the gate must let through (the self-test checks that the patterns are not too wide). */
export const ALLOWED_SAMPLES = [
  "shadow-md",
  "shadow-lg",
  "shadow-xl",
  "shadow-none",
  "rounded-sm",
  "rounded-md",
  "rounded-lg",
  "rounded-t-lg",
  "rounded-pill",
  "bg-surface-raised",
  "text-muted-foreground",
  "border-input",
  "hover:bg-hover",
  "focus-visible:outline-2",
  "text-[13px]",
  "bg-(--hover)",
];
