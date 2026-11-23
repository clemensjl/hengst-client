import { describe, expect, it } from "vitest";
import type { PlayEvent } from "@/ipc/bindings/PlayEvent";
import { initI18n } from "@/i18n";
import i18n from "@/i18n";
import { countLine, INITIAL, reduce, type LaunchProgress } from "./progress";

const run = (events: PlayEvent[], from: LaunchProgress = INITIAL) => events.reduce(reduce, from);

describe("progress reducer", () => {
  it("starts with nothing known and takes the operation id from preparing", () => {
    expect(INITIAL).toMatchObject({ op: null, phase: null, files: null, step: null });
    const p = run([{ event: "preparing", data: { op: 7 } }]);
    expect(p.op).toBe(7);
    // No phase until the first real one: the phase line (a live region) starts empty.
    expect(p.phase).toBeNull();
    expect(p.outcome).toBeNull();
  });

  it("maps every phase event to its phase line", () => {
    const cases: [PlayEvent, LaunchProgress["phase"]][] = [
      [{ event: "resolvingLoader", data: { kind: "fabric", game: "1.21.11" } }, "resolvingLoader"],
      [{ event: "downloadingInstaller", data: { kind: "forge" } }, "downloadingInstaller"],
      [{ event: "downloading", data: { what: "gameFiles", count: 4118 } }, "downloadingFiles"],
      [
        { event: "installStarted", data: { kind: "forge", version: "47.4.28", game: "1.20.1" } },
        "installing",
      ],
      [{ event: "installStep", data: { index: 3, total: 6, name: "jarsplitter" } }, "installing"],
      [{ event: "verifyingInstall" }, "verifying"],
      [{ event: "installFinished", data: { elapsedMs: 900 } }, "launching"],
      [{ event: "installReused" }, "launching"],
    ];
    for (const [event, phase] of cases)
      expect(reduce(INITIAL, event).phase, event.event).toBe(phase);
  });

  it("keeps the file count of the running batch and drops it when the batch is done", () => {
    let p = run([{ event: "downloading", data: { what: "assets", count: 4118 } }]);
    expect(p.files).toEqual({ count: 4118 });
    p = reduce(p, { event: "downloaded", data: { what: "assets" } });
    expect(p.files).toBeNull();
    // The phase stays: no flip to "launching" and back between batches.
    expect(p.phase).toBe("downloadingFiles");
  });

  it("a normal start changes the phase line only when the phase really changes", () => {
    const phases = [
      { event: "preparing", data: { op: 1 } },
      { event: "resolvingLoader", data: { kind: "fabric", game: "1.21.11" } },
      { event: "downloading", data: { what: "gameFiles", count: 30 } },
      { event: "downloaded", data: { what: "gameFiles" } },
      { event: "downloading", data: { what: "assets", count: 4118 } },
      { event: "downloaded", data: { what: "assets" } },
      { event: "downloading", data: { what: "java", count: 3 } },
      { event: "downloaded", data: { what: "java" } },
    ] as PlayEvent[];
    const seen: (string | null)[] = [];
    let p = INITIAL;
    for (const e of phases) {
      p = reduce(p, e);
      if (seen.at(-1) !== p.phase) seen.push(p.phase);
    }
    expect(seen).toEqual([null, "resolvingLoader", "downloadingFiles"]);
  });

  it("keeps the installer step and drops it for the next phase", () => {
    let p = run([
      { event: "installStarted", data: { kind: "forge", version: "47.4.28", game: "1.20.1" } },
      { event: "installStep", data: { index: 3, total: 6, name: "jarsplitter" } },
    ]);
    expect(p.step).toEqual({ index: 3, total: 6, name: "jarsplitter" });
    expect(p.files).toBeNull();
    p = reduce(p, { event: "verifyingInstall" });
    expect(p.step).toBeNull();
  });

  it("remembers the install log for the rest of the start, also after a failure", () => {
    const p = run([
      { event: "installLog", data: { path: "logs/loader-install-forge-1.20.1.log" } },
      { event: "installStep", data: { index: 1, total: 6, name: "a" } },
      {
        event: "failed",
        data: { error: { code: "installStepFailed", params: {}, action: null, detail: "" } },
      },
    ]);
    expect(p.installLog).toBe("logs/loader-install-forge-1.20.1.log");
    expect(p.outcome).toEqual({
      kind: "failed",
      error: { code: "installStepFailed", params: {}, action: null, detail: "" },
    });
  });

  it("keeps the last two notices", () => {
    const p = run(
      ["a", "b", "c"].map((message) => ({ event: "notice", data: { message } }) as PlayEvent),
    );
    expect(p.notices).toEqual(["b", "c"]);
  });

  it("ends with started, failed or cancelled", () => {
    expect(run([{ event: "started", data: { runId: 4 } }]).outcome).toEqual({
      kind: "started",
      runId: 4,
    });
    expect(run([{ event: "cancelled" }]).outcome).toEqual({ kind: "cancelled" });
  });

  it("does not change after its end (a late event cannot revive it)", () => {
    const ended = run([{ event: "cancelled" }]);
    expect(reduce(ended, { event: "downloading", data: { what: "java", count: 3 } })).toBe(ended);
  });
});

describe("count line", () => {
  it("groups the file count in the UI language and says the step with its number", () => {
    initI18n("de", "de-AT");
    const de = i18n.t.bind(i18n);
    const files = (count: number, t: typeof de, lang: string) =>
      countLine({ ...INITIAL, files: { count } }, t, lang);
    expect(files(4118, de, "de")).toEqual({ text: "4.118 Dateien", name: null });
    expect(files(1, de, "de")).toEqual({ text: "1 Datei", name: null });
    expect(
      countLine({ ...INITIAL, step: { index: 3, total: 6, name: "jarsplitter" } }, de, "de"),
    ).toEqual({ text: "Schritt 3 von 6:", name: "jarsplitter" });
    initI18n("en", "en-US");
    const en = i18n.t.bind(i18n);
    expect(files(4118, en, "en")).toEqual({ text: "4,118 files", name: null });
    expect(files(1, en, "en")).toEqual({ text: "1 file", name: null });
    expect(
      countLine({ ...INITIAL, step: { index: 1200, total: 3402, name: "x" } }, en, "en"),
    ).toEqual({ text: "Step 1,200 of 3,402:", name: "x" });
    expect(countLine(INITIAL, en, "en")).toBeNull();
  });
});
