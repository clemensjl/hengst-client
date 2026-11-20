# Hengst Client

An open-source Minecraft: Java Edition launcher focused on a clean interface and maximum FPS on every game version, with mods and modpacks from Modrinth and CurseForge.

**Status:** pre-alpha. The CLI has Microsoft sign-in (`hengst login`), multiple accounts and vanilla launching implemented; sign-in is not usable until Mojang approves the app id (see below). The desktop app and mod support are still to come, see [docs/PLAN.md](docs/PLAN.md) (German).

Microsoft sign-in is implemented, tested against mocked service responses; no real sign-in has been verified yet. It becomes usable once Mojang approves the launcher's Azure app id; until then the Minecraft services answer with HTTP 403 and the CLI reports that the app is not approved. In the meantime, set `HENGST_MS_CLIENT_ID` to your own approved app id.

## Install

    cargo install --git https://github.com/clemensjl/hengst-client hengst-cli --locked
    hengst versions

Requires Rust 1.88 or newer. No prebuilt binaries yet.

## Build

Requires Rust 1.88 or newer.

    cargo build --release
    target/release/hengst versions
    target/release/hengst launch 1.21.11 --offline Dev --dry-run

## Usage

    hengst login                              # device code sign-in, saves the account
    hengst accounts                           # list saved accounts
    hengst accounts use <name>                # change the default account
    hengst logout <name>                      # remove an account and its stored tokens
    hengst launch <version> [--account <name>]
    hengst launch <version> --offline Dev --dry-run  # download and print the command line only

`<name>` is an account name or UUID. Without `--account`, `launch` uses the default account. Tokens are stored in the Windows Credential Manager, not in files.

`--offline <name>` starts an offline development account. It works only in builds with the cargo feature `dev-offline`; with `--dry-run` it works in every build.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

GPL-3.0-or-later, see [LICENSE](LICENSE).

Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft.
