import { CircleAlertIcon } from "lucide-react";
import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { api } from "@/ipc/client";
import { countLine, type LaunchProgress } from "./progress";

/**
 * DESIGN.md 10.12: 160 px high from the moment preparing starts, so nothing below moves between
 * phases. `progress` is `null` for a start this page did not make (after a reload): nothing but
 * the phase is known then.
 */
export function ProgressPanel({ progress }: { progress: LaunchProgress | null }) {
  const { t, i18n } = useTranslation();
  const [openFailed, setOpenFailed] = useState(false);
  const opening = useRef(false);
  // A start of this page shows no phase until its first real one; a start found after a reload
  // (no channel, nothing known) shows "Launching game".
  const phase = progress ? progress.phase : "launching";
  const count = progress && countLine(progress, t, i18n.language);
  const step = progress?.step;
  const log = progress?.installLog ?? null;
  const notices = (progress?.notices ?? []).slice(openFailed ? -1 : -2);

  const openLog = async (path: string) => {
    if (opening.current) return;
    opening.current = true;
    setOpenFailed(false);
    try {
      // Only the path the backend reported; Rust checks it before anything is opened.
      await api.openLocation({ kind: "logFile", path });
    } catch {
      setOpenFailed(true);
    } finally {
      opening.current = false;
    }
  };

  return (
    // Fixed height and nothing drawn past it: the bottom slot holds at most two lines.
    <div data-slot="launch-progress" className="flex h-40 max-w-120 flex-col overflow-hidden">
      {/* The phase is announced; the counts below are not, they change too often. */}
      <p aria-live="polite" className="text-body">
        {phase && t(`play.progress.phase.${phase}`)}
      </p>
      <p className="mt-1 flex h-5 min-w-0 gap-1 text-small text-muted-foreground tabular-nums">
        {count && <span className="shrink-0">{count.text}</span>}
        {/* The step name comes from the loader's metadata: plain text, isolated so a bidi
            character cannot reorder the line, cut at the panel's edge. */}
        {count?.name && <bdi className="min-w-0 truncate">{count.name}</bdi>}
      </p>
      <div className="mt-2 h-1.5">
        {step && step.total > 0 && (
          <div
            role="progressbar"
            aria-label={t("play.progress.label")}
            aria-valuemin={0}
            aria-valuemax={step.total}
            aria-valuenow={step.index - 1}
            aria-valuetext={count ? `${count.text} ${count.name ?? ""}`.trim() : undefined}
            className="h-1.5 overflow-hidden rounded-pill bg-border-strong"
          >
            <div
              className="h-full w-full rounded-pill bg-foreground motion-progress"
              style={{
                transform: `scaleX(${Math.min(1, Math.max(0, (step.index - 1) / step.total))})`,
              }}
            />
          </div>
        )}
      </div>
      {log && (
        <div className="mt-3">
          <Button variant="link" onClick={() => void openLog(log)}>
            {t("play.progress.openLog")}
          </Button>
        </div>
      )}
      {/* The bottom slot, two lines at most: a failed log open takes the place of the older
          notice (an inline error stays next to what failed, DESIGN.md 10.11). */}
      {(openFailed || notices.length > 0) && (
        <div data-slot="launch-notes" className="mt-auto flex flex-col">
          {openFailed && (
            <p role="alert" className="flex items-center gap-2 text-small text-danger">
              <CircleAlertIcon className="size-4 shrink-0" aria-hidden="true" />
              <span className="min-w-0 truncate">{t("play.logOpenFailed")}</span>
            </p>
          )}
          {notices.map((n, i) => (
            // The core writes notices in English (like error details).
            <p key={i} lang="en" className="truncate text-caption text-muted-foreground">
              {n}
            </p>
          ))}
        </div>
      )}
    </div>
  );
}
