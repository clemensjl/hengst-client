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

/**
 * Where it applies: `vite build` (the real app), and `vite dev` because vite.config.ts keeps
 * Sonner out of dependency pre-bundling (`optimizeDeps.exclude`). Where it does not: Vitest, which
 * loads Sonner from node_modules untransformed, so Sonner still inserts its style element in the
 * jsdom tests (harmless there, no CSP).
 *
 * A build that never transformed Sonner (a new Sonner entry file name, a changed import) fails at
 * the end instead of silently shipping the blocked inline style again.
 */
export function sonnerStaticStyle(): Plugin {
  let building = false;
  let transformed = 0;
  return {
    name: "sonner-static-style",
    enforce: "pre",
    configResolved(config) {
      building = config.command === "build";
    },
    transform(code, id) {
      if (!/[\\/]sonner[\\/]dist[\\/]index\.mjs$/.test(id)) return null;
      transformed += 1;
      return { code: removeSonnerRuntimeStyle(code), map: null };
    },
    buildEnd(error) {
      if (error || !building) return;
      assertTransformedOnce(transformed);
    },
  };
}

export function assertTransformedOnce(count: number): void {
  if (count !== 1) {
    throw new Error(
      `sonner-static-style: Sonner's module was transformed ${count} times, expected once. ` +
        "Sonner's entry file may have been renamed; its inline style would be blocked by the CSP.",
    );
  }
}
