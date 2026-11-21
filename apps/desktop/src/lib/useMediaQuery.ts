import { useSyncExternalStore } from "react";

/** `true` while the media query matches; `fallback` where `matchMedia` does not exist. */
export function useMediaQuery(query: string, fallback = false): boolean {
  return useSyncExternalStore(
    (notify) => {
      if (typeof matchMedia !== "function") return () => {};
      const list = matchMedia(query);
      list.addEventListener("change", notify);
      return () => list.removeEventListener("change", notify);
    },
    () => (typeof matchMedia === "function" ? matchMedia(query).matches : fallback),
    () => fallback,
  );
}

/** From 1200 px wide (`wide:`): the sidebar shows its labels, below it is an icon rail. */
export const useWide = () => useMediaQuery("(min-width: 1200px)", true);
