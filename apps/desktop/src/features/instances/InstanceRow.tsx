import { CheckIcon, EllipsisIcon, FolderOpenIcon, Trash2Icon } from "lucide-react";
import { useId, useRef } from "react";
import { useTranslation } from "react-i18next";
import { Badge } from "@/components/Badge";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { InstanceDto } from "@/ipc/bindings/InstanceDto";
import { isolate } from "@/lib/bidi";
import { cn } from "@/lib/utils";
import { useTruncated } from "@/lib/useTruncated";
import { lastPlayedText, loaderText } from "./format";

/**
 * DESIGN.md 10.4 and 7.3. The regular grid: check slot, name, version, loader, last played and the
 * action column; the narrow one (below 880 CSS px): check slot, name with version and loader as
 * two spans below it, action column.
 */
export const ROW_GRID = "grid grid-cols-[24px_minmax(0,1fr)_128px_168px_136px_40px] gap-4 px-3";
export const NARROW_GRID = "grid grid-cols-[24px_minmax(0,1fr)_40px] gap-4 px-3";

interface Props {
  instance: InstanceDto;
  selected: boolean;
  narrow: boolean;
  /** The instance is being prepared or runs: it cannot be deleted now. */
  running: boolean;
  onSelect: (id: string) => void;
  onOpenFolder: (id: string) => void;
  /** Asks to delete; `from` is where focus returns when the dialog is left without deleting. */
  onDelete: (instance: InstanceDto, from: HTMLElement | null) => void;
  /** Hands the row button and the action button to the view, which moves focus between rows. */
  register: (id: string, part: "row" | "action", el: HTMLElement | null) => void;
}

export function InstanceRow({
  instance,
  selected,
  narrow,
  running,
  onSelect,
  onOpenFolder,
  onDelete,
  register,
}: Props) {
  const { t, i18n } = useTranslation();
  const reasonId = useId();
  const nameRef = useRef<HTMLSpanElement>(null);
  const actionRef = useRef<HTMLElement | null>(null);
  const truncated = useTruncated(nameRef, instance.name);
  const readable = instance.status === "ready";
  // Product decision: an instance saved by a newer build is never deleted from this one (its
  // folder can be removed in Explorer); an unreadable one may go to the Recycle Bin.
  const deletable = instance.status !== "tooNew";
  const badge =
    instance.status === "tooNew"
      ? t("instances.badge.tooNew")
      : instance.status === "broken"
        ? t("instances.badge.broken")
        : null;
  // Version ids can be long ("1.14 Pre-Release 1"): one line, cut with an ellipsis.
  const version = (
    <span className="min-w-0 truncate text-small text-muted-foreground tabular-nums">
      {instance.game}
    </span>
  );
  const loader = (
    <span className="truncate text-small text-muted-foreground">{loaderText(instance, t)}</span>
  );
  // The badge is part of the row's name; it is not also its description (read twice otherwise).
  const warning = badge && (
    <span>
      <Badge>{badge}</Badge>
    </span>
  );
  // Why "Delete" cannot be used, shown under it in the menu and read as its description.
  const deleteBlocked = !deletable
    ? t("instances.tooNewHint")
    : running
      ? t("instances.running")
      : null;
  const askDelete = (from: HTMLElement | null) => {
    if (deletable && !running) onDelete(instance, from);
  };

  return (
    <li className="relative" data-instance={instance.id}>
      <Tooltip disabled={!truncated}>
        <TooltipTrigger
          render={
            <button
              type="button"
              ref={(el) => register(instance.id, "row", el)}
              data-instance-row={instance.id}
              aria-current={selected ? "true" : undefined}
              aria-disabled={readable ? undefined : true}
              onClick={() => readable && onSelect(instance.id)}
              onKeyDown={(e) => {
                if (e.key === "Delete") {
                  e.preventDefault();
                  askDelete(e.currentTarget);
                }
              }}
              // Selection and hover never animate (DESIGN.md 8).
              className={cn(
                narrow ? NARROW_GRID : ROW_GRID,
                narrow ? "h-16" : "h-12",
                "w-full items-center rounded-md text-left select-none",
                selected ? "bg-selected" : readable ? "hover:bg-hover active:bg-selected" : "",
              )}
            />
          }
        >
          <span data-slot="check" className="flex size-6 items-center justify-center">
            {selected && <CheckIcon className="size-[18px]" aria-hidden="true" />}
          </span>
          {narrow ? (
            <span className="flex min-w-0 flex-col gap-1">
              <span
                ref={nameRef}
                className={cn("truncate text-body", readable ? "" : "text-muted-foreground")}
              >
                {instance.name}
              </span>
              {warning ?? (
                <span className="flex min-w-0 gap-4">
                  {version}
                  {loader}
                </span>
              )}
            </span>
          ) : (
            <>
              <span
                ref={nameRef}
                className={cn("truncate text-body", readable ? "" : "text-muted-foreground")}
              >
                {instance.name}
              </span>
              {warning ? (
                <span className="col-span-2">{warning}</span>
              ) : (
                <>
                  {version}
                  {loader}
                </>
              )}
              <span className="text-right text-caption text-muted-foreground tabular-nums">
                {readable ? lastPlayedText(instance.lastPlayed, i18n.language, t) : ""}
              </span>
            </>
          )}
        </TooltipTrigger>
        <TooltipContent>{instance.name}</TooltipContent>
      </Tooltip>
      <DropdownMenu>
        <Tooltip>
          <TooltipTrigger
            render={
              <DropdownMenuTrigger
                ref={(el: HTMLElement | null) => {
                  actionRef.current = el;
                  register(instance.id, "action", el);
                }}
                aria-label={t("instances.actions", { name: instance.name })}
                data-instance-action={instance.id}
                render={
                  <Button
                    variant="ghost"
                    size="icon"
                    className={cn("absolute right-3", narrow ? "top-3" : "top-1")}
                  />
                }
              />
            }
          >
            <EllipsisIcon aria-hidden="true" />
          </TooltipTrigger>
          <TooltipContent>
            {t("instances.actions", { name: isolate(instance.name) })}
          </TooltipContent>
        </Tooltip>
        <DropdownMenuContent align="end">
          <DropdownMenuItem onClick={() => onOpenFolder(instance.id)}>
            <FolderOpenIcon aria-hidden="true" />
            {t("instances.openFolder")}
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          {/* A disabled item stays reachable with the arrow keys (Base UI), so its reason, a
              second line inside it, is read as its description. */}
          <DropdownMenuItem
            disabled={deleteBlocked !== null}
            aria-label={t("instances.delete")}
            aria-describedby={deleteBlocked ? reasonId : undefined}
            onClick={() => askDelete(actionRef.current)}
          >
            <span className="flex min-w-0 flex-col py-2">
              <span className={cn("flex items-center gap-2", deleteBlocked ? "" : "text-danger")}>
                <Trash2Icon className="size-4" aria-hidden="true" />
                {t("instances.delete")}
              </span>
              {deleteBlocked && (
                <span
                  id={reasonId}
                  className="max-w-80 pl-6 text-caption whitespace-normal text-muted-foreground"
                >
                  {deleteBlocked}
                </span>
              )}
            </span>
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </li>
  );
}
