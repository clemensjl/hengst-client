import { CircleAlertIcon } from "lucide-react";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import mark from "../../../brand/mark.svg";
import { Badge } from "@/components/Badge";
import { Delayed } from "@/components/Delayed";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useApp } from "@/stores/app";
import type { VersionFilter } from "@/ipc/bindings/VersionFilter";

const ALL_KINDS: VersionFilter = { release: true, snapshot: true, beta: true, alpha: true };

/** The facts block of "About Hengst Client" (DESIGN.md 11.7). Its own load state, its own retry. */
export function AboutPanel() {
  const { t } = useTranslation();
  const info = useApp((s) => s.info);
  const versions = useApp((s) => s.versions);
  const loadInfo = useApp((s) => s.loadInfo);
  const loadVersions = useApp((s) => s.loadVersions);

  useEffect(() => {
    if (info.status === "idle") void loadInfo();
    if (versions.status === "idle") void loadVersions(ALL_KINDS);
  }, [info.status, versions.status, loadInfo, loadVersions]);

  const facts = () => {
    if (versions.status === "error" && !versions.data) {
      const offline = versions.error?.code === "network";
      return (
        <div className="flex items-center gap-2 text-small text-danger">
          <CircleAlertIcon className="size-4 shrink-0" aria-hidden="true" />
          <span>{t(offline ? "errors.versionsOffline" : "errors.versionsFailed")}</span>
          <Button variant="ghost" onClick={() => void loadVersions(ALL_KINDS)}>
            {t("common.retry")}
          </Button>
        </div>
      );
    }
    if (!versions.data)
      return (
        <Delayed>
          <Skeleton className="h-5 w-60" />
        </Delayed>
      );
    const { versions: list, latestRelease, latestSnapshot } = versions.data;
    if (list.length === 0)
      return <p className="text-small text-muted-foreground">{t("about.noVersions")}</p>;
    return (
      <div className="flex flex-col text-small text-muted-foreground tabular-nums">
        <p>{t("about.versionsCount", { count: list.length })}</p>
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
      {facts()}
    </div>
  );
}
