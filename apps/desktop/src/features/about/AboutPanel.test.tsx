import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
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
});
