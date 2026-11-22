import { useEffect, useState } from "react";
import { toast } from "sonner";
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { AccountDialogs } from "@/features/accounts/AccountDialogs";
import { SettingsView } from "@/features/settings/SettingsView";
import i18n, { pickLanguage, setLanguage } from "@/i18n";
import type { SettingsNotice } from "@/ipc/bindings/SettingsNotice";
import { useApp } from "@/stores/app";
import { currentTheme, watchTheme, type ResolvedTheme } from "@/theme/theme";
import { Shell } from "./Shell";

/** The theme and the language follow the saved settings the moment they change. */
function useAppearance(): ResolvedTheme {
  const settings = useApp((s) => s.settings.data);
  const theme = settings?.theme ?? "system";
  const language = settings?.language ?? null;
  const [resolved, setResolved] = useState<ResolvedTheme>(() => currentTheme(theme));
  useEffect(() => watchTheme(theme, setResolved), [theme]);
  useEffect(() => setLanguage(pickLanguage(language, navigator.language)), [language]);
  return resolved;
}

export function App({ notice }: { notice?: SettingsNotice | null }) {
  const view = useApp((s) => s.view);
  const setView = useApp((s) => s.setView);
  const devBuild = useApp((s) => s.info.data?.devBuild ?? false);
  const resolved = useAppearance();

  // Declared after the children's effects run first, so the Toaster is mounted. The id makes a
  // second run (React strict mode) update the same toast instead of adding one.
  useEffect(() => {
    if (notice === "reset") toast(i18n.t("settings.resetNotice"), { id: "settings-reset" });
  }, [notice]);

  return (
    <TooltipProvider>
      <Shell view={view} onSelect={setView} devBuild={devBuild}>
        {view === "settings" ? <SettingsView /> : null /* Task 11 brings the instance list. */}
      </Shell>
      <AccountDialogs />
      <Toaster theme={resolved} />
    </TooltipProvider>
  );
}
