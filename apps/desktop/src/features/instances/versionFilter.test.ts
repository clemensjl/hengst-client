import { describe, expect, it } from "vitest";
import type { GameVersionDto } from "@/ipc/bindings/GameVersionDto";
import { DEFAULT_FILTER, filterVersions } from "./versionFilter";

const v = (id: string, kind: GameVersionDto["kind"]): GameVersionDto => ({
  id,
  kind,
  releaseTime: "2020-01-01T00:00:00Z",
});
const manifest = [
  v("1.21.11", "release"),
  v("25w14a", "snapshot"),
  v("1.20.1", "release"),
  v("1.14 Pre-Release 1", "snapshot"),
  v("1.14", "release"),
  v("b1.7.3", "beta"),
  v("a1.2.6", "alpha"),
  v("combat-test", "other"),
];
const ids = (list: GameVersionDto[]) => list.map((x) => x.id);

describe("filterVersions", () => {
  it("default filter keeps releases only", () => {
    expect(ids(filterVersions(manifest, DEFAULT_FILTER, ""))).toEqual([
      "1.21.11",
      "1.20.1",
      "1.14",
    ]);
  });

  it("each kind follows its own checkbox and other kinds are never shown", () => {
    const all = { release: true, snapshot: true, beta: true, alpha: true };
    expect(ids(filterVersions(manifest, all, ""))).not.toContain("combat-test");
    expect(ids(filterVersions(manifest, { ...all, release: false, snapshot: false }, ""))).toEqual([
      "b1.7.3",
      "a1.2.6",
    ]);
  });

  it("query matches case-insensitively and keeps manifest order", () => {
    const f = { ...DEFAULT_FILTER, snapshot: true };
    expect(ids(filterVersions(manifest, f, "PRE"))).toEqual(["1.14 Pre-Release 1"]);
    expect(ids(filterVersions(manifest, f, "1.14"))).toEqual(["1.14 Pre-Release 1", "1.14"]);
    expect(ids(filterVersions(manifest, f, "  "))).toHaveLength(5);
  });
});
