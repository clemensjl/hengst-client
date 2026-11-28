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

CI also builds the app once (`pnpm -C apps/desktop tauri build --debug --no-bundle`; it is not started).

### End-to-end tests

The suite in `apps/desktop/e2e` drives the real app through WebDriver ([tauri-driver](https://v2.tauri.app/develop/tests/webdriver/) and Microsoft Edge WebDriver). It only tests debug builds with the `e2e` feature: every service is the local mock `hengst-mock`, secrets stay in memory, and each spec file runs on its own fresh data folder in `%TEMP%` (`HENGST_HOME`). No Microsoft or Mojang server is contacted, and no real Java or game is started: the specs that play use the test kit's `fake-java`, copied into the data folder as its Java. Every run of `instances.e2e.ts` leaves a test instance folder `9.9-fabric` in the Recycle Bin, every run of `run-lock.e2e.ts` one named `e2e-game`.

CI does not run this suite: the Edge WebDriver must match the WebView2 runtime of the machine, which a hosted runner does not pin. Run it locally before a pull request that changes the desktop app.

One-time setup (tauri-driver, and the Edge WebDriver that matches the installed WebView2 runtime, saved to the git-ignored `apps/desktop/e2e/.driver/`):

    cargo install tauri-driver --locked
    powershell -File apps/desktop/e2e/setup-driver.ps1

The script installs the driver only if it is validly signed by Microsoft and reports the expected version, and records both in `.driver/version.txt`. It does not fall back on its own: if no session starts with the WebView2 driver, run it again with `-UseEdge` to get the driver of the installed Edge instead.

Build, then run (`pnpm e2e` does not build; it expects `target/debug/hengst-desktop.exe`, `target/debug/hengst-mock.exe` and `target/debug/fake-java.exe`):

    pnpm -C apps/desktop tauri build --debug --no-bundle --features e2e,dev-offline
    cargo build -p hengst-testkit --bins
    pnpm -C apps/desktop e2e

If cargo writes its output somewhere else (a `target-dir` in a cargo config, for example), set `CARGO_TARGET_DIR` to that folder when running `pnpm e2e`: the suite then looks in `<CARGO_TARGET_DIR>/debug` (a relative path counts from the workspace root).

`cargo test` and `cargo build` of the workspace write a plain debug `hengst-desktop.exe` to the same path; build the e2e app again after them. The suite stops before the first click when it finds such a build ("the app page did not load", or "the app is not an e2e build": `app_info` reports `e2eBuild`).

The suite needs the app window in the foreground for the whole run: another window that takes the focus (a game that starts, for example) closes open menus and popups, and the tests that use them fail; a failed test logs `document.hasFocus()` to tell this apart. Run it on a desktop that is not in use, or in a VM. The app windows open on the desktop of the process that runs `pnpm -C apps/desktop e2e`, so a run started from a process on a separate Windows desktop (one created with `CreateDesktop`, as tools for invisible desktops do; not a Task View desktop) keeps its windows off the screen you use. Whether the focus-dependent tests pass there has not been verified yet.

`tauri:options.args` reach WebView2, not the app; the app gets its data folder and the mock URL from the environment that `wdio.conf.ts` gives `tauri-driver`. The run stops with exit code 1 before any app starts if something already listens on port 4444 (a hand-started or leftover tauri-driver would otherwise get the sessions and start the app on the real data folder). Each spec file starts its own tauri-driver, and the session is only created once every listener on port 4444 belongs to that driver's process. Before the first test of each spec file the suite also checks that its own tauri-driver is running and that the app reports the temp folder as its data folder; if not, or if the check itself fails, it ends the session and, if its tauri-driver is still running, that driver with the app it started; later spec files stop before they start anything, and the run exits with code 1. A tauri-driver that has already exited is not touched (its process id may belong to another program by then), so an app window it left behind has to be closed by hand. `restartApp()` (a new session on the same data folder) runs the same check again (`onReload`).

The spec files and what they need beyond the default (`setupFor` in `support/app.ts`). `play`, `log`, `settings`, `settings-java`, `tray` and `run-lock` are new and have not run yet; expect a first run to need fixes:

- `accounts`, `sign-in-*`, `instances*`: sign-in and instances against the mock scenarios named there.
- `play`: signs in E2EPlayer through the mock and plays "E2E Game" with fake-java; every Mojang answer of the mock waits 400 ms (`HENGST_MOCK_MOJANG_DELAY_MS`), so the preparation can be cancelled. The app's Copy is read back from a recorder put in place of `navigator.clipboard.writeText`; the machine's clipboard is never touched.
- `log`: a burst of 5,000 lines, follow and jump to end, level filter, run switcher, CSP and the frozen prototype with the licences dialog open. "Open log file" is never pressed (it would open a viewer window).
- `settings`, `settings-java`: memory by keyboard, language and theme kept over a restart; a stored Java on a network share refused at once. "Choose Java…" is never pressed (native dialog).
- `tray`: the window hidden on launch and on close, the tray's Open, Stop game and Quit, the quit dialog. WebDriver cannot click the tray menu or the close button, so the e2e build reads requests from `e2e-request.txt` in its data folder (`close`, `tray-open`, `tray-stop`, `tray-quit`; `src-tauri/src/e2e_hook.rs`) and calls the same handlers. Not covered: the icon, its tooltip and menu as drawn, and Quit without a game. The last test quits the app.
- `run-lock`: ends only the app's process (by its PID) while fake-java runs, starts the next app on the same folder and checks that Play and Delete are refused until the game has exited.

The specs find the app's and the game's processes below their own tauri-driver (by PID and executable path, never by name) and read window visibility from the process's visible top-level windows of class "Tauri Window" (not `MainWindowHandle`: a debug build is a console program, and its console window would count as the main window). A deliberate click on a button that just turned into another action waits `GUARD_WAIT_MS` (`support/game.ts`), because the button ignores clicks for 600 ms after such a change. A fake-java left behind by a failed test ends itself after 120 s.

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
