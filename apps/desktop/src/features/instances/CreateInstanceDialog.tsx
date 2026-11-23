import { CircleAlertIcon, LoaderCircleIcon, TriangleAlertIcon, WifiOffIcon } from "lucide-react";
import { useCallback, useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Segments, type SegmentOption } from "@/components/Segments";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Combobox,
  ComboboxContent,
  ComboboxEmpty,
  ComboboxItem,
  ComboboxList,
  ComboboxSearch,
  ComboboxTrigger,
} from "@/components/ui/combobox";
import {
  Dialog,
  DialogBody,
  DialogContent,
  DialogFooter,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectItemText,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Badge } from "@/components/Badge";
import { describeError } from "@/features/errors/errorCopy";
import { ErrorPanel } from "@/features/errors/ErrorPanel";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";
import type { InstanceDto } from "@/ipc/bindings/InstanceDto";
import type { LoaderInfoDto } from "@/ipc/bindings/LoaderInfoDto";
import type { LoaderKindDto } from "@/ipc/bindings/LoaderKindDto";
import type { LoaderVersionsDto } from "@/ipc/bindings/LoaderVersionsDto";
import type { VersionFilter } from "@/ipc/bindings/VersionFilter";
import { api, IpcError } from "@/ipc/client";
import { useApp, type InstanceDraft } from "@/stores/app";
import { MAX_NAME, nameLength, nameProblem, suggestName, trimName } from "./naming";
import { DEFAULT_FILTER, filterVersions } from "./versionFilter";

const errorOf = (e: unknown): ErrorDto =>
  e instanceof IpcError ? e.dto : { code: "internal", params: {}, action: null, detail: String(e) };

/** The whole version list; the dialog filters it itself, so a filter change needs no request. */
const ALL_KINDS: VersionFilter = { release: true, snapshot: true, beta: true, alpha: true };
const KINDS = ["release", "snapshot", "beta", "alpha"] as const;
/** The codes that mean "no connection" (DESIGN.md 10.5, 10.6). */
const OFFLINE = ["offline", "network"];
/** DESIGN.md 10.7: the counter shows from 80 % of the limit. */
const COUNTER_FROM = Math.ceil(MAX_NAME * 0.8);

const NEW_DRAFT: InstanceDraft = {
  game: null,
  filter: DEFAULT_FILTER,
  loader: "none",
  loaderVersion: null,
  name: "",
  nameEdited: false,
};

interface LoaderVersions {
  /** `<kind>|<game>`: an answer for another choice is dropped. */
  key: string;
  status: "loading" | "ready" | "error";
  data?: LoaderVersionsDto;
  error?: ErrorDto;
}

/** An inline error with "Try again" as a link button (DESIGN.md 10.5, 10.11). */
function InlineError({ id, text, onRetry }: { id: string; text: string; onRetry: () => void }) {
  const { t } = useTranslation();
  return (
    <div className="flex flex-wrap items-center gap-x-4">
      <p id={id} role="alert" className="flex items-center gap-2 text-small text-danger">
        <CircleAlertIcon className="size-4 shrink-0" aria-hidden="true" />
        {text}
      </p>
      <Button variant="link" onClick={onRetry}>
        {t("common.retry")}
      </Button>
    </div>
  );
}

function RecommendedBadge({ children }: { children: string }) {
  return (
    <span className="inline-flex h-5 items-center rounded-pill bg-hover px-2 text-badge whitespace-nowrap text-foreground">
      {children}
    </span>
  );
}

/** DESIGN.md 11.2, 10.5 to 10.7. Width 560; a press outside never closes it. */
export function CreateInstanceDialog() {
  const { t } = useTranslation();
  const open = useApp((s) => s.createOpen);
  const closeCreate = useApp((s) => s.closeCreate);
  const [busy, setBusy] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  // Created: focus goes to the new row (the list moves it there), not back to the trigger.
  const created = useRef(false);

  const onCreated = (made: InstanceDto) => {
    const app = useApp.getState();
    created.current = true;
    app.setInstanceDraft(null);
    app.setInstanceFocus({ kind: "row", id: made.id });
    toast(t("instances.created", { name: made.name }));
    closeCreate();
    void app.loadInstances();
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(next, details) => {
        if (next) return;
        // Escape and the close button keep the draft; a press outside never closes; nothing
        // closes while the instance is being created.
        if (details.reason === "outside-press" || busy) {
          details.cancel();
          return;
        }
        closeCreate();
      }}
      onOpenChangeComplete={(isOpen) => {
        if (isOpen) created.current = false;
      }}
    >
      <DialogContent
        className="w-140"
        closeLabel={t("common.close")}
        closeBusyReason={busy ? t("instances.form.closeBusy") : undefined}
        // The version trigger, also while the list loads: it is only aria-disabled then, so it
        // can hold focus from the first moment (DESIGN.md 11.2).
        initialFocus={triggerRef}
        finalFocus={() => !created.current}
      >
        <DialogTitle>{t("instances.create")}</DialogTitle>
        {/* Mounted with the panel: each opening starts from the draft. */}
        <CreateForm
          triggerRef={triggerRef}
          busy={busy}
          setBusy={setBusy}
          onCancel={closeCreate}
          onCreated={onCreated}
        />
      </DialogContent>
    </Dialog>
  );
}

function CreateForm({
  triggerRef,
  busy,
  setBusy,
  onCancel,
  onCreated,
}: {
  triggerRef: React.RefObject<HTMLButtonElement | null>;
  busy: boolean;
  setBusy: (busy: boolean) => void;
  onCancel: () => void;
  onCreated: (made: InstanceDto) => void;
}) {
  const { t } = useTranslation();
  const id = useId();
  const ids = {
    versionLabel: `${id}-vl`,
    version: `${id}-v`,
    versionNote: `${id}-vn`,
    loaderLabel: `${id}-ll`,
    loaderKinds: `${id}-lk`,
    loaderVersionLabel: `${id}-lvl`,
    loaderVersion: `${id}-lv`,
    loaderNote: `${id}-ln`,
    name: `${id}-n`,
    nameHelp: `${id}-nh`,
  };
  const versions = useApp((s) => s.versions);
  const stale = useApp((s) => s.versionsStale);
  const loadVersions = useApp((s) => s.loadVersions);
  const draft = useApp((s) => s.instanceDraft) ?? NEW_DRAFT;
  const setDraft = useApp((s) => s.setInstanceDraft);
  const update = (change: Partial<InstanceDraft>) =>
    setDraft({ ...(useApp.getState().instanceDraft ?? NEW_DRAFT), ...change });

  const [query, setQuery] = useState("");
  const [kinds, setKinds] = useState<LoaderInfoDto[] | null>(null);
  const [kindsError, setKindsError] = useState<ErrorDto | null>(null);
  const [loaderVersions, setLoaderVersions] = useState<LoaderVersions | null>(null);
  const [validated, setValidated] = useState(false);
  const [error, setError] = useState<ErrorDto | null>(null);
  const nameRef = useRef<HTMLInputElement>(null);
  // Before the first await: the second click of a double click creates nothing.
  const running = useRef(false);

  useEffect(() => {
    const s = useApp.getState().versions;
    if (s.status !== "ready" && s.status !== "loading") void loadVersions(ALL_KINDS);
  }, [loadVersions]);

  // Mounted: false once the dialog went away, so a late answer changes nothing.
  const mounted = useRef(true);
  useEffect(
    () => () => {
      mounted.current = false;
    },
    [],
  );
  const loadKinds = useCallback(() => {
    setKindsError(null);
    api.loaderKinds().then(
      (k) => mounted.current && setKinds(k),
      (e: unknown) => mounted.current && setKindsError(errorOf(e)),
    );
  }, []);
  useEffect(() => loadKinds(), [loadKinds]);
  const [versionsOpen, setVersionsOpen] = useState(false);

  const all = versions.data?.versions ?? [];
  const kindOf = new Map(all.map((v) => [v.id, v.kind]));
  const game = draft.game ?? versions.data?.latestRelease ?? null;
  const loaderInfo =
    draft.loader === "none" ? null : (kinds?.find((k) => k.kind === draft.loader) ?? null);
  const key = draft.loader !== "none" && game ? `${draft.loader}|${game}` : null;

  const fetchLoaderVersions = useCallback((kind: LoaderKindDto, forGame: string, k: string) => {
    setLoaderVersions({ key: k, status: "loading" });
    api.loaderVersions(kind, forGame).then(
      (data) =>
        setLoaderVersions((cur) => (cur?.key === k ? { key: k, status: "ready", data } : cur)),
      (e: unknown) =>
        setLoaderVersions((cur) =>
          cur?.key === k ? { key: k, status: "error", error: errorOf(e) } : cur,
        ),
    );
  }, []);

  useEffect(() => {
    if (key && game && draft.loader !== "none" && loaderVersions?.key !== key) {
      fetchLoaderVersions(draft.loader, game, key);
    }
  }, [key, game, draft.loader, loaderVersions?.key, fetchLoaderVersions]);

  const current = key && loaderVersions?.key === key ? loaderVersions : null;
  const ready = current?.status === "ready" ? current.data! : null;
  const loaderVersion = ready
    ? draft.loaderVersion && ready.versions.some((v) => v.id === draft.loaderVersion)
      ? draft.loaderVersion
      : ready.recommended
    : null;

  const name = draft.nameEdited ? draft.name : game ? suggestName(game, loaderInfo) : "";
  const problem = nameProblem(name);
  const shownProblem = validated ? problem : null;
  // The check and the counter count the same string the backend keeps.
  const count = nameLength(name);

  // Why "Create instance" cannot be used now: the id of the text that says so (DESIGN.md 10.0).
  let blockedBy: string | null = null;
  if (!versions.data) blockedBy = versions.status === "error" ? ids.versionNote : ids.version;
  else if (!game) blockedBy = ids.version;
  else if (draft.loader !== "none" && !ready)
    blockedBy = current?.status === "error" ? ids.loaderNote : ids.loaderVersion;

  const submit = async () => {
    if (running.current || blockedBy || !game) return;
    if (problem) {
      setValidated(true);
      nameRef.current?.focus();
      return;
    }
    running.current = true;
    setBusy(true);
    setError(null);
    try {
      const made = await api.instanceCreate({
        name: trimName(name),
        game,
        // Slugs and versions only; the display name stays in the backend.
        loader: draft.loader === "none" ? null : { kind: draft.loader, version: loaderVersion },
      });
      onCreated(made);
    } catch (e) {
      setError(errorOf(e));
    } finally {
      running.current = false;
      setBusy(false);
    }
  };

  const filtered = filterVersions(all, draft.filter, query);
  const filteredIds = filtered.map((v) => v.id);
  // "Reset filters" only where resetting shows more.
  const resetHelps = filterVersions(all, DEFAULT_FILTER, query).length > filtered.length;

  const loaders: SegmentOption<LoaderKindDto | "none">[] = [
    { value: "none", label: t("instances.noLoader") },
    ...(kinds ?? []).map((k) => ({ value: k.kind, label: k.displayName })),
  ];

  const loaderError = current?.status === "error" ? current.error! : null;
  const noBuilds = loaderError?.code === "noLoaderVersions" ? describeError(loaderError, t) : null;

  return (
    <>
      <DialogBody className="flex flex-col gap-4">
        <div className="flex flex-col gap-2">
          <Label id={ids.versionLabel}>{t("instances.form.version")}</Label>
          <Combobox<string>
            items={filteredIds}
            filteredItems={filteredIds}
            value={game}
            onValueChange={(v) => {
              if (typeof v === "string") update({ game: v, loaderVersion: null });
            }}
            inputValue={query}
            onInputValueChange={setQuery}
            // Without a list it does not open: the trigger says why.
            open={versionsOpen && versions.data !== undefined}
            onOpenChange={(next) => setVersionsOpen(next && versions.data !== undefined)}
          >
            <ComboboxTrigger
              ref={triggerRef}
              id={ids.version}
              aria-labelledby={`${ids.versionLabel} ${ids.version}`}
              aria-disabled={versions.data ? undefined : true}
              className="aria-disabled:border-border aria-disabled:bg-card aria-disabled:text-text-disabled aria-disabled:hover:border-border"
            >
              <span className="truncate">
                {!versions.data
                  ? versions.status === "error"
                    ? t("instances.form.chooseVersion")
                    : t("instances.form.versionsLoading")
                  : (game ?? t("instances.form.chooseVersion"))}
              </span>
            </ComboboxTrigger>
            <ComboboxContent>
              <ComboboxSearch
                placeholder={t("instances.form.search")}
                aria-labelledby={ids.versionLabel}
              />
              {/* The empty text is a live region; the button stays outside it. */}
              <ComboboxEmpty>
                <p className="px-3 py-2 text-small text-muted-foreground [overflow-wrap:anywhere]">
                  {query.trim()
                    ? t("instances.form.noMatch", { query: query.trim() })
                    : t("instances.form.allFiltered")}
                </p>
              </ComboboxEmpty>
              {filteredIds.length === 0 && resetHelps && (
                <Button
                  variant="ghost"
                  className="self-start"
                  onClick={() => update({ filter: DEFAULT_FILTER })}
                >
                  {t("instances.form.resetFilters")}
                </Button>
              )}
              <ComboboxList>
                {(v: string) => (
                  <ComboboxItem key={v} value={v}>
                    <span className="min-w-0 truncate tabular-nums">{v}</span>
                    {kindOf.get(v) !== "release" && (
                      <span className="ml-auto text-caption text-muted-foreground">
                        {t(`instances.form.kind.${kindOf.get(v)}`)}
                      </span>
                    )}
                  </ComboboxItem>
                )}
              </ComboboxList>
            </ComboboxContent>
          </Combobox>
          {versions.status === "error" && !versions.data && versions.error ? (
            <InlineError
              id={ids.versionNote}
              text={
                OFFLINE.includes(versions.error.code)
                  ? t("errors.versionsOffline")
                  : t("errors.versionsFailed")
              }
              onRetry={() => void loadVersions(ALL_KINDS)}
            />
          ) : versions.data && stale ? (
            <p className="flex items-center gap-2 text-caption text-muted-foreground">
              {stale === "offline" && (
                <WifiOffIcon className="size-4 shrink-0" aria-hidden="true" />
              )}
              {stale === "offline"
                ? t("instances.form.offlineCached")
                : t("instances.form.staleCached")}
            </p>
          ) : null}
          <div
            role="group"
            aria-label={t("instances.form.filters")}
            className="flex flex-wrap gap-x-4"
          >
            {KINDS.map((k) => (
              <label key={k} className="flex h-10 items-center gap-2 text-small select-none">
                <Checkbox
                  checked={draft.filter[k]}
                  onCheckedChange={(checked) =>
                    update({ filter: { ...draft.filter, [k]: checked === true } })
                  }
                />
                {t(`instances.form.filter.${k}`)}
              </label>
            ))}
          </div>
        </div>

        <div className="flex flex-col gap-2">
          <Label id={ids.loaderLabel}>{t("instances.form.loader")}</Label>
          <Segments
            value={draft.loader}
            options={loaders}
            onChange={(v) => update({ loader: v, loaderVersion: null })}
            labelledBy={ids.loaderLabel}
            className="flex flex-wrap gap-2"
          />
          {kindsError && (
            <InlineError
              id={ids.loaderKinds}
              text={t("instances.form.loaderKindsFailed")}
              onRetry={loadKinds}
            />
          )}
        </div>

        {draft.loader !== "none" && (
          <div className="flex flex-col gap-2">
            <Label id={ids.loaderVersionLabel}>{t("instances.form.loaderVersion")}</Label>
            {ready && loaderVersion ? (
              <Select<string>
                // Its own instance: the loading select below is uncontrolled.
                key={`ready-${key}`}
                items={ready.versions.map((v) => ({ value: v.id, label: v.id }))}
                value={loaderVersion}
                onValueChange={(v) => {
                  if (typeof v === "string") update({ loaderVersion: v });
                }}
              >
                <SelectTrigger
                  id={ids.loaderVersion}
                  aria-labelledby={`${ids.loaderVersionLabel} ${ids.loaderVersion}`}
                >
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {ready.versions.map((v) => (
                    <SelectItem key={v.id} value={v.id}>
                      <SelectItemText className="min-w-0 truncate tabular-nums">
                        {v.id}
                      </SelectItemText>
                      {v.id === ready.recommended ? (
                        <RecommendedBadge>{t("instances.form.recommended")}</RecommendedBadge>
                      ) : !v.stable ? (
                        <Badge>{t("instances.form.unstable")}</Badge>
                      ) : null}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            ) : noBuilds ? (
              <div id={ids.loaderNote} className="flex items-start gap-2 text-small">
                <TriangleAlertIcon
                  className="mt-0.5 size-4 shrink-0 text-warning"
                  aria-hidden="true"
                />
                <span className="flex flex-col">
                  <span className="text-foreground">{noBuilds.title}</span>
                  <span className="text-muted-foreground">{noBuilds.body}</span>
                </span>
              </div>
            ) : loaderError ? (
              <InlineError
                id={ids.loaderNote}
                text={
                  OFFLINE.includes(loaderError.code)
                    ? t("instances.form.loaderOffline")
                    : t("instances.form.loaderFailed")
                }
                onRetry={() =>
                  key &&
                  game &&
                  draft.loader !== "none" &&
                  fetchLoaderVersions(draft.loader, game, key)
                }
              />
            ) : (
              <Select key="loading" disabled>
                <SelectTrigger
                  id={ids.loaderVersion}
                  aria-labelledby={`${ids.loaderVersionLabel} ${ids.loaderVersion}`}
                >
                  <span className="truncate">{t("instances.form.loaderLoading")}</span>
                </SelectTrigger>
              </Select>
            )}
          </div>
        )}

        <div className="flex flex-col gap-2">
          <Label htmlFor={ids.name}>{t("instances.form.name")}</Label>
          <Input
            ref={nameRef}
            id={ids.name}
            value={name}
            aria-invalid={shownProblem ? true : undefined}
            aria-describedby={ids.nameHelp}
            onChange={(e) => {
              // A pasted line break never becomes part of the name; any other control character
              // (a tab) is refused by the check, as by the backend.
              update({ name: e.target.value.replace(/[\r\n]+/g, ""), nameEdited: true });
            }}
            onBlur={() => {
              if (problem) setValidated(true);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                void submit();
              }
            }}
          />
          <p id={ids.nameHelp} className="flex h-5 items-center gap-2 text-small">
            {shownProblem ? (
              <span className="flex min-w-0 items-center gap-2 text-danger">
                <CircleAlertIcon className="size-4 shrink-0" aria-hidden="true" />
                {t(shownProblem)}
              </span>
            ) : (
              <span className="min-w-0 truncate text-muted-foreground">
                {t("instances.form.nameHelp")}
              </span>
            )}
            {count >= COUNTER_FROM && (
              <span className="ml-auto text-muted-foreground tabular-nums">
                {count}/{MAX_NAME}
              </span>
            )}
          </p>
        </div>

        {error && <ErrorPanel error={error} autoFocus onRetry={submit} />}
      </DialogBody>
      <DialogFooter>
        <Button aria-disabled={busy || undefined} onClick={() => !busy && onCancel()}>
          {t("common.cancel")}
        </Button>
        <Button
          variant="primary"
          aria-disabled={blockedBy || busy ? true : undefined}
          aria-describedby={blockedBy ?? undefined}
          className="aria-disabled:border-border aria-disabled:bg-hover aria-disabled:text-text-disabled aria-disabled:hover:bg-hover aria-disabled:active:bg-hover"
          onClick={() => void submit()}
        >
          {busy && <LoaderCircleIcon className="spinner size-4" aria-hidden="true" />}
          {busy ? t("instances.form.creating") : t("instances.create")}
        </Button>
      </DialogFooter>
    </>
  );
}

/** For `React.lazy`: the dialog loads as its own chunk. */
export default CreateInstanceDialog;
