# Hengst Client

An open-source Minecraft: Java Edition launcher focused on a clean interface and maximum FPS on every game version, with mods and modpacks from Modrinth and CurseForge.

**Status:** pre-alpha. The CLI signs in with Microsoft accounts (`hengst login`), manages several accounts and starts any vanilla version. The desktop app and mod support are still to come, see [docs/PLAN.md](docs/PLAN.md) (German).

Login is implemented and tested against recorded responses. A real sign-in works only after Mojang approves the launcher's Azure app id. Until then the Minecraft services answer with HTTP 403 and the CLI reports that the app is not approved yet. Until the launcher's Azure app is approved by Mojang, set `HENGST_MS_CLIENT_ID` to your own approved app id.

## Install

    cargo install --git https://github.com/clemensjl/hengst-client hengst-cli --locked
    hengst versions

Requires Rust 1.88 or newer. No prebuilt binaries yet.

## Build

Requires Rust 1.88 or newer.

    cargo build --release
    target/release/hengst versions
    target/release/hengst launch 1.21.11 --dry-run

## Usage

    hengst login                              # device code sign-in, saves the account
    hengst accounts                           # list saved accounts
    hengst accounts use <name>                # change the default account
    hengst logout <name>                      # remove an account and its stored tokens
    hengst launch <version> [--account <name>]
    hengst launch <version> --dry-run         # download and print the command line only

`<name>` is an account name or UUID. Without `--account`, `launch` uses the default account. Tokens are stored in the Windows Credential Manager, not in files.

`--offline <name>` starts an offline development account. It works only in builds with the cargo feature `dev-offline`; with `--dry-run` it works in every build.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

GPL-3.0-or-later, see [LICENSE](LICENSE).

Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft.
