// @vitest-environment node
import { join, resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { targetDebugDir } from "./app.ts";

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
