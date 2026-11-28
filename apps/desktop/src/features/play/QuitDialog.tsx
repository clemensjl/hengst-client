import { CircleAlertIcon, LoaderCircleIcon } from "lucide-react";
import { useEffect, useRef, useState } from "react";
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
import { ErrorPanel } from "@/features/errors/ErrorPanel";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";
import { api, IpcError } from "@/ipc/client";
import { useApp } from "@/stores/app";
import { useChangeGuard } from "./changeGuard";

/** DESIGN.md 11.11: after this long without an exit the dialog offers the force stop. */
export const NOT_RESPONDING_MS = 10_000;

const errorOf = (e: unknown): ErrorDto =>
  e instanceof IpcError ? e.dto : { code: "internal", params: {}, action: null, detail: String(e) };

/**
 * - `ask`: the question, nothing sent yet.
 * - `stopping`: the game was asked to close; waiting for its exit (busy).
 * - `noWindow`: the game has no window, so it was not asked; only the force stop is left.
 * - `notResponding`: no exit within 10 s of the stop or of the force stop.
 * - `forcing`: the force stop runs; waiting for the exit (busy).
 * - `quitting`: the game is gone, the app quits (busy).
 * - `quitFailed`: the game is gone but the app could not quit; the error stays with a retry.
 */
type Phase =
  "ask" | "stopping" | "noWindow" | "notResponding" | "forcing" | "quitting" | "quitFailed";

const BUSY: Phase[] = ["stopping", "forcing", "quitting"];
/** The phases that wait for the game's exit: after 10 s they give the buttons back. */
const WAITING: Phase[] = ["stopping", "forcing"];
const FORCE: Phase[] = ["noWindow", "notResponding"];

/** What the destructive button does right now. */
type Action = "stop" | "force" | "none";
const actionOf = (phase: Phase): Action =>
  BUSY.includes(phase) || phase === "quitFailed"
    ? "none"
    : FORCE.includes(phase)
      ? "force"
      : "stop";

const REASON_ID = "quit-keep-reason";

/** The quit dialog while the tray asked for it (`quitAsked`). */
export function QuitDialogHost() {
  const asked = useApp((s) => s.quitAsked);
  const close = useApp((s) => s.closeQuit);
  const dismiss = useApp((s) => s.dismissQuit);
  return asked === null ? null : (
    <QuitDialog key={asked} onDismiss={dismiss} onClosed={() => close(asked)} />
  );
}

/**
 * "Quit Hengst Client" while a game runs (DESIGN.md 11.11, 10.9): a confirmation, the safe button
 * first in focus. The stop and the force stop are the app's own (`game_stop`, `game_force_stop`),
 * so the exit counts as requested. The app quits once the game has exited, also when it exits by
 * itself while the dialog still asks: the user chose to quit.
 */
function QuitDialog({ onDismiss, onClosed }: { onDismiss: () => void; onClosed: () => void }) {
  const { t } = useTranslation();
  const game = useApp((s) => s.game.data);
  const [open, setOpen] = useState(true);
  const [phase, setPhase] = useState<Phase>("ask");
  const [error, setError] = useState<ErrorDto | null>(null);
  const keepRef = useRef<HTMLButtonElement>(null);
  // Before the first await, so a double click sends one request.
  const sending = useRef(false);
  const busy = BUSY.includes(phase);
  const runs = game?.state === "running" || game?.state === "stopping";
  const action = actionOf(phase);
  // The button turning into the force stop ignores clicks for 600 ms (DESIGN.md 10.0): the second
  // click of a double click on "Stop game and quit" must not kill the game.
  const guarded = useChangeGuard(action, (left, now) => now === "force" && left !== "force");

  const quit = async () => {
    setError(null);
    setPhase("quitting");
    try {
      // `false`: a game runs again (a new start); the question stands again.
      if (!(await api.appQuit())) setPhase("ask");
    } catch (e) {
      setError(errorOf(e));
      setPhase("quitFailed");
    }
  };

  // The game is gone: quit, whatever was confirmed (a failed quit waits for its retry). Not after
  // "Keep playing": the dialog may still be closing, but the user chose to stay.
  useEffect(() => {
    if (!open || runs || game === undefined || phase === "quitting" || phase === "quitFailed")
      return;
    void quit();
    // `quit` only uses state setters and the API, so it is not a dependency.
  }, [open, runs, game, phase]);

  useEffect(() => {
    if (!WAITING.includes(phase)) return;
    const timer = setTimeout(() => setPhase("notResponding"), NOT_RESPONDING_MS);
    return () => clearTimeout(timer);
  }, [phase]);

  const stop = async () => {
    if (sending.current) return;
    sending.current = true;
    setError(null);
    setPhase("stopping");
    try {
      const result = await api.gameStop();
      if (result === "noWindow") setPhase("noWindow");
      else if (result === "notRunning") await quit();
    } catch (e) {
      setError(errorOf(e));
      setPhase("ask");
    } finally {
      sending.current = false;
    }
  };

  const force = async () => {
    if (sending.current) return;
    sending.current = true;
    const before = phase;
    setError(null);
    setPhase("forcing");
    try {
      if (!(await api.gameForceStop())) await quit();
    } catch (e) {
      setError(errorOf(e));
      setPhase(before);
    } finally {
      sending.current = false;
    }
  };

  const keep = () => {
    if (busy) return;
    setOpen(false);
    onDismiss();
  };

  const label = busy
    ? t("play.quit.busy")
    : action === "force"
      ? t("play.quit.force")
      : t("play.quit.confirm");

  return (
    <AlertDialog
      open={open}
      onOpenChange={(next, details) => {
        // Escape and a press outside keep playing; nothing closes while the game is being stopped.
        if (!next && !busy) keep();
        else if (!next) details.cancel();
      }}
      onOpenChangeComplete={(isOpen) => {
        if (!isOpen) onClosed();
      }}
    >
      <AlertDialogContent initialFocus={keepRef}>
        <AlertDialogTitle className="pr-0">{t("play.quit.title")}</AlertDialogTitle>
        <AlertDialogBody className="flex flex-col gap-4">
          <AlertDialogDescription>{t("play.quit.body")}</AlertDialogDescription>
          {phase === "noWindow" && <p className="text-body">{t("play.noWindow")}</p>}
          {phase === "notResponding" && (
            <div role="alert" className="flex items-start gap-2 text-body">
              <CircleAlertIcon className="mt-1 size-4 shrink-0 text-danger" aria-hidden="true" />
              <span>{t("play.quit.notResponding")}</span>
            </div>
          )}
          {busy && (
            <p id={REASON_ID} className="text-small text-muted-foreground">
              {phase === "quitting" ? t("play.quit.quittingReason") : t("play.quit.keepReason")}
            </p>
          )}
          {error && (
            <ErrorPanel
              error={error}
              autoFocus
              onRetry={phase === "quitFailed" ? () => void quit() : undefined}
            />
          )}
        </AlertDialogBody>
        <AlertDialogFooter>
          <Button
            ref={keepRef}
            aria-disabled={busy || undefined}
            aria-describedby={busy ? REASON_ID : undefined}
            onClick={keep}
          >
            {phase === "quitFailed" ? t("common.close") : t("play.quit.keep")}
          </Button>
          {phase !== "quitFailed" && (
            <Button
              variant="destructive"
              aria-disabled={busy || undefined}
              onClick={() => {
                if (busy || guarded()) return;
                void (action === "force" ? force() : stop());
              }}
            >
              {busy ? <LoaderCircleIcon className="spinner size-4" aria-hidden="true" /> : null}
              {label}
            </Button>
          )}
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
