// @vitest-environment node
import { join, resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { descendantsOf, parseProcessTable, targetDebugDir } from "./app.ts";

const workspaceTarget = resolve(import.meta.dirname, "../../../../target/debug");

describe("targetDebugDir", () => {
  it("uses CARGO_TARGET_DIR when it is set", () => {
    const dir = resolve("D:/build-cache/x");
    expect(targetDebugDir({ CARGO_TARGET_DIR: dir })).toBe(join(dir, "debug"));
  });

  it("resolves a relative CARGO_TARGET_DIR against the workspace root, not the current folder", () => {
    const workspace = resolve(import.meta.dirname, "../../../..");
    expect(targetDebugDir({ CARGO_TARGET_DIR: "out/build" })).toBe(
      join(workspace, "out", "build", "debug"),
    );
  });

  it("falls back to target/ of the workspace without it", () => {
    expect(targetDebugDir({})).toBe(workspaceTarget);
    expect(targetDebugDir({ CARGO_TARGET_DIR: "" })).toBe(workspaceTarget);
    expect(targetDebugDir({ CARGO_TARGET_DIR: "   " })).toBe(workspaceTarget);
  });
});

describe("process table", () => {
  it("reads pid, parent and path per line and skips the rest", () => {
    const rows = parseProcessTable(
      "4\t0\t\r\n100\t4\tC:/x/tauri-driver.exe\nnoise\n200\t100\tC:/x/app.exe\n",
    );
    expect(rows).toEqual([
      { pid: 4, parent: 0, path: "" },
      { pid: 100, parent: 4, path: "C:/x/tauri-driver.exe" },
      { pid: 200, parent: 100, path: "C:/x/app.exe" },
    ]);
  });

  it("finds children and grandchildren, in any order, and nothing outside the tree", () => {
    const rows = [
      { pid: 300, parent: 200, path: "game" },
      { pid: 200, parent: 100, path: "app" },
      { pid: 100, parent: 1, path: "driver" },
      { pid: 400, parent: 999, path: "orphan" },
      { pid: 500, parent: 1, path: "other" },
    ];
    expect(
      descendantsOf(rows, 100)
        .map((r) => r.pid)
        .sort(),
    ).toEqual([200, 300]);
    expect(descendantsOf(rows, 400)).toEqual([]);
  });
});
