import { mockIPC } from "@tauri-apps/api/mocks";
import { useEffect, useState } from "react";
import { TooltipProvider } from "@/components/ui/tooltip";
import { Toaster } from "@/components/ui/sonner";
import { AccountChip } from "@/features/accounts/AccountChip";
import { AccountDialogs } from "@/features/accounts/AccountDialogs";
import i18n, { initI18n, setLanguage } from "@/i18n";
import type { AccountDto } from "@/ipc/bindings/AccountDto";
import type { ErrorCode } from "@/ipc/bindings/ErrorCode";
import type { LoginEvent } from "@/ipc/bindings/LoginEvent";
import { useApp } from "@/stores/app";
import { applyTheme } from "@/theme/theme";

/*
 * Dev-only page: the real sign-in dialog, the chip and the confirmation dialogs against a fake
 * backend, one state per address, for the pixel check in the Vite dev server:
 *
 *   #sign-in-gallery?phase=code&theme=dark&lang=de
 *
 * `phase` is one of PHASES below. It lets the dialog run through the same events the backend sends
 * (no sign-in is ever started against a real service). main.tsx imports it only when
 * `import.meta.env.DEV` is true, so a production build contains neither this file nor its texts
 * (vite-plugins/noDevGallery.ts checks the output).
 */

const PHASES = [
  "idle",
  "resign",
  "starting",
  "waiting",
  "waiting-reopen-failed",
  "code",
  "completing",
  "failed-network",
  "failed-loopback",
  "failed-openFailed",
  "failed-noXbox",
  "failed-gameNotOwned",
  "failed-appMisconfigured",
  "not-approved",
  "no-client-id",
  "remove",
  "remove-failed",
  "menu",
  "menu-sign-out",
  "menu-error",
] as const;
type Phase = (typeof PHASES)[number];

const acct = (id: string, name: string, extra: Partial<AccountDto> = {}): AccountDto => ({
  id,
  name,
  kind: "microsoft",
  selected: false,
  signInRequired: false,
  ...extra,
});

/** Accounts with the awkward cases: the longest names (16), one that needs signing in, the dev one. */
const ACCOUNTS: AccountDto[] = [
  acct("a", "Mira_K", { selected: true }),
  acct("b", "Sixteen_Chars_Nm", { signInRequired: true }),
  acct("c", "WWWWWWWWWWWWWWWW", { signInRequired: true }),
  acct("offline-dev", "Dev", { kind: "offlineDev" }),
];

const failed = (code: ErrorCode, action: unknown = null): LoginEvent => ({
  event: "failed",
  data: {
    error: {
      code,
      params: {},
      action: action as never,
      detail: "gallery sample: the details are the English core message",
    },
  },
});

interface Channel {
  onmessage?: (e: LoginEvent) => void;
}

function startBackend(phase: Phase): { send: (e: LoginEvent) => void } {
  let channel: Channel | undefined;
  mockIPC((cmd, args) => {
    const payload = (args ?? {}) as Record<string, unknown>;
    switch (cmd) {
      case "accounts_list":
        if (phase === "menu-error") {
          throw { code: "fileSystem", params: {}, action: null, detail: "gallery sample" };
        }
        return {
          canSignIn: phase !== "no-client-id",
          accounts: phase === "no-client-id" ? [] : ACCOUNTS,
        };
      case "login_start":
        channel = payload.channel as Channel;
        return 1;
      case "login_reopen_browser":
        throw { code: "openFailed", params: {}, action: null, detail: "gallery sample" };
      case "account_remove":
        if (phase === "remove-failed") {
          throw {
            code: "credentialStore",
            params: {},
            action: { kind: "retry" },
            detail: "gallery sample: access denied",
          };
        }
        return;
      default:
        return true;
    }
  });
  return { send: (e) => channel?.onmessage?.(e) };
}

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function run(phase: Phase) {
  const backend = startBackend(phase);
  const app = useApp.getState();
  await app.loadAccounts();
  const click = (selector: string) => document.querySelector<HTMLElement>(selector)?.click();
  const clickByText = (text: string) =>
    [...document.querySelectorAll<HTMLElement>("button, [role=menuitem]")]
      .find((b) => b.textContent?.trim() === text)
      ?.click();
  const startBrowser = async () => {
    await wait(300);
    clickByText(label("signIn.start"));
    await wait(100);
    backend.send({ event: "started", data: { op: 1 } });
  };
  switch (phase) {
    case "idle":
      app.openSignIn(null);
      break;
    case "resign":
      app.openSignIn({ id: "b", name: "Sixteen_Chars_Nm" });
      break;
    case "starting":
      app.openSignIn(null);
      await wait(300);
      clickByText(label("signIn.start"));
      break;
    case "waiting":
    case "waiting-reopen-failed":
      app.openSignIn(null);
      await startBrowser();
      backend.send({ event: "browserOpened" });
      if (phase === "waiting-reopen-failed") {
        await wait(300);
        clickByText(label("signIn.reopen"));
      }
      break;
    case "code":
      app.openSignIn(null);
      await wait(300);
      clickByText(label("signIn.useCode"));
      await wait(100);
      backend.send({ event: "started", data: { op: 1 } });
      backend.send({
        event: "deviceCode",
        data: {
          userCode: "ABCD1234",
          verificationUri: "https://example.invalid/link",
          expiresAt: Math.floor(Date.now() / 1000) + 14 * 60 + 32,
        },
      });
      break;
    case "completing":
      app.openSignIn(null);
      await startBrowser();
      backend.send({ event: "browserOpened" });
      backend.send({ event: "completing" });
      break;
    case "not-approved":
      app.openSignIn(null);
      await startBrowser();
      backend.send(failed("appNotApproved"));
      break;
    case "no-client-id":
      click("button");
      break;
    case "remove":
    case "remove-failed":
      app.openRemoveAccount(ACCOUNTS[0]);
      if (phase === "remove-failed") {
        await wait(300);
        clickByText(label("accounts.signOut"));
      }
      break;
    case "menu":
    case "menu-error":
      await wait(200);
      click("[data-slot=tooltip-trigger], [data-slot=dropdown-menu-trigger]");
      break;
    case "menu-sign-out":
      await wait(200);
      click("[data-slot=tooltip-trigger], [data-slot=dropdown-menu-trigger]");
      await wait(300);
      clickByText(label("accounts.signOut"));
      break;
    default: {
      // failed-<code>
      app.openSignIn(null);
      await startBrowser();
      const code = phase.slice("failed-".length);
      const event: Record<string, LoginEvent> = {
        network: failed("network", { kind: "retry" }),
        loopback: failed("loopbackUnavailable"),
        openFailed: failed("openFailed"),
        noXbox: failed("noXboxAccount", { kind: "openUrl", target: "xboxSignup" }),
        gameNotOwned: failed("gameNotOwned", { kind: "retry" }),
        appMisconfigured: failed("appMisconfigured"),
      };
      backend.send(event[code]);
    }
  }
}

/** The text of a key in the language the page shows, so the script finds its buttons in de and en. */
const label = (key: string) => i18n.t(key);

function params(): { phase: Phase; theme: "light" | "dark"; lang: "de" | "en" } {
  const query = new URLSearchParams(location.hash.split("?")[1] ?? "");
  const phase = (query.get("phase") ?? "idle") as Phase;
  return {
    phase: PHASES.includes(phase) ? phase : "idle",
    theme: query.get("theme") === "dark" ? "dark" : "light",
    lang: query.get("lang") === "de" ? "de" : "en",
  };
}

export function SignInGallery() {
  const [{ phase, theme, lang }] = useState(params);
  useEffect(() => {
    initI18n(lang, lang);
    setLanguage(lang);
    applyTheme(theme);
    document.documentElement.dataset.ready = "";
    void run(phase);
  }, [phase, theme, lang]);

  return (
    <TooltipProvider>
      <div className="flex h-dvh flex-col bg-surface text-foreground">
        <header className="flex min-h-14 items-center gap-4 border-b border-transparent px-6 py-3">
          <h1 className="min-w-40 font-display text-display">Gallery: {phase}</h1>
          <div className="ml-auto flex shrink-0 items-center gap-4">
            <AccountChip />
          </div>
        </header>
        <main className="p-6 text-small text-muted-foreground">
          sign-in gallery, {theme}, {lang}
        </main>
      </div>
      <AccountDialogs />
      <Toaster theme={theme} />
    </TooltipProvider>
  );
}
