import { useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import type { ErrorContext } from "@/features/errors/errorCopy";
import { ErrorPanel } from "@/features/errors/ErrorPanel";
import { initI18n, setLanguage, type Lang } from "@/i18n";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";
import { applyTheme, type ResolvedTheme } from "@/theme/theme";

/*
 * Dev-only page: every error state of Task 4 on one screen, for the visual check in the Vite dev
 * server (`#error-gallery`). main.tsx imports it only when `import.meta.env.DEV` is true, so a
 * production build contains neither this file nor its sample texts (devOnly.test.ts).
 */

const SAMPLES: ErrorDto[] = [
  {
    code: "network",
    params: { url: "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json" },
    action: { kind: "retry" },
    detail:
      "request to https://piston-meta.mojang.com/mc/game/version_manifest_v2.json failed: error sending request",
  },
  {
    code: "appNotApproved",
    params: {},
    action: { kind: "openUrl", target: "signInHelp" },
    detail:
      "the Minecraft services rejected this launcher's Azure app (HTTP 403): the client id is not approved by Mojang yet",
  },
  {
    code: "noLoaderVersions",
    params: {
      loader: "Legacy Fabric",
      loaderSlug: "legacy-fabric",
      game: "1.21.11",
      hint: "legacyFabricServesOld",
    },
    action: null,
    detail:
      "no legacy-fabric versions available for Minecraft 1.21.11 (Legacy Fabric serves 1.13.2 and older; try --loader fabric)",
  },
  {
    code: "offline",
    params: {},
    action: { kind: "retry" },
    detail: "offline: files are missing and cannot be downloaded",
  },
  {
    code: "credentialStore",
    params: {},
    action: { kind: "retry" },
    detail: "Windows Credential Manager error: access denied",
  },
  {
    code: "internal",
    params: {},
    action: { kind: "retry" },
    detail: "gallery-sample-detail:" + "0123456789abcdef".repeat(18).slice(0, 278),
  },
];

/** The places where the wording differs (a sample without one shows the generic copy). */
const CONTEXTS: Partial<Record<ErrorDto["code"], ErrorContext>> = {
  offline: "launch",
  credentialStore: "signIn",
};

/** Opens the details of the panel it wraps once, so the expanded state shows without a click. */
function Expanded({ children }: { children: React.ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    // Strict mode runs effects twice: click only while it is still closed.
    const trigger = ref.current?.querySelector<HTMLButtonElement>(
      "[data-slot=collapsible-trigger][aria-expanded=false]",
    );
    trigger?.click();
  }, []);
  return <div ref={ref}>{children}</div>;
}

export function ErrorGallery() {
  const [lang, setLang] = useState<Lang>(() =>
    initI18n(null, navigator.language).language === "de" ? "de" : "en",
  );
  const [theme, setTheme] = useState<ResolvedTheme>(() =>
    matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light",
  );
  useEffect(() => applyTheme(theme), [theme]);
  useEffect(() => setLanguage(lang), [lang]);

  return (
    <main className="min-h-screen bg-background p-6">
      <div className="mb-6 flex flex-wrap gap-2">
        <Button onClick={() => setLang("de")} aria-pressed={lang === "de"}>
          Deutsch
        </Button>
        <Button onClick={() => setLang("en")} aria-pressed={lang === "en"}>
          English
        </Button>
        <Button onClick={() => setTheme("light")} aria-pressed={theme === "light"}>
          Light
        </Button>
        <Button onClick={() => setTheme("dark")} aria-pressed={theme === "dark"}>
          Dark
        </Button>
      </div>
      <div className="flex flex-wrap items-start gap-8">
        {[false, true].map((expanded) => (
          <div key={String(expanded)} className="flex w-full max-w-[512px] flex-col gap-6">
            {SAMPLES.map((error) => {
              const panel = (
                <ErrorPanel
                  error={error}
                  context={CONTEXTS[error.code]}
                  onRetry={error.action?.kind === "retry" ? () => {} : undefined}
                />
              );
              return expanded ? (
                <Expanded key={error.code}>{panel}</Expanded>
              ) : (
                <div key={error.code}>{panel}</div>
              );
            })}
          </div>
        ))}
      </div>
    </main>
  );
}
