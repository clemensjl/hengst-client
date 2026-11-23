import { Dialog as DialogPrimitive } from "@base-ui/react/dialog";
import { cn } from "@/lib/utils";
import { DialogBody, DialogDescription, DialogFooter, DialogOverlay, DialogTitle } from "./dialog";

/* DESIGN.md 10.9: a confirmation. It is the dialog of dialog.tsx with the role `alertdialog`, 440
   wide, without a close button, and a pointer-down outside the panel closes it (Base UI's own
   AlertDialog would never close on an outside press, which the design asks for). Initial focus is
   the safe button: the caller passes `initialFocus`. */

function AlertDialog({ ...props }: DialogPrimitive.Root.Props) {
  return <DialogPrimitive.Root data-slot="alert-dialog" {...props} />;
}

function AlertDialogContent({ className, children, ...props }: DialogPrimitive.Popup.Props) {
  return (
    <DialogPrimitive.Portal>
      <DialogOverlay />
      <DialogPrimitive.Popup
        data-slot="alert-dialog-content"
        role="alertdialog"
        className={cn(
          "motion-dialog fixed top-1/2 left-1/2 z-50 flex max-h-[calc(100dvh-48px)] w-110 max-w-[calc(100vw-48px)] -translate-x-1/2 -translate-y-1/2 flex-col gap-4 rounded-lg border layer-outline bg-popover p-6 text-body text-popover-foreground shadow-md outline-none",
          className,
        )}
        {...props}
      >
        {children}
      </DialogPrimitive.Popup>
    </DialogPrimitive.Portal>
  );
}

export {
  AlertDialog,
  AlertDialogContent,
  DialogBody as AlertDialogBody,
  DialogDescription as AlertDialogDescription,
  DialogFooter as AlertDialogFooter,
  DialogTitle as AlertDialogTitle,
};
