import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { initI18n, setLanguage } from "@/i18n";
import type { JavaChoice } from "@/ipc/bindings/JavaChoice";
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

let installedMemoryMb: number | null = 16384;
let infoFails = false;
let infoCalls = 0;
let javaAnswer: () => JavaChoice | Promise<JavaChoice> = () => ({ kind: "cancelled" });
let javaCalls = 0;
/** The channel of the last `java_choose`: the backend's "checking" arrives on it. */
let javaChannel: { onmessage?: (phase: string) => void } | null = null;
let update: (patch: Record<string, unknown>) => unknown = () => saved;
let patches: Record<string, unknown>[] = [];

beforeEach(() => {
  setLanguage("en");
  useApp.setState({ ...initial, settings: { status: "ready", data: saved } }, true);
  installedMemoryMb = 16384;
  infoFails = false;
  infoCalls = 0;
  javaCalls = 0;
  javaAnswer = () => ({ kind: "cancelled" });
  patches = [];
  update = (p) => ({ ...saved, ...plain(p) });
  mockIPC(async (cmd, args) => {
    if (cmd === "app_info") {
      infoCalls += 1;
      if (infoFails) throw { code: "internal", params: {}, action: null, detail: "x" };
      return {
        version: "0.1.0",
        dataDir: "C:\\data\\HengstClient",
        devBuild: false,
        devOffline: false,
        installedMemoryMb,
      };
    }
    if (cmd === "game_versions")
      return { versions: [], latestRelease: "", latestSnapshot: "", fromCache: true };
    if (cmd === "settings_update") {
      const patch = (args as { patch: Record<string, unknown> }).patch;
      patches.push(patch);
      return update(patch);
    }
    if (cmd === "java_choose") {
      javaCalls += 1;
      javaChannel = (args as { channel: typeof javaChannel }).channel;
      return javaAnswer();
    }
    return undefined;
  });
});
afterEach(() => {
  cleanup();
  clearMocks();
});

/** The fields a patch changes (without the `keep` entries the client adds). */
function plain(p: Record<string, unknown>): Partial<SettingsDto> {
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(p)) {
    if (typeof v === "object" && v !== null && (v as { op: string }).op === "keep") continue;
    if (k === "javaOverride") out[k] = null;
    else out[k] = v;
  }
  return out as Partial<SettingsDto>;
}

const view = () =>
  render(
    <TooltipProvider>
      <SettingsView />
    </TooltipProvider>,
  );

const memory = () => screen.getByRole("textbox", { name: "Memory" });

describe("memory", () => {
  it("is a numeric text field with the recommendation and the installed memory", async () => {
    view();
    const field = memory();
    expect(field.getAttribute("inputmode")).toBe("numeric");
    expect(field.getAttribute("type")).not.toBe("number");
    expect(field.getAttribute("spellcheck")).toBe("false");
    expect((field as HTMLInputElement).value).toBe("2048");
    expect(
      await screen.findByText("Recommended: 2048 to 4096 MB. Installed: 16.0 GB."),
    ).toBeTruthy();
    expect(field.getAttribute("aria-describedby")).toBeTruthy();
  });

  it("formats the installed memory for German", async () => {
    setLanguage("de");
    installedMemoryMb = 16076;
    view();
    expect(
      await screen.findByText("Empfohlen: 2048 bis 4096 MB. Installiert: 15,7 GB."),
    ).toBeTruthy();
  });

  it("checks on blur, then live, and never saves a value out of range", async () => {
    view();
    const field = memory();
    fireEvent.change(field, { target: { value: "100" } });
    // Not while typing the first time.
    expect(screen.queryByText("Choose a value between 512 and 65,536 MB.")).toBeNull();
    fireEvent.blur(field);
    expect(await screen.findByText("Choose a value between 512 and 65,536 MB.")).toBeTruthy();
    expect(field.getAttribute("aria-invalid")).toBe("true");
    for (const bad of ["70000", "4k", "", "2048.5"]) {
      fireEvent.change(field, { target: { value: bad } });
      expect(screen.getByText("Choose a value between 512 and 65,536 MB.")).toBeTruthy();
    }
    fireEvent.change(field, { target: { value: "4096" } });
    expect(screen.queryByText("Choose a value between 512 and 65,536 MB.")).toBeNull();
    expect(field.getAttribute("aria-invalid")).toBeNull();
    expect(patches).toEqual([]);
  });

  // A node that flips between role status and alert is not announced reliably: the help line
  // holds one fixed status region and one fixed alert region, and the text moves between them.
  it("the line under the field keeps a status and an alert region and fills the right one", async () => {
    view();
    const field = memory();
    const line = document.getElementById(field.getAttribute("aria-describedby")!)!;
    const status = line.querySelector('[role="status"]');
    const alert = line.querySelector('[role="alert"]');
    expect(status).not.toBeNull();
    expect(alert).not.toBeNull();
    expect(line.getAttribute("role")).toBeNull();
    await waitFor(() =>
      expect(status!.textContent).toBe("Recommended: 2048 to 4096 MB. Installed: 16.0 GB."),
    );
    expect(alert!.textContent).toBe("");
    fireEvent.change(field, { target: { value: "100" } });
    fireEvent.blur(field);
    await waitFor(() =>
      expect(alert!.textContent).toBe("Choose a value between 512 and 65,536 MB."),
    );
    expect(status!.textContent).toBe("");
    expect(line.querySelector('[role="alert"]')).toBe(alert);
  });

  it("saves on Enter and on blur, once", async () => {
    view();
    const field = memory();
    fireEvent.change(field, { target: { value: "4096" } });
    fireEvent.keyDown(field, { key: "Enter" });
    fireEvent.blur(field);
    await waitFor(() => expect(useApp.getState().settings.data?.memoryMb).toBe(4096));
    expect(patches.map(plain)).toEqual([{ memoryMb: 4096 }]);
  });

  it("Escape restores the saved value", () => {
    view();
    const field = memory();
    fireEvent.change(field, { target: { value: "100" } });
    fireEvent.blur(field);
    fireEvent.keyDown(field, { key: "Escape" });
    expect((field as HTMLInputElement).value).toBe("2048");
    expect(screen.queryByText("Choose a value between 512 and 65,536 MB.")).toBeNull();
    expect(patches).toEqual([]);
  });

  it("warns, but saves, above the installed memory", async () => {
    installedMemoryMb = 8192;
    view();
    await screen.findByText("Recommended: 2048 to 4096 MB. Installed: 8.0 GB.");
    const field = memory();
    fireEvent.change(field, { target: { value: "16384" } });
    fireEvent.blur(field);
    await waitFor(() => expect(useApp.getState().settings.data?.memoryMb).toBe(16384));
    expect(await screen.findByText("More than installed. The game may not start.")).toBeTruthy();
  });

  it("a failed save returns to the saved value and says so", async () => {
    update = () => {
      throw { code: "internal", params: {}, action: null, detail: "disk full" };
    };
    view();
    const field = memory();
    fireEvent.change(field, { target: { value: "4096" } });
    fireEvent.blur(field);
    expect(await screen.findByText("The setting could not be saved.")).toBeTruthy();
    expect((field as HTMLInputElement).value).toBe("2048");
  });
});

const openAdvanced = () => fireEvent.click(screen.getByRole("button", { name: "Advanced" }));

describe("Java", () => {
  it("sits behind Advanced and shows automatic when unset, without Reset", () => {
    view();
    expect(screen.queryByText("Automatic, matching the game version")).toBeNull();
    openAdvanced();
    expect(screen.getByText("Java (optional)")).toBeTruthy();
    expect(screen.getByText("Automatic, matching the game version")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Choose Java…" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Reset" })).toBeNull();
  });

  it("choosing asks Rust with no path, shows Checking only once a file was picked, then the version and the path", async () => {
    let finish: (c: JavaChoice) => void = () => undefined;
    javaAnswer = () => new Promise((r) => (finish = r));
    view();
    openAdvanced();
    const choose = screen.getByRole("button", { name: "Choose Java…" });
    fireEvent.click(choose);
    fireEvent.click(choose);
    await waitFor(() => expect(javaCalls).toBe(1));
    // The native dialog is open: nothing is being checked yet, and nothing says so.
    await new Promise((r) => setTimeout(r, 20));
    expect(screen.queryByText("Checking…")).toBeNull();
    const args = Object.keys(javaChannel ?? {});
    expect(args).not.toContain("path");
    act(() => javaChannel?.onmessage?.("checking"));
    expect(await screen.findByText("Checking…")).toBeTruthy();
    await act(async () =>
      finish({
        kind: "chosen",
        version: "21.0.5",
        settings: {
          ...saved,
          javaOverride: { path: "C:\\jdk-21\\bin\\java.exe", problem: null },
        },
      }),
    );
    expect(await screen.findByText("Java 21.0.5")).toBeTruthy();
    expect(screen.getByText("C:\\jdk-21\\bin\\java.exe")).toBeTruthy();
    expect(javaCalls).toBe(1);
    expect(useApp.getState().settings.data?.javaOverride?.path).toBe("C:\\jdk-21\\bin\\java.exe");
  });

  it.each([
    ["name", "Choose a java.exe or javaw.exe file."],
    ["missing", "This file no longer exists."],
    ["unavailable", "This drive is not available right now."],
    ["notStarted", "This Java installation could not be started."],
    ["location", "Choose a Java file on a local drive."],
  ])("a refused file (%s) says why and keeps the previous value", async (reason, text) => {
    useApp.setState({
      settings: {
        status: "ready",
        data: { ...saved, javaOverride: { path: "C:\\old\\java.exe", problem: null } },
      },
    });
    javaAnswer = () => {
      throw { code: "javaInvalid", params: { reason }, action: null, detail: "x" };
    };
    view();
    openAdvanced();
    fireEvent.click(screen.getByRole("button", { name: "Choose Java…" }));
    const alert = await screen.findByText(text);
    expect(alert).toBeTruthy();
    expect(screen.getByText("C:\\old\\java.exe")).toBeTruthy();
  });

  it("Reset is aria-disabled while a picked file is checked, with Checking as its reason", async () => {
    useApp.setState({
      settings: {
        status: "ready",
        data: { ...saved, javaOverride: { path: "C:\\old\\java.exe", problem: null } },
      },
    });
    let finish: (c: JavaChoice) => void = () => undefined;
    javaAnswer = () => new Promise((r) => (finish = r));
    view();
    openAdvanced();
    fireEvent.click(screen.getByRole("button", { name: "Choose Java…" }));
    await waitFor(() => expect(javaCalls).toBe(1));
    act(() => javaChannel?.onmessage?.("checking"));
    await screen.findByText("Checking…");
    const reset = screen.getByRole("button", { name: "Reset" });
    expect(reset.getAttribute("aria-disabled")).toBe("true");
    const reason = document.getElementById(reset.getAttribute("aria-describedby")!)!;
    expect(reason.textContent).toBe("Checking…");
    fireEvent.click(reset);
    expect(patches).toEqual([]);
    await act(async () => finish({ kind: "cancelled" }));
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Reset" }).getAttribute("aria-disabled"),
      ).toBeNull(),
    );
  });

  it("a cancelled dialog changes nothing", async () => {
    view();
    openAdvanced();
    fireEvent.click(screen.getByRole("button", { name: "Choose Java…" }));
    await waitFor(() => expect(javaCalls).toBe(1));
    await waitFor(() => expect(screen.queryByText("Checking…")).toBeNull());
    expect(screen.getByText("Automatic, matching the game version")).toBeTruthy();
  });

  it("a stored path that cannot be used is shown with the reason and can be reset", async () => {
    useApp.setState({
      settings: {
        status: "ready",
        data: {
          ...saved,
          javaOverride: { path: "\\\\fileserver\\share\\java.exe", problem: "location" },
        },
      },
    });
    view();
    expect(screen.getByText("\\\\fileserver\\share\\java.exe")).toBeTruthy();
    expect(screen.getByText("Choose a Java file on a local drive.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Reset" }));
    await waitFor(() => expect(patches).toHaveLength(1));
    expect(patches[0].javaOverride).toEqual({ op: "clear" });
    expect(await screen.findByText("Automatic, matching the game version")).toBeTruthy();
    // Reset is gone: focus moves to the button that is left.
    await waitFor(() =>
      expect(document.activeElement).toBe(screen.getByRole("button", { name: "Choose Java…" })),
    );
  });

  it("renders a path with bidi controls as plain left-to-right text", () => {
    useApp.setState({
      settings: {
        status: "ready",
        data: { ...saved, javaOverride: { path: "C:\\x\u202Eexe.avaj", problem: "location" } },
      },
    });
    view();
    const path = screen.getByText("C:\\xexe.avaj");
    expect(path.closest("[dir=ltr]")).toBeTruthy();
  });

  // DESIGN.md 11.7: "Erweitert" is open while the stored Java path cannot be used, so
  // "Open settings" from a refused start lands on the reason, not on a page that looks fine.
  it("is open on arrival while the stored path cannot be used, and stays open", async () => {
    useApp.setState({
      settings: {
        status: "ready",
        data: { ...saved, javaOverride: { path: "D:\\gone\\java.exe", problem: "missing" } },
      },
    });
    view();
    const advanced = screen.getByRole("button", { name: "Advanced" });
    expect(advanced.getAttribute("aria-expanded")).toBe("true");
    expect(screen.getByText("D:\\gone\\java.exe")).toBeTruthy();
    expect(screen.getByText("This file no longer exists.")).toBeTruthy();
    // A click on the trigger does not hide the reason.
    fireEvent.click(advanced);
    expect(screen.getByText("This file no longer exists.")).toBeTruthy();
    // Reset clears the problem; the section stays open around the focused button.
    fireEvent.click(screen.getByRole("button", { name: "Reset" }));
    expect(await screen.findByText("Automatic, matching the game version")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Advanced" }).getAttribute("aria-expanded")).toBe(
      "true",
    );
  });

  it("is closed on arrival when the stored path is fine", () => {
    useApp.setState({
      settings: {
        status: "ready",
        data: { ...saved, javaOverride: { path: "C:\\jdk\\java.exe", problem: null } },
      },
    });
    view();
    expect(screen.getByRole("button", { name: "Advanced" }).getAttribute("aria-expanded")).toBe(
      "false",
    );
    expect(screen.queryByText("C:\\jdk\\java.exe")).toBeNull();
  });
});

describe("launch", () => {
  it("the minimize switch shows and saves the setting", async () => {
    view();
    const toggle = screen.getByRole("switch", {
      name: "Minimize to the notification area when the game starts",
    });
    expect(toggle.getAttribute("aria-checked")).toBe("true");
    fireEvent.click(toggle);
    await waitFor(() => expect(patches.map(plain)).toEqual([{ minimizeOnLaunch: false }]));
    await waitFor(() =>
      expect(
        screen
          .getByRole("switch", { name: "Minimize to the notification area when the game starts" })
          .getAttribute("aria-checked"),
      ).toBe("false"),
    );
  });
});

describe("app info failing", () => {
  it("shows an error with a retry instead of a skeleton forever", async () => {
    infoFails = true;
    view();
    const folder = screen.getByRole("region", { name: "Data folder" });
    expect(await within(folder).findByRole("alert")).toBeTruthy();
    infoFails = false;
    const before = infoCalls;
    fireEvent.click(within(folder).getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("C:\\data\\HengstClient")).toBeTruthy();
    expect(infoCalls).toBe(before + 1);
  });
});
