import { cn } from "@/lib/utils";

/* DESIGN.md 10.7: above the field, 14/20 weight 600. */
function Label({ className, ...props }: React.ComponentProps<"label">) {
  return <label data-slot="label" className={cn("text-label", className)} {...props} />;
}

export { Label };
