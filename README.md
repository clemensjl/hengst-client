# Hengst Client

An open-source Minecraft: Java Edition launcher focused on a clean interface and maximum FPS on every game version, with mods and modpacks from Modrinth and CurseForge.

**Status:** pre-alpha. The core can download and start any vanilla version from the command line. The desktop app, Microsoft login and mod support are in progress, see [docs/PLAN.md](docs/PLAN.md) (German).

## Build

Requires Rust 1.85 or newer.

    cargo build --release
    target/release/hengst versions
    target/release/hengst launch 1.21.11 --dry-run

## License

GPL-3.0-or-later, see [LICENSE](LICENSE).

Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft.
