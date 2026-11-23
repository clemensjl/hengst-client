import type { GameVersionDto } from "@/ipc/bindings/GameVersionDto";
import type { VersionFilter } from "@/ipc/bindings/VersionFilter";

/** DESIGN.md 10.5: releases only until the user ticks more. */
export const DEFAULT_FILTER: VersionFilter = {
  release: true,
  snapshot: false,
  beta: false,
  alpha: false,
};

const shown = (kind: GameVersionDto["kind"], f: VersionFilter): boolean =>
  kind === "release"
    ? f.release
    : kind === "snapshot"
      ? f.snapshot
      : kind === "beta"
        ? f.beta
        : kind === "alpha"
          ? f.alpha
          : false;

/** The versions the filter shows whose id contains the query (any case), in manifest order. */
export function filterVersions(
  list: GameVersionDto[],
  f: VersionFilter,
  query: string,
): GameVersionDto[] {
  const q = query.trim().toLowerCase();
  return list.filter((v) => shown(v.kind, f) && (q === "" || v.id.toLowerCase().includes(q)));
}
