import { toast } from "sonner";
import { create } from "zustand";
import { START_VIEW, type View } from "@/app/views";
import i18n from "@/i18n";
import { api, IpcError } from "@/ipc/client";
import { subscribeEvents } from "@/ipc/events";
import type { AccountDto } from "@/ipc/bindings/AccountDto";
import type { AccountsDto } from "@/ipc/bindings/AccountsDto";
import type { AppInfo } from "@/ipc/bindings/AppInfo";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";
import type { GameVersionsDto } from "@/ipc/bindings/GameVersionsDto";
import type { InstanceDto } from "@/ipc/bindings/InstanceDto";
import type { LoaderKindDto } from "@/ipc/bindings/LoaderKindDto";
import type { SettingsDto } from "@/ipc/bindings/SettingsDto";
import type { SettingsPatch } from "@/ipc/bindings/SettingsPatch";
import type { VersionFilter } from "@/ipc/bindings/VersionFilter";

export type Resource<T> = {
  status: "idle" | "loading" | "ready" | "error";
  data?: T;
  error?: ErrorDto;
};

const idle = <T>(): Resource<T> => ({ status: "idle" });

function errorOf(e: unknown): ErrorDto {
  if (e instanceof IpcError) return e.dto;
  return { code: "internal", params: {}, action: null, detail: String(e) };
}

/** Marks a resource as loading without dropping what it already holds (no flicker on a reload). */
function loading<T>(r: Resource<T>): Resource<T> {
  return r.data === undefined ? { status: "loading" } : r;
}

/** What About shows about the version list; separate from `versions`, which carries the filter of the version picker. */
export interface VersionsSummary {
  count: number;
  latestRelease: string;
  latestSnapshot: string;
}

/** The dialog of the accounts feature that is open, if any (one at a time). */
export type AccountDialog =
  /** `resign`: the account that is asked to sign in again. `id` tells one opening from the next. */
  | { kind: "signIn"; resign: { id: string; name: string } | null; id: number }
  | { kind: "noClientId" }
  | { kind: "remove"; account: AccountDto };

/** What the create dialog keeps when it is closed without creating (DESIGN.md 10.9). */
export interface InstanceDraft {
  game: string | null;
  filter: VersionFilter;
  loader: LoaderKindDto | "none";
  loaderVersion: string | null;
  name: string;
  /** The user changed the name: it no longer follows the version and the loader. */
  nameEdited: boolean;
}

/**
 * Where focus goes once the instance list shows the change that moved it (DESIGN.md 13): a row
 * after it was created, a row's action button after a delete, or "Create instance".
 */
export type InstanceFocus =
  { kind: "row"; id: string } | { kind: "action"; id: string } | { kind: "create" };

const ALL_KINDS: VersionFilter = { release: true, snapshot: true, beta: true, alpha: true };

interface AppState {
  view: View;
  info: Resource<AppInfo>;
  settings: Resource<SettingsDto>;
  versions: Resource<GameVersionsDto>;
  /** Why the shown version list may be old: no connection, or another refresh failure. */
  versionsStale: null | "offline" | "other";
  versionsSummary: Resource<VersionsSummary>;
  accounts: Resource<AccountsDto>;
  accountDialog: AccountDialog | null;
  instances: Resource<InstanceDto[]>;
  /** Some folders could not be read: the list may miss instances. */
  instancesIncomplete: boolean;
  /** The instance being prepared or running. Always `null` in 4a; 4b sets it from the game state. */
  activeInstance: string | null;
  createOpen: boolean;
  instanceDraft: InstanceDraft | null;
  instanceFocus: InstanceFocus | null;
  setView: (view: View) => void;
  loadInfo: () => Promise<void>;
  /** Resolves with the answer, rejects with an `IpcError`. */
  loadSettings: () => Promise<SettingsDto>;
  /** Saves a change; resolves with the saved settings, rejects with an `IpcError` (nothing changed). */
  updateSettings: (change: Partial<SettingsPatch>) => Promise<SettingsDto>;
  loadVersions: (filter: VersionFilter) => Promise<void>;
  loadVersionsSummary: () => Promise<void>;
  loadAccounts: () => Promise<void>;
  /**
   * Chooses the account the game starts with. Resolves with `null`, or with the error (the list is
   * reloaded either way, so it shows what is true).
   */
  selectAccount: (id: string) => Promise<ErrorDto | null>;
  /**
   * Opens the sign-in dialog, for a new account (`resign` null) or for one that has to sign in
   * again. A build without a client id gets the no-client-id dialog instead (DESIGN.md 11.6).
   */
  openSignIn: (resign: { id: string; name: string } | null) => void;
  openNoClientId: () => void;
  openRemoveAccount: (account: AccountDto) => void;
  closeAccountDialog: () => void;
  /** Loads the instance list; the list shown so far stays while it loads and on a failure. */
  loadInstances: () => Promise<void>;
  openCreate: () => void;
  closeCreate: () => void;
  setInstanceDraft: (draft: InstanceDraft | null) => void;
  setInstanceFocus: (focus: InstanceFocus | null) => void;
  /** Subscribes to backend events. Call it before the first load, so no event in between is lost. */
  startEventLoop: () => Promise<void>;
}

// The filter of the last `loadVersions`, used when an event asks for a reload.
let versionsFilter: VersionFilter | null = null;
// Counts the openings of the sign-in dialog; each one is a new sign-in with its own state.
let signInOpenings = 0;

export const useApp = create<AppState>()((set, get) => ({
  view: START_VIEW,
  info: idle(),
  settings: idle(),
  versions: idle(),
  versionsStale: null,
  versionsSummary: idle(),
  accounts: idle(),
  accountDialog: null,
  instances: idle(),
  instancesIncomplete: false,
  activeInstance: null,
  createOpen: false,
  instanceDraft: null,
  instanceFocus: null,

  setView: (view) => set({ view }),

  loadInfo: async () => {
    set((s) => ({ info: loading(s.info) }));
    try {
      set({ info: { status: "ready", data: await api.appInfo() } });
    } catch (e) {
      set((s) => ({ info: { ...s.info, status: "error", error: errorOf(e) } }));
    }
  },

  loadSettings: async () => {
    set((s) => ({ settings: loading(s.settings) }));
    try {
      const data = await api.settingsGet();
      set({ settings: { status: "ready", data } });
      return data;
    } catch (e) {
      set((s) => ({ settings: { ...s.settings, status: "error", error: errorOf(e) } }));
      throw e;
    }
  },

  updateSettings: async (change) => {
    const data = await api.settingsUpdate(change);
    set({ settings: { status: "ready", data } });
    // An update that found the file damaged resets it and says so in its own answer: the follow-up
    // reload (which may fail) is not needed to learn it. The id keeps a second delivery to one toast.
    if (data.notice === "reset") toast(i18n.t("settings.resetNotice"), { id: "settings-reset" });
    return data;
  },

  loadVersions: async (filter) => {
    versionsFilter = filter;
    set((s) => ({ versions: loading(s.versions) }));
    try {
      set({ versions: { status: "ready", data: await api.gameVersions(filter) } });
    } catch (e) {
      set((s) => ({ versions: { ...s.versions, status: "error", error: errorOf(e) } }));
    }
  },

  loadVersionsSummary: async () => {
    set((s) => ({ versionsSummary: loading(s.versionsSummary) }));
    try {
      const all = await api.gameVersions(ALL_KINDS);
      set({
        versionsSummary: {
          status: "ready",
          data: {
            count: all.versions.length,
            latestRelease: all.latestRelease,
            latestSnapshot: all.latestSnapshot,
          },
        },
      });
    } catch (e) {
      set((s) => ({
        versionsSummary: { ...s.versionsSummary, status: "error", error: errorOf(e) },
      }));
    }
  },

  loadAccounts: async () => {
    set((s) => ({ accounts: loading(s.accounts) }));
    try {
      set({ accounts: { status: "ready", data: await api.accountsList() } });
    } catch (e) {
      // The list shown so far stays; the error is what the chip and its menu explain.
      set((s) => ({ accounts: { ...s.accounts, status: "error", error: errorOf(e) } }));
    }
  },

  selectAccount: async (id) => {
    let failure: ErrorDto | null = null;
    try {
      await api.accountSelect(id);
    } catch (e) {
      failure = errorOf(e);
    }
    await get().loadAccounts();
    return failure;
  },

  openSignIn: (resign) => {
    // `false` is only known once the list is: until then the backend decides (it refuses with
    // `missingClientId`, which the dialog shows).
    if (get().accounts.data?.canSignIn === false) {
      set({ accountDialog: { kind: "noClientId" } });
      return;
    }
    signInOpenings += 1;
    set({ accountDialog: { kind: "signIn", resign, id: signInOpenings } });
  },
  openNoClientId: () => set({ accountDialog: { kind: "noClientId" } }),
  openRemoveAccount: (account) => set({ accountDialog: { kind: "remove", account } }),
  closeAccountDialog: () => set({ accountDialog: null }),

  loadInstances: async () => {
    set((s) => ({ instances: loading(s.instances) }));
    try {
      const answer = await api.instancesList();
      set({
        instances: { status: "ready", data: answer.instances },
        instancesIncomplete: answer.incomplete,
      });
    } catch (e) {
      set((s) => ({ instances: { ...s.instances, status: "error", error: errorOf(e) } }));
    }
  },
  openCreate: () => set({ createOpen: true }),
  closeCreate: () => set({ createOpen: false }),
  setInstanceDraft: (draft) => set({ instanceDraft: draft }),
  setInstanceFocus: (focus) => set({ instanceFocus: focus }),

  startEventLoop: () =>
    subscribeEvents((event) => {
      if (event.event === "dataChanged") {
        if (event.data.scope === "settings")
          // A save that found the file damaged resets it and leaves a notice for this reload.
          void get()
            .loadSettings()
            .then((data) => {
              if (data.notice === "reset")
                toast(i18n.t("settings.resetNotice"), { id: "settings-reset" });
            })
            .catch(() => {});
        if (event.data.scope === "accounts") void get().loadAccounts();
        // Only a list somebody has loaded is kept current.
        if (event.data.scope === "instances" && get().instances.status !== "idle")
          void get().loadInstances();
        if (event.data.scope === "versions") {
          set({ versionsStale: null });
          if (versionsFilter) void get().loadVersions(versionsFilter);
          if (get().versionsSummary.status !== "idle") void get().loadVersionsSummary();
        }
      } else if (event.event === "notice") {
        const offline = event.data.kind === "versionsRefreshOffline";
        set({ versionsStale: offline ? "offline" : "other" });
        toast(i18n.t("errors.versionsRefreshFailed"), { id: "versions-refresh" });
      }
    }),
}));
