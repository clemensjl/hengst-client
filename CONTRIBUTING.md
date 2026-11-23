# Contributing

Bug reports, ideas and pull requests are welcome. The project is pre-alpha, so check [docs/PLAN.md](docs/PLAN.md) (German) for what is planned before starting larger work, and open an issue first for anything beyond a small fix.

## Setup

1. Fork the repository and clone your fork.
2. Install Rust 1.89 or newer (rustup, with the `rustfmt` and `clippy` components).
3. Build and run the CLI:

       cargo build
       cargo run -p hengst-cli -- versions

The MVP targets Windows x64.

## Before you open a pull request

Run the same checks as CI:

    cargo fmt --all --check
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    cargo test --workspace --all-features

### Live launch tests

`crates/hengst-core/tests/loader_live.rs` contains ignored tests that really start the game with every supported loader. They are not part of the checks above and not meant for casual use: they download several gigabytes into `%TEMP%\hengst-live`, open game windows (one at a time) and take about 13 minutes.

    cargo test -p hengst-core --features dev-offline --test loader_live -- --ignored --test-threads=1

## Desktop app tests

Toolchain: the core and the CLI build with Rust 1.89 or newer. `cargo test --workspace` and `cargo clippy --workspace` also compile the desktop crate (`apps/desktop/src-tauri`) and therefore need Rust 1.90 or newer, but neither pnpm nor WebView2: the crate builds against a placeholder `dist` and its tests use Tauri's mock runtime. Node and pnpm versions for the frontend come from `apps/desktop/.nvmrc` and the `packageManager` field of `apps/desktop/package.json`.

Frontend checks:

    pnpm -C apps/desktop typecheck
    pnpm -C apps/desktop test
    pnpm -C apps/desktop format:check

### End-to-end tests

The suite in `apps/desktop/e2e` drives the real app through WebDriver ([tauri-driver](https://v2.tauri.app/develop/tests/webdriver/) and Microsoft Edge WebDriver). It only tests debug builds with the `e2e` feature: every service is the local mock `hengst-mock`, secrets stay in memory, and each spec file runs on its own fresh data folder in `%TEMP%` (`HENGST_HOME`). No Microsoft or Mojang server is contacted. One spec moves a test instance folder (`9.9-fabric`) into the Recycle Bin.

One-time setup (tauri-driver, and the Edge WebDriver that matches the installed WebView2 runtime, saved to the git-ignored `apps/desktop/e2e/.driver/`):

    cargo install tauri-driver --locked
    powershell -File apps/desktop/e2e/setup-driver.ps1

If no session starts, run the script again with `-UseEdge` to get the driver of the installed Edge instead.

Build, then run (`pnpm e2e` does not build; it expects `target/debug/hengst-desktop.exe` and `target/debug/hengst-mock.exe`):

    pnpm -C apps/desktop tauri build --debug --no-bundle --features e2e,dev-offline
    cargo build -p hengst-testkit --bin hengst-mock
    pnpm -C apps/desktop e2e

The app window needs to keep the focus during the run: a window that takes the foreground (a game that starts, for example) closes open menus and popups, and the tests that use them fail.

`tauri:options.args` reach WebView2, not the app; the app gets its data folder and the mock URL from the environment that `wdio.conf.ts` gives `tauri-driver`. Before the first test of each spec file the suite checks that the app really runs on its temp folder and stops otherwise.

## Rules

- Code, error messages and the README are in English. Files under `docs/` are in German.
- Never commit secrets and never log or print tokens.
- "Minecraft" must not appear in the product name. The "Not an official Minecraft product" notice stays in the README and the app.
- Starting the game with an offline account is only allowed behind the `dev-offline` feature, never in release builds.

## Pull requests

1. Create a branch from `main`.
2. Keep the change focused: one fix or feature per pull request.
3. Open the pull request and fill in the template. CI must be green before it is merged.

By contributing you agree that your contribution is licensed under GPL-3.0-or-later, see [LICENSE](LICENSE).
