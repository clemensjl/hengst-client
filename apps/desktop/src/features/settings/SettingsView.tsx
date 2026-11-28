import {
  ChevronDownIcon,
  ChevronUpIcon,
  CircleAlertIcon,
  FolderOpenIcon,
  LoaderCircleIcon,
  TriangleAlertIcon,
} from "lucide-react";
import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Delayed } from "@/components/Delayed";
import { PathText } from "@/components/PathText";
import { Segments, type SegmentOption } from "@/components/Segments";
import { Button } from "@/components/ui/button";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { AboutPanel } from "@/features/about/AboutPanel";
import { describeError } from "@/features/errors/errorCopy";
import { ErrorPanel } from "@/features/errors/ErrorPanel";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";
import type { JavaProblem } from "@/ipc/bindings/JavaProblem";
import { api, IpcError } from "@/ipc/client";
import type { Lang } from "@/i18n";
import type { ThemeSetting } from "@/theme/theme";
import { useApp } from "@/stores/app";

/** DESIGN.md 11.7: the memory range, as Rust checks it too (`MEMORY_MB`). */
const MEMORY_MIN = 512;
const MEMORY_MAX = 65536;

const errorOf = (e: unknown): ErrorDto =>
  e instanceof IpcError ? e.dto : { code: "internal", params: {}, action: null, detail: String(e) };

/**
 * A setting that saves when chosen (DESIGN.md 11.7). The chosen value shows at once; if saving fails
 * it falls back to the saved one and the help line says so. "Saving" shows after 400 ms.
 */
function useChoice<V extends string | boolean>(saved: V, save: (value: V) => Promise<unknown>) {
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

/**
 * The 20 px line under a field (DESIGN.md 10.7): help, a warning or an error, never more than one.
 * It holds a status and an alert region that both stay mounted; the text goes into the one its
 * tone needs (a node whose role flips between the two is not announced reliably).
 */
function FieldLine({
  id,
  tone,
  busy,
  children,
}: {
  id?: string;
  tone: "help" | "warning" | "error";
  busy?: boolean;
  children: ReactNode;
}) {
  const icon =
    tone === "error" ? (
      <CircleAlertIcon className="size-4 shrink-0" aria-hidden="true" />
    ) : tone === "warning" ? (
      <TriangleAlertIcon className="size-4 shrink-0" aria-hidden="true" />
    ) : busy ? (
      <LoaderCircleIcon className="spinner size-4 shrink-0" aria-hidden="true" />
    ) : null;
  const content = children ? (
    <>
      {icon}
      {children}
    </>
  ) : null;
  return (
    <p
      id={id}
      className={
        "mt-2 min-h-5 text-small " +
        (tone === "error"
          ? "text-danger"
          : tone === "warning"
            ? "text-warning"
            : "text-muted-foreground")
      }
    >
      <span role="status" className="flex items-center gap-2">
        {tone === "error" ? null : content}
      </span>
      <span role="alert" className="flex items-center gap-2">
        {tone === "error" ? content : null}
      </span>
    </p>
  );
}

/** Digits only, inside the range; anything else is out of range (DESIGN.md 11.7). */
function parseMemory(text: string): number | null {
  if (!/^\d{1,6}$/.test(text.trim())) return null;
  const value = Number(text.trim());
  return value >= MEMORY_MIN && value <= MEMORY_MAX ? value : null;
}

/**
 * DESIGN.md 11.7 "Memory" and 10.7: validated on blur, then live until fixed; saved on blur and on
 * Enter, Escape restores the saved value. Above the installed memory it is saved with a warning.
 */
function MemoryField({ titleId, saved }: { titleId: string; saved: number }) {
  const { t, i18n } = useTranslation();
  const lineId = useId();
  const info = useApp((s) => s.info.data);
  const update = useApp((s) => s.updateSettings);
  const [text, setText] = useState(String(saved));
  const [invalid, setInvalid] = useState(false);
  const [failed, setFailed] = useState(false);
  const [slow, setSlow] = useState(false);
  const busy = useRef(false);

  // A saved value that changes elsewhere (a reload) shows here, unless the user is editing.
  const shown = useRef(saved);
  useEffect(() => {
    if (shown.current !== saved) {
      shown.current = saved;
      setText(String(saved));
    }
  }, [saved]);

  const number = (n: number, digits = 0) =>
    new Intl.NumberFormat(i18n.language, {
      minimumFractionDigits: digits,
      maximumFractionDigits: digits,
    }).format(n);

  const commit = async () => {
    const value = parseMemory(text);
    if (value === null) {
      setInvalid(true);
      return;
    }
    setInvalid(false);
    // The guard sits before the first await: Enter and the blur after it save once.
    if (busy.current || value === saved) return;
    busy.current = true;
    setFailed(false);
    const timer = setTimeout(() => setSlow(true), 400);
    try {
      await update({ memoryMb: value });
    } catch (e) {
      const dto = errorOf(e);
      if (dto.code === "invalidInput") setInvalid(true);
      else {
        setFailed(true);
        setText(String(saved));
      }
    } finally {
      clearTimeout(timer);
      busy.current = false;
      setSlow(false);
    }
  };

  const installed = info?.installedMemoryMb ?? null;
  let line: { tone: "help" | "warning" | "error"; text: string };
  if (invalid) {
    line = {
      tone: "error",
      text: t("settings.memory.range", { min: number(MEMORY_MIN), max: number(MEMORY_MAX) }),
    };
  } else if (failed) {
    line = { tone: "error", text: t("settings.saveFailed") };
  } else if (slow) {
    line = { tone: "help", text: t("common.saving") };
  } else if (installed !== null && saved > installed) {
    line = { tone: "warning", text: t("settings.memory.aboveInstalled") };
  } else if (installed !== null) {
    line = {
      tone: "help",
      text: t("settings.memory.help", { installed: number(installed / 1024, 1) }),
    };
  } else {
    line = { tone: "help", text: info ? t("settings.memory.helpNoInstalled") : "" };
  }

  return (
    <div>
      <div className="flex w-40 items-center gap-2">
        <Input
          aria-labelledby={titleId}
          aria-describedby={lineId}
          aria-invalid={invalid || undefined}
          inputMode="numeric"
          value={text}
          onChange={(e) => {
            setText(e.target.value);
            setFailed(false);
            // After the first error the check runs on every keystroke until it passes (FO2).
            if (invalid) setInvalid(parseMemory(e.target.value) === null);
          }}
          onBlur={() => void commit()}
          onKeyDown={(e) => {
            if (e.key === "Enter") void commit();
            if (e.key === "Escape") {
              setText(String(saved));
              setInvalid(false);
            }
          }}
          className="tabular-nums"
        />
        <span className="text-small text-muted-foreground" aria-hidden="true">
          {t("settings.memory.unit")}
        </span>
      </div>
      <FieldLine id={lineId} tone={line.tone}>
        {line.text}
      </FieldLine>
    </div>
  );
}

const PROBLEM_KEYS: Record<JavaProblem, string> = {
  location: "settings.java.location",
  name: "settings.java.name",
  missing: "settings.java.missing",
  unavailable: "settings.java.unavailable",
  notStarted: "settings.java.notStarted",
};

/** DESIGN.md 11.7 "Java (optional)": the path is chosen in a Windows dialog opened by Rust. */
function JavaSetting() {
  const { t } = useTranslation();
  const labelId = useId();
  const lineId = useId();
  const java = useApp((s) => s.settings.data?.javaOverride ?? null);
  const chooseJava = useApp((s) => s.chooseJava);
  const update = useApp((s) => s.updateSettings);
  const [checking, setChecking] = useState(false);
  const [version, setVersion] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const busy = useRef(false);
  const chooseRef = useRef<HTMLButtonElement>(null);
  const focusChoose = useRef(false);

  useEffect(() => {
    if (!java && focusChoose.current) {
      focusChoose.current = false;
      chooseRef.current?.focus();
    }
  }, [java]);

  const run = async (action: () => Promise<void>) => {
    if (busy.current) return;
    busy.current = true;
    setError(null);
    try {
      await action();
    } catch (e) {
      const dto = errorOf(e);
      const reason = dto.params.reason as JavaProblem | undefined;
      setError(
        dto.code === "javaInvalid" && reason && reason in PROBLEM_KEYS
          ? t(PROBLEM_KEYS[reason])
          : t("settings.saveFailed"),
      );
    } finally {
      busy.current = false;
      setChecking(false);
    }
  };

  const choose = () =>
    run(async () => {
      // While the native dialog is open nothing is checked yet: "Checking" only after the pick.
      const answer = await chooseJava(() => setChecking(true));
      if (answer.kind === "chosen") setVersion(answer.version);
    });
  const reset = () =>
    run(async () => {
      focusChoose.current = true;
      await update({ javaOverride: { op: "clear" } });
      setVersion(null);
    });

  const stored = java?.problem ? t(PROBLEM_KEYS[java.problem]) : null;
  const line = checking
    ? { tone: "help" as const, text: t("settings.java.checking") }
    : error
      ? { tone: "error" as const, text: error }
      : stored
        ? { tone: "error" as const, text: stored }
        : { tone: "help" as const, text: version ? t("settings.java.version", { version }) : "" };

  return (
    <div role="group" aria-labelledby={labelId} className="flex flex-col gap-2">
      <p id={labelId} className="text-label">
        {t("settings.java.label")}
      </p>
      {java ? (
        <PathText path={java.path} />
      ) : (
        <p className="text-small text-muted-foreground">{t("settings.java.automatic")}</p>
      )}
      <div className="flex flex-wrap gap-2">
        <Button ref={chooseRef} aria-describedby={lineId} onClick={() => void choose()}>
          {t("settings.java.choose")}
        </Button>
        {java && (
          // While a picked file is checked, Reset waits; the help line ("Checking…") says why
          // (DESIGN.md 10.0).
          <Button
            variant="ghost"
            aria-disabled={checking || undefined}
            aria-describedby={checking ? lineId : undefined}
            onClick={() => {
              if (!checking) void reset();
            }}
          >
            {t("settings.java.reset")}
          </Button>
        )}
      </div>
      <FieldLine id={lineId} tone={line.tone} busy={checking}>
        {line.text}
      </FieldLine>
    </div>
  );
}

/**
 * The "Advanced" collapsible under Memory (DESIGN.md 11.7, 10.15). Open while the stored Java path
 * cannot be used (`forced`), from the first render on; it stays open once that is fixed.
 */
function Advanced({ forced, children }: { forced: boolean; children: ReactNode }) {
  const { t } = useTranslation();
  const [chosen, setChosen] = useState(forced);
  useEffect(() => {
    if (forced) setChosen(true);
  }, [forced]);
  const open = chosen || forced;
  const reasonId = useId();
  return (
    <Collapsible open={open} onOpenChange={(next) => setChosen(next || forced)} className="mt-4">
      {/* While forced the trigger does nothing: aria-disabled with its reason (DESIGN.md 10.0). */}
      <CollapsibleTrigger
        render={<Button variant="ghost" />}
        aria-disabled={forced || undefined}
        aria-describedby={forced ? reasonId : undefined}
      >
        {t("settings.advanced")}
        {open ? <ChevronUpIcon aria-hidden="true" /> : <ChevronDownIcon aria-hidden="true" />}
      </CollapsibleTrigger>
      {forced && (
        <span id={reasonId} className="sr-only">
          {t("settings.advancedOpenReason")}
        </span>
      )}
      <CollapsibleContent className="mt-4">{children}</CollapsibleContent>
    </Collapsible>
  );
}

/** DESIGN.md 11.7 "Launch": the whole 40 px row (label and switch) toggles. */
function MinimizeSwitch({ saved }: { saved: boolean }) {
  const { t } = useTranslation();
  const update = useApp((s) => s.updateSettings);
  const choice = useChoice<boolean>(saved, (v) => update({ minimizeOnLaunch: v }));
  return (
    <>
      <label className="flex min-h-10 cursor-pointer items-center gap-3 text-small">
        <Switch checked={choice.value} onCheckedChange={(v) => void choice.choose(v)} />
        {t("settings.launch.minimize")}
      </label>
      <HelpLine failed={choice.failed} slow={choice.slow} />
    </>
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
      setError(errorOf(e));
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
  const ids = [useId(), useId(), useId(), useId(), useId(), useId()];
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

  const dataFolder = () => {
    if (info.data) {
      return (
        <>
          <PathText path={info.data.dataDir} />
          <OpenDataFolder />
        </>
      );
    }
    if (info.status === "error" && info.error) {
      return <ErrorPanel error={info.error} onRetry={() => loadInfo()} />;
    }
    return (
      <Delayed>
        <Skeleton className="h-5 w-80" />
      </Delayed>
    );
  };

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
      <Section title={t("settings.memory.title")} titleId={ids[2]}>
        <MemoryField titleId={ids[2]} saved={settings.memoryMb} />
        <Advanced forced={Boolean(settings.javaOverride?.problem)}>
          <JavaSetting />
        </Advanced>
      </Section>
      <Section title={t("settings.launch.title")} titleId={ids[3]}>
        <MinimizeSwitch saved={settings.minimizeOnLaunch} />
      </Section>
      <Section title={t("settings.dataFolder.title")} titleId={ids[4]}>
        {dataFolder()}
      </Section>
      <Section title={t("about.title")} titleId={ids[5]}>
        <AboutPanel />
      </Section>
    </div>
  );
}
