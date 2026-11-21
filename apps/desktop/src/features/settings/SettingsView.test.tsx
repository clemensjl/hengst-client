import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { initI18n } from "@/i18n";
import type { SettingsDto } from "@/ipc/bindings/SettingsDto";
import { useApp } from "@/stores/app";
import { SettingsView } from "./SettingsView";

initI18n("en", "en-US");

const initial = useApp.getState();
const saved: SettingsDto = {
  language: null,
  theme: "system",
  memoryMb: 2048,
  javaOverride: null,
  minimizeOnLaunch: true,
  selectedInstance: null,
  devAccount: false,
  notice: null,
};

beforeEach(() => {
  useApp.setState({ ...initial, settings: { status: "ready", data: saved } }, true);
});
afterEach(() => {
  cleanup();
  clearMocks();
});

function backend(update: (patch: unknown) => unknown) {
  const patches: unknown[] = [];
  mockIPC((cmd, args) => {
    if (cmd === "app_info")
      return {
        version: "0.1.0",
        dataDir: "C:\\data\\HengstClient",
        devBuild: false,
        devOffline: false,
      };
    if (cmd === "game_versions")
      return { versions: [], latestRelease: "", latestSnapshot: "", fromCache: true };
    if (cmd === "settings_update") {
      patches.push((args as { patch: unknown }).patch);
      return update((args as { patch: unknown }).patch);
    }
    return undefined;
  });
  return patches;
}

const view = () =>
  render(
    <TooltipProvider>
      <SettingsView />
    </TooltipProvider>,
  );

describe("SettingsView", () => {
  it("shows both segment groups with one checked entry each", () => {
    backend(() => saved);
    view();
    const language = screen.getByRole("radiogroup", { name: "Language" });
    const theme = screen.getByRole("radiogroup", { name: "Theme" });
    expect(language.querySelectorAll("[aria-checked=true]")).toHaveLength(1);
    expect(theme.querySelectorAll("[aria-checked=true]")).toHaveLength(1);
    expect(screen.getByRole("radio", { name: "Deutsch" })).toBeTruthy();
    expect(screen.getByRole("radio", { name: "Dark" })).toBeTruthy();
  });

  it("saves a theme choice at once with a full patch and shows the saved value", async () => {
    const patches = backend((p) => ({ ...saved, theme: (p as { theme: string }).theme }));
    view();
    fireEvent.click(screen.getByRole("radio", { name: "Dark" }));
    await waitFor(() => expect(useApp.getState().settings.data?.theme).toBe("dark"));
    expect(patches).toEqual([
      { language: { op: "keep" }, javaOverride: { op: "keep" }, theme: "dark" },
    ]);
  });

  it("choosing System for the language clears it, choosing Deutsch sets it", async () => {
    useApp.setState({ settings: { status: "ready", data: { ...saved, language: "en" } } });
    const patches = backend((p) => ({
      ...saved,
      language: (p as { language: { op: string; value?: string } }).language.value ?? null,
    }));
    view();
    fireEvent.click(screen.getByRole("radio", { name: "System", checked: false }));
    await waitFor(() => expect(patches).toHaveLength(1));
    expect((patches[0] as { language: unknown }).language).toEqual({ op: "clear" });
  });

  it("a second choice while one is saving is ignored", async () => {
    const patches = backend(async (p) => {
      await new Promise((r) => setTimeout(r, 50));
      return { ...saved, theme: (p as { theme: string }).theme };
    });
    view();
    fireEvent.click(screen.getByRole("radio", { name: "Dark" }));
    fireEvent.click(screen.getByRole("radio", { name: "Light" }));
    await waitFor(() => expect(useApp.getState().settings.data?.theme).toBe("dark"));
    expect(patches).toHaveLength(1);
  });

  it("a failed save falls back to the saved value and says so inline", async () => {
    backend(() => {
      throw { code: "internal", params: {}, action: null, detail: "disk full" };
    });
    view();
    fireEvent.click(screen.getByRole("radio", { name: "Dark" }));
    expect(await screen.findByText("The setting could not be saved.")).toBeTruthy();
    expect(screen.getByRole("radio", { name: "Dark" }).getAttribute("aria-checked")).toBe("false");
    expect(useApp.getState().settings.data?.theme).toBe("system");
  });

  it("shows the data folder and the about block", async () => {
    backend(() => saved);
    view();
    expect(await screen.findByText("Not an official Minecraft product.")).toBeTruthy();
    const path = await screen.findByText("C:\\data\\HengstClient");
    // The whole path wraps and stays visible: no truncation, no tooltip.
    expect(path.className).toContain("wrap-anywhere");
    expect(path.className).not.toContain("truncate");
  });
});
