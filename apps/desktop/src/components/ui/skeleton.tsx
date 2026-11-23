import { cn } from "@/lib/utils";

/* Static block, no pulse (DESIGN.md 8 and 10.15). */
function Skeleton({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div data-slot="skeleton" className={cn("rounded-md bg-muted", className)} {...props} />
  );
}

export { Skeleton };
