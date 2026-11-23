import { CircleAlertIcon, ExternalLinkIcon } from "lucide-react";
import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import {
  AlertDialog,
  AlertDialogBody,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { chipRef } from "./AccountChip";
import { InfoBlock, useOpenUrl } from "./InfoBlock";

/**
 * DESIGN.md 11.6, the second block: this build has no Microsoft client id and cannot sign in. Its
 * own confirmation-sized dialog (440), never the sign-in dialog. In builds of this repository the
 * id is built in, so this only shows in a fork that removed it.
 */
export function NoClientIdDialog({ onClosed }: { onClosed: () => void }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(true);
  const closeRef = useRef<HTMLButtonElement>(null);
  const help = useOpenUrl("buildHelp");
  return (
    <AlertDialog
      open={open}
      onOpenChange={(next) => {
        if (!next) setOpen(false);
      }}
      onOpenChangeComplete={(isOpen) => {
        if (!isOpen) onClosed();
      }}
    >
      <AlertDialogContent initialFocus={closeRef} finalFocus={chipRef}>
        <AlertDialogBody>
          <InfoBlock title={<AlertDialogTitle>{t("signIn.noClientId.title")}</AlertDialogTitle>}>
            <AlertDialogDescription>{t("signIn.noClientId.body")}</AlertDialogDescription>
            {help.failed && (
              <p className="flex items-center gap-2 text-small text-danger">
                <CircleAlertIcon className="size-4 shrink-0" aria-hidden="true" />
                {t("errors.openFailed.title")}
              </p>
            )}
          </InfoBlock>
        </AlertDialogBody>
        <AlertDialogFooter>
          <Button ref={closeRef} onClick={() => setOpen(false)}>
            {t("common.close")}
          </Button>
          <Button variant="primary" onClick={help.open}>
            {t("signIn.noClientId.help")}
            <ExternalLinkIcon aria-hidden="true" />
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
