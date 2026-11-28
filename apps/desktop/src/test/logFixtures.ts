// Test helper: lines and batches of the live log as the backend sends them.
import type { LogBatch } from "@/ipc/bindings/LogBatch";
import type { LogLineDto } from "@/ipc/bindings/LogLineDto";

export const line = (seq: number, message = `line ${seq}`, extra: Partial<LogLineDto> = {}) =>
  ({
    seq,
    time: null,
    level: "info",
    thread: "main",
    message,
    stream: "stdout",
    cut: false,
    part: 0,
    more: false,
    ...extra,
  }) as LogLineDto;

export const batch = (lines: LogLineDto[], extra: Partial<LogBatch> = {}): LogBatch => ({
  ack: 1,
  runId: 7,
  dropped: 0,
  lines,
  end: null,
  ...extra,
});
