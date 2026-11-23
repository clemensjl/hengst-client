import { PlusIcon, TriangleAlertIcon } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Delayed } from "@/components/Delayed";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { ErrorPanel } from "@/features/errors/ErrorPanel";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";
import type { InstanceDto } from "@/ipc/bindings/InstanceDto";
import { api, IpcError } from "@/ipc/client";
import { cn } from "@/lib/utils";
import { useMediaQuery } from "@/lib/useMediaQuery";
import { useApp, type InstanceFocus } from "@/stores/app";
import { useCreateDialogHost } from "./CreateDialogHost";
import { DeleteInstanceDialog } from "./DeleteInstanceDialog";
import { InstanceRow, NARROW_GRID, ROW_GRID } from "./InstanceRow";
import { effectiveSelection } from "./selection";

const errorOf = (e: unknown): ErrorDto =>
  e instanceof IpcError ? e.dto : { code: "internal", params: {}, action: null, detail: String(e) };

/** Below 880 CSS px (zoom or a narrow work area): the narrow layout of DESIGN.md 7.3. */
const useNarrow = () => useMediaQuery("(width < 880px)");

/**
 * "Create instance" as it is mounted now (the header's, or the empty state's): where focus goes
 * after the last row was deleted (DESIGN.md 13).
 */
const createButton: { current: HTMLElement | null } = { current: null };
const setCreateButton = (el: HTMLElement | null) => {
  if (el) createButton.current = el;
  else if (createButton.current && !createButton.current.isConnected) createButton.current = null;
};

/**
 * The view's primary action in the header (DESIGN.md 11.1). Hidden in the empty state, where the
 * block holds it, so there is one primary per screen; icon-only in the narrow layout.
 */
export function InstancesHeaderAction() {
  const { t } = useTranslation();
  const instances = useApp((s) => s.instances);
  const openCreate = useApp((s) => s.openCreate);
  const narrow = useNarrow();
  const shown = instances.data ? instances.data.length > 0 : instances.status === "error";
  if (!shown) return null;
  const label = t("instances.create");
  if (narrow) {
    return (
      <Tooltip>
        <TooltipTrigger
          render={
            <Button
              ref={setCreateButton}
              variant="primary"
              size="icon"
              aria-label={label}
              onClick={openCreate}
            />
          }
        >
          <PlusIcon aria-hidden="true" />
        </TooltipTrigger>
        <TooltipContent side="bottom">{label}</TooltipContent>
      </Tooltip>
    );
  }
  return (
    <Button ref={setCreateButton} variant="primary" onClick={openCreate}>
      <PlusIcon aria-hidden="true" />
      {label}
    </Button>
  );
}

function ColumnHeader({ narrow }: { narrow: boolean }) {
  const { t } = useTranslation();
  return (
    <div
      aria-hidden="true"
      className={cn(
        narrow ? NARROW_GRID : ROW_GRID,
        "h-8 items-center border-b text-caption text-muted-foreground",
      )}
    >
      <span />
      <span>{t("instances.column.name")}</span>
      {!narrow && (
        <>
          <span>{t("instances.column.version")}</span>
          <span>{t("instances.column.loader")}</span>
          <span className="text-right">{t("instances.column.lastPlayed")}</span>
        </>
      )}
    </div>
  );
}

/** DESIGN.md 11.1 loading: the column header and three static rows, after 200 ms. */
function ListSkeleton({ narrow }: { narrow: boolean }) {
  return (
    <Delayed>
      <div aria-busy="true">
        <ColumnHeader narrow={narrow} />
        {[0, 1, 2].map((i) => (
          <div
            key={i}
            data-slot="skeleton-row"
            className={cn(narrow ? NARROW_GRID : ROW_GRID, "h-12 items-center")}
          >
            <span />
            <Skeleton className="h-5 w-2/5" />
            {!narrow && (
              <>
                <Skeleton className="h-5 w-20" />
                <Skeleton className="h-5 w-30" />
                <Skeleton className="ml-auto h-5 w-24" />
              </>
            )}
          </div>
        ))}
      </div>
    </Delayed>
  );
}

function EmptyState() {
  const { t } = useTranslation();
  const openCreate = useApp((s) => s.openCreate);
  return (
    <div className="flex max-w-120 flex-col items-start">
      <h2 className="text-heading">{t("instances.empty.title")}</h2>
      <p className="text-small text-muted-foreground">{t("instances.empty.body")}</p>
      <Button ref={setCreateButton} variant="primary" className="mt-4" onClick={openCreate}>
        <PlusIcon aria-hidden="true" />
        {t("instances.create")}
      </Button>
    </div>
  );
}

interface DeleteRequest {
  instance: InstanceDto;
  wasSelected: boolean;
  returnTo: HTMLElement | null;
  after: InstanceFocus;
}

/** The next row's action button, else the previous one's, else "Create instance". */
function focusAfterDelete(list: InstanceDto[], id: string): InstanceFocus {
  const at = list.findIndex((i) => i.id === id);
  const neighbour = list[at + 1] ?? list[at - 1];
  return neighbour ? { kind: "action", id: neighbour.id } : { kind: "create" };
}

/** DESIGN.md 10.4 and 11.1. */
export function InstancesView() {
  const { t } = useTranslation();
  const instances = useApp((s) => s.instances);
  const incomplete = useApp((s) => s.instancesIncomplete);
  const active = useApp((s) => s.activeInstance);
  const focus = useApp((s) => s.instanceFocus);
  const loadInstances = useApp((s) => s.loadInstances);
  const setInstanceFocus = useApp((s) => s.setInstanceFocus);
  const narrow = useNarrow();
  const [deleting, setDeleting] = useState<DeleteRequest | null>(null);
  // An action on a row that failed (open folder, select); it shows above the list.
  const [actionError, setActionError] = useState<{ error: ErrorDto; folder: boolean } | null>(null);
  const parts = useRef(new Map<string, { row?: HTMLElement; action?: HTMLElement }>());

  const create = useCreateDialogHost();

  useEffect(() => {
    if (instances.status === "idle") void loadInstances();
  }, [instances.status, loadInstances]);

  const register = useCallback((id: string, part: "row" | "action", el: HTMLElement | null) => {
    const entry = parts.current.get(id) ?? {};
    if (el) entry[part] = el;
    else delete entry[part];
    parts.current.set(id, entry);
  }, []);

  const list = instances.data;
  // Focus that waits for the list: a created row, the neighbour of a deleted one, or Create.
  useEffect(() => {
    if (!focus || !list) return;
    // A created row arrives with the reloaded list: wait for it. The neighbour of a deleted row is
    // in the list already; if it is not (it went away too), Create takes the focus.
    if (focus.kind === "row" && !list.some((i) => i.id === focus.id)) return;
    const el =
      focus.kind === "create"
        ? createButton.current
        : (parts.current.get(focus.id)?.[focus.kind] ?? createButton.current);
    if (!el) return;
    el.scrollIntoView?.({ block: "nearest" });
    el.focus();
    setInstanceFocus(null);
  }, [focus, list, setInstanceFocus]);

  const select = async (id: string) => {
    setActionError(null);
    try {
      await api.instanceSelect(id);
    } catch (e) {
      setActionError({ error: errorOf(e), folder: false });
    }
    await loadInstances();
  };

  const openFolder = async (id: string) => {
    setActionError(null);
    try {
      await api.openLocation({ kind: "instance", id });
    } catch (e) {
      setActionError({ error: errorOf(e), folder: true });
    }
  };

  if (!list) {
    if (instances.status === "error" && instances.error) {
      return <ErrorPanel error={instances.error} onRetry={loadInstances} />;
    }
    return <ListSkeleton narrow={narrow} />;
  }

  const selected = effectiveSelection(list);
  const requestDelete = (instance: InstanceDto, from: HTMLElement | null) =>
    setDeleting({
      instance,
      wasSelected: instance.id === selected,
      returnTo: from,
      after: focusAfterDelete(list, instance.id),
    });

  return (
    <>
      <div className="flex flex-col gap-4">
        {instances.status === "error" && instances.error && (
          <ErrorPanel error={instances.error} onRetry={loadInstances} />
        )}
        {actionError && (
          <ErrorPanel
            error={actionError.error}
            context={actionError.folder ? "folder" : undefined}
            extraAction={{ label: t("common.close"), onClick: () => setActionError(null) }}
          />
        )}
        {create.failure}
        {incomplete && (
          <p className="flex items-center gap-2 text-small text-muted-foreground">
            <TriangleAlertIcon className="size-4 shrink-0 text-warning" aria-hidden="true" />
            {t("instances.incomplete")}
          </p>
        )}
        {list.length === 0 ? (
          <EmptyState />
        ) : (
          <div>
            <ColumnHeader narrow={narrow} />
            <ul aria-label={t("instances.list")} className="flex flex-col">
              {list.map((i) => (
                <InstanceRow
                  key={i.id}
                  instance={i}
                  selected={i.id === selected}
                  narrow={narrow}
                  running={i.id === active}
                  onSelect={(id) => void select(id)}
                  onOpenFolder={(id) => void openFolder(id)}
                  onDelete={requestDelete}
                  register={register}
                />
              ))}
            </ul>
          </div>
        )}
      </div>
      {create.dialog}
      {deleting && (
        <DeleteInstanceDialog
          key={deleting.instance.id}
          instance={deleting.instance}
          wasSelected={deleting.wasSelected}
          returnTo={deleting.returnTo}
          after={deleting.after}
          onClosed={() => setDeleting(null)}
        />
      )}
    </>
  );
}
