import { stripBidi } from "@/lib/bidi";

/**
 * DESIGN.md 10.15 path display: mono, selectable, wraps anywhere so the whole path is visible. The
 * path is plain text, always left to right in its own isolate, and its bidi controls are dropped,
 * so it can neither reorder itself nor the text around it.
 */
export function PathText({ path }: { path: string }) {
  return (
    <p
      dir="ltr"
      className="font-mono text-mono wrap-anywhere text-muted-foreground select-text [unicode-bidi:isolate]"
    >
      {stripBidi(path)}
    </p>
  );
}
