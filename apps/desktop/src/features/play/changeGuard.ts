import { useLayoutEffect, useRef } from "react";

/** DESIGN.md 10.0: after a button turned into another action, clicks are ignored this long. */
export const CHANGE_GUARD_MS = 600;

/**
 * The click guard of DESIGN.md 10.0 for a button whose action follows `key`: when `key` changes
 * and `arms(left, now)` says the change counts, clicks within 600 ms are ignored, so the second
 * click of a double click never hits the new action. Returns the check to call in the click
 * handler: `true` while clicks are ignored.
 */
export function useChangeGuard<K>(key: K, arms: (left: K, now: K) => boolean): () => boolean {
  const last = useRef(key);
  const changedAt = useRef(-Infinity);
  // Before the browser paints the new label, so no click can reach it unguarded.
  useLayoutEffect(() => {
    if (last.current === key) return;
    const left = last.current;
    last.current = key;
    if (arms(left, key)) changedAt.current = Date.now();
    // `arms` is a pure rule of the caller, not a dependency: only a change of `key` arms the guard.
  }, [key]);
  return () => Date.now() - changedAt.current < CHANGE_GUARD_MS;
}
