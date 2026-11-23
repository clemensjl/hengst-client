import { Dialog as DialogPrimitive } from "@base-ui/react/dialog";
import { XIcon } from "lucide-react";
import { useId } from "react";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

/* DESIGN.md 10.9: --surface-raised panel, radius 10, padding 24, the layer outline and level-1
   elevation, on --overlay. The panel is at most the window height minus 48 px; the body scrolls,
   title and footer stay. The width comes from the caller (440, 560 or 640). It fades and scales in
   from the center, the overlay only fades (motion-dialog, motion-overlay). */

function Dialog({ ...props }: DialogPrimitive.Root.Props) {
  return <DialogPrimitive.Root data-slot="dialog" {...props} />;
}

function DialogTrigger({ ...props }: DialogPrimitive.Trigger.Props) {
  return <DialogPrimitive.Trigger data-slot="dialog-trigger" {...props} />;
}

function DialogClose({ ...props }: DialogPrimitive.Close.Props) {
  return <DialogPrimitive.Close data-slot="dialog-close" {...props} />;
}

function DialogOverlay({ className, ...props }: DialogPrimitive.Backdrop.Props) {
  return (
    <DialogPrimitive.Backdrop
      data-slot="dialog-overlay"
      className={cn("motion-overlay fixed inset-0 isolate z-50 bg-overlay", className)}
      {...props}
    />
  );
}

/** The close button is an icon button: it carries its name and a tooltip with the same text. */
function DialogCloseButton({ label, busyReason }: { label: string; busyReason?: string }) {
  const reasonId = useId();
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <DialogPrimitive.Close
            data-slot="dialog-close"
            aria-label={label}
            // Not `disabled`: a disabled button cannot be reached to learn why. The dialog's own
            // close handler refuses while the reason holds.
            aria-disabled={busyReason ? true : undefined}
            aria-describedby={busyReason ? reasonId : undefined}
            render={
              <Button
                variant="ghost"
                size="icon"
                className="absolute top-3 right-3 aria-disabled:text-text-disabled aria-disabled:hover:bg-transparent aria-disabled:active:bg-transparent"
              />
            }
          />
        }
      >
        <XIcon aria-hidden="true" />
        {busyReason && (
          <span id={reasonId} className="sr-only">
            {busyReason}
          </span>
        )}
      </TooltipTrigger>
      {/* While closing waits, the tooltip says why (a disabled control needs a visible reason). */}
      <TooltipContent>{busyReason ?? label}</TooltipContent>
    </Tooltip>
  );
}

function DialogContent({
  className,
  children,
  closeLabel,
  closeBusyReason,
  ...props
}: DialogPrimitive.Popup.Props & {
  /** The name of the close button; without it the dialog has none. */
  closeLabel?: string;
  /** Set while closing is not possible: why. Shown as the tooltip and described to assistive technology. */
  closeBusyReason?: string;
}) {
  return (
    <DialogPrimitive.Portal>
      <DialogOverlay />
      <DialogPrimitive.Popup
        data-slot="dialog-content"
        className={cn(
          "motion-dialog fixed top-1/2 left-1/2 z-50 flex max-h-[calc(100dvh-48px)] max-w-[calc(100vw-48px)] -translate-x-1/2 -translate-y-1/2 flex-col gap-4 rounded-lg border layer-outline bg-popover p-6 text-body text-popover-foreground shadow-md outline-none",
          className,
        )}
        {...props}
      >
        {children}
        {closeLabel && <DialogCloseButton label={closeLabel} busyReason={closeBusyReason} />}
      </DialogPrimitive.Popup>
    </DialogPrimitive.Portal>
  );
}

function DialogTitle({ className, ...props }: DialogPrimitive.Title.Props) {
  return (
    <DialogPrimitive.Title
      data-slot="dialog-title"
      className={cn("pr-10 text-heading [overflow-wrap:anywhere]", className)}
      {...props}
    />
  );
}

function DialogDescription({ className, ...props }: DialogPrimitive.Description.Props) {
  return (
    <DialogPrimitive.Description
      data-slot="dialog-description"
      className={cn("max-w-[72ch] text-body text-foreground", className)}
      {...props}
    />
  );
}

/** The part that scrolls. The 4 px of padding leave room for the focus outline at its edges. */
function DialogBody({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="dialog-body"
      className={cn("-mx-1 min-h-0 flex-1 overflow-y-auto px-1 py-1", className)}
      {...props}
    />
  );
}

/** At most two buttons, right-aligned, the primary or destructive one last, 8 px apart, 24 px below the body. */
function DialogFooter({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="dialog-footer"
      className={cn("mt-2 flex shrink-0 flex-wrap justify-end gap-2", className)}
      {...props}
    />
  );
}

export {
  Dialog,
  DialogBody,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogOverlay,
  DialogTitle,
  DialogTrigger,
};
