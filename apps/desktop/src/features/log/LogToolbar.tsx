import {
  ChevronDownIcon,
  ChevronUpIcon,
  CopyIcon,
  ExternalLinkIcon,
  LoaderCircleIcon,
} from "lucide-react";
import type { ReactNode, Ref } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { LEVEL_GROUPS, type CopyState, type LevelGroup, type Levels } from "./logStore";

const SEARCH_ID = "log-search";
const HITS_ID = "log-search-hits";

/** An icon-only button with its name as tooltip (DESIGN.md 6, item 4). */
function IconButton({
  label,
  icon,
  inert,
  onClick,
}: {
  label: string;
  icon: ReactNode;
  inert: boolean;
  onClick: () => void;
}) {
  const button = (
    <Button
      variant="ghost"
      size="icon"
      aria-label={label}
      // Without matches there is nothing to step to; the count next to it says why.
      aria-disabled={inert || undefined}
      aria-describedby={inert ? HITS_ID : undefined}
      onClick={() => {
        if (!inert) onClick();
      }}
    >
      {icon}
    </Button>
  );
  return (
    <Tooltip>
      <TooltipTrigger render={button} />
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}

/** A label that keeps the width of `idle` while it shows another text (DESIGN.md 10.1). */
function SteadyLabel({ idle, shown }: { idle: string; shown: string }) {
  return (
    <span className="grid">
      <span aria-hidden="true" className="invisible col-start-1 row-start-1">
        {idle}
      </span>
      <span className="col-start-1 row-start-1">{shown}</span>
    </span>
  );
}

export interface LogToolbarProps {
  query: string;
  onQuery: (query: string) => void;
  /** Matches of the query; `null` while there is no query. */
  hits: number | null;
  onStep: (direction: 1 | -1) => void;
  levels: Levels;
  onLevel: (group: LevelGroup, on: boolean) => void;
  copy: CopyState;
  onCopy: () => void;
  onOpenFile: () => void;
  searchRef: Ref<HTMLInputElement>;
}

/**
 * DESIGN.md 10.13: row 1 search (label, field, count, previous, next), row 2 levels on the left
 * and copy and open file on the right; one row from 1280 px (`full:`). A row that does not fit
 * wraps instead of being cut (10.1).
 */
export function LogToolbar({
  query,
  onQuery,
  hits,
  onStep,
  levels,
  onLevel,
  copy,
  onCopy,
  onOpenFile,
  searchRef,
}: LogToolbarProps) {
  const { t } = useTranslation();
  const shown = LEVEL_GROUPS.filter((g) => levels[g]).length;
  const noHits = !hits;
  const copyLabel =
    copy.status === "busy"
      ? t("log.copying")
      : copy.status === "copied"
        ? t("log.copied")
        : t("log.copy");
  return (
    <div className="flex flex-col gap-2 full:flex-row full:items-center full:justify-between">
      <div className="flex flex-wrap items-center gap-2">
        <label htmlFor={SEARCH_ID} className="text-label">
          {t("log.search.label")}
        </label>
        <div className="w-60">
          <Input
            ref={searchRef}
            id={SEARCH_ID}
            type="text"
            maxLength={256}
            value={query}
            aria-describedby={HITS_ID}
            onChange={(e) => onQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                onStep(e.shiftKey ? -1 : 1);
              } else if (e.key === "Escape" && query !== "") {
                // Escape clears the search and stays in the field.
                e.preventDefault();
                onQuery("");
              }
            }}
          />
        </div>
        <span id={HITS_ID} className="min-w-16 text-small text-muted-foreground tabular-nums">
          {hits === null ? "" : t("log.search.hits", { count: hits })}
        </span>
        <IconButton
          label={t("log.search.previous")}
          icon={<ChevronUpIcon aria-hidden="true" />}
          inert={noHits}
          onClick={() => onStep(-1)}
        />
        <IconButton
          label={t("log.search.next")}
          icon={<ChevronDownIcon aria-hidden="true" />}
          inert={noHits}
          onClick={() => onStep(1)}
        />
      </div>
      <div className="flex flex-wrap items-center gap-2 full:flex-nowrap">
        <DropdownMenu>
          <DropdownMenuTrigger render={<Button variant="secondary" />}>
            {t("log.levels.label", { shown, total: LEVEL_GROUPS.length })}
            <ChevronDownIcon aria-hidden="true" />
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start">
            {LEVEL_GROUPS.map((g) => (
              <DropdownMenuCheckboxItem
                key={g}
                checked={levels[g]}
                onCheckedChange={(on) => onLevel(g, on)}
              >
                {t(`log.levels.${g}`)}
              </DropdownMenuCheckboxItem>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>
        <div className="ml-auto flex flex-wrap items-center gap-2">
          <Button
            variant="secondary"
            aria-disabled={copy.status === "busy" || undefined}
            onClick={onCopy}
          >
            {copy.status === "busy" ? (
              <LoaderCircleIcon className="spinner" aria-hidden="true" />
            ) : (
              <CopyIcon aria-hidden="true" />
            )}
            <SteadyLabel idle={t("log.copy")} shown={copyLabel} />
          </Button>
          <span role="status" className="sr-only">
            {copy.status === "copied" ? t("log.copied") : ""}
          </span>
          <Button variant="ghost" onClick={onOpenFile}>
            <ExternalLinkIcon aria-hidden="true" />
            {t("log.openFile")}
          </Button>
        </div>
      </div>
    </div>
  );
}
