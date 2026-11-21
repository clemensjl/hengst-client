import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { AboutPanel } from "./AboutPanel";

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("AboutPanel", () => {
  it("subscribes to events before it loads, so a refresh in between is not lost", async () => {
    const calls: string[] = [];
    mockIPC((cmd) => {
      calls.push(cmd);
      if (cmd === "app_info") {
        return { version: "0.1.0", dataDir: "C:\data", devBuild: false, devOffline: false };
      }
      if (cmd === "game_versions") {
        return { versions: [], latestRelease: "1.21.11", latestSnapshot: "x", fromCache: true };
      }
      return undefined;
    });
    render(<AboutPanel />);
    expect(await screen.findByText("1.21.11")).toBeTruthy();
    expect(calls[0]).toBe("subscribe_events");
    expect(calls.slice(1).sort()).toEqual(["app_info", "game_versions"]);
  });
});
