# Hengst Client

An open-source Minecraft: Java Edition launcher focused on a clean interface and maximum FPS on every game version, with mods and modpacks from Modrinth and CurseForge.

**Status:** pre-alpha. The core can download and prepare any vanilla version from the command line (`hengst launch <version> --dry-run`). Starting the game needs Microsoft login, which is in progress, as are the desktop app and mod support, see [docs/PLAN.md](docs/PLAN.md) (German).

## Build

Requires Rust 1.85 or newer.

    cargo build --release
    target/release/hengst versions
    target/release/hengst launch 1.21.11 --dry-run

## License

GPL-3.0-or-later, see [LICENSE](LICENSE).

Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft.
