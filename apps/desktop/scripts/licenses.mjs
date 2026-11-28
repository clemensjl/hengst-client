// Generates src/features/about/licenses.json, the third-party license list of the About dialog
// (DESIGN.md 11.10), from `cargo about` (Rust, configured by src-tauri/about.toml) and
// `pnpm licenses list --json --prod` (JavaScript). Run from apps/desktop: `node scripts/licenses.mjs`.
//
// The output is reproducible: entries are sorted, texts deduplicated in order of first use, and no
// local path, author or URL is written (only name, version, license expression and license texts).
// `inputs` holds SHA-256 hashes of everything the list depends on; `licenses.test.ts` recomputes
// them and fails when the file is stale.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const CARGO_ABOUT = "0.9.3";
const desktop = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const root = resolve(desktop, "../..");
const out = join(desktop, "src/features/about/licenses.json");

/** The files the list is made from; their hashes go into the output. */
export const INPUTS = {
  cargoLock: join(root, "Cargo.lock"),
  pnpmLock: join(desktop, "pnpm-lock.yaml"),
  aboutToml: join(desktop, "src-tauri/about.toml"),
  generator: join(desktop, "scripts/licenses.mjs"),
};

/** SHA-256 of a text file with LF line endings (a CRLF checkout hashes the same). */
export function hashFile(path) {
  const text = readFileSync(path, "utf8").replace(/\r\n/g, "\n");
  return createHash("sha256").update(text).digest("hex");
}

const run = (cmd, args, cwd) =>
  execFileSync(cmd, args, {
    cwd,
    encoding: "utf8",
    maxBuffer: 256 * 1024 * 1024,
    shell: process.platform === "win32" && cmd === "pnpm",
  });

function main() {
  const version = run("cargo", ["about", "--version"], desktop).trim();
  if (version !== `cargo-about ${CARGO_ABOUT}`) {
    throw new Error(
      `need cargo-about ${CARGO_ABOUT} (cargo install cargo-about --version ${CARGO_ABOUT} --locked --features cli), found "${version}"`,
    );
  }
  const texts = [];
  const textIndex = new Map();
  const addText = (text) => {
    const clean = text.replace(/\r\n/g, "\n").trim();
    if (!textIndex.has(clean)) {
      textIndex.set(clean, texts.length);
      texts.push(clean);
    }
    return textIndex.get(clean);
  };
  const byName = (a, b) =>
    a.name < b.name
      ? -1
      : a.name > b.name
        ? 1
        : a.version < b.version
          ? -1
          : a.version > b.version
            ? 1
            : 0;

  // Rust: cargo about resolves each crate's license expression against about.toml.
  const about = JSON.parse(
    run(
      "cargo",
      [
        "about",
        "generate",
        "--format",
        "json",
        "-c",
        "src-tauri/about.toml",
        "-m",
        "src-tauri/Cargo.toml",
      ],
      desktop,
    ),
  );
  const own = new Set(["hengst-core", "hengst-desktop"]);
  const rustTexts = new Map();
  for (const license of about.licenses) {
    for (const user of license.used_by) {
      const key = `${user.crate.name}@${user.crate.version}`;
      if (!rustTexts.has(key)) rustTexts.set(key, []);
      rustTexts.get(key).push(license.text);
    }
  }
  const rust = about.crates
    .map((c) => c.package)
    .filter((p) => !own.has(p.name))
    .map((p) => ({
      name: p.name,
      version: p.version,
      license: p.license.replace(/\s+/g, " ").trim(),
    }))
    .sort(byName)
    .map((p) => ({ ...p, texts: (rustTexts.get(`${p.name}@${p.version}`) ?? []).map(addText) }));

  // JavaScript: the production dependencies; their texts are the license files in the package.
  const pnpm = JSON.parse(run("pnpm", ["licenses", "list", "--json", "--prod"], desktop));
  const javascript = Object.values(pnpm)
    .flat()
    .flatMap((p) =>
      p.versions.map((version, i) => {
        const dir = p.paths[i];
        const files = readdirSync(dir)
          .filter((f) => /^(licen[cs]e|copying)/i.test(f))
          .sort();
        if (files.length === 0) throw new Error(`no license file in ${p.name}@${version}`);
        return {
          name: p.name,
          version,
          license: p.license,
          files: files.map((f) => readFileSync(join(dir, f), "utf8")),
        };
      }),
    )
    .sort(byName)
    .map(({ files, ...p }) => ({ ...p, texts: files.map(addText) }));

  const inputs = Object.fromEntries(Object.entries(INPUTS).map(([k, path]) => [k, hashFile(path)]));
  writeFileSync(
    out,
    `${JSON.stringify({ format: 1, inputs, rust, javascript, texts }, null, 1)}\n`,
  );
  console.log(
    `${rust.length} Rust crates, ${javascript.length} JavaScript packages, ${texts.length} texts`,
  );
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
