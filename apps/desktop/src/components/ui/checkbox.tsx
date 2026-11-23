import { Checkbox as CheckboxPrimitive } from "@base-ui/react/checkbox";
import { CheckIcon } from "lucide-react";
import { cn } from "@/lib/utils";

/* DESIGN.md 10.8: a 20 px box, radius 4, --border-control on --surface-raised; checked it is filled
   with --text and shows a 16 px check in --bg. Not the accent. */
function Checkbox({ className, ...props }: CheckboxPrimitive.Root.Props) {
  return (
    <CheckboxPrimitive.Root
      data-slot="checkbox"
      className={cn(
        "checkbox-box flex size-5 shrink-0 items-center justify-center border border-input bg-secondary text-background hover:border-muted-foreground data-checked:border-foreground data-checked:bg-foreground data-disabled:border-border data-disabled:text-text-disabled",
        className,
      )}
      {...props}
    >
      <CheckboxPrimitive.Indicator>
        <CheckIcon className="size-4" aria-hidden="true" />
      </CheckboxPrimitive.Indicator>
    </CheckboxPrimitive.Root>
  );
}

export { Checkbox };
