import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useApp } from "@/stores/app";
import { Boot } from "./Boot";

const initial = useApp.getState();
beforeEach(() => {
  useApp.setState(initial, true);
  vi.stubGlobal("matchMedia", () => ({
    matches: false,
    addEventListener: () => {},
    removeEventListener: () => {},
  }));
});
afterEach(() => {
  cleanup();
  clearMocks();
  vi.unstubAllGlobals();
  document.documentElement.classList.remove("dark");
  document.documentElement.removeAttribute("data-ready");
});

const dto = (over: object = {}) => ({
  language: null,
  theme: "system",
  memoryMb: 2048,
  javaOverride: null,
  minimizeOnLaunch: true,
  selectedInstance: null,
  devAccount: false,
  notice: null,
  ...over,
});
const info = { version: "0.1.0", dataDir: "C:\\data", devBuild: false, devOffline: false };

function backend(settings: () => object, calls: string[] = []) {
  mockIPC((cmd) => {
    calls.push(cmd);
    if (cmd === "settings_get") return settings();
    if (cmd === "app_info") return info;
    return undefined;
  });
  return calls;
}

describe("Boot", () => {
  it("subscribes to events before it reads the settings, then renders the shell", async () => {
    const calls = backend(() => dto({ language: "en", theme: "dark" }));
    render(<Boot />);
    expect(await screen.findByRole("navigation", { name: "Main navigation" })).toBeTruthy();
    expect(calls.slice(0, 2)).toEqual(["subscribe_events", "settings_get"]);
    expect(document.documentElement.classList.contains("dark")).toBe(true);
    expect(document.documentElement.lang).toBe("en");
    expect(document.documentElement.hasAttribute("data-ready")).toBe(true);
  });

  it("the damaged-settings toast appears once, from the first answer only", async () => {
    let first = true;
    backend(() => {
      const notice = first ? "reset" : null;
      first = false;
      return dto({ language: "de", notice });
    });
    render(<Boot />);
    expect(
      await screen.findByText("Einstellungen waren beschädigt und wurden zurückgesetzt"),
    ).toBeTruthy();
    // A reload (what a DataChanged event does) brings no second toast.
    await useApp.getState().loadSettings();
    expect(
      screen.getAllByText("Einstellungen waren beschädigt und wurden zurückgesetzt"),
    ).toHaveLength(1);
  });

  it("applies the system theme before the settings are known, so the error block is not light on dark", async () => {
    vi.stubGlobal("matchMedia", () => ({
      matches: true,
      addEventListener: () => {},
      removeEventListener: () => {},
    }));
    backend(() => {
      throw { code: "internal", params: {}, action: null, detail: "x" };
    });
    render(<Boot />);
    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(document.documentElement.classList.contains("dark")).toBe(true);
  });

  it("shows an error with a retry when the settings cannot be read, then recovers", async () => {
    let fail = true;
    backend(() => {
      if (fail) throw { code: "internal", params: {}, action: null, detail: "x" };
      return dto({ language: "en" });
    });
    render(<Boot />);
    expect(await screen.findByRole("alert")).toBeTruthy();
    // The error block shows in the theme the WebView already has (here: light), painted at once.
    expect(document.documentElement.hasAttribute("data-ready")).toBe(true);
    expect(document.documentElement.classList.contains("dark")).toBe(false);
    fail = false;
    fireEvent.click(screen.getByRole("button"));
    await waitFor(() => expect(screen.getByRole("navigation")).toBeTruthy());
  });
});
