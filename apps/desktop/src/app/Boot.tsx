import { CircleAlertIcon } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Delayed } from "@/components/Delayed";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { initI18n } from "@/i18n";
import type { SettingsNotice } from "@/ipc/bindings/SettingsNotice";
import { useApp } from "@/stores/app";
import { applyTheme, currentTheme } from "@/theme/theme";
import { App } from "./App";

type Phase =
  { kind: "loading" } | { kind: "ready"; notice: SettingsNotice | null } | { kind: "error" };

/** DESIGN.md 11.0: a shell skeleton if loading takes over 200 ms. No text, so no language is needed yet. */
function ShellSkeleton() {
  return (
    <Delayed>
      <div className="flex h-dvh" aria-busy="true">
        <div className="flex w-16 shrink-0 flex-col gap-2 border-r bg-background p-2 wide:w-54">
          <Skeleton className="h-10" />
          <Skeleton className="h-10" />
        </div>
        <div className="flex-1 bg-surface px-6 py-4 wide:px-8">
          <Skeleton className="h-8 w-48" />
        </div>
      </div>
    </Delayed>
  );
}

/** The settings could not be read even after the damaged-file fallback (DESIGN.md 11.0). */
function StartError({ onRetry }: { onRetry: () => void }) {
  const { t } = useTranslation();
  return (
    <div className="mx-auto mt-24 w-120 max-w-[calc(100%-48px)]">
      <div role="alert" className="flex flex-col gap-3 rounded-lg border bg-surface-raised p-4">
        <div className="flex items-start gap-3">
          <CircleAlertIcon className="mt-0.5 size-[18px] shrink-0 text-danger" aria-hidden="true" />
          <div className="flex flex-col">
            <h1 className="text-body font-semibold">{t("errors.settingsLoadFailed.title")}</h1>
            <p className="text-small text-muted-foreground">
              {t("errors.settingsLoadFailed.body")}
            </p>
          </div>
        </div>
        <div className="pl-[30px]">
          <Button onClick={onRetry}>{t("common.retry")}</Button>
        </div>
      </div>
    </div>
  );
}

/**
 * Start: subscribe to backend events first, then read the settings, then set up the language and
 * the theme, then render. Until then the page paints nothing and the window shows the native
 * background that Rust set from `--bg`.
 */
export function Boot() {
  const [phase, setPhase] = useState<Phase>({ kind: "loading" });
  const started = useRef(false);

  const start = useCallback(async () => {
    setPhase({ kind: "loading" });
    // The skeleton and the error block may show before the settings are known. The WebView's own
    // color scheme is what Rust set at window creation from the saved theme (a forced light or
    // dark, or the system's), so it is the right theme for them; the saved setting then takes over.
    applyTheme(currentTheme("system"));
    document.documentElement.dataset.ready = "";
    const app = useApp.getState();
    // Events before the first load; a failed subscription only loses live updates.
    await app.startEventLoop().catch((e) => console.error("event channel:", e));
    try {
      const settings = await app.loadSettings();
      initI18n(settings.language, navigator.language);
      applyTheme(currentTheme(settings.theme));
      void app.loadInfo();
      void app.loadAccounts();
      setPhase({ kind: "ready", notice: settings.notice });
    } catch {
      initI18n(null, navigator.language);
      setPhase({ kind: "error" });
    }
  }, []);

  useEffect(() => {
    // React strict mode runs effects twice in development; the settings notice must not be consumed twice.
    if (started.current) return;
    started.current = true;
    void start();
  }, [start]);

  if (phase.kind === "loading") return <ShellSkeleton />;
  if (phase.kind === "error") return <StartError onRetry={() => void start()} />;
  return <App notice={phase.notice} />;
}
