import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

/** Splits at the last separator: everything before shrinks, the last folder stays visible. */
export function splitPath(path: string): [string, string] {
  const i = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"));
  return i <= 0 ? ["", path] : [path.slice(0, i), path.slice(i)];
}

/** DESIGN.md 10.15 path display: mono, one line, middle truncation, the full path in a tooltip, selectable. */
export function PathText({ path }: { path: string }) {
  const [head, tail] = splitPath(path);
  return (
    <Tooltip>
      <TooltipTrigger
        render={<span className="flex min-w-0 font-mono text-mono text-muted-foreground" />}
      >
        <span className="min-w-0 truncate">{head}</span>
        <span className="shrink-0">{tail}</span>
      </TooltipTrigger>
      <TooltipContent className="font-mono break-all">{path}</TooltipContent>
    </Tooltip>
  );
}
