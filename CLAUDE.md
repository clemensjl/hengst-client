# Hengst Client

Open-Source-Minecraft-Launcher (Alternative zu Lunar Client). Gesamtplan: docs/PLAN.md. Umsetzungspläne: docs/superpowers/plans/.

## Stack
- Rust-Workspace: crates/hengst-core (Kern), apps/cli (Binary `hengst`). Desktop-App (Tauri 2 + React) ab Phase 4 unter apps/desktop.
- Tests: `cargo test --workspace --all-features`. Lint: `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all`.

## Regeln
- MVP nur Windows x64.
- Kein Spielstart mit Offline-Konto in Release-Builds: nur mit Feature `dev-offline`.
- "Minecraft" nie im Produktnamen. Hinweis "Not an official Minecraft product" bleibt in README und App.
- Code, Fehlertexte, README auf Englisch; docs/ auf Deutsch.
- Keine Secrets im Repo (CurseForge-Key nur als CI-Secret, Microsoft-Client-ID ist kein Secret).
- Tokens nie loggen oder ausgeben.
- Sicht- und Funktionsprüfung am PC mit Agentfenster.
- Projektordner liegt im Google-Drive-Mirror: Ordner umbenennen schlägt fehl, solange Drive synchronisiert.
