import type { Plugin } from "vite";

/**
 * Sonner inserts its style sheet with a `<style>` element at run time, which the app's CSP
 * (`style-src 'self'`, no inline styles) refuses, and it has no nonce option. This plugin removes
 * that call from Sonner's module; `src/styles/index.css` imports the same CSS from
 * `sonner/dist/styles.css` instead, so it ships as part of the app's own style sheet.
 * A Sonner version that inserts its CSS differently makes the build fail instead of silently
 * bringing the inline style back.
 */
const INSERT_CALL = /^__insertCSS\("(?:[^"\\]|\\.)*"\);?$/m;

export function removeSonnerRuntimeStyle(code: string): string {
  if (!INSERT_CALL.test(code)) {
    throw new Error("sonner: the runtime style insertion was not found; check the CSP again");
  }
  return code.replace(INSERT_CALL, "");
}

export function sonnerStaticStyle(): Plugin {
  return {
    name: "sonner-static-style",
    enforce: "pre",
    transform(code, id) {
      if (!/[\/]sonner[\/]dist[\/]index\.mjs$/.test(id)) return null;
      return { code: removeSonnerRuntimeStyle(code), map: null };
    },
  };
}
