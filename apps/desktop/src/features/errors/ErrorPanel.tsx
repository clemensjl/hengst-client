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
import { describeError } from "./errorCopy";

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

/** The text copied from the details gets "Copied" for two seconds. */
function CopyButton({ text }: { text: string }) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(() => () => clearTimeout(timer.current), []);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      return;
    }
    setCopied(true);
    clearTimeout(timer.current);
    timer.current = setTimeout(() => setCopied(false), 2000);
  };
  return (
    <Button variant="ghost" onClick={() => void copy()}>
      {copied ? t("errors.copied") : t("errors.copy")}
    </Button>
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
  /** The panel takes focus on its title when it appears (DESIGN.md 11.2); off for a page of samples. */
  autoFocus?: boolean;
}

/** DESIGN.md 10.11. */
export function ErrorPanel({
  error,
  onRetry,
  onAction,
  extraAction,
  autoFocus = true,
}: ErrorPanelProps) {
  const { t } = useTranslation();
  const guard = useClickGuard();
  const titleRef = useRef<HTMLParagraphElement>(null);
  const [open, setOpen] = useState(false);
  const [openFailed, setOpenFailed] = useState(false);
  const { title, body, actionLabel } = describeError(error, t);
  const action = error.action;

  useEffect(() => {
    if (!autoFocus) return;
    // Focus scrolls the title into view, which matters in a dialog body at the minimum window height.
    titleRef.current?.focus();
    titleRef.current?.scrollIntoView?.({ block: "nearest" });
  }, [autoFocus]);

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
