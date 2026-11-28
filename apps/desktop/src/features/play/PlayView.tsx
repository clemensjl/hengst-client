import { ChevronDownIcon } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Badge } from "@/components/Badge";
import { Delayed } from "@/components/Delayed";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Skeleton } from "@/components/ui/skeleton";
import { ErrorPanel } from "@/features/errors/ErrorPanel";
import { useLog } from "@/features/log/logStore";
import { useCreateDialogHost } from "@/features/instances/CreateDialogHost";
import { lastPlayedText, loaderText } from "@/features/instances/format";
import { effectiveSelection } from "@/features/instances/selection";
import type { ErrorAction } from "@/ipc/bindings/ErrorAction";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";
import type { GameStateDto } from "@/ipc/bindings/GameStateDto";
import type { InstanceDto } from "@/ipc/bindings/InstanceDto";
import { api, IpcError } from "@/ipc/client";
import { isolate } from "@/lib/bidi";
import { useApp, type Launch, type Resource } from "@/stores/app";
import type { AccountsDto } from "@/ipc/bindings/AccountsDto";
import { PlayButton } from "./PlayButton";
import { playButtonState, type PlayAccount, type PlayButtonState } from "./playState";
import { ProgressPanel } from "./ProgressPanel";

/** DESIGN.md 10.2: "Force stop" joins the stopping button this long after stopping began. */
export const FORCE_STOP_AFTER_MS = 10_000;

const errorOf = (e: unknown): ErrorDto =>
  e instanceof IpcError ? e.dto : { code: "internal", params: {}, action: null, detail: String(e) };

/** The account the game would start with. A failed account list does not block: the backend decides. */
function accountOf(accounts: Resource<AccountsDto>): PlayAccount {
  const list = accounts.data?.accounts;
  if (!list) return { kind: "ok" };
  const selected = list.find((a) => a.selected);
  if (!selected) return list.length === 0 ? { kind: "none" } : { kind: "choose" };
  if (selected.signInRequired) return { kind: "signInAgain", id: selected.id, name: selected.name };
  return { kind: "ok" };
}

const launchInput = (l: Launch | null) =>
  l && {
    op: l.progress.op,
    // `started` arrived but the game state does not say running yet: still starting.
    cancelRefused: l.cancelRefused || l.progress.outcome?.kind === "started",
  };

/** The game ended on its own with a code other than 0 (DESIGN.md 10.2), not yet dismissed. */
function unexpectedExit(game: GameStateDto | undefined, dismissed: number | null) {
  if (game?.state !== "idle" || !game.data.lastExit) return null;
  const exit = game.data.lastExit;
  if (exit.code === 0 || exit.stopRequested || exit.runId === dismissed) return null;
  return exit;
}

/** True after `ms` from `since` (Unix ms); re-renders once when that moment comes. */
function useReached(since: number | null, ms: number): boolean {
  const [, tick] = useState(0);
  const left = since === null ? null : since + ms - Date.now();
  useEffect(() => {
    if (left === null || left <= 0) return;
    const id = setTimeout(() => tick((n) => n + 1), left);
    return () => clearTimeout(id);
  }, [left]);
  return left !== null && left <= 0;
}

/** DESIGN.md 11.8 loading: title, meta and button blocks, after 200 ms. */
function StartSkeleton() {
  return (
    <Delayed>
      <div aria-busy="true" className="flex flex-col pt-12">
        <Skeleton className="h-10 w-80" />
        <Skeleton className="mt-2 h-5 w-60" />
        <Skeleton className="mt-20 h-14 w-60" />
      </div>
    </Delayed>
  );
}

function Meta({ instance }: { instance: InstanceDto }) {
  const { t, i18n } = useTranslation();
  if (instance.status !== "ready") {
    return (
      <p className="mt-2 flex h-5 items-center text-small text-muted-foreground">
        <Badge>{t(`instances.badge.${instance.status}`)}</Badge>
      </p>
    );
  }
  return (
    <p className="mt-2 flex flex-wrap gap-x-6 text-small text-muted-foreground tabular-nums">
      <span>{instance.game}</span>
      <span>{loaderText(instance, t)}</span>
      <span>
        {instance.lastPlayed === null
          ? t("instances.neverPlayed")
          : t("play.lastPlayed", { when: lastPlayedText(instance.lastPlayed, i18n.language, t) })}
      </span>
    </p>
  );
}

function Switcher({
  list,
  selected,
  open,
  onOpenChange,
  onSelect,
}: {
  list: InstanceDto[];
  selected: string | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSelect: (id: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <DropdownMenu open={open} onOpenChange={onOpenChange}>
      <DropdownMenuTrigger render={<Button variant="ghost" className="-ml-4 self-start" />}>
        {t("play.switch")}
        <ChevronDownIcon aria-hidden="true" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start">
        <DropdownMenuRadioGroup value={selected} onValueChange={(id) => onSelect(id as string)}>
          {list.map((i) => (
            <DropdownMenuRadioItem key={i.id} value={i.id} disabled={i.status !== "ready"}>
              <span className="min-w-0 truncate">{i.name}</span>
              {i.status !== "ready" && (
                <Badge className="ml-auto shrink-0">{t(`instances.badge.${i.status}`)}</Badge>
              )}
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

/** DESIGN.md 11.8: the start page, one block around the Play button. */
export function PlayView() {
  const { t, i18n } = useTranslation();
  const instances = useApp((s) => s.instances);
  const accounts = useApp((s) => s.accounts);
  const game = useApp((s) => s.game);
  const launch = useApp((s) => s.launch);
  const launchError = useApp((s) => s.launchError);
  const active = useApp((s) => s.activeInstance);
  const stopNoWindow = useApp((s) => s.stopNoWindow);
  const stoppingSeen = useApp((s) => s.stoppingSeen);
  const dismissedExit = useApp((s) => s.dismissedExit);
  const actionError = useApp((s) => s.playActionError);
  const app = useApp.getState;
  const create = useCreateDialogHost();
  const [switcherOpen, setSwitcherOpen] = useState(false);
  const [selectError, setSelectError] = useState<ErrorDto | null>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    // The view rebuilds from the backend's state, also after a reload.
    void app().loadGame();
    if (app().instances.status === "idle") void app().loadInstances();
  }, [app]);

  const forceDue = useReached(stoppingSeen?.at ?? null, FORCE_STOP_AFTER_MS);

  // A run this page saw end: announced once in the caption region, cleared by the next state.
  const gameState = game.data?.state;
  const [ended, setEnded] = useState(false);
  const previous = useRef(gameState);
  useEffect(() => {
    const was = previous.current;
    previous.current = gameState;
    if (was === gameState) return;
    setEnded((was === "running" || was === "stopping") && gameState === "idle");
  }, [gameState]);

  const list = instances.data;
  const loaded = list && game.data && (accounts.data || accounts.status === "error");

  // Opened from the sidebar, the Play button takes focus, so Enter plays (DESIGN.md 11.8).
  const focused = useRef(false);
  useEffect(() => {
    if (!loaded || focused.current) return;
    focused.current = true;
    if (document.activeElement?.closest("nav")) buttonRef.current?.focus();
  }, [loaded]);

  if (!loaded) {
    const failed =
      (!list && instances.status === "error" && instances.error) ||
      (!game.data && game.status === "error" && game.error);
    if (failed) {
      return (
        <div className="max-w-160 pt-12">
          <ErrorPanel
            error={failed}
            onRetry={() => Promise.all([app().loadInstances(), app().loadGame()])}
          />
        </div>
      );
    }
    return <StartSkeleton />;
  }

  const state = playButtonState({
    game: game.data!,
    launch: launchInput(launch),
    instances: list,
    account: accountOf(accounts),
  });
  const busy = ["stopping", "running", "starting", "preparing"].includes(state.kind);
  const selectedId = effectiveSelection(list);
  const shownId =
    (busy && (active ?? launch?.instance)) ||
    (state.kind === "chooseInstance" ? list.find((i) => i.selected)?.id : selectedId);
  const shown = list.find((i) => i.id === shownId) ?? null;
  const runId = game.data!.state === "running" ? game.data!.data.runId : null;
  const noWindow = runId !== null && stopNoWindow === runId;
  const showForce = (state.kind === "stopping" && forceDue) || noWindow;
  const exit = !launch && !launchError ? unexpectedExit(game.data, dismissedExit) : null;

  const select = async (id: string) => {
    setSelectError(null);
    try {
      await api.instanceSelect(id);
    } catch (e) {
      setSelectError(errorOf(e));
    }
    await app().loadInstances();
  };

  const activate = (s: PlayButtonState, button: HTMLElement) => {
    switch (s.kind) {
      case "ready":
        return void app().startGame(s.instance);
      case "preparing":
        return void app().cancelStart();
      case "running":
        return void app().stopGame();
      case "noInstance":
        return app().openCreate();
      case "chooseInstance":
        return setSwitcherOpen(true);
      case "signIn":
        return app().openSignIn(null, button);
      case "chooseAccount":
        // The menu of the header chip, as "Konto wählen" there (DESIGN.md 10.3).
        return app().setAccountMenuOpen(true);
      case "signInAgain":
        return app().openSignIn({ id: s.id, name: s.name }, button);
      default:
        return;
    }
  };

  // DESIGN.md 10.2 and 11.8: the log view with this run chosen.
  const showLog = (run: number) => {
    useLog.getState().choose(run);
    app().setView("log");
  };

  const onErrorAction = (a: ErrorAction) => {
    if (a.kind === "signIn") app().openSignIn(null, buttonRef.current);
    if (a.kind === "openSettings") app().setView("settings");
  };

  // A failed installer start offers its log when one was reported (DESIGN.md 10.12).
  const failedStart = launchError && {
    ...launchError.error,
    action:
      launchError.installLog &&
      (launchError.error.code === "installFailed" ||
        launchError.error.code === "installStepFailed") &&
      launchError.error.action?.kind !== "openLog"
        ? { kind: "openLog" as const, path: launchError.installLog }
        : launchError.error.action,
  };
  const retryStart = () => {
    if (selectedId) void app().startGame(selectedId);
  };

  // One line, cut with the full text as its title: a 64-character name must not wrap over the
  // button. A name is isolated (`isolate`) so bidi characters in it cannot reorder the sentence
  // around it.
  const line = (text: string) => (
    <span className="min-w-0 truncate" title={text}>
      {text}
    </span>
  );
  const caption =
    state.kind === "running" ? (
      <>
        <span aria-hidden="true" className="size-2 shrink-0 rounded-pill bg-success" />
        {line(
          t("play.running", {
            time: new Intl.DateTimeFormat(i18n.language, { timeStyle: "short" }).format(
              new Date(state.since),
            ),
          }),
        )}
      </>
    ) : state.kind === "noInstance" && state.unreadable ? (
      line(t("play.noneStartable"))
    ) : state.kind === "chooseInstance" ? (
      line(t("play.cannotStart", { name: isolate(state.name) }))
    ) : ended && !exit ? (
      // Heard, not seen: the button saying "Play" again shows the end.
      <span className="sr-only">{t("play.ended")}</span>
    ) : null;

  return (
    <div className="flex max-w-160 flex-col pt-12">
      {list.length === 0 || state.kind === "noInstance" ? (
        <>
          {list.length === 0 && <h2 className="text-heading">{t("instances.empty.title")}</h2>}
          <p className="text-small text-muted-foreground">{t("play.empty.body")}</p>
        </>
      ) : (
        shown && (
          <>
            <h2 className="line-clamp-2 font-display text-display-lg [overflow-wrap:anywhere]">
              {shown.name}
            </h2>
            <Meta instance={shown} />
          </>
        )
      )}
      {list.length > 1 && (
        // While a game is busy the page shows that game, so the switcher only keeps its place.
        <div className={busy ? "invisible mt-4" : "mt-4"} aria-hidden={busy || undefined}>
          {!busy && (
            <Switcher
              list={list}
              // The check marks what is selected, also an unreadable selection (row 6 of 10.2),
              // so choosing the readable fallback is a change.
              selected={list.find((i) => i.selected)?.id ?? selectedId}
              open={switcherOpen}
              onOpenChange={setSwitcherOpen}
              onSelect={(id) => void select(id)}
            />
          )}
          {busy && <div className="h-10" />}
        </div>
      )}
      {/* The caption above the button has its own line, so the button never moves. */}
      {/* Always there, so screen readers hear what appears in it: running, ended, why not. */}
      <p
        data-slot="play-caption"
        aria-live="polite"
        className="mt-2 mb-1 flex h-5 min-w-0 items-center gap-2 text-small text-muted-foreground"
      >
        {caption}
      </p>
      <div className="flex flex-wrap items-center gap-2">
        <PlayButton state={state} onActivate={activate} buttonRef={buttonRef} />
        {showForce && (
          <Button variant="destructive" onClick={() => void app().forceStopGame()}>
            {t("play.forceStop")}
          </Button>
        )}
      </div>
      <div className="mt-6 flex flex-col gap-4">
        {/* Always there (out of the layout while empty), so its text is announced, as the
            caption's is. */}
        <p role="status" className={noWindow ? "text-small text-muted-foreground" : "sr-only"}>
          {noWindow ? t("play.noWindow") : null}
        </p>
        {runId !== null && state.kind === "running" && (
          <Button variant="link" className="self-start" onClick={() => showLog(runId)}>
            {t("play.showLog")}
          </Button>
        )}
        {(state.kind === "starting" || state.kind === "preparing") && (
          <ProgressPanel progress={launch?.progress ?? null} />
        )}
        {failedStart && !busy && (
          <div className="max-w-120">
            <ErrorPanel
              error={failedStart}
              context="launch"
              onRetry={retryStart}
              onAction={onErrorAction}
            />
          </div>
        )}
        {exit && (
          <div className="max-w-120">
            <ErrorPanel
              error={{ code: "internal", params: {}, action: null, detail: "" }}
              copy={{ title: t("play.exit.title"), body: t("play.exit.body") }}
              extraAction={{ label: t("play.showLog"), onClick: () => showLog(exit.runId) }}
              dismiss={{
                label: t("play.exit.dismiss"),
                onClick: () => app().dismissExit(exit.runId),
              }}
            />
          </div>
        )}
        {actionError && (
          <ErrorPanel
            error={actionError}
            dismiss={{ label: t("common.close"), onClick: app().dismissPlayActionError }}
          />
        )}
        {selectError && (
          <ErrorPanel
            error={selectError}
            dismiss={{ label: t("common.close"), onClick: () => setSelectError(null) }}
          />
        )}
        {create.failure}
      </div>
      {create.dialog}
    </div>
  );
}
