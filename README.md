# Hengst Client

An open-source Minecraft: Java Edition launcher focused on a clean interface and maximum FPS on every game version, with mods and modpacks from Modrinth and CurseForge.

**Status:** pre-alpha. The CLI has Microsoft sign-in (`hengst login`), multiple accounts, vanilla launching and mod loader launching (Fabric, Quilt, Legacy Fabric, Forge, NeoForge) implemented; sign-in is not usable until Mojang approves the app id (see below). A desktop app is in progress: it manages accounts and instances; starting the game from it, mod and modpack management are still to come, see [docs/PLAN.md](docs/PLAN.md) (German). Windows x64 only.

Microsoft sign-in is implemented, tested against mocked service responses; no real sign-in has been verified yet. It becomes usable once Mojang approves the launcher's Azure app id; until then the Minecraft services answer with HTTP 403 and the CLI reports that the app is not approved. In the meantime, set `HENGST_MS_CLIENT_ID` to your own approved app id.

## Install

    cargo install --git https://github.com/clemensjl/hengst-client hengst-cli --locked
    hengst versions

Requires Rust 1.89 or newer. No prebuilt binaries yet.

## Build

Windows x64 only. The CLI requires Rust 1.89 or newer.

    cargo build --release -p hengst-cli
    target/release/hengst versions
    target/release/hengst launch 1.21.11 --offline Dev --dry-run
    target/release/hengst loaders fabric 1.21.11
    target/release/hengst launch 1.21.11 --loader fabric --offline Dev --dry-run
    target/release/hengst launch 1.20.1 --loader forge --offline Dev --dry-run

A plain `cargo build --release` of the whole workspace fails on purpose: it includes the desktop app, whose release build needs the `custom-protocol` feature that `tauri build` turns on.

### Desktop app

The desktop app in `apps/desktop` needs Rust 1.90 or newer with the MSVC toolchain (Visual Studio Build Tools with the C++ workload), Node.js 22.12 or newer and pnpm (the version is pinned in the `packageManager` field of `apps/desktop/package.json`), and the WebView2 runtime, which Windows 11 ships.

    pnpm -C apps/desktop install
    pnpm -C apps/desktop tauri build

This builds the front end and then `target/release/hengst-desktop.exe`. No installer is produced yet.

### Microsoft client id

Sign-in uses the application (client) id of an Azure app registration. It is not a secret. A local build runs with the project's built-in id, the constant `CLIENT_ID` in `crates/hengst-core/src/auth/mod.rs`. Sign-in with it works only once Mojang has approved that id (see Status above).

Setting the environment variable `HENGST_MS_CLIENT_ID` before you start the app or the CLI overrides the built-in id at run time (the build does not read it). Whoever distributes their own build registers their own Azure app, which must allow personal Microsoft accounts and public client flows, uses its id (variable or `CLIENT_ID`) and needs Mojang's approval for it.

## Usage

    hengst login                              # device code sign-in, saves the account
    hengst accounts                           # list saved accounts
    hengst accounts use <name>                # change the default account
    hengst logout <name>                      # remove an account and its stored tokens
    hengst launch <version> [--account <name>]
    hengst launch <version> --offline Dev --dry-run  # download and print the command line only

    hengst loaders <loader> <game>            # list loader builds for a game version, newest first
    hengst launch <version> --loader <fabric|quilt|legacy-fabric|forge|neoforge> [--loader-version <build>]
    hengst launch <version> [--loader <loader>] --resolve-only  # print what would start, download nothing

`--resolve-only` prints one JSON line on stdout and exits with 0 (1 and a message on stderr on errors), for example `{"game":"1.21.11","loader":{"kind":"fabric","version":"0.17.3","stable":true}}`, or `{"game":"1.21.11","loader":null}` without `--loader`. It fetches only the version manifest and the loader's version list; it downloads no libraries, assets or installers and needs no account and no Java. A named `--loader-version` must be on the loader's list.

`<name>` is an account name or UUID. Without `--account`, `launch` uses the default account. Tokens are stored in the Windows Credential Manager, not in files.

`--offline <name>` starts an offline development account. It works only in builds with the cargo feature `dev-offline`; with `--dry-run` it works in every build.

## Mod loaders

`--loader` prepares and starts a game version with a loader. Without `--loader-version` the newest stable build is used (for Forge the recommended build); if only an unstable build exists, the CLI says so. `hengst loaders <loader> <game>` lists the available builds. The default instance folder is `<game>-<loader>`, for example `1.20.1-forge`.

- Fabric, Quilt and Legacy Fabric come from their meta services.
- Forge is supported from game version 1.7.10 upward, in both installer formats; Forge for older versions is refused. NeoForge is supported from 1.20.1.
- For Forge and NeoForge the installer is downloaded from the official Forge or NeoForge maven, verified by hash, and its install steps are run locally with the game's Java runtime. This runs third-party installer code on your machine. A `--dry-run` with Forge or NeoForge performs this real install before it prints the command line.
- The first start of a Forge or NeoForge build takes one to two minutes. The CLI prints one line to stderr for each phase (resolving the build, downloading files, "Step 3 of 6: jarsplitter", "Install finished in 26 s") so the terminal does not look frozen. The output of the installer steps goes to `<data directory>\logs\loader-install-<loader>-<build id>.log`, and the CLI prints its path as "Install log: ..." when the steps start; a failed install names the same log in its error.
- An install is reused on later launches. An installed Fabric, Quilt, Forge or NeoForge version also starts offline.
- Every library, jar and installer download is hash-verified; a file without a published hash is refused.

A real launch needs a signed-in Microsoft account (see above) or a development build with the `dev-offline` feature.

### What was verified

These combinations were started on Windows 11 with an offline development account, sound muted, and passed. "Started" means: the loader reported itself in the game log, the game finished loading its textures, stayed alive for 15 seconds and wrote no crash report.

| Loader | Game version | Loader build |
|---|---|---|
| Fabric | 1.21.11 | Fabric Loader 0.20.1 |
| Fabric | 26.3 (latest at test time) | Fabric Loader 0.20.1 |
| Quilt | 1.21.11 | Quilt Loader 0.30.2 |
| Legacy Fabric | 1.8.9 | Fabric Loader 0.20.1 |
| Legacy Fabric | 1.12.2 | Fabric Loader 0.20.1 |
| Forge | 1.7.10 | 10.13.4.1614 |
| Forge | 1.8.9 | 11.15.1.2318 |
| Forge | 1.12.2 | 14.23.5.2859 and 14.23.5.2847 (older installer format) |
| Forge | 1.16.5 | 36.2.34 |
| Forge | 1.20.1 | 47.4.28 |
| Forge | 26.3 | 66.1.9 |
| NeoForge | 1.20.1 | 47.1.106 |
| NeoForge | 1.21.1 | 21.1.267 |
| NeoForge | 26.3 | 26.3.0.115 |

No gameplay, no mods and no server join were tested. Other game versions are expected to work but were not started.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

GPL-3.0-or-later, see [LICENSE](LICENSE).

Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft.
