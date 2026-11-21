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
