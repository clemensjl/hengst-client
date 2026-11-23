// Test helpers: read index.css and DESIGN.md so the tests compare the two instead of repeating values.
import { readFileSync } from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "../..");
export const CSS = readFileSync(path.join(root, "src/styles/index.css"), "utf8");
export const DESIGN = readFileSync(path.join(root, "../../DESIGN.md"), "utf8").replace(
  /\r\n/g,
  "\n",
);
export const UI_DIR = path.join(root, "src/components/ui");

/** The body of the first top-level block that starts with `selector {`. */
export function block(css: string, selector: string): string {
  const start = css.search(
    new RegExp(`^${selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")} \\{`, "m"),
  );
  if (start < 0) throw new Error(`no block ${selector}`);
  const open = css.indexOf("{", start);
  let depth = 0;
  for (let i = open; i < css.length; i++) {
    if (css[i] === "{") depth++;
    if (css[i] === "}" && --depth === 0) return css.slice(open + 1, i);
  }
  throw new Error(`unterminated block ${selector}`);
}

/** Value of `--name` declared directly in the block, or undefined. */
export function declared(body: string, name: string): string | undefined {
  const m = body.match(new RegExp(`(?:^|[;\\s])${name}:\\s*([^;]+);`));
  return m?.[1].trim();
}

export function cssVar(theme: ":root" | ".dark", name: string): string | undefined {
  return declared(block(CSS, theme), name);
}

export function themeInline(): string {
  return block(CSS, "@theme inline");
}

/** Lower case, single spaces, no trailing zeros in decimals: `0.40` equals `0.4`. */
export function normalize(value: string): string {
  return value
    .toLowerCase()
    .replace(/\s+/g, " ")
    .replace(/(\d+\.\d*?)0+(?=\D|$)/g, "$1")
    .replace(/(\d+)\.(?=\D|$)/g, "$1")
    .trim();
}

/** The cells of the table rows in the section that starts with `heading` (without the header rows). */
export function tableRows(heading: string): string[][] {
  const start = DESIGN.indexOf(heading);
  if (start < 0) throw new Error(`no section ${heading}`);
  const rest = DESIGN.slice(start + heading.length);
  const end = rest.search(/\n#{2,3} /);
  const section = end < 0 ? rest : rest.slice(0, end);
  return section
    .split("\n")
    .filter((l) => l.startsWith("|") && !/^\|[\s|:-]+\|$/.test(l))
    .slice(1)
    .map((l) =>
      l
        .replace(/^\||\|$/g, "")
        .split(/(?<!\\)\|/)
        .map((c) => c.trim()),
    );
}

/** First `--token` in a cell, or undefined. */
export function firstToken(cell: string): string | undefined {
  return cell.match(/`(--[a-z0-9-]+)`/)?.[1];
}

function channel(v: number): number {
  const s = v / 255;
  return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
}

export function luminance(hex: string): number {
  const h = hex.replace("#", "");
  const [r, g, b] = [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16));
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

export function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}
