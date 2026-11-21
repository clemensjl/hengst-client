import { useCallback, useEffect, useState } from "react";
import { api, IpcError } from "@/ipc/client";
import { subscribeEvents } from "@/ipc/events";
import type { AppInfo } from "@/ipc/bindings/AppInfo";
import type { GameVersionsDto } from "@/ipc/bindings/GameVersionsDto";

type Loaded = { info: AppInfo; versions: GameVersionsDto };
type State =
  { kind: "loading" } | { kind: "ready"; data: Loaded } | { kind: "error"; message: string };

const ALL_KINDS = { release: true, snapshot: true, beta: true, alpha: true };

export function AboutPanel() {
  const [state, setState] = useState<State>({ kind: "loading" });

  const load = useCallback(async () => {
    try {
      const [info, versions] = await Promise.all([api.appInfo(), api.gameVersions(ALL_KINDS)]);
      setState({ kind: "ready", data: { info, versions } });
    } catch (e) {
      setState({
        kind: "error",
        message: e instanceof IpcError ? e.dto.detail || e.dto.code : String(e),
      });
    }
  }, []);

  useEffect(() => {
    void load();
    void subscribeEvents((e) => {
      if (e.event === "dataChanged" && e.data.scope === "versions") void load();
    }).catch(() => {});
  }, [load]);

  if (state.kind === "loading") return <p>Loading</p>;
  if (state.kind === "error") {
    return (
      <div role="alert">
        <p>Could not load data: {state.message}</p>
        <button type="button" onClick={() => void load()}>
          Try again
        </button>
      </div>
    );
  }
  const { info, versions } = state.data;
  return (
    <dl>
      <dt>Version</dt>
      <dd>
        {info.version}
        {info.devBuild ? " (Development build)" : ""}
      </dd>
      <dt>Data folder</dt>
      <dd>{info.dataDir}</dd>
      <dt>Game versions</dt>
      <dd>{versions.versions.length === 0 ? "None" : versions.versions.length}</dd>
      <dt>Latest release</dt>
      <dd>{versions.latestRelease}</dd>
      <dt>Latest snapshot</dt>
      <dd>{versions.latestSnapshot}</dd>
    </dl>
  );
}
