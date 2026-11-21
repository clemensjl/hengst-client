// Test helper: finds two classes of one class list that set the same property under the same
// variant. `cn` only joins (no later-class-wins merging), so a list like this would depend on CSS
// order. Covers the properties the components use; it is a gate, not a full Tailwind parser.

const TEXT_SIZES = new Set([
  ...["xs", "sm", "base", "lg", "xl", "2xl", "3xl", "4xl", "5xl", "6xl", "7xl", "8xl", "9xl"],
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

const PADDING: Record<string, string[]> = {
  p: ["pt", "pr", "pb", "pl"],
  px: ["pr", "pl"],
  py: ["pt", "pb"],
};

/** The properties a utility sets, or none if it is not one of the tracked groups. */
export function properties(utility: string): string[] {
  const u = utility.replace(/^!|!$/g, "").replace(/^-/, "");
  let m: RegExpMatchArray | null;
  if ((m = u.match(/^(p|px|py|pt|pr|pb|pl)-/))) return PADDING[m[1]] ?? [m[1]];
  if ((m = u.match(/^(h|w|min-h|min-w|max-h|max-w)-/))) return [m[1]];
  if (u.startsWith("size-")) return ["h", "w"];
  if (u.startsWith("gap-")) return ["gap"];
  if (/^rounded(-|$)/.test(u)) return ["rounded"];
  if (u.startsWith("bg-")) return /^bg-(clip|no-repeat|cover|contain|center)/.test(u) ? [] : ["bg"];
  if ((m = u.match(/^text-(.+)$/))) {
    if (TEXT_OTHER.has(m[1]) || /^(balance|pretty|ellipsis|clip)$/.test(m[1])) return [];
    return [TEXT_SIZES.has(m[1]) ? "text-size" : "text-color"];
  }
  if (/^border(-[xytrbl])?(-\d+)?$/.test(u)) return ["border-width"];
  if (u.startsWith("border-")) {
    return /^border-(solid|dashed|collapse|separate)/.test(u) ? [] : ["border-color"];
  }
  if ((m = u.match(/^font-(.+)$/))) return [FONT_FAMILY.has(m[1]) ? "font-family" : "font-weight"];
  if (u.startsWith("shadow")) return ["shadow"];
  if (u.startsWith("opacity-")) return ["opacity"];
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
