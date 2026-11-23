import { CircleAlertIcon, ExternalLinkIcon, LoaderCircleIcon } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogTitle,
} from "@/components/ui/dialog";
import { ErrorPanel } from "@/features/errors/ErrorPanel";
import type { Language } from "@/ipc/bindings/Language";
import { api } from "@/ipc/client";
import { useApp } from "@/stores/app";
import { chipRef } from "./AccountChip";
import { InfoBlock, useOpenUrl } from "./InfoBlock";
import { signedInMessage, signInErrorAction, type SignInState } from "./signIn";
import { useSignIn, type SignIn } from "./useSignIn";

const Spinner = () => <LoaderCircleIcon className="spinner size-4" aria-hidden="true" />;

/** A button that is busy: it looks like itself with a spinner and ignores clicks (DESIGN.md 10.0). */
function InertButton({
  children,
  reserve,
  focusOnMount,
  ...props
}: Omit<React.ComponentProps<typeof Button>, "onClick"> & {
  /** Takes the focus when it appears: it replaces the button that had it. */
  focusOnMount?: boolean;
  /** The label of the idle button: the width does not shrink below it (DESIGN.md 10.1). */
  reserve?: string;
}) {
  const ref = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (focusOnMount) ref.current?.focus();
  }, [focusOnMount]);
  return (
    <Button
      ref={ref}
      aria-disabled="true"
      aria-busy="true"
      onClick={(e) => e.preventDefault()}
      {...props}
    >
      <Spinner />
      <span className="grid">
        {reserve && (
          <span aria-hidden="true" className="invisible col-start-1 row-start-1">
            {reserve}
          </span>
        )}
        <span className="col-start-1 row-start-1">{children}</span>
      </span>
    </Button>
  );
}

const pad = (n: number) => String(n).padStart(2, "0");
/** `14:32`: minutes and seconds, never an hour. */
export const formatRemaining = (seconds: number) =>
  `${Math.floor(seconds / 60)}:${pad(seconds % 60)}`;

/** Whole seconds left until `expiresAt` (unix seconds), counting down once a second. */
function useRemaining(expiresAt: number): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    setNow(Date.now());
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [expiresAt]);
  return Math.max(0, Math.ceil(expiresAt - now / 1000));
}

/** The code box, the copy button and the time left (DESIGN.md 11.5, showingCode). */
function CodeView({
  state,
  copyRef,
  openFailed,
}: {
  state: Extract<SignInState, { phase: "showingCode" }>;
  copyRef: React.RefObject<HTMLButtonElement | null>;
  /** "Open microsoft.com/link" was refused by the system. */
  openFailed: boolean;
}) {
  const { t } = useTranslation();
  const remaining = useRemaining(state.expiresAt);
  // Announced once, when the code appears: the region is there empty first, then it is filled.
  const [announcement, setAnnouncement] = useState("");
  useEffect(() => {
    setAnnouncement(t("signIn.code.announce", { code: Array.from(state.userCode).join(" ") }));
    // Once per code: a change of language does not announce it again.
  }, [state.userCode]);
  const [copied, setCopied] = useState(false);
  const [copyFailed, setCopyFailed] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(() => () => clearTimeout(timer.current), []);

  const copy = async () => {
    clearTimeout(timer.current);
    try {
      await navigator.clipboard.writeText(state.userCode);
    } catch {
      setCopyFailed(true);
      return;
    }
    setCopyFailed(false);
    setCopied(true);
    timer.current = setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="flex flex-col gap-3">
      <DialogDescription>{t("signIn.code.body")}</DialogDescription>
      <span role="status" aria-live="polite" className="sr-only">
        {announcement}
      </span>
      <div className="flex flex-wrap items-center gap-2">
        <div className="rounded-md bg-background px-4 py-3 font-mono text-code tracking-[0.04em] [overflow-wrap:anywhere]">
          {state.userCode}
        </div>
        <Button ref={copyRef} onClick={() => void copy()}>
          {copied ? t("signIn.code.copied") : t("signIn.code.copy")}
        </Button>
        <span role="status" aria-live="polite" className="sr-only">
          {copied ? t("signIn.code.copied") : ""}
        </span>
      </div>
      {copyFailed && (
        <p className="flex items-center gap-2 text-small text-danger">
          <CircleAlertIcon className="size-4 shrink-0" aria-hidden="true" />
          {t("errors.copyFailed")}
        </p>
      )}
      {openFailed && <OpenFailed text={t("signIn.code.openFailed")} />}
      {/* Plain text, no live region: a screen reader does not read it every second. The digits
          are tabular so the line does not shift. */}
      <p className="text-caption text-muted-foreground tabular-nums">
        {t("signIn.code.remaining", { time: formatRemaining(remaining) })}
      </p>
      {/* The address the service named, as text and only if it is https. It is never a link:
          "Open" goes to the fixed target of the app (DESIGN.md 11.5, plan ruling). */}
      {state.verificationUri.startsWith("https://") && (
        <p className="text-caption break-all text-muted-foreground">
          {t("signIn.code.address", { uri: state.verificationUri })}
        </p>
      )}
    </div>
  );
}

/** A link button the system refused to open: said beside it, so the click does not do nothing. */
function OpenFailed({ text }: { text: string }) {
  return (
    <p role="alert" className="flex items-start gap-2 text-small text-danger">
      <CircleAlertIcon className="mt-0.5 size-4 shrink-0" aria-hidden="true" />
      {text}
    </p>
  );
}

function Body({
  signIn,
  copyRef,
  bodyRef,
  devOffline,
  deviceLoginFailed,
  helpFailed,
}: {
  signIn: SignIn;
  copyRef: React.RefObject<HTMLButtonElement | null>;
  bodyRef: React.RefObject<HTMLDivElement | null>;
  devOffline: boolean;
  deviceLoginFailed: boolean;
  helpFailed: boolean;
}) {
  const { t } = useTranslation();
  const { state } = signIn;
  const useCode = (inert: boolean) => (
    <Button
      variant="link"
      aria-disabled={inert || undefined}
      onClick={() => (inert ? undefined : signIn.start("deviceCode"))}
    >
      {t("signIn.useCode")}
    </Button>
  );
  switch (state.phase) {
    case "idle":
    case "starting":
      return (
        <div className="flex flex-col items-start gap-1">
          <DialogDescription>{t("signIn.idle.body")}</DialogDescription>
          {useCode(state.phase === "starting")}
        </div>
      );
    case "waitingBrowser":
      return (
        <div className="flex flex-col items-start gap-3">
          <div
            ref={bodyRef}
            tabIndex={-1}
            role="status"
            className="flex flex-col gap-1 outline-none"
          >
            <p className="flex items-start gap-2">
              <span className="mt-1 shrink-0">
                <Spinner />
              </span>
              {t("signIn.waiting.body")}
            </p>
            <p className="pl-6 text-caption text-muted-foreground">{t("signIn.waiting.limit")}</p>
          </div>
          {state.reopenFailed && (
            <p className="flex items-start gap-2 text-small text-danger">
              <CircleAlertIcon className="mt-0.5 size-4 shrink-0" aria-hidden="true" />
              {t("signIn.reopenFailed")}
            </p>
          )}
          <Button variant="link" onClick={signIn.switchToCode}>
            {t("signIn.useCode")}
          </Button>
        </div>
      );
    case "showingCode":
      return <CodeView state={state} copyRef={copyRef} openFailed={deviceLoginFailed} />;
    case "completing":
      return (
        <p
          id="sign-in-completing"
          ref={bodyRef as React.RefObject<HTMLParagraphElement | null>}
          tabIndex={-1}
          role="status"
          className="flex items-center gap-2 outline-none"
        >
          <Spinner />
          {t("signIn.completing")}
        </p>
      );
    case "failed": {
      const action = signInErrorAction(state.error.code);
      const own = state.error.action;
      return (
        <ErrorPanel
          error={state.error}
          context="signIn"
          autoFocus
          // The same method again, except where it cannot work (no browser, or nothing to retry).
          onRetry={
            action.kind === "retry" || action.kind === "openUrl"
              ? () => signIn.start(state.method)
              : undefined
          }
          extraAction={
            action.kind === "useCode"
              ? { label: t("signIn.useCode"), onClick: () => signIn.start("deviceCode") }
              : // The backend already gives these errors their own link; without one it is added here.
                action.kind === "openUrl" &&
                  !(own?.kind === "openUrl" && own.target === action.target)
                ? {
                    label: t(action.labelKey),
                    onClick: () => void api.openUrl(action.target).catch(() => {}),
                  }
                : undefined
          }
        />
      );
    }
    case "appNotApproved":
      return (
        <div ref={bodyRef} tabIndex={-1} className="outline-none">
          <InfoBlock title={<h3 className="text-heading">{t("signIn.notApproved.title")}</h3>}>
            <p>{t("signIn.notApproved.body")}</p>
            {devOffline && (
              <p className="text-caption text-muted-foreground">{t("signIn.notApproved.dev")}</p>
            )}
            {helpFailed && <OpenFailed text={t("signIn.openFailed")} />}
          </InfoBlock>
        </div>
      );
    default:
      return null;
  }
}

/** DESIGN.md 11.5: Cancel (or Close) on the left, the one primary action last, at most two buttons. */
function Footer({
  signIn,
  primaryRef,
  requestClose,
  close,
  help,
  deviceLogin,
}: {
  signIn: SignIn;
  primaryRef: React.RefObject<HTMLButtonElement | null>;
  requestClose: () => void;
  close: () => void;
  help: ReturnType<typeof useOpenUrl>;
  deviceLogin: ReturnType<typeof useOpenUrl>;
}) {
  const { t } = useTranslation();
  const { state } = signIn;
  const cancel = <Button onClick={requestClose}>{t("common.cancel")}</Button>;
  const inertCancel = (
    <Button aria-disabled="true" onClick={(e) => e.preventDefault()}>
      {t("common.cancel")}
    </Button>
  );
  switch (state.phase) {
    case "idle":
      return (
        <>
          {cancel}
          <Button ref={primaryRef} variant="primary" onClick={() => signIn.start("browser")}>
            {t("signIn.start")}
            <ExternalLinkIcon aria-hidden="true" />
          </Button>
        </>
      );
    case "starting":
      return (
        <>
          {cancel}
          <InertButton variant="primary" reserve={t("signIn.start")} focusOnMount>
            {t("signIn.opening")}
          </InertButton>
        </>
      );
    case "waitingBrowser":
      return (
        <>
          {cancel}
          <Button onClick={signIn.reopen}>{t("signIn.reopen")}</Button>
        </>
      );
    case "showingCode":
      return (
        <>
          {cancel}
          <Button variant="primary" onClick={deviceLogin.open}>
            {t("signIn.code.open")}
            <ExternalLinkIcon aria-hidden="true" />
          </Button>
        </>
      );
    case "completing":
      return (
        <>
          {inertCancel}
          <InertButton variant="primary">{t("signIn.completing")}</InertButton>
        </>
      );
    case "failed":
      return <Button onClick={close}>{t("common.close")}</Button>;
    case "appNotApproved":
      return (
        <>
          <Button onClick={close}>{t("common.close")}</Button>
          <Button variant="primary" onClick={help.open}>
            {t("signIn.notApproved.help")}
            <ExternalLinkIcon aria-hidden="true" />
          </Button>
        </>
      );
    default:
      return null;
  }
}

function resolvedLanguage(language: string): Language {
  return language.toLowerCase().startsWith("de") ? "de" : "en";
}

/**
 * DESIGN.md 11.5. `resign` is the account that is asked to sign in again (the title says so and
 * the toast names it if another account signed in); `onClosed` runs when the dialog is gone.
 */
export function SignInDialog({
  resign,
  onClosed,
}: {
  resign: { id: string; name: string } | null;
  onClosed: () => void;
}) {
  const { t, i18n } = useTranslation();
  const [open, setOpen] = useState(true);
  const signIn = useSignIn(resolvedLanguage(i18n.language));
  const { state } = signIn;
  const devOffline = useApp((s) => s.info.data?.devOffline ?? false);
  const primaryRef = useRef<HTMLButtonElement>(null);
  const copyRef = useRef<HTMLButtonElement>(null);
  const bodyRef = useRef<HTMLDivElement>(null);
  // Here and not in the footer: the body says when opening failed.
  const help = useOpenUrl("signInHelp");
  const deviceLogin = useOpenUrl("deviceLogin");

  // Escape, the close button and Cancel are one path: cancel what runs, close if that worked. A
  // cancel that is refused (the account is being saved) leaves the dialog; the terminal event decides.
  const requestClose = () => {
    void signIn.cancel().then((done) => {
      if (done) setOpen(false);
    });
  };

  // What a phase asks of the screen: the toast and the end, the focus where the old one is gone.
  const loadAccounts = useApp((s) => s.loadAccounts);
  useEffect(() => {
    switch (state.phase) {
      case "done": {
        const { key, params } = signedInMessage(state.account, resign);
        toast(t(key, params));
        void loadAccounts();
        setOpen(false);
        break;
      }
      case "cancelled":
        void loadAccounts();
        setOpen(false);
        break;
      case "failed":
        void loadAccounts();
        break;
      case "showingCode":
        copyRef.current?.focus();
        break;
      case "waitingBrowser":
      case "completing":
      case "appNotApproved":
        bodyRef.current?.focus();
        break;
    }
    // Only a change of phase matters; `resign`, `t` and `loadAccounts` do not change meanwhile.
  }, [state.phase]);

  const title = resign ? t("signIn.titleAgain", { name: resign.name }) : t("signIn.title");
  return (
    <Dialog
      open={open}
      disablePointerDismissal
      onOpenChange={(next) => {
        if (!next) requestClose();
      }}
      onOpenChangeComplete={(isOpen) => {
        if (!isOpen) onClosed();
      }}
    >
      <DialogContent
        className="w-140"
        closeLabel={t("common.close")}
        closeBusyReason={state.phase === "completing" ? t("signIn.closeBusy") : undefined}
        initialFocus={primaryRef}
        finalFocus={chipRef}
      >
        <DialogTitle>{title}</DialogTitle>
        <DialogBody>
          <Body
            signIn={signIn}
            copyRef={copyRef}
            bodyRef={bodyRef}
            devOffline={devOffline}
            deviceLoginFailed={deviceLogin.failed}
            helpFailed={help.failed}
          />
        </DialogBody>
        <DialogFooter>
          <Footer
            signIn={signIn}
            primaryRef={primaryRef}
            requestClose={requestClose}
            close={() => setOpen(false)}
            help={help}
            deviceLogin={deviceLogin}
          />
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
