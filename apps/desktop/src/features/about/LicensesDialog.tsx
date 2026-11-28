import { ChevronDownIcon, ChevronUpIcon, CircleAlertIcon } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Delayed } from "@/components/Delayed";
import { Button } from "@/components/ui/button";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import {
  Dialog,
  DialogBody,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Skeleton } from "@/components/ui/skeleton";
import { ErrorPanel } from "@/features/errors/ErrorPanel";
import { loadLicenses, type LicenseEntry, type Licenses } from "./licenseData";

type State =
  { status: "loading" } | { status: "error"; detail: string } | { status: "ready"; data: Licenses };

/** One library: name, version and license expression; expanded, its license texts as plain text. */
function Entry({ entry, texts }: { entry: LicenseEntry; texts: string[] }) {
  const [open, setOpen] = useState(false);
  return (
    <li>
      <Collapsible open={open} onOpenChange={setOpen}>
        <CollapsibleTrigger
          render={<Button variant="ghost" className="h-auto min-h-10 w-full justify-start py-2" />}
        >
          <span className="flex min-w-0 flex-1 flex-wrap items-baseline gap-x-2 text-left">
            <span className="wrap-anywhere">{entry.name}</span>
            <span className="text-small font-normal text-muted-foreground tabular-nums">
              {entry.version}
            </span>
            <span className="text-caption font-normal text-muted-foreground">{entry.license}</span>
          </span>
          {open ? <ChevronUpIcon aria-hidden="true" /> : <ChevronDownIcon aria-hidden="true" />}
        </CollapsibleTrigger>
        <CollapsibleContent className="flex flex-col gap-2 pb-2">
          {entry.texts.map((i) => (
            <pre
              key={i}
              className="max-h-60 overflow-auto rounded-md bg-bg p-3 font-mono text-mono whitespace-pre-wrap select-text"
            >
              {texts[i] ?? ""}
            </pre>
          ))}
        </CollapsibleContent>
      </Collapsible>
    </li>
  );
}

function Group({
  title,
  entries,
  texts,
}: {
  title: string;
  entries: LicenseEntry[];
  texts: string[];
}) {
  return (
    <section className="flex flex-col gap-2">
      <h3 className="text-body font-semibold">{title}</h3>
      <ul className="flex flex-col">
        {entries.map((e) => (
          <Entry key={`${e.name}@${e.version}`} entry={e} texts={texts} />
        ))}
      </ul>
    </section>
  );
}

function Rows() {
  return (
    <Delayed>
      <div className="flex flex-col gap-2" aria-busy="true">
        {[0, 1, 2, 3, 4].map((i) => (
          <Skeleton key={i} className="h-10 w-full" />
        ))}
      </div>
    </Delayed>
  );
}

/**
 * DESIGN.md 11.10: the trigger in About and the dialog. The list is a static file of the bundle,
 * loaded on demand (its own chunk); no request leaves the app. `load` is replaceable for tests.
 */
export function LicensesDialog({ load = loadLicenses }: { load?: () => Promise<Licenses> }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [state, setState] = useState<State>({ status: "loading" });
  const attempt = useRef(0);

  const fetchList = useCallback(async () => {
    const mine = ++attempt.current;
    setState({ status: "loading" });
    try {
      const data = await load();
      if (attempt.current === mine) setState({ status: "ready", data });
    } catch (e) {
      if (attempt.current === mine) setState({ status: "error", detail: String(e) });
    }
  }, [load]);

  useEffect(() => {
    if (open && state.status !== "ready") void fetchList();
    // Only opening starts a load; a failed one waits for "Try again".
  }, [open]);

  const body = () => {
    if (state.status === "loading") {
      return (
        <>
          <h3 className="text-body font-semibold">{t("about.licenses.rust")}</h3>
          <Rows />
          <h3 className="text-body font-semibold">{t("about.licenses.javascript")}</h3>
          <Rows />
        </>
      );
    }
    if (state.status === "error") {
      return (
        <ErrorPanel
          error={{ code: "internal", params: {}, action: null, detail: state.detail }}
          copy={{ title: t("about.licenses.failed.title"), body: t("about.licenses.failed.body") }}
          onRetry={() => fetchList()}
        />
      );
    }
    const { rust, javascript, texts } = state.data;
    if (rust.length === 0 && javascript.length === 0) {
      return (
        <p role="alert" className="flex items-center gap-2 text-small text-danger">
          <CircleAlertIcon className="size-4 shrink-0" aria-hidden="true" />
          {t("about.licenses.empty")}
        </p>
      );
    }
    return (
      <>
        <Group title={t("about.licenses.rust")} entries={rust} texts={texts} />
        <Group title={t("about.licenses.javascript")} entries={javascript} texts={texts} />
      </>
    );
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger render={<Button className="self-start" />}>
        {t("about.thirdParty")}
      </DialogTrigger>
      <DialogContent className="w-[640px]">
        <DialogTitle className="pr-0">{t("about.thirdParty")}</DialogTitle>
        <DialogDescription>{t("about.licenses.intro")}</DialogDescription>
        <DialogBody className="flex flex-col gap-6">{body()}</DialogBody>
        <DialogFooter>
          <DialogClose render={<Button />}>{t("common.close")}</DialogClose>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
