import { Switch as SwitchPrimitive } from "@base-ui/react/switch";
import { cn } from "@/lib/utils";

/* DESIGN.md 10.8: a 36 x 20 pill. Off: --hover track with a --border-control hairline and a 14 px
   --text-muted thumb. On: --text track, --bg thumb. Not the accent. The track color changes over
   --dur-press; the thumb travels over --dur-press with the one curve (pointer only: the keyboard
   flag drops every transition) and jumps under reduced motion. Focus is the global outline. */
function Switch({ className, ...props }: SwitchPrimitive.Root.Props) {
  return (
    <SwitchPrimitive.Root
      data-slot="switch"
      className={cn(
        "relative inline-flex h-5 w-9 shrink-0 items-center rounded-pill border border-input bg-hover transition-[background-color,border-color] duration-(--dur-press) ease-out data-checked:border-foreground data-checked:bg-foreground data-disabled:border-border",
        className,
      )}
      {...props}
    >
      <SwitchPrimitive.Thumb
        data-slot="switch-thumb"
        className="pointer-events-none block size-3.5 translate-x-0.5 rounded-pill bg-muted-foreground transition-[translate,background-color] duration-(--dur-press) ease-out motion-reduce:transition-[background-color] data-checked:translate-x-[18px] data-checked:bg-background"
      />
    </SwitchPrimitive.Root>
  );
}

export { Switch };
