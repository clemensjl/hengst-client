import type { TFunction } from "i18next";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";
import type { PlayEvent } from "@/ipc/bindings/PlayEvent";

/** The phase line of the launch panel (DESIGN.md 10.12). */
export type LaunchPhase =
  | "resolvingLoader"
  | "downloadingInstaller"
  | "downloadingFiles"
  | "verifying"
  | "installing"
  | "launching";

export type LaunchOutcome =
  { kind: "started"; runId: number } | { kind: "failed"; error: ErrorDto } | { kind: "cancelled" };

/** What one start has reported on its channel so far. */
export interface LaunchProgress {
  /** The operation id from `preparing`; until then the start cannot be cancelled. */
  op: number | null;
  phase: LaunchPhase | null;
  /**
   * The files of the batch being fetched. The core reports a batch's size at its start and its
   * end only, never a running count, so there is no "done" to show.
   */
  files: { count: number } | null;
  step: { index: number; total: number; name: string } | null;
  /** The install log the core reported, relative to the data folder. */
  installLog: string | null;
  /** The last two notices. */
  notices: string[];
  outcome: LaunchOutcome | null;
}

export const INITIAL: LaunchProgress = {
  op: null,
  phase: null,
  files: null,
  step: null,
  installLog: null,
  notices: [],
  outcome: null,
};

/** Next state after one event. Pure; an ended start ignores everything after its end. */
export function reduce(p: LaunchProgress, e: PlayEvent): LaunchProgress {
  if (p.outcome) return p;
  const phase = (phase: LaunchPhase): LaunchProgress => ({ ...p, phase, files: null, step: null });
  switch (e.event) {
    case "preparing":
      // No phase yet: the phase line starts empty until the first real phase.
      return { ...p, op: e.data.op };
    case "resolvingLoader":
      return phase("resolvingLoader");
    case "downloadingInstaller":
      return phase("downloadingInstaller");
    case "downloading":
      return { ...phase("downloadingFiles"), files: { count: e.data.count } };
    case "downloaded":
      // The batch is done; the next one (or the next phase) follows. Keeping the phase avoids a
      // flip to "launching" and back, which the live region would announce each time.
      return { ...p, files: null };
    case "installFinished":
    case "installReused":
      return phase("launching");
    case "installStarted":
      return phase("installing");
    case "installStep":
      return { ...phase("installing"), step: { ...e.data } };
    case "verifyingInstall":
      return phase("verifying");
    case "installLog":
      return { ...p, installLog: e.data.path };
    case "notice":
      return { ...p, notices: [...p.notices, e.data.message].slice(-2) };
    case "started":
      return { ...p, outcome: { kind: "started", runId: e.data.runId } };
    case "failed":
      return { ...p, outcome: { kind: "failed", error: e.data.error } };
    case "cancelled":
      return { ...p, outcome: { kind: "cancelled" } };
  }
}

/**
 * Line 2 of the panel in the UI language: the file count, or the installer step as its label
 * ("Schritt 3 von 6:") plus the step's name. The name comes from the loader's metadata, so it is
 * kept apart for the view to show as isolated plain text. `null` without either.
 */
export function countLine(
  p: LaunchProgress,
  t: TFunction,
  lang: string,
): { text: string; name: string | null } | null {
  const n = (v: number) => new Intl.NumberFormat(lang).format(v);
  if (p.step) {
    const text = t("play.progress.step", { index: n(p.step.index), total: n(p.step.total) });
    return { text, name: p.step.name };
  }
  if (p.files) return { text: t("play.progress.files", { count: p.files.count }), name: null };
  return null;
}
