import { useLayoutEffect, useState, type RefObject } from "react";

/** `true` while the text of the element is cut off by an ellipsis (its content is wider than its box). */
export function useTruncated(ref: RefObject<HTMLElement | null>, text: string): boolean {
  const [truncated, setTruncated] = useState(false);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const measure = () => setTruncated(el.scrollWidth > el.clientWidth);
    measure();
    if (typeof ResizeObserver !== "function") return;
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, [ref, text]);
  return truncated;
}
