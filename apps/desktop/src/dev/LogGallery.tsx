import { mockIPC } from "@tauri-apps/api/mocks";
import { Profiler, useEffect, useState } from "react";
import { Shell } from "@/app/Shell";
import { TooltipProvider } from "@/components/ui/tooltip";
import { LogView } from "@/features/log/LogView";
import { initI18n, setLanguage } from "@/i18n";
import type { LogBatch } from "@/ipc/bindings/LogBatch";
import type { LogLevelDto } from "@/ipc/bindings/LogLevelDto";
import type { LogLineDto } from "@/ipc/bindings/LogLineDto";
import type { LogRunDto } from "@/ipc/bindings/LogRunDto";
import { useApp } from "@/stores/app";
import { applyTheme } from "@/theme/theme";

/*
 * Dev-only page: the real log view inside the real shell against a fake backend, for the visual
 * and performance check with many lines in the Vite dev server:
 *
 *   #log-gallery?lines=10000&theme=dark&lang=de&live=1&state=lines
 *
 * `state`: lines (default), empty (no run), gone (the run expired). `live=1` keeps sending a
 * batch every 100 ms after the backfill. Render times of the log view go to `window.__logPerf`.
 * main.tsx imports it only when `import.meta.env.DEV` is true, so a production build contains
 * neither this file nor its texts (vite-plugins/noDevGallery.ts checks the output).
 */

const params = new URLSearchParams(location.hash.split("?")[1] ?? "");
const COUNT = Number(params.get("lines") ?? 10_000);
const STATE = params.get("state") ?? "lines";
const LIVE = params.get("live") === "1";

const LEVELS: (LogLevelDto | null)[] = ["info", "info", "info", "warn", "error", "debug", null];
const THREADS = ["Render thread", "Server thread", "Worker-Main-12", "main", "Netty Client IO #3"];

/** One sample message per seq: mixed lengths, a few 8 KiB lines, bidi, emoji, controls. */
function message(seq: number): string {
  if (seq % 2500 === 7) return `log gallery 8 KiB ${"x".repeat(8 * 1024)}`;
  if (seq % 997 === 0) return `log gallery unbroken ${"a".repeat(1200)}`;
  if (seq % 401 === 0) return "log gallery bidi ‮evil‬ and שלום after";
  if (seq % 333 === 0) return "log gallery emoji \u{1F600} and control \u0007\u001b[31mred";
  if (seq % 50 === 0)
    return `log gallery exception\njava.lang.IllegalStateException: boom ${seq}\n\tat a.b.C.run(C.java:42)\n\tat a.b.D.call(D.java:7)`;
  return `log gallery line ${seq} ${"word ".repeat(seq % 23)}`;
}

function line(seq: number, text = message(seq), part = 0, more = false): LogLineDto {
  return {
    seq,
    time: part === 0 ? { kind: "unixMillis", ms: Date.now() - (COUNT - seq) * 50 } : null,
    level: part === 0 ? LEVELS[seq % LEVELS.length] : null,
    thread: part === 0 ? THREADS[seq % THREADS.length] : null,
    message: text,
    stream: "stdout",
    cut: text.startsWith("log gallery 8 KiB"),
    part,
    more,
  };
}

/** The lines as the backend sends them: batches of at most 46, one line of 40 KiB in parts. */
function batches(from: number, to: number): LogBatch[] {
  const out: LogBatch[] = [];
  let lines: LogLineDto[] = [];
  const push = (l: LogLineDto) => {
    lines.push(l);
    if (lines.length >= 46) {
      out.push({ ack: out.length + 1, runId: 1, dropped: 0, lines, end: null });
      lines = [];
    }
  };
  for (let seq = from; seq <= to; seq++) {
    if (seq === 5000) {
      const parts = Array.from({ length: 6 }, (_, i) => `part ${i} ${"y".repeat(7000)} `);
      parts.forEach((p, i) => push(line(seq, `log gallery multi-part ${p}`, i, i < 5)));
    } else push(line(seq));
  }
  if (lines.length) out.push({ ack: out.length + 1, runId: 1, dropped: 0, lines, end: null });
  return out;
}

const RUN: LogRunDto = {
  runId: 1,
  instance: "survival",
  since: Date.now() - COUNT * 50,
  firstSeq: 1,
  nextSeq: COUNT + 1,
  closed: false,
  state: { state: "running" },
};

declare global {
  interface Window {
    __logPerf?: { phase: string; ms: number }[];
  }
}

export function LogGallery() {
  const [ready, setReady] = useState(false);
  useEffect(() => {
    initI18n((params.get("lang") as "de" | "en" | null) ?? "de", navigator.language);
    setLanguage((params.get("lang") as "de" | "en" | null) ?? "de");
    applyTheme(params.get("theme") === "light" ? "light" : "dark");
    let next = COUNT + 1;
    let timer: ReturnType<typeof setInterval> | undefined;
    mockIPC((cmd, args) => {
      switch (cmd) {
        case "log_runs":
          return STATE === "empty" ? [] : [RUN];
        case "log_attach": {
          if (STATE === "gone")
            throw { code: "invalidInput", params: { field: "runId" }, action: null, detail: "" };
          const channel = (args as { channel: { onmessage?: (b: LogBatch) => void } }).channel;
          const send = (b: LogBatch) => channel.onmessage?.(b);
          setTimeout(() => batches(1, COUNT).forEach(send), 0);
          if (LIVE) {
            clearInterval(timer);
            timer = setInterval(() => {
              batches(next, next + 9).forEach(send);
              next += 10;
            }, 100);
          }
          return null;
        }
        case "log_copy":
          return { text: "log gallery copy\n", lines: 1, omitted: 0 };
        case "instances_list":
          return { instances: [], incomplete: false };
        case "accounts_list":
          return { accounts: [], canSignIn: true };
        case "game_state":
          return { state: "running", data: { instance: "survival", runId: 1, since: RUN.since } };
        default:
          return null;
      }
    });
    useApp.setState({ view: "log" });
    window.__logPerf = [];
    setReady(true);
    return () => clearInterval(timer);
  }, []);
  if (!ready) return null;
  return (
    <TooltipProvider>
      <Shell view="log" onSelect={() => {}} devBuild fill>
        <Profiler
          id="log"
          onRender={(_id, phase, ms) =>
            window.__logPerf?.push({ phase, ms: Math.round(ms * 10) / 10 })
          }
        >
          <LogView />
        </Profiler>
      </Shell>
    </TooltipProvider>
  );
}
