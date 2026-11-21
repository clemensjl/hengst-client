/*
 * WebView defaults of DESIGN.md Appendix A, for release builds. Tauri 2.12 gives no builder option
 * for the WebView2 context menu or accelerator keys (wry has them, tauri-runtime-wry does not pass
 * them on), and switching all accelerator keys off would also take away the zoom keys that
 * DESIGN.md 13 keeps. So the page handles both itself. Zoom (Ctrl+Plus, Ctrl+Minus, Ctrl+0) is
 * Tauri's `zoom_hotkeys_enabled`, set in startup.rs.
 */

const TEXT_INPUT_TYPES = new Set(["text", "search", "url", "tel", "email", "password", "number"]);

/** Text inputs and the log area (`[data-native-menu]`) keep the native menu (cut, copy, paste). */
export function allowsNativeContextMenu(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  if (target.closest("[data-native-menu]")) return true;
  if (target instanceof HTMLTextAreaElement) return true;
  if (target instanceof HTMLInputElement) return TEXT_INPUT_TYPES.has(target.type);
  return false;
}

/** F5, Ctrl+R, Ctrl+Shift+R, Ctrl+F5, Ctrl+P, F12, Ctrl+F and F3 (the find bar; the log search of 4b brings its own). */
export function blocksBrowserShortcut(e: KeyboardEvent): boolean {
  if (e.key === "F5" || e.key === "F12" || e.key === "F3") return true;
  if (!e.ctrlKey || e.altKey) return false;
  return ["r", "p", "f"].includes(e.key.toLowerCase());
}

export function installWebviewDefaults(enabled: boolean = import.meta.env.PROD): () => void {
  if (!enabled) return () => {};
  const menu = (e: MouseEvent) => {
    if (!allowsNativeContextMenu(e.target)) e.preventDefault();
  };
  const keys = (e: KeyboardEvent) => {
    if (blocksBrowserShortcut(e)) e.preventDefault();
  };
  document.addEventListener("contextmenu", menu);
  document.addEventListener("keydown", keys);
  return () => {
    document.removeEventListener("contextmenu", menu);
    document.removeEventListener("keydown", keys);
  };
}
