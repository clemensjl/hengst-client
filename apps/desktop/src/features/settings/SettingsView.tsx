import { CircleAlertIcon, FolderOpenIcon } from "lucide-react";
import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Delayed } from "@/components/Delayed";
import { PathText } from "@/components/PathText";
import { Segments, type SegmentOption } from "@/components/Segments";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { AboutPanel } from "@/features/about/AboutPanel";
import { describeError } from "@/features/errors/errorCopy";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";
import { api, IpcError } from "@/ipc/client";
import type { Lang } from "@/i18n";
import type { ThemeSetting } from "@/theme/theme";
import { useApp } from "@/stores/app";

/**
 * A setting that saves when chosen (DESIGN.md 11.7). The chosen value shows at once; if saving fails
 * it falls back to the saved one and the help line says so. "Saving" shows after 400 ms.
 */
function useChoice<V extends string>(saved: V, save: (value: V) => Promise<unknown>) {
  const [pending, setPending] = useState<V | null>(null);
  const [failed, setFailed] = useState(false);
  const [slow, setSlow] = useState(false);
  const busy = useRef(false);
  const choose = async (value: V) => {
    // The guard sits before the first await: a second choice while one is saving is ignored.
    if (busy.current || value === saved) return;
    busy.current = true;
    setPending(value);
    setFailed(false);
    const timer = setTimeout(() => setSlow(true), 400);
    try {
      await save(value);
    } catch {
      setFailed(true);
    } finally {
      clearTimeout(timer);
      busy.current = false;
      setSlow(false);
      setPending(null);
    }
  };
  return { value: pending ?? saved, choose, failed, slow };
}

function Section({
  title,
  titleId,
  children,
}: {
  title: string;
  titleId: string;
  children: ReactNode;
}) {
  return (
    <section aria-labelledby={titleId}>
      <h2 id={titleId} className="text-heading">
        {title}
      </h2>
      <div className="mt-6">{children}</div>
    </section>
  );
}

/** One line of fixed height under a control: nothing moves when it fills. */
function HelpLine({ failed, slow }: { failed: boolean; slow: boolean }) {
  const { t } = useTranslation();
  return (
    <p
      className="mt-2 flex h-5 items-center gap-2 text-small text-muted-foreground"
      role={failed ? "alert" : "status"}
    >
      {failed ? (
        <span className="flex items-center gap-2 text-danger">
          <CircleAlertIcon className="size-4 shrink-0" aria-hidden="true" />
          {t("settings.saveFailed")}
        </span>
      ) : slow ? (
        t("common.saving")
      ) : null}
    </p>
  );
}

/** DESIGN.md 11.7 "Datenordner": the path, then "Open folder"; a refusal shows inline under it. */
function OpenDataFolder() {
  const { t } = useTranslation();
  const [error, setError] = useState<ErrorDto | null>(null);
  const busy = useRef(false);
  const open = async () => {
    if (busy.current) return;
    busy.current = true;
    setError(null);
    try {
      await api.openLocation({ kind: "dataDir" });
    } catch (e) {
      setError(
        e instanceof IpcError
          ? e.dto
          : { code: "internal", params: {}, action: null, detail: String(e) },
      );
    } finally {
      busy.current = false;
    }
  };
  const copy = error ? describeError(error, t, "folder") : null;
  return (
    <div className="mt-4 flex flex-col items-start gap-2">
      <Button onClick={() => void open()}>
        <FolderOpenIcon aria-hidden="true" />
        {t("instances.openFolder")}
      </Button>
      {copy && (
        <div role="alert" className="flex items-start gap-2 text-small">
          <CircleAlertIcon className="mt-0.5 size-4 shrink-0 text-danger" aria-hidden="true" />
          <span className="flex flex-col">
            <span className="text-danger">{copy.title}</span>
            <span className="text-muted-foreground">{copy.body}</span>
          </span>
        </div>
      )}
    </div>
  );
}

function SettingsSkeleton() {
  return (
    <Delayed>
      <div className="flex flex-col gap-12" aria-busy="true">
        {[0, 1].map((i) => (
          <div key={i} className="flex flex-col gap-6">
            <Skeleton className="h-[26px] w-[120px]" />
            <Skeleton className="h-10 w-80" />
          </div>
        ))}
      </div>
    </Delayed>
  );
}

export function SettingsView() {
  const { t } = useTranslation();
  const ids = [useId(), useId(), useId(), useId()];
  const settings = useApp((s) => s.settings.data);
  const info = useApp((s) => s.info);
  const loadInfo = useApp((s) => s.loadInfo);
  const update = useApp((s) => s.updateSettings);

  useEffect(() => {
    if (info.status === "idle") void loadInfo();
  }, [info.status, loadInfo]);

  const language = useChoice<Lang | "system">(settings?.language ?? "system", (v) =>
    update({ language: v === "system" ? { op: "clear" } : { op: "set", value: v } }),
  );
  const theme = useChoice<ThemeSetting>(settings?.theme ?? "system", (v) => update({ theme: v }));

  if (!settings) return <SettingsSkeleton />;

  const languages: SegmentOption<Lang | "system">[] = [
    { value: "system", label: t("settings.language.system") },
    { value: "de", label: t("settings.language.de") },
    { value: "en", label: t("settings.language.en") },
  ];
  const themes: SegmentOption<ThemeSetting>[] = [
    { value: "system", label: t("settings.theme.system") },
    { value: "light", label: t("settings.theme.light") },
    { value: "dark", label: t("settings.theme.dark") },
  ];

  return (
    <div className="flex max-w-160 flex-col gap-12">
      <Section title={t("settings.language.title")} titleId={ids[0]}>
        <Segments
          value={language.value}
          options={languages}
          onChange={language.choose}
          labelledBy={ids[0]}
        />
        <HelpLine failed={language.failed} slow={language.slow} />
      </Section>
      <Section title={t("settings.theme.title")} titleId={ids[1]}>
        <Segments
          value={theme.value}
          options={themes}
          onChange={theme.choose}
          labelledBy={ids[1]}
        />
        <HelpLine failed={theme.failed} slow={theme.slow} />
      </Section>
      <Section title={t("settings.dataFolder.title")} titleId={ids[2]}>
        {info.data ? (
          <>
            <PathText path={info.data.dataDir} />
            <OpenDataFolder />
          </>
        ) : (
          <Delayed>
            <Skeleton className="h-5 w-80" />
          </Delayed>
        )}
      </Section>
      <Section title={t("about.title")} titleId={ids[3]}>
        <AboutPanel />
      </Section>
    </div>
  );
}
