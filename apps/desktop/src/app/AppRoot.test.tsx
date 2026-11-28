import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { VirtuosoMockContext } from "react-virtuoso";
import { afterEach, describe, expect, it } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { resetLog } from "@/features/log/logStore";
import { LogView } from "@/features/log/LogView";
import { initI18n } from "@/i18n";
import type { LogBatch } from "@/ipc/bindings/LogBatch";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectItemText,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { AppRoot } from "./AppRoot";

afterEach(() => {
  cleanup();
  clearMocks();
  resetLog();
});

function OpenSelect() {
  return (
    <Select<string> items={[{ value: "a", label: "A" }]} defaultValue="a" defaultOpen>
      <SelectTrigger>
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="a">
          <SelectItemText>A</SelectItemText>
        </SelectItem>
      </SelectContent>
    </Select>
  );
}

const styleElements = () => document.querySelectorAll("style").length;

describe("AppRoot", () => {
  // The production CSP (`style-src 'self'`) blocks inline <style> elements in the WebView.
  it("an open select inside the app root adds no <style> element", async () => {
    const before = styleElements();
    render(
      <AppRoot>
        <OpenSelect />
      </AppRoot>,
    );
    await screen.findByRole("option", { name: "A" });
    expect(styleElements()).toBe(before);
  });

  it("control: without it the same select adds one", async () => {
    const before = styleElements();
    render(<OpenSelect />);
    await screen.findByRole("option", { name: "A" });
    expect(styleElements()).toBeGreaterThan(before);
  });

  // Virtuoso positions its rows with style attributes (allowed by `style-src-attr`), never with
  // a <style> element.
  it("the log list with lines inside the app root adds no <style> element", async () => {
    initI18n("en", "en-US");
    let channel: { onmessage?: (b: LogBatch) => void } = {};
    mockIPC((cmd, args) => {
      if (cmd === "log_runs")
        return [
          {
            runId: 1,
            instance: "a",
            since: 0,
            firstSeq: 1,
            nextSeq: 3,
            closed: false,
            state: { state: "running" },
          },
        ];
      if (cmd === "log_attach") channel = (args as { channel: typeof channel }).channel;
      if (cmd === "instances_list") return { instances: [], incomplete: false };
      if (cmd === "game_state") return { state: "idle", data: { lastExit: null } };
      return null;
    });
    const before = styleElements();
    render(
      <AppRoot>
        <VirtuosoMockContext.Provider value={{ viewportHeight: 400, itemHeight: 20 }}>
          <TooltipProvider>
            <LogView />
          </TooltipProvider>
        </VirtuosoMockContext.Provider>
      </AppRoot>,
    );
    await waitFor(() => expect(channel.onmessage).toBeDefined());
    const line = (seq: number) => ({
      seq,
      time: null,
      level: "info" as const,
      thread: "main",
      message: `line ${seq}`,
      stream: "stdout" as const,
      cut: false,
      part: 0,
      more: false,
    });
    await act(async () => {
      channel.onmessage?.({ ack: 1, runId: 1, dropped: 0, lines: [line(1), line(2)], end: null });
      await new Promise((r) => setTimeout(r, 300));
    });
    const scroller = document.querySelector("[data-virtuoso-scroller]")!;
    for (let i = 0; i < 3; i++) {
      await act(async () => {
        fireEvent.scroll(scroller);
        await new Promise((r) => setTimeout(r, 50));
      });
    }
    expect(await screen.findByText("line 2")).toBeTruthy();
    // Open the levels menu too (Base UI layers, the menu checkbox items).
    fireEvent.click(screen.getByRole("button", { name: "Levels: 5 of 5" }));
    await screen.findByRole("menuitemcheckbox", { name: "Info" });
    expect(styleElements()).toBe(before);
  });
});
