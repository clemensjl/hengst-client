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

/** DESIGN.md 11.11: after this long without an exit the dialog offers the force stop. */
export const NOT_RESPONDING_MS = 10_000;

const errorOf = (e: unknown): ErrorDto =>
  e instanceof IpcError ? e.dto : { code: "internal", params: {}, action: null, detail: String(e) };

/**
 * - `ask`: the question, nothing sent yet.
 * - `stopping`: the game was asked to close; waiting for its exit (busy).
 * - `noWindow`: the game has no window, so it was not asked; only the force stop is left.
 * - `notResponding`: asked, but no exit within 10 s.
 * - `forcing`: the force stop runs; waiting for the exit (busy).
 * - `quitting`: the game is gone, the app quits (busy).
 */
type Phase = "ask" | "stopping" | "noWindow" | "notResponding" | "forcing" | "quitting";

const BUSY: Phase[] = ["stopping", "forcing", "quitting"];

/** The quit dialog while the tray asked for it (`quitAsked`). */
export function QuitDialogHost() {
  const asked = useApp((s) => s.quitAsked);
  const close = useApp((s) => s.closeQuit);
  return asked === null ? null : <QuitDialog key={asked} onClosed={close} />;
}

/**
 * "Quit Hengst Client" while a game runs (DESIGN.md 11.11, 10.9): a confirmation, the safe button
 * first in focus. The stop and the force stop are the app's own (`game_stop`, `game_force_stop`),
 * so the exit counts as requested. The app quits only once the game has exited.
 */
function QuitDialog({ onClosed }: { onClosed: () => void }) {
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

  const quit = async () => {
    setPhase("quitting");
    try {
      // `false`: a game runs again (a new start); the question stands again.
      if (!(await api.appQuit())) setPhase("ask");
    } catch (e) {
      setError(errorOf(e));
      setPhase("ask");
    }
  };

  // The game is gone. Confirmed: quit. Not confirmed yet: nothing left to ask.
  useEffect(() => {
    if (runs || game === undefined) return;
    if (phase === "ask") setOpen(false);
    else if (phase !== "quitting") void quit();
    // `quit` only uses state setters and the API, so it is not a dependency.
  }, [runs, game, phase]);

  useEffect(() => {
    if (phase !== "stopping") return;
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
    if (!busy) setOpen(false);
  };

  const forceOffered = phase === "noWindow" || phase === "notResponding" || phase === "forcing";
  const label = busy
    ? t("play.quit.busy")
    : forceOffered
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
          {error && <ErrorPanel error={error} autoFocus />}
        </AlertDialogBody>
        <AlertDialogFooter>
          <Button ref={keepRef} aria-disabled={busy || undefined} onClick={keep}>
            {t("play.quit.keep")}
          </Button>
          <Button
            variant="destructive"
            aria-disabled={busy || undefined}
            onClick={() => {
              if (busy) return;
              void (forceOffered ? force() : stop());
            }}
          >
            {busy ? <LoaderCircleIcon className="spinner size-4" aria-hidden="true" /> : null}
            {label}
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
