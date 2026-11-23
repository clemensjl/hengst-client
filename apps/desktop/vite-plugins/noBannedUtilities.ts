import type { Plugin } from "vite";

/**
 * Tailwind's default palette is reset in `index.css` (`--color-*: initial`), so `bg-white`,
 * `text-black`, `bg-gray-100` and the rest generate nothing. This check reads the production style
 * sheet and fails the build if such a utility is in it anyway (the reset removed, a theme import
 * bringing a palette back), so the design's tokens stay the only colors.
 */
const UTILITIES =
  "bg|text|border|ring|outline|fill|stroke|from|to|via|decoration|divide|shadow|accent|caret|placeholder";
const NAMED =
  "slate|gray|zinc|neutral|stone|red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose";
// A class selector in CSS: variants are escaped colons (`.hover\:bg-white`), an alpha modifier an
// escaped slash. The name must end there, so `text-black-ish` or `text-white-space` do not match.
const BANNED = new RegExp(
  String.raw`\.(?:[\w\\-]*\\:)?(?:${UTILITIES})-(?:white|black|(?:${NAMED})-\d{2,3})(?![\w-])`,
  "g",
);

/** Selectors that must be flagged, one per spelling the detector is for. */
export const BANNED_UTILITY_SAMPLES = [
  ".bg-white",
  ".bg-black",
  ".text-black",
  ".text-white",
  ".bg-gray-100",
  ".text-slate-500",
  ".border-red-500",
  String.raw`.hover\:bg-blue-600:hover`,
  String.raw`.dark\:text-zinc-100`,
  String.raw`.bg-white\/50`,
  ".ring-neutral-300",
  ".fill-emerald-500",
];

/** `file: .utility` for every banned utility in the style sheets among the outputs. */
export function findBannedUtilities(outputs: Record<string, string>): string[] {
  return Object.entries(outputs)
    .filter(([file]) => file.endsWith(".css"))
    .flatMap(([file, text]) =>
      [...text.matchAll(BANNED)].map((m) => `${file}: ${m[0].split("\\").join("")}`),
    );
}

export function noBannedUtilities(): Plugin {
  return {
    name: "no-banned-utilities",
    apply: "build",
    enforce: "post",
    generateBundle(_options, bundle) {
      const outputs: Record<string, string> = {};
      for (const [name, item] of Object.entries(bundle)) {
        if (item.type === "asset" && typeof item.source === "string") outputs[name] = item.source;
      }
      const hits = findBannedUtilities(outputs);
      if (hits.length > 0) {
        this.error(
          `the production style sheet contains utilities of Tailwind's default palette: ${hits.join(", ")}`,
        );
      }
    },
  };
}
