import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { toast } from "sonner";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { initI18n } from "@/i18n";
import { useApp } from "./app";

vi.mock("sonner", () => ({ toast: vi.fn(), Toaster: () => null }));

const initial = useApp.getState();
beforeEach(() => {
  initI18n("en", "en-US");
  useApp.setState(initial, true);
  vi.mocked(toast).mockClear();
});
afterEach(() => clearMocks());

const dto = (notice: "reset" | null) => ({
  language: null,
  theme: "dark",
  memoryMb: 2048,
  javaOverride: null,
  minimizeOnLaunch: false,
  devAccount: false,
  notice,
});

/** Starts the event loop with a fake backend and returns a function that sends it an event. */
async function backend(settingsAnswers: ReturnType<typeof dto>[]) {
  let sink: ((e: unknown) => void) | undefined;
  mockIPC((cmd, args) => {
    if (cmd === "subscribe_events") {
      sink = (args as { channel: { onmessage: (e: unknown) => void } }).channel.onmessage;
      return;
    }
    if (cmd === "settings_get") return settingsAnswers.shift();
  });
  await useApp.getState().startEventLoop();
  return (e: unknown) => sink!(e);
}

describe("settings update", () => {
  it("shows the reset toast from the update's own answer, so a failed reload cannot lose it", async () => {
    mockIPC((cmd) => {
      if (cmd === "settings_update") return dto("reset");
    });
    await useApp.getState().updateSettings({ theme: "dark" });
    expect(toast).toHaveBeenCalledWith("Settings were damaged and have been reset", {
      id: "settings-reset",
    });
  });

  it("an update without a notice shows no toast", async () => {
    mockIPC((cmd) => {
      if (cmd === "settings_update") return dto(null);
    });
    await useApp.getState().updateSettings({ theme: "dark" });
    expect(toast).not.toHaveBeenCalled();
  });
});

describe("settings events", () => {
  it("a reset notice that arrives with a settings change is shown as the reset toast", async () => {
    const send = await backend([dto("reset")]);
    send({ event: "dataChanged", data: { scope: "settings" } });
    await vi.waitFor(() => expect(toast).toHaveBeenCalledTimes(1));
    expect(toast).toHaveBeenCalledWith("Settings were damaged and have been reset", {
      id: "settings-reset",
    });
  });

  it("a settings change without a notice shows no toast", async () => {
    const send = await backend([dto(null)]);
    send({ event: "dataChanged", data: { scope: "settings" } });
    await vi.waitFor(() => expect(useApp.getState().settings.status).toBe("ready"));
    expect(toast).not.toHaveBeenCalled();
  });
});
