// Test helper: finds two classes of one class list that set the same property under the same
// variant. `cn` only joins (no later-class-wins merging), so a list like this would depend on CSS
// order. Covers the properties the components use; it is a gate, not a full Tailwind parser.
//
// What is deliberately not a conflict:
// - a general utility followed by a more specific one (`p-4 px-2`, `border border-b`,
//   `rounded-md rounded-t-lg`): Tailwind emits the general one first, so the specific one wins in
//   every build. Two utilities of the same specificity (`px-2 px-4`) conflict.
// - `font-semibold` or `leading-5` after a type-scale utility (`text-body font-semibold`): the
//   scale utilities declare `font-weight: var(--tw-font-weight, 400)` and
//   `line-height: var(--tw-leading, 1.5rem)`, so the explicit class wins whatever the order.

const TEXT_SIZES = new Set([
  ...["xs", "sm", "base", "lg", "xl", "2xl", "3xl", "4xl", "5xl", "6xl", "7xl", "8xl", "9xl"],
]);
/** DESIGN.md 2.5: each of these also sets line height and weight. */
const TYPE_SCALE = new Set([
  ...["display-lg", "display", "heading", "body", "label", "small", "caption", "badge"],
  ...["play", "mono", "code"],
]);
const TEXT_OTHER = new Set([
  "left",
  "center",
  "right",
  "justify",
  "start",
  "end",
  "wrap",
  "nowrap",
]);
const FONT_FAMILY = new Set(["sans", "serif", "mono", "display"]);
const DISPLAY = new Set([
  ...["block", "inline-block", "inline", "flex", "inline-flex", "grid", "inline-grid"],
  ...["contents", "hidden", "table", "flow-root", "list-item"],
]);

/** Splits `hover:wide:px-4` into the variant prefix and the utility; arbitrary variants stay in the prefix. */
export function splitVariant(token: string): [string, string] {
  let depth = 0;
  let cut = -1;
  for (let i = 0; i < token.length; i++) {
    const c = token[i];
    if (c === "[" || c === "(") depth++;
    else if (c === "]" || c === ")") depth--;
    else if (c === ":" && depth === 0) cut = i;
  }
  return cut < 0 ? ["", token] : [token.slice(0, cut), token.slice(cut + 1)];
}

/** An arbitrary text value that is a length is a size, anything else a color. */
function arbitraryTextProperty(value: string): string {
  const inner = value.replace(/^\[|\]$/g, "").replace(/^\(|\)$/g, "");
  if (/^length:/.test(inner) || /^-?[\d.]+(px|rem|em|%|vw|vh|ch)?$/.test(inner)) return "text-size";
  return "text-color";
}

/**
 * The properties a utility sets, or none if it is not one of the tracked groups. A key names the
 * property and its specificity (`pad:all`, `pad:x`, `pad:t`), so a general utility and a specific
 * one do not collide.
 */
export function properties(utility: string): string[] {
  const u = utility.replace(/^!|!$/g, "").replace(/^-/, "");
  let m: RegExpMatchArray | null;
  if ((m = u.match(/^(p|px|py|pt|pr|pb|pl|ps|pe)-/))) return [`pad:${m[1]}`];
  if ((m = u.match(/^(m|mx|my|mt|mr|mb|ml|ms|me)-/))) return [`margin:${m[1]}`];
  if ((m = u.match(/^(h|w|min-h|min-w|max-h|max-w)-/))) return [m[1]];
  if (u.startsWith("size-")) return ["h", "w"];
  if (u.startsWith("gap-")) return ["gap"];
  if ((m = u.match(/^rounded(?:-(t|r|b|l|tl|tr|bl|br|s|e|ss|se|es|ee))?(?:-.+)?$/))) {
    return [`rounded:${m[1] ?? "all"}`];
  }
  if (u.startsWith("bg-")) {
    if (/^bg-(clip|origin|no-repeat|repeat|cover|contain|center|top|bottom|left|right)/.test(u)) {
      return [];
    }
    if (/^bg-(gradient|linear|radial|conic|none$|\[(?:url|image:|linear))/.test(u))
      return ["bg-image"];
    return ["bg"];
  }
  if ((m = u.match(/^text-(.+)$/))) {
    const value = m[1];
    if (TEXT_OTHER.has(value) || /^(balance|pretty|ellipsis|clip)$/.test(value)) return [];
    if (/^[[(]/.test(value)) return [arbitraryTextProperty(value)];
    if (TYPE_SCALE.has(value)) return ["text-size", "type:leading", "type:weight"];
    return [TEXT_SIZES.has(value) ? "text-size" : "text-color"];
  }
  if ((m = u.match(/^border(?:-([xytrbl]|[se]))?(?:-(\d+|\[[^\]]+\]))?$/))) {
    return [`border-width:${m[1] ?? "all"}`];
  }
  if (u.startsWith("border-")) {
    return /^border-(solid|dashed|dotted|double|none|hidden|collapse|separate|spacing)/.test(u)
      ? []
      : ["border-color"];
  }
  if ((m = u.match(/^font-(.+)$/))) return [FONT_FAMILY.has(m[1]) ? "font-family" : "font-weight"];
  if (u.startsWith("leading-")) return ["leading"];
  if (u.startsWith("tracking-")) return ["tracking"];
  if (u.startsWith("shadow")) return ["shadow"];
  if (u.startsWith("opacity-")) return ["opacity"];
  if (DISPLAY.has(u)) return ["display"];
  if (u.startsWith("items-")) return ["items"];
  if (/^justify-(items|self)-/.test(u))
    return [u.startsWith("justify-items") ? "justify-items" : "justify-self"];
  if (u.startsWith("justify-")) return ["justify"];
  if (u.startsWith("self-")) return ["self"];
  if ((m = u.match(/^overflow(?:-([xy]))?-/))) return [`overflow:${m[1] ?? "all"}`];
  if (u.startsWith("duration-")) return ["duration"];
  if (/^ring(?:-(?:\d+|\[[^\]]+\]))?$/.test(u)) return ["ring-width"];
  if (u === "ring-inset") return ["ring-inset"];
  if (u.startsWith("ring-offset-")) return ["ring-offset"];
  if (u.startsWith("ring-")) return ["ring-color"];
  if (/^outline(?:-(?:\d+|\[[^\]]+\]))?$/.test(u)) return ["outline-width"];
  if (u.startsWith("outline-offset-")) return ["outline-offset"];
  if (/^outline-(none|hidden|solid|dashed|dotted|double)$/.test(u)) return ["outline-style"];
  if (u.startsWith("outline-")) return ["outline-color"];
  return [];
}

/** Findings as `variant|property: classA, classB`; empty when no property is set twice. */
export function classConflicts(classList: string): string[] {
  const seen = new Map<string, string>();
  const found: string[] = [];
  for (const token of classList.split(/\s+/).filter(Boolean)) {
    const [variant, utility] = splitVariant(token);
    for (const prop of properties(utility)) {
      const key = `${variant}|${prop}`;
      const earlier = seen.get(key);
      if (earlier && earlier !== token) found.push(`${key}: ${earlier}, ${token}`);
      else seen.set(key, token);
    }
  }
  return found;
}

/** Class-list-like string literals of a source file: at least two tokens, every one looks like a class. */
export function classLiterals(source: string): string[] {
  const out: string[] = [];
  for (const m of source.matchAll(/(["'`])((?:\\.|(?!\1)[^\\\n])*)\1/g)) {
    const text = m[2].replace(/\$\{[^}]*\}/g, "x").trim();
    const tokens = text.split(/\s+/);
    if (tokens.length < 2) continue;
    if (!tokens.every((t) => /^[a-z0-9!\-:[\]&_*().,%='#/>~+@]+$/i.test(t))) continue;
    if (!tokens.some((t) => /[-:]/.test(t))) continue;
    out.push(text);
  }
  return out;
}
