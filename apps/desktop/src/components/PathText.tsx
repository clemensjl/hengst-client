/** DESIGN.md 10.15 path display: mono, selectable, wraps anywhere so the whole path is visible. */
export function PathText({ path }: { path: string }) {
  return (
    <p className="font-mono text-mono wrap-anywhere text-muted-foreground select-text">{path}</p>
  );
}
