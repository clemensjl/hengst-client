import { CircleAlertIcon, LoaderCircleIcon } from "lucide-react";
import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  AlertDialog,
  AlertDialogBody,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { describeError } from "@/features/errors/errorCopy";
import { ErrorPanel } from "@/features/errors/ErrorPanel";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";
import type { InstanceDto } from "@/ipc/bindings/InstanceDto";
import { api, IpcError } from "@/ipc/client";
import { useApp, type InstanceFocus } from "@/stores/app";
import { effectiveSelection } from "./selection";

const errorOf = (e: unknown): ErrorDto =>
  e instanceof IpcError ? e.dto : { code: "internal", params: {}, action: null, detail: String(e) };

/**
 * Outcomes after which the folder is gone or changed: there is nothing left to move, so the
 * dialog offers only closing. `recycleBinUnconfirmed`: gone, but not confirmed in the bin;
 * `recycleBinUnknown`: no result, the folder may be gone or partly moved; `recycleBinPending`:
 * an earlier delete has not ended, trying again would only say so; `recycleBinPartial`: partly
 * moved; `instanceNotFound`: it was already gone.
 */
const SETTLED = [
  "recycleBinUnconfirmed",
  "recycleBinUnknown",
  "recycleBinPending",
  "recycleBinPartial",
  "instanceNotFound",
];

/**
 * Outcomes after which the folder is worth a look: nothing moved, or what is left is unclear. Not
 * `recycleBinUnconfirmed`: the folder is gone there, so opening it is a dead end.
 */
const OPEN_FOLDER = [
  "recycleBinFailed",
  "recycleBinUnknown",
  "recycleBinPartial",
  "recycleBinPending",
];

/** After the folder went away: if it was the selection, the fallback of DESIGN.md 10.2 is saved. */
async function settleSelection(wasSelected: boolean) {
  const app = useApp.getState();
  await app.loadInstances();
  const list = useApp.getState().instances.data;
  if (!wasSelected || !list) return;
  const next = effectiveSelection(list);
  if (next === null || list.find((i) => i.id === next)?.selected) return;
  // Best effort: the list already shows the fallback as selected (`effectiveSelection`).
  await api.instanceSelect(next).catch(() => {});
  await useApp.getState().loadInstances();
}

/**
 * DESIGN.md 11.3. A confirmation with named buttons, the safe one first in focus. Every outcome of
 * the backend is told as it is: only a move the backend confirmed says "in the Recycle Bin".
 */
export function DeleteInstanceDialog({
  instance,
  wasSelected,
  returnTo,
  after,
  onClosed,
}: {
  instance: InstanceDto;
  /** The instance is the one shown as selected. */
  wasSelected: boolean;
  /** Where focus goes back when nothing was deleted. */
  returnTo: HTMLElement | null;
  /** Where focus goes once the row is gone (DESIGN.md 13). */
  after: InstanceFocus;
  onClosed: () => void;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ErrorDto | null>(null);
  const [openError, setOpenError] = useState<ErrorDto | null>(null);
  const keepRef = useRef<HTMLButtonElement>(null);
  const setInstanceFocus = useApp((s) => s.setInstanceFocus);
  // Before the first await, so a double click deletes once.
  const running = useRef(false);
  // The row went away: once the dialog is closed the list moves focus here, not back to the trigger.
  const moveFocus = useRef<InstanceFocus | null>(null);
  const settled = error !== null && SETTLED.includes(error.code);

  const remove = async () => {
    if (running.current) return;
    running.current = true;
    setBusy(true);
    setError(null);
    try {
      await api.instanceDelete(instance.id);
      moveFocus.current = after;
      toast(t("instances.deleted", { name: instance.name }));
      await settleSelection(wasSelected);
      setOpen(false);
    } catch (e) {
      const dto = errorOf(e);
      setError(dto);
      // The list shows what is true now, whatever the error was.
      if (SETTLED.includes(dto.code)) await settleSelection(wasSelected);
      else void useApp.getState().loadInstances();
    } finally {
      running.current = false;
      setBusy(false);
    }
  };

  const openFolder = () => {
    setOpenError(null);
    return api.openLocation({ kind: "instance", id: instance.id }).catch((e: unknown) => {
      setOpenError(errorOf(e));
    });
  };

  const close = () => {
    if (busy) return;
    if (settled) {
      // The row may be gone: focus goes to where it would after a delete, or back to its button.
      const still = useApp.getState().instances.data?.some((i) => i.id === instance.id);
      moveFocus.current = still ? { kind: "action", id: instance.id } : after;
    }
    setOpen(false);
  };

  const folderCopy = openError ? describeError(openError, t, "folder") : null;

  return (
    <AlertDialog
      open={open}
      onOpenChange={(next, details) => {
        // Escape and a press outside keep the instance; nothing closes while it is being moved.
        if (!next && !busy) close();
        else if (!next) details.cancel();
      }}
      onOpenChangeComplete={(isOpen) => {
        if (isOpen) return;
        // Only now: while the dialog is open it keeps the focus inside.
        if (moveFocus.current) setInstanceFocus(moveFocus.current);
        onClosed();
      }}
    >
      <AlertDialogContent
        initialFocus={keepRef}
        finalFocus={() => (moveFocus.current ? false : (returnTo ?? true))}
      >
        <AlertDialogTitle className="pr-0">
          {t("instances.deleteDialog.title", { name: instance.name })}
        </AlertDialogTitle>
        <AlertDialogBody className="flex flex-col gap-4">
          <AlertDialogDescription>{t("instances.deleteDialog.body")}</AlertDialogDescription>
          {error && (
            <ErrorPanel
              error={error}
              autoFocus
              onRetry={error.code === "busy" ? remove : undefined}
              extraAction={
                OPEN_FOLDER.includes(error.code)
                  ? { label: t("instances.openFolder"), onClick: openFolder }
                  : undefined
              }
            />
          )}
          {folderCopy && (
            <div role="alert" className="flex items-start gap-2 text-small">
              <CircleAlertIcon className="mt-0.5 size-4 shrink-0 text-danger" aria-hidden="true" />
              <span className="flex flex-col">
                <span className="text-danger">{folderCopy.title}</span>
                <span className="text-muted-foreground">{folderCopy.body}</span>
              </span>
            </div>
          )}
        </AlertDialogBody>
        <AlertDialogFooter>
          {settled ? (
            <Button ref={keepRef} onClick={close}>
              {t("common.close")}
            </Button>
          ) : (
            <>
              <Button ref={keepRef} aria-disabled={busy || undefined} onClick={close}>
                {t("common.keep")}
              </Button>
              <Button
                variant="destructive"
                aria-disabled={busy || undefined}
                onClick={() => void remove()}
              >
                {busy ? <LoaderCircleIcon className="spinner size-4" aria-hidden="true" /> : null}
                {busy ? t("instances.deleteDialog.busy") : t("instances.deleteDialog.confirm")}
              </Button>
            </>
          )}
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
