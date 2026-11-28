import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { parseLicenses } from "./licenseData";

const desktop = resolve(import.meta.dirname, "../../..");
const raw = readFileSync(resolve(import.meta.dirname, "licenses.json"), "utf8");
const data = parseLicenses(raw);

/** As `hashFile` in scripts/licenses.mjs: SHA-256 of the text with LF line endings. */
const hash = (path: string) =>
  createHash("sha256")
    .update(readFileSync(resolve(desktop, path), "utf8").replace(/\r\n/g, "\n"))
    .digest("hex");

describe("the third-party license list", () => {
  it("is generated from the current lock files, config and generator (run node scripts/licenses.mjs)", () => {
    expect(data.inputs).toEqual({
      cargoLock: hash("../../Cargo.lock"),
      pnpmLock: hash("pnpm-lock.yaml"),
      aboutToml: hash("src-tauri/about.toml"),
      generator: hash("scripts/licenses.mjs"),
    });
  });

  it("is not empty and every entry has a license text", () => {
    expect(data.rust.length).toBeGreaterThan(100);
    expect(data.javascript.length).toBeGreaterThan(10);
    for (const entry of [...data.rust, ...data.javascript]) {
      expect(entry.license, entry.name).not.toBe("");
      expect(entry.texts.length, entry.name).toBeGreaterThan(0);
      for (const i of entry.texts) expect(data.texts[i]?.length, entry.name).toBeGreaterThan(0);
    }
    const names = (list: { name: string }[]) => list.map((e) => e.name);
    expect(names(data.rust)).toContain("tauri");
    expect(names(data.javascript)).toContain("react");
  });

  it("leaves out the app's own crates", () => {
    const names = data.rust.map((e) => e.name);
    expect(names).not.toContain("hengst-core");
    expect(names).not.toContain("hengst-desktop");
  });

  it("holds no local path of the machine that generated it", () => {
    expect(raw).not.toMatch(/[A-Za-z]:[\\/]+(Users|Documents and Settings)[\\/]/i);
    expect(raw).not.toMatch(/node_modules[\\/]/);
    expect(raw).not.toMatch(/[\\/]\.cargo[\\/]registry/);
  });
});
