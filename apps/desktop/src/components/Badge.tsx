import { cn } from "@/lib/utils";

/** DESIGN.md 10.14: a pill with text; the warning kind sits on --surface-raised with a hairline. */
export function Badge({ className, ...props }: React.ComponentProps<"span">) {
  return (
    <span
      className={cn(
        "inline-flex h-5 items-center rounded-pill border border-border bg-surface-raised px-2 text-badge whitespace-nowrap text-warning",
        className,
      )}
      {...props}
    />
  );
}
