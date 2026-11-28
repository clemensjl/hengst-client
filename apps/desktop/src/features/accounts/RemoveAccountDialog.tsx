import { LoaderCircleIcon } from "lucide-react";
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
import { ErrorPanel } from "@/features/errors/ErrorPanel";
import type { AccountDto } from "@/ipc/bindings/AccountDto";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";
import { api, IpcError } from "@/ipc/client";
import { isolate } from "@/lib/bidi";
import { useApp } from "@/stores/app";
import { chipRef } from "./AccountChip";

/** DESIGN.md 11.4: a confirmation with named buttons; the safe one holds the first focus. */
export function RemoveAccountDialog({
  account,
  onClosed,
}: {
  account: AccountDto;
  onClosed: () => void;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ErrorDto | null>(null);
  const keepRef = useRef<HTMLButtonElement>(null);
  const loadAccounts = useApp((s) => s.loadAccounts);
  // Before the first await, so a double click removes once.
  const running = useRef(false);

  const remove = async () => {
    if (running.current) return;
    running.current = true;
    setBusy(true);
    setError(null);
    try {
      await api.accountRemove(account.id);
      toast(t("accounts.signedOut", { name: isolate(account.name) }));
      setOpen(false);
    } catch (e) {
      setError(
        e instanceof IpcError
          ? e.dto
          : { code: "internal", params: {}, action: null, detail: String(e) },
      );
    } finally {
      running.current = false;
      setBusy(false);
      void loadAccounts();
    }
  };

  return (
    <AlertDialog
      open={open}
      onOpenChange={(next, details) => {
        // Escape and a press outside keep the account; nothing closes while it is being removed.
        if (!next && !busy) setOpen(false);
        else if (!next) details.cancel();
      }}
      onOpenChangeComplete={(isOpen) => {
        if (!isOpen) onClosed();
      }}
    >
      <AlertDialogContent initialFocus={keepRef} finalFocus={chipRef}>
        <AlertDialogTitle className="pr-0">
          {t("accounts.remove.title", { name: isolate(account.name) })}
        </AlertDialogTitle>
        <AlertDialogBody className="flex flex-col gap-4">
          <AlertDialogDescription>{t("accounts.remove.body")}</AlertDialogDescription>
          {error && <ErrorPanel error={error} autoFocus onRetry={remove} />}
        </AlertDialogBody>
        <AlertDialogFooter>
          <Button
            ref={keepRef}
            aria-disabled={busy || undefined}
            onClick={() => !busy && setOpen(false)}
          >
            {t("common.keep")}
          </Button>
          <Button
            variant="destructive"
            aria-disabled={busy || undefined}
            onClick={() => void remove()}
          >
            {busy ? <LoaderCircleIcon className="spinner size-4" aria-hidden="true" /> : null}
            {busy ? t("accounts.remove.busy") : t("accounts.signOut")}
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
