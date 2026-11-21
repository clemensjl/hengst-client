import {
  ChevronDownIcon,
  ChevronUpIcon,
  CircleAlertIcon,
  ExternalLinkIcon,
  RefreshCwIcon,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { api, IpcError } from "@/ipc/client";
import type { ErrorAction } from "@/ipc/bindings/ErrorAction";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";
import { describeError, type ErrorContext } from "./errorCopy";

/** DESIGN.md 10.0: after a click that starts something, a second activation is ignored until it ends. */
const GUARD_MS = 600;

/**
 * Wraps a handler so a double click runs it once. A handler that returns a promise holds the guard
 * until the promise settles; any other handler holds it for `GUARD_MS`.
 */
function useClickGuard(): (run: () => unknown) => void {
  const busy = useRef(false);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(() => () => clearTimeout(timer.current), []);
  return useCallback((run) => {
    if (busy.current) return;
    busy.current = true;
    const release = () => {
      busy.current = false;
    };
    let result: unknown;
    try {
      result = run();
    } catch (e) {
      release();
      throw e;
    }
    if (result instanceof Promise) void result.then(release, release);
    else timer.current = setTimeout(release, GUARD_MS);
  }, []);
}

/**
 * Copies the details. "Copied" shows on the button for two seconds and is announced politely; a
 * refused clipboard write says so inline.
 */
function CopyButton({ text }: { text: string }) {
  const { t } = useTranslation();
  const [state, setState] = useState<"idle" | "copied" | "failed">("idle");
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(() => () => clearTimeout(timer.current), []);
  const copy = async () => {
    clearTimeout(timer.current);
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      setState("failed");
      return;
    }
    setState("copied");
    timer.current = setTimeout(() => setState("idle"), 2000);
  };
  return (
    <>
      <Button variant="ghost" onClick={() => void copy()}>
        {state === "copied" ? t("errors.copied") : t("errors.copy")}
      </Button>
      <span role="status" aria-live="polite" className="sr-only">
        {state === "copied" ? t("errors.copied") : ""}
      </span>
      {state === "failed" && (
        <p className="flex items-center gap-2 text-small text-danger">
          <CircleAlertIcon className="size-4 shrink-0" aria-hidden="true" />
          {t("errors.copyFailed")}
        </p>
      )}
    </>
  );
}

export interface ErrorPanelProps {
  error: ErrorDto;
  /** Shows "Try again"; without it retrying is not possible here. */
  onRetry?: () => void | Promise<unknown>;
  /**
   * Receives the error's own action (`signIn`, `openSettings`); without a handler that action is
   * not shown. `openUrl` is carried out by the panel itself through `api.openUrl` and never reaches
   * this handler.
   */
  onAction?: (action: ErrorAction) => void;
  /** An action that does not come from the error, for example "Open folder". */
  extraAction?: { label: string; onClick: () => void };
  /** Which wording applies where the codes differ by place (see `ErrorContext`). */
  context?: ErrorContext;
  /**
   * Moves focus to the title when the panel appears and whenever it receives a different error
   * (DESIGN.md 11.2). Off by default: only an action the user just took may move focus, never a
   * panel that appears on its own (a background refresh failing).
   */
  autoFocus?: boolean;
}

/** DESIGN.md 10.11. */
export function ErrorPanel({
  error,
  onRetry,
  onAction,
  extraAction,
  context,
  autoFocus = false,
}: ErrorPanelProps) {
  const { t } = useTranslation();
  const guard = useClickGuard();
  const titleRef = useRef<HTMLParagraphElement>(null);
  const [open, setOpen] = useState(false);
  const [openFailed, setOpenFailed] = useState(false);
  const { title, body, actionLabel } = describeError(error, t, context);
  const action = error.action;

  // A new error is a different panel to the user, so focus follows it; an equal one in a new
  // object (a parent re-rendering) does not count.
  const errorKey = JSON.stringify([error.code, error.detail, error.params]);
  useEffect(() => {
    if (!autoFocus) return;
    // Focus scrolls the title into view, which matters in a dialog body at the minimum window height.
    titleRef.current?.focus();
    titleRef.current?.scrollIntoView?.({ block: "nearest" });
  }, [autoFocus, errorKey]);

  // An action that needs a handler the caller did not give would vanish without a trace.
  const needsHandler = action?.kind === "signIn" || action?.kind === "openSettings";
  useEffect(() => {
    if (import.meta.env.DEV && needsHandler && !onAction) {
      console.warn(
        `ErrorPanel: the error "${error.code}" has the action "${action?.kind}" but no onAction handler was given, so the action is not shown`,
      );
    }
  }, [needsHandler, onAction, error.code, action?.kind]);

  const runAction = (a: ErrorAction) => {
    if (a.kind === "openUrl") {
      setOpenFailed(false);
      return api.openUrl(a.target).catch((e: unknown) => {
        if (e instanceof IpcError) setOpenFailed(true);
        else throw e;
      });
    }
    onAction?.(a);
  };
  // The panel opens links itself, so an `openUrl` button needs no handler; the others do.
  const showAction = actionLabel && action && (action.kind === "openUrl" || onAction);

  return (
    <div
      role="alert"
      data-slot="error-panel"
      className="flex flex-col gap-3 rounded-lg border bg-surface-raised p-4"
    >
      <div className="flex items-start gap-3">
        <CircleAlertIcon className="mt-[3px] size-[18px] shrink-0 text-danger" aria-hidden="true" />
        <div className="flex min-w-0 flex-col">
          <p
            ref={titleRef}
            tabIndex={-1}
            className="text-body font-semibold [overflow-wrap:anywhere]"
          >
            {title}
          </p>
          <p className="text-small text-muted-foreground">{body}</p>
        </div>
      </div>
      <Collapsible open={open} onOpenChange={setOpen} className="flex flex-col gap-3 pl-[30px]">
        <div className="flex flex-wrap gap-2">
          {onRetry && (
            <Button onClick={() => guard(onRetry)}>
              <RefreshCwIcon aria-hidden="true" />
              {t("common.retry")}
            </Button>
          )}
          {showAction && (
            <Button onClick={() => guard(() => runAction(action))}>
              {actionLabel}
              {action.kind === "openUrl" && <ExternalLinkIcon aria-hidden="true" />}
            </Button>
          )}
          {extraAction && (
            <Button onClick={() => guard(extraAction.onClick)}>{extraAction.label}</Button>
          )}
          <CollapsibleTrigger render={<Button variant="ghost" />}>
            {t("errors.details")}
            {open ? <ChevronUpIcon aria-hidden="true" /> : <ChevronDownIcon aria-hidden="true" />}
          </CollapsibleTrigger>
        </div>
        {openFailed && (
          <p className="flex items-center gap-2 text-small text-danger">
            <CircleAlertIcon className="size-4 shrink-0" aria-hidden="true" />
            {t("errors.openFailed.title")}
          </p>
        )}
        <CollapsibleContent className="flex flex-col items-start gap-2">
          <div
            role="region"
            lang="en"
            aria-label={t("errors.details")}
            tabIndex={0}
            className="max-h-40 w-full overflow-auto rounded-md bg-background p-3 font-mono text-mono whitespace-pre-wrap text-muted-foreground [font-variant-ligatures:none] [overflow-wrap:anywhere]"
          >
            {error.detail}
          </div>
          <CopyButton text={error.detail} />
        </CollapsibleContent>
      </Collapsible>
    </div>
  );
}
