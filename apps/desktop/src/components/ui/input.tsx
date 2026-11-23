import { cn } from "@/lib/utils";

/* DESIGN.md 10.7: 40 px, 16/24, padding 0 12, --surface-raised with --border-control; hover border
   --text-muted, invalid border --danger. Focus is the global outline. */
function Input({ className, ...props }: React.ComponentProps<"input">) {
  return (
    <input
      data-slot="input"
      spellCheck={false}
      autoComplete="off"
      className={cn(
        "h-10 w-full min-w-0 rounded-md border border-input bg-secondary px-3 text-body text-foreground hover:border-muted-foreground aria-invalid:border-danger disabled:border-border disabled:bg-card disabled:text-text-disabled",
        className,
      )}
      {...props}
    />
  );
}

export { Input };
