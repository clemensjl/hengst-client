// Test helper: a fake backend for the instance commands, on top of `mockIPC`. It records every call,
// keeps a list that create and delete change, and lets a test decide what each command answers.
import { mockIPC } from "@tauri-apps/api/mocks";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";
import type { GameVersionDto } from "@/ipc/bindings/GameVersionDto";
import type { GameVersionsDto } from "@/ipc/bindings/GameVersionsDto";
import type { InstanceDto } from "@/ipc/bindings/InstanceDto";
import type { LoaderInfoDto } from "@/ipc/bindings/LoaderInfoDto";
import type { LoaderVersionsDto } from "@/ipc/bindings/LoaderVersionsDto";
import type { CreateInstanceRequest } from "@/ipc/bindings/CreateInstanceRequest";

export const instance = (id: string, extra: Partial<InstanceDto> = {}): InstanceDto => ({
  id,
  name: id,
  game: "1.21.11",
  loader: null,
  memoryMb: null,
  lastPlayed: null,
  status: "ready",
  reason: null,
  selected: false,
  ...extra,
});

export const err = (code: string, params: Record<string, string> = {}): ErrorDto =>
  ({ code, params, action: null, detail: "detail" }) as ErrorDto;

const version = (id: string, kind: GameVersionDto["kind"]): GameVersionDto => ({
  id,
  kind,
  releaseTime: "2026-01-01T00:00:00Z",
});

export const MANIFEST: GameVersionsDto = {
  versions: [
    version("1.21.11", "release"),
    version("26.1-snapshot-3", "snapshot"),
    version("1.20.1", "release"),
    version("1.14 Pre-Release 1", "snapshot"),
    version("1.8.9", "release"),
    version("b1.7.3", "beta"),
  ],
  latestRelease: "1.21.11",
  latestSnapshot: "26.1-snapshot-3",
  fromCache: false,
};

export const LOADERS: LoaderInfoDto[] = [
  { kind: "fabric", displayName: "Fabric" },
  { kind: "quilt", displayName: "Quilt" },
  { kind: "legacy-fabric", displayName: "Legacy Fabric" },
  { kind: "forge", displayName: "Forge" },
  { kind: "neoforge", displayName: "NeoForge" },
];

export interface Call {
  cmd: string;
  args: Record<string, unknown>;
}

/** A promise a test resolves by hand. */
export function deferred<T = void>() {
  let resolve!: (v: T) => void;
  const promise = new Promise<T>((r) => (resolve = r));
  return { promise, resolve };
}

export function instancesBackend(list: InstanceDto[] = []) {
  const calls: Call[] = [];
  const b = {
    calls,
    list,
    incomplete: false,
    versions: MANIFEST as GameVersionsDto,
    versionsFails: null as ErrorDto | null,
    /** Held until resolved: `game_versions` answers only then. */
    versionsGate: null as Promise<void> | null,
    loaders: LOADERS,
    loaderKindsFails: null as ErrorDto | null,
    /** What `loader_versions` answers for a kind and a game; a thrown `ErrorDto` is a refusal. */
    loaderVersions: (_kind: string, _game: string): LoaderVersionsDto => ({
      versions: [
        { id: "0.17.3", stable: true },
        { id: "0.17.2", stable: true },
        { id: "0.18.0-beta.1", stable: false },
      ],
      recommended: "0.17.3",
    }),
    createFails: null as ErrorDto | null,
    /** Held until resolved: `instance_create` answers only then. */
    createGate: null as Promise<void> | null,
    deleteFails: null as ErrorDto | null,
    deleteGate: null as Promise<void> | null,
    openFails: null as ErrorDto | null,
    selectFails: null as ErrorDto | null,
    listFails: null as ErrorDto | null,
    count: (cmd: string) => calls.filter((c) => c.cmd === cmd).length,
    of: (cmd: string) => calls.filter((c) => c.cmd === cmd).map((c) => c.args),
  };
  mockIPC(async (cmd, payload) => {
    const args = (payload ?? {}) as Record<string, unknown>;
    calls.push({ cmd, args });
    switch (cmd) {
      case "instances_list":
        if (b.listFails) throw b.listFails;
        return { instances: b.list, incomplete: b.incomplete };
      case "game_versions":
        if (b.versionsGate) await b.versionsGate;
        if (b.versionsFails) throw b.versionsFails;
        return b.versions;
      case "loader_kinds":
        if (b.loaderKindsFails) throw b.loaderKindsFails;
        return b.loaders;
      case "loader_versions":
        return b.loaderVersions(args.kind as string, args.game as string);
      case "instance_create": {
        if (b.createGate) await b.createGate;
        if (b.createFails) throw b.createFails;
        const req = args.req as CreateInstanceRequest;
        const id = `${req.game}${req.loader ? `-${req.loader.kind}` : ""}`;
        const made = instance(id, {
          name: req.name.trim(),
          game: req.game,
          loader: req.loader
            ? {
                kind: req.loader.kind,
                displayName: b.loaders.find((l) => l.kind === req.loader!.kind)!.displayName,
                version: req.loader.version,
              }
            : null,
          selected: true,
        });
        b.list = [...b.list.map((i) => ({ ...i, selected: false })), made];
        return made;
      }
      case "instance_select":
        if (b.selectFails) throw b.selectFails;
        b.list = b.list.map((i) => ({ ...i, selected: i.id === args.id }));
        return;
      case "instance_delete":
        if (b.deleteGate) await b.deleteGate;
        if (b.deleteFails) throw b.deleteFails;
        b.list = b.list.filter((i) => i.id !== args.id);
        return;
      case "open_location":
        if (b.openFails) throw b.openFails;
        return;
      default:
        return;
    }
  });
  return b;
}
