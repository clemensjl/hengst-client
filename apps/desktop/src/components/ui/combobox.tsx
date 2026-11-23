import { Combobox as ComboboxPrimitive } from "@base-ui/react/combobox";
import { CheckIcon, SearchIcon } from "lucide-react";
import { cn } from "@/lib/utils";
import { TriggerChevron, triggerClass } from "./select";

/* DESIGN.md 10.5: a trigger like an input that opens a level-1 panel of the trigger's width with a
   search field on top and a list of 40 px options below it (at most 320 px, it scrolls). Built on
   the Base UI combobox with its input inside the popup (shadcn's Base UI combobox), so list,
   highlight and keyboard come from one primitive instead of a popover plus a command list. */

function Combobox<Value>({ ...props }: ComboboxPrimitive.Root.Props<Value>) {
  return <ComboboxPrimitive.Root data-slot="combobox" {...props} />;
}

function ComboboxTrigger({ className, children, ...props }: ComboboxPrimitive.Trigger.Props) {
  return (
    <ComboboxPrimitive.Trigger
      data-slot="combobox-trigger"
      className={cn(triggerClass, className)}
      {...props}
    >
      {children}
      <TriggerChevron />
    </ComboboxPrimitive.Trigger>
  );
}

function ComboboxContent({ className, children, ...props }: ComboboxPrimitive.Popup.Props) {
  return (
    <ComboboxPrimitive.Portal>
      <ComboboxPrimitive.Positioner
        className="isolate z-50 outline-none"
        align="start"
        sideOffset={4}
      >
        <ComboboxPrimitive.Popup
          data-slot="combobox-content"
          className={cn(
            "motion-popup z-50 flex max-h-(--available-height) w-(--anchor-width) flex-col gap-1 rounded-lg border layer-outline bg-popover p-1 text-popover-foreground shadow-md outline-none",
            className,
          )}
          {...props}
        >
          {children}
        </ComboboxPrimitive.Popup>
      </ComboboxPrimitive.Positioner>
    </ComboboxPrimitive.Portal>
  );
}

/** The search field at the top of the panel, with the search icon inside it. */
function ComboboxSearch({ className, ...props }: ComboboxPrimitive.Input.Props) {
  return (
    <div className="relative shrink-0">
      <SearchIcon
        className="pointer-events-none absolute top-3 left-3 size-4 text-muted-foreground"
        aria-hidden="true"
      />
      <ComboboxPrimitive.Input
        data-slot="combobox-input"
        spellCheck={false}
        autoComplete="off"
        className={cn(
          "h-10 w-full rounded-md border border-input bg-secondary pr-3 pl-9 text-body text-foreground hover:border-muted-foreground",
          className,
        )}
        {...props}
      />
    </div>
  );
}

function ComboboxList({ className, ...props }: ComboboxPrimitive.List.Props) {
  return (
    <ComboboxPrimitive.List
      data-slot="combobox-list"
      className={cn("max-h-80 min-h-0 overflow-y-auto overscroll-contain empty:hidden", className)}
      {...props}
    />
  );
}

function ComboboxItem({ className, children, ...props }: ComboboxPrimitive.Item.Props) {
  return (
    <ComboboxPrimitive.Item
      data-slot="combobox-item"
      className={cn(
        "option-item flex h-10 cursor-default items-center gap-2 rounded-md px-3 text-small text-foreground outline-hidden select-none",
        className,
      )}
      {...props}
    >
      <span className="flex size-4 shrink-0 items-center justify-center">
        <ComboboxPrimitive.ItemIndicator>
          <CheckIcon className="size-4" aria-hidden="true" />
        </ComboboxPrimitive.ItemIndicator>
      </span>
      {children}
    </ComboboxPrimitive.Item>
  );
}

const ComboboxEmpty = ComboboxPrimitive.Empty;

export {
  Combobox,
  ComboboxContent,
  ComboboxEmpty,
  ComboboxItem,
  ComboboxList,
  ComboboxSearch,
  ComboboxTrigger,
};
