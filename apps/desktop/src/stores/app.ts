import { toast } from "sonner";
import { create } from "zustand";
import { START_VIEW, type View } from "@/app/views";
import i18n from "@/i18n";
import { api, IpcError } from "@/ipc/client";
import { subscribeEvents } from "@/ipc/events";
import { setDialogOpener } from "@/features/accounts/dialogReturn";
import type { AccountDto } from "@/ipc/bindings/AccountDto";
import type { AccountsDto } from "@/ipc/bindings/AccountsDto";
import type { AppInfo } from "@/ipc/bindings/AppInfo";
import { INITIAL, reduce, type LaunchProgress } from "@/features/play/progress";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";
import type { GameStateDto } from "@/ipc/bindings/GameStateDto";
import type { GameVersionsDto } from "@/ipc/bindings/GameVersionsDto";
import type { PlayEvent } from "@/ipc/bindings/PlayEvent";
import type { InstanceDto } from "@/ipc/bindings/InstanceDto";
import type { JavaChoice } from "@/ipc/bindings/JavaChoice";
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

/** A start this page made, from the click until its channel ends (`started`, `failed`, `cancelled`). */
export interface Launch {
  /** Tells this start's events from those of an earlier one. */
  token: number;
  instance: string;
  progress: LaunchProgress;
  /** `play_cancel` answered `false`: the start can no longer be cancelled and goes on. */
  cancelRefused: boolean;
}

/** The instance the game state names, if a game is being prepared, runs or stops. */
const activeOf = (g: GameStateDto): string | null => (g.state === "idle" ? null : g.data.instance);

/** The run the game state is about, if any (`idle` names the last one that ended). */
const runOf = (g: GameStateDto | undefined): number | null => {
  if (!g || g.state === "preparing") return null;
  if (g.state === "idle") return g.data.lastExit?.runId ?? null;
  return g.data.runId;
};

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
  /** The account menu of the "Choose account" chip is open (the Play button can open it too). */
  accountMenuOpen: boolean;
  setAccountMenuOpen: (open: boolean) => void;
  instances: Resource<InstanceDto[]>;
  /** Some folders could not be read: the list may miss instances. */
  instancesIncomplete: boolean;
  /** The instance being prepared, running or stopping, from the game state. */
  activeInstance: string | null;
  game: Resource<GameStateDto>;
  launch: Launch | null;
  /** The last start of this page failed (until the next start); `installLog` if one was reported. */
  launchError: { error: ErrorDto; installLog: string | null } | null;
  /** `game_stop` reached no window for this run: nothing was asked to close. */
  stopNoWindow: number | null;
  /** When this page first saw the run in `stopping` (Unix ms); "Force stop" shows 10 s later. */
  stoppingSeen: { runId: number; at: number } | null;
  /** The unexpected exit of this run was dismissed. */
  dismissedExit: number | null;
  /** Cancel, stop or force stop failed. */
  playActionError: ErrorDto | null;
  /**
   * The tray asked to quit while a game runs: the quit dialog is open (DESIGN.md 11.11). The
   * number tells one opening from the next.
   */
  quitAsked: number | null;
  /** The open quit dialog was answered with "Keep playing" and is closing (its exit animation). */
  quitClosing: boolean;
  /** "Keep playing": a new tray request from now on opens a new dialog. */
  dismissQuit: () => void;
  /** The dialog `id` has finished closing. */
  closeQuit: (id: number) => void;
  /** Loads the game state; an event that arrives meanwhile wins over the answer. */
  loadGame: () => Promise<void>;
  /** Starts the instance; a second call while a start of this page runs does nothing. */
  startGame: (instanceId: string) => Promise<void>;
  cancelStart: () => Promise<void>;
  stopGame: () => Promise<void>;
  forceStopGame: () => Promise<void>;
  dismissExit: (runId: number) => void;
  dismissPlayActionError: () => void;
  createOpen: boolean;
  instanceDraft: InstanceDraft | null;
  instanceFocus: InstanceFocus | null;
  setView: (view: View) => void;
  loadInfo: () => Promise<void>;
  /** Resolves with the answer, rejects with an `IpcError`. */
  loadSettings: () => Promise<SettingsDto>;
  /** Saves a change; resolves with the saved settings, rejects with an `IpcError` (nothing changed). */
  updateSettings: (change: Partial<SettingsPatch>) => Promise<SettingsDto>;
  /** "Choose Java": the dialog and the check run in Rust; a chosen file updates the settings. Rejects with an `IpcError`. */
  chooseJava: () => Promise<JavaChoice>;
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
  openSignIn: (resign: { id: string; name: string } | null, from?: HTMLElement | null) => void;
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
// The newest request per resource. Answers can arrive out of order: one counts only if no newer
// request for the same resource was made after it (an update of the settings counts as one).
const requests = new Map<string, number>();
/** Starts a request for `key`; the returned check says whether it is still the newest one. */
function latest(key: string): () => boolean {
  const mine = (requests.get(key) ?? 0) + 1;
  requests.set(key, mine);
  return () => requests.get(key) === mine;
}
// Counts the openings of the sign-in dialog; each one is a new sign-in with its own state.
let signInOpenings = 0;
// Counts the starts of this page; a start's events count only while it is the current one.
let launchTokens = 0;
// A stop or force stop request is on its way: a second click is ignored until it answers.
let stopBusy = false;
// A cancel request is on its way: a second click is ignored until it answers (DESIGN.md 10.0).
let cancelBusy = false;
// Counts the openings of the quit dialog.
let quitOpenings = 0;

/**
 * Takes a game state the backend reported (event or `game_state()` answer). A start of this page
 * that reported `started` is over once the game state is about that run; the 10 s until
 * "Force stop" count from the first time this page sees the run stopping (after a reload, from then).
 */
function applyGame(state: GameStateDto) {
  const s = useApp.getState();
  const run = runOf(state);
  const outcome = s.launch?.progress.outcome;
  const startedRun = outcome?.kind === "started" ? outcome.runId : null;
  const seen =
    state.state !== "stopping"
      ? null
      : s.stoppingSeen?.runId === state.data.runId
        ? s.stoppingSeen
        : { runId: state.data.runId, at: Date.now() };
  useApp.setState({
    game: { status: "ready", data: state },
    activeInstance: activeOf(state),
    launch: startedRun !== null && run !== null && run >= startedRun ? null : s.launch,
    stoppingSeen: seen,
    stopNoWindow:
      state.state === "running" && state.data.runId === s.stopNoWindow ? s.stopNoWindow : null,
  });
}

/** One event of the page's start `token`; events of an earlier start are ignored. */
function onPlayEvent(token: number, e: PlayEvent) {
  const s = useApp.getState();
  const l = s.launch;
  if (!l || l.token !== token) return;
  const progress = reduce(l.progress, e);
  const outcome = progress.outcome;
  if (outcome?.kind === "failed") {
    useApp.setState({
      launch: null,
      launchError: { error: outcome.error, installLog: progress.installLog },
    });
  } else if (outcome?.kind === "cancelled") {
    useApp.setState({ launch: null });
  } else if (outcome?.kind === "started") {
    // The game state may already be past this run's start (the two channels are not ordered).
    const run = runOf(s.game.data);
    useApp.setState({ launch: run !== null && run >= outcome.runId ? null : { ...l, progress } });
  } else {
    useApp.setState({ launch: { ...l, progress } });
  }
}

export const useApp = create<AppState>()((set, get) => ({
  view: START_VIEW,
  info: idle(),
  settings: idle(),
  versions: idle(),
  versionsStale: null,
  versionsSummary: idle(),
  accounts: idle(),
  accountDialog: null,
  accountMenuOpen: false,
  setAccountMenuOpen: (open) => set({ accountMenuOpen: open }),
  instances: idle(),
  instancesIncomplete: false,
  activeInstance: null,
  createOpen: false,
  instanceDraft: null,
  instanceFocus: null,
  game: idle(),
  launch: null,
  launchError: null,
  stopNoWindow: null,
  stoppingSeen: null,
  dismissedExit: null,
  playActionError: null,
  quitAsked: null,
  quitClosing: false,
  dismissQuit: () => set({ quitClosing: true }),
  closeQuit: (id) =>
    set((s) => (s.quitAsked === id ? { quitAsked: null, quitClosing: false } : {})),

  setView: (view) => set({ view }),

  loadInfo: async () => {
    const current = latest("info");
    set((s) => ({ info: loading(s.info) }));
    try {
      const data = await api.appInfo();
      if (current()) set({ info: { status: "ready", data } });
    } catch (e) {
      if (current()) set((s) => ({ info: { ...s.info, status: "error", error: errorOf(e) } }));
    }
  },

  loadSettings: async () => {
    const current = latest("settings");
    set((s) => ({ settings: loading(s.settings) }));
    try {
      const data = await api.settingsGet();
      if (current()) set({ settings: { status: "ready", data } });
      return data;
    } catch (e) {
      if (current())
        set((s) => ({ settings: { ...s.settings, status: "error", error: errorOf(e) } }));
      throw e;
    }
  },

  updateSettings: async (change) => {
    const current = latest("settings");
    const data = await api.settingsUpdate(change);
    if (current()) set({ settings: { status: "ready", data } });
    // An update that found the file damaged resets it and says so in its own answer: the follow-up
    // reload (which may fail) is not needed to learn it. The id keeps a second delivery to one toast.
    if (data.notice === "reset") toast(i18n.t("settings.resetNotice"), { id: "settings-reset" });
    return data;
  },

  chooseJava: async () => {
    const answer = await api.javaChoose();
    if (answer.kind === "chosen") {
      // Counts as an update of the settings: an older answer that is still on its way loses.
      const current = latest("settings");
      if (current()) set({ settings: { status: "ready", data: answer.settings } });
    }
    return answer;
  },

  loadVersions: async (filter) => {
    versionsFilter = filter;
    const current = latest("versions");
    set((s) => ({ versions: loading(s.versions) }));
    try {
      const data = await api.gameVersions(filter);
      if (current()) set({ versions: { status: "ready", data } });
    } catch (e) {
      if (current())
        set((s) => ({ versions: { ...s.versions, status: "error", error: errorOf(e) } }));
    }
  },

  loadVersionsSummary: async () => {
    const current = latest("versionsSummary");
    set((s) => ({ versionsSummary: loading(s.versionsSummary) }));
    try {
      const all = await api.gameVersions(ALL_KINDS);
      if (!current()) return;
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
      if (!current()) return;
      set((s) => ({
        versionsSummary: { ...s.versionsSummary, status: "error", error: errorOf(e) },
      }));
    }
  },

  loadAccounts: async () => {
    const current = latest("accounts");
    set((s) => ({ accounts: loading(s.accounts) }));
    try {
      const data = await api.accountsList();
      if (current()) set({ accounts: { status: "ready", data } });
    } catch (e) {
      // The list shown so far stays; the error is what the chip and its menu explain.
      if (current())
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

  openSignIn: (resign, from = null) => {
    // Focus returns to `from` (the Play button) when the dialog closes, else to the chip.
    setDialogOpener(from);
    // `false` is only known once the list is: until then the backend decides (it refuses with
    // `missingClientId`, which the dialog shows).
    if (get().accounts.data?.canSignIn === false) {
      set({ accountDialog: { kind: "noClientId" } });
      return;
    }
    signInOpenings += 1;
    set({ accountDialog: { kind: "signIn", resign, id: signInOpenings } });
  },
  openNoClientId: () => {
    setDialogOpener(null);
    set({ accountDialog: { kind: "noClientId" } });
  },
  openRemoveAccount: (account) => set({ accountDialog: { kind: "remove", account } }),
  closeAccountDialog: () => set({ accountDialog: null }),

  loadInstances: async () => {
    const current = latest("instances");
    set((s) => ({ instances: loading(s.instances) }));
    try {
      const answer = await api.instancesList();
      if (!current()) return;
      set({
        instances: { status: "ready", data: answer.instances },
        instancesIncomplete: answer.incomplete,
      });
    } catch (e) {
      if (!current()) return;
      set((s) => ({ instances: { ...s.instances, status: "error", error: errorOf(e) } }));
    }
  },
  openCreate: () => set({ createOpen: true }),
  closeCreate: () => set({ createOpen: false }),
  setInstanceDraft: (draft) => set({ instanceDraft: draft }),
  setInstanceFocus: (focus) => set({ instanceFocus: focus }),

  loadGame: async () => {
    const current = latest("game");
    set((s) => ({ game: loading(s.game) }));
    try {
      const data = await api.gameState();
      if (current()) applyGame(data);
    } catch (e) {
      if (current()) set((s) => ({ game: { ...s.game, status: "error", error: errorOf(e) } }));
    }
  },

  startGame: async (instanceId) => {
    if (get().launch) return;
    launchTokens += 1;
    const token = launchTokens;
    set({
      launch: { token, instance: instanceId, progress: INITIAL, cancelRefused: false },
      launchError: null,
      playActionError: null,
    });
    try {
      await api.play(instanceId, (e) => onPlayEvent(token, e));
    } catch (e) {
      // Refused before anything started (busy, unknown instance): no event follows.
      if (get().launch?.token === token)
        set({ launch: null, launchError: { error: errorOf(e), installLog: null } });
    }
  },

  cancelStart: async () => {
    const l = get().launch;
    const op = l?.progress.op;
    if (cancelBusy || !l || op === null || op === undefined || l.cancelRefused) return;
    cancelBusy = true;
    try {
      // `true`: `cancelled` follows on the channel. `false`: too late, the start goes on.
      const cancelled = await api.playCancel(op);
      if (!cancelled && get().launch?.token === l.token)
        set((s) => ({ launch: s.launch && { ...s.launch, cancelRefused: true } }));
    } catch (e) {
      set({ playActionError: errorOf(e) });
    } finally {
      cancelBusy = false;
    }
  },

  stopGame: async () => {
    const game = get().game.data;
    if (stopBusy || game?.state !== "running") return;
    stopBusy = true;
    try {
      const result = await api.gameStop();
      // `asked`: the `stopping` state follows as an event. `notRunning`: it ended meanwhile.
      if (result === "noWindow") set({ stopNoWindow: game.data.runId });
      if (result === "notRunning") void get().loadGame();
    } catch (e) {
      set({ playActionError: errorOf(e) });
    } finally {
      stopBusy = false;
    }
  },

  forceStopGame: async () => {
    if (stopBusy) return;
    stopBusy = true;
    try {
      if (!(await api.gameForceStop())) void get().loadGame();
    } catch (e) {
      set({ playActionError: errorOf(e) });
    } finally {
      stopBusy = false;
    }
  },

  dismissExit: (runId) => set({ dismissedExit: runId }),
  dismissPlayActionError: () => set({ playActionError: null }),

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
      } else if (event.event === "gameState") {
        // Newer than any `game_state()` answer still on its way: that answer must not win.
        latest("game");
        applyGame(event.data.state);
      } else if (event.event === "quitRequested") {
        // A second request while the dialog is open keeps that dialog; one while it is closing
        // after "Keep playing" opens a new one.
        if (get().quitAsked === null || get().quitClosing) {
          quitOpenings += 1;
          set({ quitAsked: quitOpenings, quitClosing: false });
        }
      } else if (event.event === "stopNoWindow") {
        // The tray's stop reached no window: the start page says so and offers the force stop.
        const game = get().game.data;
        if (game?.state === "running" && game.data.runId === event.data.runId)
          set({ stopNoWindow: event.data.runId });
      } else if (event.event === "notice") {
        const offline = event.data.kind === "versionsRefreshOffline";
        set({ versionsStale: offline ? "offline" : "other" });
        toast(i18n.t("errors.versionsRefreshFailed"), { id: "versions-refresh" });
      }
    }),
}));
