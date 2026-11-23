import { RadioGroup as RadioGroupPrimitive } from "@base-ui/react/radio-group";
import { cn } from "@/lib/utils";

/* Only the group: DESIGN.md 10.6 shows a radio group as separate segments, built in
   components/Segments.tsx. shadcn's round radio item is not used in this design. */
function RadioGroup({ className, ...props }: RadioGroupPrimitive.Props) {
  return (
    <RadioGroupPrimitive
      data-slot="radio-group"
      className={cn("flex flex-wrap gap-2", className)}
      {...props}
    />
  );
}

export { RadioGroup };
