import { CircleAlertIcon, ExternalLinkIcon } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import mark from "../../../brand/mark.svg";
import { Badge } from "@/components/Badge";
import { Delayed } from "@/components/Delayed";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { ErrorPanel } from "@/features/errors/ErrorPanel";
import { api, IpcError } from "@/ipc/client";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";
import { useApp } from "@/stores/app";

/** The "Source code" link: the address is fixed in Rust, only the target name is sent. */
function SourceLink() {
  const { t } = useTranslation();
  const [error, setError] = useState<ErrorDto | null>(null);
  const busy = useRef(false);
  const open = async () => {
    // A second click while the first is being handled opens nothing more (DESIGN.md 10.0).
    if (busy.current) return;
    busy.current = true;
    setError(null);
    try {
      await api.openUrl("repository");
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
  return (
    <div className="flex flex-col gap-3">
      <Button
        variant="link"
        className="self-start"
        aria-label={t("common.opensInBrowser", { name: t("about.source") })}
        onClick={() => void open()}
      >
        {t("about.source")}
        <ExternalLinkIcon aria-hidden="true" />
      </Button>
      {error && <ErrorPanel error={error} autoFocus />}
    </div>
  );
}

/** The facts block of "About Hengst Client" (DESIGN.md 11.7). Its own load state, its own retry. */
export function AboutPanel() {
  const { t } = useTranslation();
  const info = useApp((s) => s.info);
  const summary = useApp((s) => s.versionsSummary);
  const loadInfo = useApp((s) => s.loadInfo);
  const loadSummary = useApp((s) => s.loadVersionsSummary);

  useEffect(() => {
    if (info.status === "idle") void loadInfo();
    if (summary.status === "idle") void loadSummary();
  }, [info.status, summary.status, loadInfo, loadSummary]);

  const facts = () => {
    if (summary.status === "error" && !summary.data) {
      const offline = summary.error?.code === "network";
      return (
        <div className="flex items-center gap-2 text-small text-danger">
          <CircleAlertIcon className="size-4 shrink-0" aria-hidden="true" />
          <span>{t(offline ? "errors.versionsOffline" : "errors.versionsFailed")}</span>
          <Button variant="ghost" onClick={() => void loadSummary()}>
            {t("common.retry")}
          </Button>
        </div>
      );
    }
    if (!summary.data) {
      return (
        <Delayed>
          <Skeleton className="h-5 w-60" />
        </Delayed>
      );
    }
    const { count, latestRelease, latestSnapshot } = summary.data;
    if (count === 0) {
      return <p className="text-small text-muted-foreground">{t("about.noVersions")}</p>;
    }
    return (
      <div className="flex flex-col text-small text-muted-foreground tabular-nums">
        <p>{t("about.versionsCount", { count })}</p>
        <p>{t("about.latestRelease", { id: latestRelease })}</p>
        <p>{t("about.latestSnapshot", { id: latestSnapshot })}</p>
      </div>
    );
  };

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center gap-4">
        <img src={mark} alt="" className="size-12 shrink-0" />
        <div className="flex min-w-0 flex-col">
          <span className="text-heading">Hengst Client</span>
          {info.data ? (
            <span className="flex items-center gap-2 text-small text-muted-foreground tabular-nums">
              {t("about.version", { version: info.data.version })}
              {info.data.devBuild && <Badge>{t("common.devBuild")}</Badge>}
            </span>
          ) : (
            <Delayed>
              <Skeleton className="h-5 w-24" />
            </Delayed>
          )}
        </div>
      </div>
      <div className="flex flex-col text-small text-muted-foreground">
        <p lang="en">{t("about.legal")}</p>
        <p>{t("about.license")}</p>
      </div>
      <SourceLink />
      {facts()}
    </div>
  );
}
