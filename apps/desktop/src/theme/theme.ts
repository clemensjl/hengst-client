export type ThemeSetting = "system" | "light" | "dark";
export type ResolvedTheme = "light" | "dark";

const DARK_QUERY = "(prefers-color-scheme: dark)";

export function resolveTheme(setting: ThemeSetting, prefersDark: boolean): ResolvedTheme {
  if (setting === "system") return prefersDark ? "dark" : "light";
  return setting;
}

/** Sets or removes `.dark` on `<html>`. */
export function applyTheme(resolved: ResolvedTheme): void {
  document.documentElement.classList.toggle("dark", resolved === "dark");
}

function systemPrefersDark(): boolean {
  return typeof matchMedia === "function" && matchMedia(DARK_QUERY).matches;
}

/** The theme for a setting right now, without applying it. */
export function currentTheme(setting: ThemeSetting): ResolvedTheme {
  return resolveTheme(setting, systemPrefersDark());
}

/**
 * Applies the theme of a setting and, for `system`, follows the system until the returned function
 * is called. `onResolved` hears every applied theme.
 */
export function watchTheme(
  setting: ThemeSetting,
  onResolved?: (resolved: ResolvedTheme) => void,
): () => void {
  const apply = (prefersDark: boolean) => {
    const resolved = resolveTheme(setting, prefersDark);
    applyTheme(resolved);
    onResolved?.(resolved);
  };
  apply(systemPrefersDark());
  if (setting !== "system" || typeof matchMedia !== "function") return () => {};
  const query = matchMedia(DARK_QUERY);
  const listener = (e: { matches: boolean }) => apply(e.matches);
  query.addEventListener("change", listener);
  return () => query.removeEventListener("change", listener);
}
