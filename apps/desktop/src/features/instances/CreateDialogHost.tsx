import { CircleAlertIcon, RefreshCwIcon } from "lucide-react";
import { Component, lazy, Suspense, useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { useApp } from "@/stores/app";

/**
 * The create dialog is its own chunk (combobox, select and checkbox stay out of the start). A view
 * that can open it asks for it as soon as it shows, so it is there by the time it is opened.
 */
const loadCreateDialog = () => import("./CreateInstanceDialog");
const CreateInstanceDialog = lazy(loadCreateDialog);

/**
 * Catches any error while the create dialog renders: a chunk that cannot be loaded, and also an
 * error thrown by the dialog itself. Without it React would unmount the whole app. It renders
 * nothing and tells the view, which closes the dialog and says the dialog could not be loaded.
 */
class ChunkBoundary extends Component<{ onError: () => void; children: ReactNode }> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  componentDidCatch() {
    this.props.onError();
  }
  render() {
    return this.state.failed ? null : this.props.children;
  }
}

/**
 * The create dialog for a view that opens it (Instances, the start page): `dialog` goes anywhere
 * in the view (it is a layer), `failure` where the view shows errors.
 */
export function useCreateDialogHost(): { dialog: ReactNode; failure: ReactNode } {
  const { t } = useTranslation();
  const createOpen = useApp((s) => s.createOpen);
  const closeCreate = useApp((s) => s.closeCreate);
  const openCreate = useApp((s) => s.openCreate);
  // Mounted from the first opening on, so its closing animation and its draft handling run.
  const [createWanted, setCreateWanted] = useState(false);
  // The dialog's chunk could not be loaded. A failed lazy component keeps its rejection, so the
  // failure puts a fresh one in place: the next opening (Try again, or any "New instance") loads
  // the chunk again, and the old failure goes away until that attempt fails too.
  const [chunkFailed, setChunkFailed] = useState(false);
  if (createOpen && !createWanted) {
    setCreateWanted(true);
    setChunkFailed(false);
  }
  const [CreateDialog, setCreateDialog] = useState(() => CreateInstanceDialog);
  const chunkTitle = useRef<HTMLParagraphElement>(null);
  useEffect(() => {
    if (!chunkFailed) return;
    // As after a failed create (DESIGN.md 11.2): the focus goes to the title of the failure.
    chunkTitle.current?.focus();
    chunkTitle.current?.scrollIntoView?.({ block: "nearest" });
  }, [chunkFailed]);
  useEffect(() => {
    // A failure here shows when the dialog is opened (the boundary below).
    loadCreateDialog().catch(() => {});
  }, []);

  const failure = chunkFailed && (
    <div role="alert" className="flex flex-col gap-3 rounded-lg border bg-surface-raised p-4">
      <div className="flex items-start gap-3">
        <CircleAlertIcon className="mt-[3px] size-[18px] shrink-0 text-danger" aria-hidden="true" />
        <div className="flex min-w-0 flex-col">
          <p
            ref={chunkTitle}
            tabIndex={-1}
            className="text-body font-semibold [overflow-wrap:anywhere]"
          >
            {t("instances.createLoadFailed.title")}
          </p>
          <p className="text-small text-muted-foreground">{t("instances.createLoadFailed.body")}</p>
        </div>
      </div>
      <div className="pl-[30px]">
        <Button onClick={openCreate}>
          <RefreshCwIcon aria-hidden="true" />
          {t("common.retry")}
        </Button>
      </div>
    </div>
  );

  const dialog = createWanted && (
    // Nothing to show while the chunk loads (it was asked for when the view appeared); the
    // dialog is a layer over the view, so nothing below moves.
    <ChunkBoundary
      onError={() => {
        setChunkFailed(true);
        setCreateWanted(false);
        setCreateDialog(() => lazy(loadCreateDialog));
        closeCreate();
      }}
    >
      <Suspense fallback={null}>
        <CreateDialog />
      </Suspense>
    </ChunkBoundary>
  );

  return { dialog, failure };
}
