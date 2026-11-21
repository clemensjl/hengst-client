import { toast } from "sonner";
import { create } from "zustand";
import { START_VIEW, type View } from "@/app/views";
import i18n from "@/i18n";
import { api, IpcError } from "@/ipc/client";
import { subscribeEvents } from "@/ipc/events";
import type { AppInfo } from "@/ipc/bindings/AppInfo";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";
import type { GameVersionsDto } from "@/ipc/bindings/GameVersionsDto";
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

const ALL_KINDS: VersionFilter = { release: true, snapshot: true, beta: true, alpha: true };

interface AppState {
  view: View;
  info: Resource<AppInfo>;
  settings: Resource<SettingsDto>;
  versions: Resource<GameVersionsDto>;
  /** Why the shown version list may be old: no connection, or another refresh failure. */
  versionsStale: null | "offline" | "other";
  versionsSummary: Resource<VersionsSummary>;
  setView: (view: View) => void;
  loadInfo: () => Promise<void>;
  /** Resolves with the answer, rejects with an `IpcError`. */
  loadSettings: () => Promise<SettingsDto>;
  /** Saves a change; resolves with the saved settings, rejects with an `IpcError` (nothing changed). */
  updateSettings: (change: Partial<SettingsPatch>) => Promise<SettingsDto>;
  loadVersions: (filter: VersionFilter) => Promise<void>;
  loadVersionsSummary: () => Promise<void>;
  /** Subscribes to backend events. Call it before the first load, so no event in between is lost. */
  startEventLoop: () => Promise<void>;
}

// The filter of the last `loadVersions`, used when an event asks for a reload.
let versionsFilter: VersionFilter | null = null;

export const useApp = create<AppState>()((set, get) => ({
  view: START_VIEW,
  info: idle(),
  settings: idle(),
  versions: idle(),
  versionsStale: null,
  versionsSummary: idle(),

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

  startEventLoop: () =>
    subscribeEvents((event) => {
      if (event.event === "dataChanged") {
        if (event.data.scope === "settings")
          void get()
            .loadSettings()
            .catch(() => {});
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
