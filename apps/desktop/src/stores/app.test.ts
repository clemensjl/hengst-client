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

const accountsDto = (canSignIn: boolean, names: string[] = []) => ({
  canSignIn,
  accounts: names.map((name, i) => ({
    id: `id-${i}`,
    name,
    kind: "microsoft",
    selected: i === 0,
    signInRequired: false,
  })),
});

describe("accounts", () => {
  it("loads the list into its resource", async () => {
    mockIPC((cmd) => {
      if (cmd === "accounts_list") return accountsDto(true, ["Mira_K"]);
    });
    expect(useApp.getState().accounts.status).toBe("idle");
    await useApp.getState().loadAccounts();
    const { accounts } = useApp.getState();
    expect(accounts.status).toBe("ready");
    expect(accounts.data?.accounts.map((a) => a.name)).toEqual(["Mira_K"]);
  });

  it("keeps the old list while a reload is under way and on a failed reload", async () => {
    mockIPC((cmd) => {
      if (cmd === "accounts_list") return accountsDto(true, ["Mira_K"]);
    });
    await useApp.getState().loadAccounts();
    mockIPC(() => {
      throw { code: "fileSystem", params: {}, action: null, detail: "x" };
    });
    await useApp.getState().loadAccounts();
    const { accounts } = useApp.getState();
    expect(accounts.status).toBe("error");
    expect(accounts.error?.code).toBe("fileSystem");
    expect(accounts.data?.accounts).toHaveLength(1);
  });

  it("an accounts change from the backend reloads the list", async () => {
    let list = accountsDto(true, ["Mira_K"]);
    let sink: ((e: unknown) => void) | undefined;
    mockIPC((cmd, args) => {
      if (cmd === "subscribe_events") {
        sink = (args as { channel: { onmessage: (e: unknown) => void } }).channel.onmessage;
        return;
      }
      if (cmd === "accounts_list") return list;
    });
    await useApp.getState().startEventLoop();
    await useApp.getState().loadAccounts();
    list = accountsDto(true, ["Mira_K", "Jonas_T"]);
    sink!({ event: "dataChanged", data: { scope: "accounts" } });
    await vi.waitFor(() => expect(useApp.getState().accounts.data?.accounts).toHaveLength(2));
  });

  it("opening the sign-in dialog without a client id opens the no-client-id dialog instead", async () => {
    mockIPC((cmd) => {
      if (cmd === "accounts_list") return accountsDto(false);
    });
    await useApp.getState().loadAccounts();
    useApp.getState().openSignIn(null);
    expect(useApp.getState().accountDialog).toEqual({ kind: "noClientId" });
  });

  it("opens the sign-in dialog for a build with a client id, and for a re-sign-in with the account", async () => {
    mockIPC((cmd) => {
      if (cmd === "accounts_list") return accountsDto(true, ["Mira_K"]);
    });
    await useApp.getState().loadAccounts();
    useApp.getState().openSignIn(null);
    expect(useApp.getState().accountDialog).toMatchObject({ kind: "signIn", resign: null });
    useApp.getState().closeAccountDialog();
    expect(useApp.getState().accountDialog).toBeNull();
    useApp.getState().openSignIn({ id: "id-0", name: "Mira_K" });
    expect(useApp.getState().accountDialog).toMatchObject({
      kind: "signIn",
      resign: { id: "id-0", name: "Mira_K" },
    });
  });

  it("opening it before the list is known opens the sign-in dialog (the backend refuses in a build without an id)", () => {
    useApp.getState().openSignIn(null);
    expect(useApp.getState().accountDialog).toMatchObject({ kind: "signIn" });
  });

  it("selecting an account calls the backend with its id and reloads", async () => {
    const calls: [string, unknown][] = [];
    mockIPC((cmd, args) => {
      calls.push([cmd, args]);
      if (cmd === "accounts_list") return accountsDto(true, ["Mira_K", "Jonas_T"]);
    });
    await useApp.getState().selectAccount("id-1");
    expect(calls).toContainEqual(["account_select", { id: "id-1" }]);
    expect(calls.filter(([cmd]) => cmd === "accounts_list")).toHaveLength(1);
  });
});

describe("instances", () => {
  const row = (id: string) => ({
    id,
    name: id,
    game: "1.21.11",
    loader: null,
    memoryMb: null,
    lastPlayed: null,
    status: "ready",
    reason: null,
    selected: false,
  });

  it("loads the list and whether it is complete", async () => {
    mockIPC((cmd) => {
      if (cmd === "instances_list") return { instances: [row("a")], incomplete: true };
    });
    await useApp.getState().loadInstances();
    const s = useApp.getState();
    expect(s.instances.status).toBe("ready");
    expect(s.instances.data?.map((i) => i.id)).toEqual(["a"]);
    expect(s.instancesIncomplete).toBe(true);
  });

  it("keeps the shown list on a failed reload and carries the error", async () => {
    let fail = false;
    mockIPC((cmd) => {
      if (cmd !== "instances_list") return;
      if (fail) throw { code: "busy", params: {}, action: null, detail: "x" };
      return { instances: [row("a")], incomplete: false };
    });
    await useApp.getState().loadInstances();
    fail = true;
    await useApp.getState().loadInstances();
    const s = useApp.getState().instances;
    expect(s.status).toBe("error");
    expect(s.error?.code).toBe("busy");
    expect(s.data?.map((i) => i.id)).toEqual(["a"]);
  });

  it("an instances change from the backend reloads a list that was loaded before", async () => {
    let lists = 0;
    let sink: ((e: unknown) => void) | undefined;
    mockIPC((cmd, args) => {
      if (cmd === "subscribe_events") {
        sink = (args as { channel: { onmessage: (e: unknown) => void } }).channel.onmessage;
        return;
      }
      if (cmd === "instances_list") {
        lists += 1;
        return { instances: [row(`v${lists}`)], incomplete: false };
      }
    });
    await useApp.getState().startEventLoop();
    // Not loaded yet: nobody shows the list, nothing is fetched.
    sink!({ event: "dataChanged", data: { scope: "instances" } });
    await new Promise((r) => setTimeout(r, 0));
    expect(lists).toBe(0);
    await useApp.getState().loadInstances();
    sink!({ event: "dataChanged", data: { scope: "instances" } });
    await vi.waitFor(() => expect(useApp.getState().instances.data?.[0].id).toBe("v2"));
  });

  it("a late answer of an older request never replaces the list of a newer one", async () => {
    const answers: ((v: unknown) => void)[] = [];
    mockIPC((cmd) => {
      if (cmd === "instances_list") return new Promise((resolve) => answers.push(resolve));
    });
    const first = useApp.getState().loadInstances();
    const second = useApp.getState().loadInstances();
    await vi.waitFor(() => expect(answers).toHaveLength(2));
    // The newer request answers first, the older one after it.
    answers[1]({ instances: [row("new")], incomplete: false });
    await second;
    answers[0]({ instances: [row("old")], incomplete: true });
    await first;
    expect(useApp.getState().instances.data?.map((i) => i.id)).toEqual(["new"]);
    expect(useApp.getState().instancesIncomplete).toBe(false);
  });

  it("a late failure of an older request does not mark a newer list as failed", async () => {
    const answers: { resolve: (v: unknown) => void; reject: (e: unknown) => void }[] = [];
    mockIPC((cmd) => {
      if (cmd === "instances_list")
        return new Promise((resolve, reject) => answers.push({ resolve, reject }));
    });
    const first = useApp.getState().loadInstances();
    const second = useApp.getState().loadInstances();
    await vi.waitFor(() => expect(answers).toHaveLength(2));
    answers[1].resolve({ instances: [row("new")], incomplete: false });
    await second;
    answers[0].reject({ code: "busy", params: {}, action: null, detail: "x" });
    await first;
    expect(useApp.getState().instances.status).toBe("ready");
  });
});
