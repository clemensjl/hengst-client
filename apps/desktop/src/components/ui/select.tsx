import { Select as SelectPrimitive } from "@base-ui/react/select";
import { CheckIcon, ChevronDownIcon, ChevronUpIcon } from "lucide-react";
import { cn } from "@/lib/utils";

/* DESIGN.md 10.6: the trigger looks like an input (40 px, --surface-raised, --border-control,
   chevron at the right); the list is a level-1 panel below it with 40 px options, a 16 px check
   slot first, at most 320 px high. Opens like a popover (motion-popup). */

function Select<Value>({ ...props }: SelectPrimitive.Root.Props<Value>) {
  return <SelectPrimitive.Root data-slot="select" {...props} />;
}

/** Shared with the combobox trigger: both look like an input. */
const triggerClass =
  "group flex h-10 w-full min-w-0 items-center justify-between gap-2 rounded-md border border-input bg-secondary px-3 text-body text-foreground select-none hover:border-muted-foreground data-disabled:border-border data-disabled:bg-card data-disabled:text-text-disabled data-popup-open:border-muted-foreground";

function TriggerChevron() {
  return (
    <>
      <ChevronDownIcon className="size-4 shrink-0 group-data-popup-open:hidden" aria-hidden="true" />
      <ChevronUpIcon
        className="hidden size-4 shrink-0 group-data-popup-open:block"
        aria-hidden="true"
      />
    </>
  );
}

function SelectTrigger({ className, children, ...props }: SelectPrimitive.Trigger.Props) {
  return (
    <SelectPrimitive.Trigger
      data-slot="select-trigger"
      className={cn(triggerClass, className)}
      {...props}
    >
      {children}
      <TriggerChevron />
    </SelectPrimitive.Trigger>
  );
}

/**
 * DESIGN.md 10.15 ghost select (only the log run switcher): no border or fill, 14/20 --text,
 * --hover on hover, 40 px high; the text truncates, the chevron stays.
 */
function SelectGhostTrigger({ className, children, ...props }: SelectPrimitive.Trigger.Props) {
  return (
    <SelectPrimitive.Trigger
      data-slot="select-ghost-trigger"
      className={cn(
        "group inline-flex h-10 max-w-full min-w-0 items-center gap-2 rounded-md px-3 text-small text-foreground select-none hover:bg-hover data-disabled:text-text-disabled data-popup-open:bg-selected",
        className,
      )}
      {...props}
    >
      {children}
      <TriggerChevron />
    </SelectPrimitive.Trigger>
  );
}

function SelectValue({ className, ...props }: SelectPrimitive.Value.Props) {
  return (
    <SelectPrimitive.Value
      data-slot="select-value"
      className={cn("truncate", className)}
      {...props}
    />
  );
}

function SelectContent({ className, children, ...props }: SelectPrimitive.Popup.Props) {
  return (
    <SelectPrimitive.Portal>
      <SelectPrimitive.Positioner
        className="isolate z-50 outline-none"
        alignItemWithTrigger={false}
        sideOffset={4}
      >
        <SelectPrimitive.Popup
          data-slot="select-content"
          className={cn(
            "motion-popup z-50 max-h-[min(320px,var(--available-height))] w-(--anchor-width) overflow-y-auto rounded-lg border layer-outline bg-popover p-1 text-popover-foreground shadow-md outline-none",
            className,
          )}
          {...props}
        >
          <SelectPrimitive.List>{children}</SelectPrimitive.List>
        </SelectPrimitive.Popup>
      </SelectPrimitive.Positioner>
    </SelectPrimitive.Portal>
  );
}

function SelectItem({ className, children, ...props }: SelectPrimitive.Item.Props) {
  return (
    <SelectPrimitive.Item
      data-slot="select-item"
      className={cn(
        "option-item menu-item flex h-10 cursor-default items-center gap-2 rounded-md px-3 text-small text-foreground outline-hidden select-none",
        className,
      )}
      {...props}
    >
      <span className="flex size-4 shrink-0 items-center justify-center">
        <SelectPrimitive.ItemIndicator>
          <CheckIcon className="size-4" aria-hidden="true" />
        </SelectPrimitive.ItemIndicator>
      </span>
      {children}
    </SelectPrimitive.Item>
  );
}

const SelectItemText = SelectPrimitive.ItemText;

export {
  Select,
  SelectContent,
  SelectGhostTrigger,
  SelectItem,
  SelectItemText,
  SelectTrigger,
  SelectValue,
  TriggerChevron,
  triggerClass,
};
