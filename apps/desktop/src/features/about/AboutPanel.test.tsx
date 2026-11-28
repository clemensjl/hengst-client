import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { initI18n } from "@/i18n";
import { useApp } from "@/stores/app";
import { AboutPanel } from "./AboutPanel";

initI18n("en", "en-US");

const initial = useApp.getState();
beforeEach(() => useApp.setState(initial, true));
afterEach(() => {
  cleanup();
  clearMocks();
});

const info = { version: "0.1.0", dataDir: "C:\\data", devBuild: true, devOffline: false };
const versions = (n: number) => ({
  versions: Array.from({ length: n }, (_, i) => ({
    id: `v${i}`,
    kind: "release",
    releaseTime: "",
  })),
  latestRelease: "1.21.11",
  latestSnapshot: "26.4-snapshot-1",
  fromCache: true,
});

describe("AboutPanel", () => {
  it("shows the version, the development badge, the legal line and the version facts", async () => {
    mockIPC((cmd) => (cmd === "app_info" ? info : versions(1234)));
    render(<AboutPanel />);
    expect(await screen.findByText("Latest release: 1.21.11")).toBeTruthy();
    expect(screen.getByText("Version 0.1.0")).toBeTruthy();
    expect(screen.getByText("Development build")).toBeTruthy();
    expect(screen.getByText("Not an official Minecraft product.")).toBeTruthy();
    expect(screen.getByText("License: GPL-3.0")).toBeTruthy();
    expect(screen.getByText("1,234 game versions known")).toBeTruthy();
    expect(screen.getByText("Latest snapshot: 26.4-snapshot-1")).toBeTruthy();
  });

  it("keeps its own count and never sets the shared version filter", async () => {
    mockIPC((cmd) => (cmd === "app_info" ? info : versions(5)));
    render(<AboutPanel />);
    expect(await screen.findByText("5 game versions known")).toBeTruthy();
    expect(useApp.getState().versions.status).toBe("idle");
    expect(useApp.getState().versionsSummary.status).toBe("ready");
  });

  it("opens the repository through the fixed target name and shows a failure as an error", async () => {
    const calls: [string, unknown][] = [];
    let fail = false;
    mockIPC((cmd, args) => {
      calls.push([cmd, args]);
      if (cmd === "app_info") return info;
      if (cmd === "open_url" && fail) {
        throw { code: "openFailed", params: {}, action: null, detail: "refused" };
      }
      return cmd === "open_url" ? undefined : versions(1);
    });
    render(<AboutPanel />);
    const link = await screen.findByRole("button", { name: "Source code (opens in browser)" });
    fireEvent.click(link);
    await waitFor(() => expect(calls).toContainEqual(["open_url", { target: "repository" }]));
    expect(screen.queryByRole("alert")).toBeNull();
    fail = true;
    fireEvent.click(link);
    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(screen.getByText("The link could not be opened")).toBeTruthy();
  });

  it("a double click on Source code opens it once", async () => {
    let calls = 0;
    mockIPC(async (cmd) => {
      if (cmd === "app_info") return info;
      if (cmd === "open_url") {
        calls += 1;
        await new Promise((r) => setTimeout(r, 50));
        return undefined;
      }
      return versions(1);
    });
    render(<AboutPanel />);
    const link = await screen.findByRole("button", { name: "Source code (opens in browser)" });
    fireEvent.click(link);
    fireEvent.click(link);
    await waitFor(() => expect(calls).toBe(1));
    await new Promise((r) => setTimeout(r, 100));
    expect(calls).toBe(1);
  });

  it("counts one version in the singular", async () => {
    mockIPC((cmd) => (cmd === "app_info" ? info : versions(1)));
    render(<AboutPanel />);
    expect(await screen.findByText("1 game version known")).toBeTruthy();
  });

  it("says so when no version list is loaded yet", async () => {
    mockIPC((cmd) => (cmd === "app_info" ? info : versions(0)));
    render(<AboutPanel />);
    expect(await screen.findByText("No version list loaded yet")).toBeTruthy();
  });

  it("a failed list shows its own inline error with a retry, and the rest stays", async () => {
    let fail = true;
    mockIPC((cmd) => {
      if (cmd === "app_info") return info;
      if (fail) throw { code: "network", params: {}, action: { kind: "retry" }, detail: "x" };
      return versions(3);
    });
    render(<AboutPanel />);
    expect(
      await screen.findByText("No connection. The version list needs the internet the first time."),
    ).toBeTruthy();
    expect(screen.getByText("Version 0.1.0")).toBeTruthy();
    fail = false;
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("3 game versions known")).toBeTruthy();
  });

  it("a failed app info shows an inline error with a retry instead of a skeleton forever", async () => {
    let fail = true;
    mockIPC((cmd) => {
      if (cmd === "app_info") {
        if (fail) throw { code: "internal", params: {}, action: null, detail: "x" };
        return info;
      }
      return versions(1);
    });
    render(<AboutPanel />);
    expect(await screen.findByText("The version could not be loaded.")).toBeTruthy();
    fail = false;
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("Version 0.1.0")).toBeTruthy();
  });

  it("Third-party licenses opens the license dialog", async () => {
    mockIPC((cmd) => (cmd === "app_info" ? info : versions(1)));
    render(<AboutPanel />);
    fireEvent.click(screen.getByRole("button", { name: "Third-party licenses" }));
    expect(await screen.findByRole("dialog", { name: "Third-party licenses" })).toBeTruthy();
  });
});
