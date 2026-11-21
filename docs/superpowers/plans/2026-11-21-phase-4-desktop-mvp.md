# Hengst Client Phase 4a (Desktop-App: Gerüst, Konten, Instanzen) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eine Windows-Desktop-App `apps/desktop` (Tauri 2, React, TypeScript, Tailwind, shadcn/ui) startet, zeigt echte Daten aus dem Kern, meldet Microsoft-Konten per Browser-Login (Auth Code + PKCE, Device Code als Ausweg) an, wechselt und entfernt sie und legt Instanzen (Spielversion, Loader, Loader-Version) an, listet und löscht sie. Phase 4b (Gliederung am Ende) ergänzt Startseite, Spielstart, Live-Log, Einstellungen und Abschluss zu v0.1.

**Architecture:** Alle Launcher-Logik bleibt in `hengst-core`; neu sind dort die Module `instances` und `auth::browser`. Die Tauri-Crate `apps/desktop/src-tauri` ist eine dünne Schicht aus Commands, eigenen serialisierbaren DTOs und Event-Kanälen; Kerntypen erreichen die Web-Ansicht nie direkt. Die Oberfläche spricht nur über einen typisierten Client (`src/ipc/client.ts`) mit dem Backend; die TypeScript-Typen erzeugt `ts-rs` aus den DTOs, eingecheckt und per Test gegen Drift geprüft.

**Tech Stack:** Tauri 2.12 (`tauri` 2.12.7, `tauri-build` 2.7.6, `@tauri-apps/api` 2.12.5, `@tauri-apps/cli` 2.12.6), Plugins `single-instance` 2.5.5, `opener` 2.7.3, `window-state` 2.5.0; Vite 8.3.8, React 19.3.1, TypeScript, Tailwind CSS 4.3.5 mit `@tailwindcss/vite` 4.3.5, shadcn CLI 4.23.0 (Basis Base UI), i18next 27.2.2 + react-i18next 18.0.3, zustand 5.1.0, Sonner, lucide-react; `ts-rs` 12.0.1; Tests Vitest 5.1.1 + Testing Library + jsdom, `@tauri-apps/api/mocks`, `tauri::test` (Feature `test`), wiremock, WebdriverIO + `tauri-driver` 2.1.0. Neue Kern-Abhängigkeiten nur für PKCE: `sha2`, `base64`, `getrandom`.

**Spec:** `docs/PLAN.md` (Abschnitte 2, 3, 5.3, 7 mit den M-Punkten, 8, 9, 10, 11, 12) und der Phase-4-Auftrag vom 21.11.2026 (Umfang, Architektur-, Test- und Design-Vorgaben). Offene Punkte aus den Abschlussreviews: `.superpowers/sdd/2026-11-20-phase-2-login/progress.md` (F10, Auth Code + PKCE) und `.superpowers/sdd/2026-11-20-phase-3-loader/progress.md` (F17, Job-Objekt, typisierte Lock-Fehler, Install-Log in der UI, `--resolve-only`).

**Aufteilung:** Der Auftrag ist für eine Plandatei zu groß. Diese Datei enthält **Phase 4a vollständig (Tasks 1 bis 12)** und am Ende eine **Gliederung von Phase 4b (Tasks 13 bis 22)**. 4b wird nach Abschluss von 4a als eigene Plandatei `docs/superpowers/plans/<Datum>-phase-4b-desktop-mvp.md` ausgeschrieben, gegen die dann echten Schnittstellen aus 4a.

## Ausgangslage

- Branch `phase-4-desktop` von `phase-3-loader` (Phasen 2 und 3 sind lokal fertig, aber nicht gemergt; siehe "Beim Menschen"). Ledger unter `.superpowers/sdd/2026-11-21-phase-4a-desktop/`.
- Kern-API, auf die sich dieser Plan stützt (Stand Commit 9f4c0ac): `launch::{prepare_with_progress, Endpoints, LaunchOptions, Prepared, ensure_launch_allowed, DEV_OFFLINE}`, `progress::{Progress, ProgressSink, Transfer}`, `auth::{MicrosoftAuth, AuthEndpoints, client_id, unix_now, AuthError}`, `auth::msa::{DeviceCode, MsaTokens}` mit `start_device_login`/`poll_device_login`, `auth::flow::{launch_account, LoginSession}` mit `complete_login`, `auth::store::{AccountStore, SecretStore, KeyringSecretStore, MemorySecretStore, StoredAccount, AccountsFile, load_accounts_file}`, `loaders::{LoaderKind, LoaderVersion, LoaderRequest, LoaderEndpoints, list_versions, pick_version}`, `manifest::{fetch_manifest, VersionManifest, VersionEntry, VersionKind}`, `paths::{Paths, validate_id}`, `Error`.
- `launch::run` erbt die Konsole und kann nicht gestoppt werden; Spielstart mit Ausgabe-Stream kommt in 4b (Task 14).

### Werkzeuge auf dem Rechner (geprüft 21.11.2026)

| Werkzeug | Befund | Folge |
|---|---|---|
| Rust | `rustc 1.97.1`, `stable-x86_64-pc-windows-msvc` | reicht; Workspace verlangt 1.89, `tauri` 2.12.7 verlangt 1.90 (https://crates.io/api/v1/crates/tauri): die Desktop-Crate setzt eigenes `rust-version = "1.90"`, Kern und CLI bleiben bei 1.89 |
| Node | `v22.19.0` | Vite 8.3.8 verlangt `^20.19.0 \|\| >=22.12.0` (https://registry.npmjs.org/vite/latest): passt |
| pnpm / npm | `11.9.0` / `10.9.3` | pnpm ist der Paketmanager der App |
| Tauri CLI | `cargo tauri` fehlt | kein Cargo-Install nötig: `@tauri-apps/cli` 2.12.6 als devDependency, Aufruf `pnpm tauri ...` |
| WebView2 Runtime | `157.0.4341.60` (HKLM `...\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}`, Wert `pv`) | vorhanden; Prüfweg laut https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution |
| tauri-driver | fehlt | Task 12: `cargo install tauri-driver --locked` (2.1.0, https://v2.tauri.app/develop/tests/webdriver/manual-setup/) |
| msedgedriver | fehlt | Task 12: Skript lädt den Treiber passend zur WebView2-Version 157.0.4341.60 |
| agentfenster, agentshot | als MCP-Werkzeuge vorhanden | Sichtprüfung jeder UI-Task |

### Geprüfte Fakten aus der Doku (21.11.2026)

- Projektlayout `src-tauri/`: `Cargo.toml`, `build.rs`, `tauri.conf.json`, `src/main.rs` (ruft `<lib>::run()`), `src/lib.rs`, `icons/`, `capabilities/`: https://v2.tauri.app/start/project-structure/
- Commands: `#[tauri::command]`, ein einziges `generate_handler!`, `Result<T, E>` mit `E: Serialize` lehnt das JS-Promise ab, Argumente kommen als camelCase an, async Commands ohne geliehene Argumente: https://v2.tauri.app/develop/calling-rust/
- Streaming: `tauri::ipc::Channel<T>` ist für schnelle, geordnete Daten gedacht, Events nicht für Durchsatz: https://v2.tauri.app/develop/calling-frontend/
- App-Commands sind ohne Einschränkung für alle Fenster erlaubt; eingeschränkt wird über `tauri_build::Attributes::new().app_manifest(AppManifest::new().commands(&[...]))`, das `allow-<cmd>`-Permissions erzeugt; Plugin-Commands sind standardmäßig gesperrt: https://v2.tauri.app/security/capabilities/, https://docs.rs/tauri-build/latest/tauri_build/struct.AppManifest.html
- CSP greift nur, wenn gesetzt; Tauri ergänzt beim Build Hashes und Nonces; `dangerousDisableAssetCspModification` nimmt eine Liste von Direktiven; `freezePrototype` friert `Object.prototype` ein: https://v2.tauri.app/security/csp/, https://v2.tauri.app/reference/config/
- single-instance muss als erstes Plugin registriert werden, Callback fokussiert das Fenster: https://v2.tauri.app/plugin/single-instance/
- Tray über Feature `tray-icon` und `TrayIconBuilder`: https://v2.tauri.app/learn/system-tray/ (für 4b)
- `tauri::test` mit `mock_builder`, `mock_context`, `noop_assets`, `get_ipc_response` hinter Feature `test`: https://docs.rs/tauri/latest/tauri/test/index.html; `mockIPC`/`clearMocks` für Vitest: https://v2.tauri.app/develop/tests/mocking/
- WebDriver unter Windows: `tauri-driver` plus passender `msedgedriver`, sonst hängt die Suite: https://v2.tauri.app/develop/tests/webdriver/
- shadcn: Tailwind v4 und React 19 sind Standard; `init` kennt `--base <base|radix|aria>` und `-t vite`; Vite-Einrichtung mit `@tailwindcss/vite`, Alias `@/*`: https://ui.shadcn.com/docs/cli, https://ui.shadcn.com/docs/installation/vite, https://ui.shadcn.com/docs/tailwind-v4
- Tailwind v4: Klassen-Dark-Mode über `@custom-variant dark (&:where(.dark, .dark *));`: https://tailwindcss.com/docs/dark-mode
- Microsoft: Redirect-URI unter Plattform "Mobile and desktop applications"; bei `http://localhost` wird der Port beim Abgleich ignoriert; keine Query-Parameter in Redirect-URIs bei persönlichen Konten; `http://127.0.0.1` nur über das Manifest eintragbar; PKCE `S256`, kein Secret für öffentliche Clients: https://learn.microsoft.com/en-us/entra/identity-platform/reply-url, https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow
- `tauri-specta` für Tauri 2 ist nur als Release Candidate verfügbar (2.0.0-rc.29), `ts-rs` 12.0.1 ist stabil: https://crates.io/api/v1/crates/tauri-specta, https://crates.io/api/v1/crates/ts-rs

Nicht in der Doku belegt und deshalb in den Tasks als Prüfschritt formuliert: der Inhalt von `core:default`, ob `opener` aus Rust ohne JS-Permission arbeitet, ob `--base base` Base UI bedeutet, ob msedgedriver der WebView2- oder der Edge-Version folgen muss.

## Nicht in Phase 4 (nicht bauen)

Inhalte suchen und Mods installieren (Phase 5), .mrpack, FPS-Profile und JVM-Tuning (Phase 6), Begleitmod bündeln (Phase 7), Auto-Update, Code-Signing, Installer für andere Plattformen als Windows x64 (auch kein MSI/NSIS-Release-Prozess, nur `--no-bundle`-Builds und ein lokaler Test-Bundle), Skins und Capes, Crash-Analyse, Instanzen kopieren/umbenennen/Gruppen/Symbole, Discord Rich Presence, Deep Links, Portabler Modus, Wiederverwendung von Prüfergebnissen innerhalb eines `prepare` (braucht eigenes Security-Review, Folgepunkt aus Phase 3).

## Entscheidungen

- **Typen zwischen Rust und TypeScript:** `ts-rs` 12 statt `tauri-specta`. Grund: stabil, keine RC-Abhängigkeit im Build-Pfad der App, erzeugt nur Typen und lässt die Command-Schicht unverändert. Die Dateien liegen eingecheckt unter `apps/desktop/src/ipc/bindings/`; der Rust-Test `bindings_are_current` erzeugt alle Typen neu und vergleicht sie zeichengenau mit den eingecheckten Dateien (Neuschreiben nur mit `HENGST_UPDATE_BINDINGS=1`). Die Command-Namen stehen einmal in `src-tauri/src/command_list.rs`; `build.rs` und `lib.rs` binden diese Datei per `include!` ein, und der Test `ts_client_covers_every_command` prüft, dass `src/ipc/client.ts` genau diese Namen aufruft.
- **Kanäle:** Ein globaler Kanal `subscribe_events(channel: Channel<AppEvent>)` für Zustandsänderungen, die das Backend selbst auslöst (`DataChanged`, in 4b Spielzustand); je Vorgang ein eigener `Channel` (Login, in 4b Startfortschritt und Log). Keine String-Events per `Emitter`.
- **UI-Bibliothek:** shadcn/ui mit Base UI als Primitiv-Schicht (`pick-ui-library`: base-ui für zugängliche Primitive; shadcn bietet sie als `--base`). Weitere Wahl nach `pick-ui-library`: Sonner (Toasts), cmdk (Combobox für Versionen, kommt mit shadcn `command`), zustand (Zustand), clsx + cva (von shadcn), in 4b Virtuoso (Log). Kein next-themes: Quelle des Themes sind die Einstellungen im Backend, das Fenster wird mit passender Hintergrundfarbe erzeugt, also kein Aufblitzen. Kein TanStack Query, kein Router: vier Ansichten über einen zustand-Wert.
- **Fenster:** native Windows-Titelleiste, ein Fenster mit Label `main`, Mindestgröße 960 × 600, Startgröße 1120 × 720, Größe und Position merkt `window-state`. Das Fenster wird in `setup` per `WebviewWindowBuilder` erzeugt (nicht in `tauri.conf.json`), damit Theme und Hintergrundfarbe aus den Einstellungen gesetzt sind, bevor die Web-Ansicht lädt. Zweiter App-Start fokussiert das erste Fenster.
- **Debug-Startargumente:** Nur in Debug-Builds (`cfg(debug_assertions)`) versteht die App `--home <dir>` und `--window-size <B>x<H>`, im Feature `e2e` zusätzlich `--e2e-base-url <url>` (gleichwertig zu `HENGST_E2E_BASE_URL`); damit startet agentfenster die App per `open_app` mit Argumenten, Testdaten, Mock und fester Größe, ohne Umgebungsvariablen. Release-Builds ignorieren alle drei (Datenordner dort weiter über `HENGST_HOME`).
- **Entwicklungs-Features der Desktop-Crate:** `dev-offline` (reicht an `hengst-core/dev-offline` durch, zeigt das Offline-Entwicklungskonto) und `e2e` (alle Endpunkte aus `HENGST_E2E_BASE_URL`, `MemorySecretStore` statt Anmeldeinformationsspeicher, Browser-Login per HTTP-Abruf statt echtem Browser). `e2e` außerhalb von Debug-Builds ist ein `compile_error!`. Weder `dev-offline` noch `e2e` sind in einem Release-Build für Nutzer; Über-Ansicht und Fenstertitel zeigen "Development build", sobald eines aktiv ist.
- **Browser-Login:** Redirect-URI `http://localhost:<Port>` ohne Pfad; der Kern lauscht auf `127.0.0.1:<Port>` und, falls möglich, `[::1]:<Port>` (Browser lösen `localhost` unterschiedlich auf). Eigener Mini-HTTP-Parser (eine GET-Zeile, höchstens 8 KiB, 5 s Lesefrist), kein neues HTTP-Server-Crate. Wartezeit 5 Minuten. Die Web-Ansicht bekommt die Authorize-URL nie (sie enthält `state`); "Browser erneut öffnen" ist ein Command. Die Texte der Antwortseite im Browser übergibt die Oberfläche beim Start (übersetzt), der Kern setzt sie HTML-escaped ein.
- **Auswahl des Kontos:** Das gewählte Microsoft-Konto ist `AccountsFile.default` des Kerns, also dasselbe wie in der CLI. Neu angemeldete Konten werden gewählt. Das Offline-Entwicklungskonto (nur `dev-offline`) wird über die Einstellung `devAccount: true` gewählt; Release-Builds lesen diese Einstellung nie.
- **Instanzen:** Ordner `<root>/instances/<id>/`, darin `instance.json`; der Ordner ist zugleich Spielordner wie heute (CLI-Kompatibilität, keine Unterordner). `id` ist der Ordnername (geprüft mit `validate_id`), der Anzeigename steht in `instance.json`. Eine neue Instanz bekommt als `id` den CLI-Standardnamen `<game>` bzw. `<game>-<loader-slug>`, wenn er frei ist, sonst `<...>-2`, `-3` usw.; so nutzt `hengst launch 1.20.1 --loader forge` denselben Ordner wie die erste App-Instanz "1.20.1 Forge". Ordner ohne `instance.json` (von der CLI angelegt) übernimmt die App, wenn sich der Name als Standardname einer bekannten Spielversion lesen lässt; alle anderen bleiben unangetastet und unsichtbar.
- **Loader-Version:** wird beim Anlegen festgeschrieben (Vorauswahl: empfohlene Version laut `pick_version`). Übernommene CLI-Ordner haben `version: null` und bekommen beim ersten erfolgreichen Start in 4b die aufgelöste Version eingetragen.
- **Löschen von Instanzen:** in den Windows-Papierkorb (Crate `trash`), damit es umkehrbar ist; der Dialog nennt Welten und Papierkorb ausdrücklich.
- **Einstellungen:** Datei `<root>/settings.json` mit eigenem `format`-Feld, gehört der Desktop-Crate (die CLI liest sie nicht). Eine kaputte Datei wird nach `settings.json.broken-<unix>` verschoben, die App startet mit Standardwerten und meldet das einmal.
- **Fehler:** Jeder Command liefert `Result<T, ErrorDto>`. `ErrorDto` trägt einen `code` (Union-Typ im Frontend), `params` (Strings für die Übersetzung), eine optionale `action` und `detail` (englischer Kerntext für "Details"). Die Abbildung `error_dto(&hengst_core::Error)` matcht erschöpfend ohne `_`-Zweig, damit jede neue Kernvariante den Build bricht, bis sie abgebildet ist.
- **Ordner und URLs öffnen:** nur über `open_location(target: OpenTarget)` und `open_url(target: UrlTarget)`, beides Enums; freie Pfade oder URLs aus JavaScript gibt es nicht. Das `opener`-Plugin wird nur aus Rust benutzt, die Capability gewährt keine `opener:`-Permission.
- **Sprache und Ton:** Deutsch und Englisch ab Task 3 über i18next; Deutsch duzt, Satzanfang groß, Rest klein; keine Emojis. Ohne Einstellung folgt die Sprache `navigator.language` (de* = Deutsch, sonst Englisch).
- **Ende-zu-Ende-Tests:** WebdriverIO über `tauri-driver` gegen den Debug-Build mit `--features e2e,dev-offline` und einen Mock-Dienst aus der neuen Crate `crates/hengst-testkit`; Selektoren über Rolle und zugänglichen Namen (prüft nebenbei die Beschriftung). Sichtprüfung getrennt davon mit agentfenster. Begründung in Task 12.
- **Branch und Arbeitsweise:** streng nacheinander 1 → 12, keine parallelen Builds (RAM), kein Spielstart in 4a.

## Global Constraints

- Plattform: Windows 10/11 x64.
- "Minecraft" nie im Produktnamen; Hinweis "Not an official Minecraft product" bleibt in README und kommt in die App (Über-Ansicht, 4b Task 20; in 4a schon im Fußbereich der Einstellungen).
- Kein Spielstart mit Offline-Konto in Release-Builds: nur mit Feature `dev-offline`.
- Code, Kommentare, Fehlertexte im Rust-Code, README: Englisch. `docs/` und dieser Plan: Deutsch mit echten Umlauten. UI-Texte nur über i18n, Deutsch und Englisch vollständig.
- Tokens nie loggen, ausgeben, serialisieren oder an die Web-Ansicht geben; DTOs enthalten keine `Secret`-Felder.
- Keine Telemetrie, keine Netzaufrufe aus JavaScript (`connect-src` nur IPC).
- Keine Emojis in Code, UI, Doku, Commits. Icons (lucide) sind erlaubt.
- Keine Minecraft-Assets, -Schriften oder -Logos.
- Alle Basis-URLs injizierbar; Standardtests ohne Live-Aufrufe; kein Kontakt zu Microsoft-Servern in Tests.
- Rust: jeder Task endet mit `cargo test --workspace --all-features`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all` ohne Diff.
- Frontend (ab Task 2): `pnpm -C apps/desktop typecheck`, `pnpm -C apps/desktop test`, `pnpm -C apps/desktop format:check` grün.
- UI-Tasks: `produkt-regelwerk` im BAUEN-Modus (Zustandsliste vor dem Code, Gegenprüfung C1 bis C27 nach dem Code), Sichtprüfung der laufenden App mit agentfenster in hell und dunkel, Deutsch und Englisch, bei 960 × 600 und 1920 × 1080; Screenshots unter `.superpowers/sdd/2026-11-21-phase-4a-desktop/evidence/task-<n>/`. Jede neue Animation nach `motion-design`, geprüft nach `~/.claude/skills/review-animations/SKILL.md`. Neue Komponenten erst nach `~/.claude/skills/pick-ui-library/SKILL.md`, sonst shadcn.
- Jeder Task endet mit einem Commit, Dateien einzeln gestaged, Trailer `Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>`. Kein Push.

## Review Focus

1. **Token erreicht die Web-Ansicht oder einen sichtbaren Text** (Login-Events, Fehlerdetails, Kontolisten, später Spielargumente): kein Token-String in irgendeinem serialisierten DTO. Tests `login_events_never_contain_tokens` (Task 7) und `error_detail_never_contains_secret` (Task 4).
2. **CLI-Ordner trifft App-Instanz** (`instances/1.20.1-forge` ohne `instance.json` existiert, Nutzer legt "1.20.1 Forge" an; Snapshot-Ids mit Bindestrichen wie `26.4-snapshot-6-fabric`): Übernahme statt Überschreiben, neue Instanz bekommt `-2`, Spielstände bleiben. Tests `create_never_reuses_an_existing_folder`, `adopts_cli_folders_by_default_name` (Task 9).
3. **Mojang hat die App noch nicht freigegeben, Netz weg, Anmeldeinformationsspeicher verweigert**: verständliche Meldung mit passender Aktion statt Absturz oder Rohtext; die Kontoliste funktioniert ohne Schlüsselspeicher. Tests `not_approved_login_maps_to_app_not_approved` (Task 7), `accounts_list_works_without_secret_store` (Task 7), E2E `shows_not_approved_explanation` (Task 12).
4. **Kaputte oder zu neue Dateien** (`instance.json` mit `format: 2`, abgeschnittenes JSON, kaputte `settings.json`): Instanz erscheint als "nicht lesbar" bzw. "neuere Version", wird nie überschrieben; Einstellungen fallen mit Sicherungskopie auf Standard zurück. Tests `too_new_instance_is_listed_and_never_written`, `broken_instance_is_listed_not_deleted` (Task 9), `broken_settings_are_moved_aside` (Task 3).
5. **Doppelklick und gleichzeitige Vorgänge** (zweimal "Instanz anlegen", zweiter Login während der erste wartet, Löschen während Anlegen): genau ein Vorgang, Knopf gesperrt mit Ladezustand. Tests `double_submit_creates_one_instance` (Task 11, Vitest), `second_login_cancels_the_first` (Task 7), `create_and_delete_are_serialized` (Task 10).

---

## Dateistruktur (4a)

```
Cargo.toml                                   + members apps/desktop/src-tauri, crates/hengst-testkit
DESIGN.md                                    Design-Tokens, Komponenten, Layout, Zustände (Task 1)
crates/hengst-core/src/
  manifest.rs                                + load_cached
  error.rs                                   + LockTimeout, Cancelled, Instance*, LoaderHint
  loaders/mod.rs                             LoaderKind: serde als Slug, display_name; LoaderHint
  loaders/forge/mod.rs                       lock_file liefert Error::LockTimeout
  auth/browser.rs                            Auth Code + PKCE mit Loopback (Task 5)
  auth/error.rs                              + BrowserLoginTimedOut, LoopbackUnavailable, BrowserLoginFailed
  auth/store.rs                              SecretStore für Box<dyn SecretStore>
  instances.rs                               instance.json, anlegen, listen, übernehmen (Task 9)
crates/hengst-testkit/                       publish = false: Mock-Dienste für Tests und E2E
  src/lib.rs, src/auth.rs, src/mojang.rs, src/bin/hengst-mock.rs
apps/desktop/
  package.json, pnpm-lock.yaml, index.html, vite.config.ts, tsconfig*.json, components.json, .prettierrc
  brand/mark.svg, brand/README.md            Platzhalter-Zeichen, einzige Tauschstelle
  src/main.tsx, src/styles/index.css
  src/app/{App.tsx,Shell.tsx,Sidebar.tsx,views.ts}
  src/ipc/{client.ts,events.ts,errors.ts}, src/ipc/bindings/*.ts (erzeugt)
  src/i18n/{index.ts,de.json,en.json}
  src/stores/app.ts
  src/theme/theme.ts
  src/features/about/AboutPanel.tsx
  src/features/settings/SettingsView.tsx
  src/features/errors/{ErrorPanel.tsx,errorCopy.ts}
  src/features/accounts/{AccountChip.tsx,AccountMenu.tsx,SignInDialog.tsx,RemoveAccountDialog.tsx}
  src/features/instances/{InstancesView.tsx,InstanceRow.tsx,CreateInstanceDialog.tsx,DeleteInstanceDialog.tsx,naming.ts,versionFilter.ts}
  src/components/ui/*                        von shadcn erzeugt
  src/lib/utils.ts
  src-tauri/Cargo.toml, build.rs, tauri.conf.json, capabilities/main.json, icons/
  src-tauri/src/{main.rs,lib.rs,command_list.rs,state.rs,events.rs,error.rs,settings.rs,open.rs,startup.rs}
  src-tauri/src/dto/{mod.rs,app.rs,versions.rs,accounts.rs,instances.rs,settings.rs,error.rs}
  src-tauri/src/commands/{mod.rs,app.rs,versions.rs,settings.rs,accounts.rs,instances.rs,open.rs}
  src-tauri/src/accounts.rs                  Kontodienst, Login-Vorgänge
  src-tauri/tests/bindings.rs
  e2e/wdio.conf.ts, e2e/setup-driver.ps1, e2e/support/app.ts, e2e/specs/{accounts.e2e.ts,instances.e2e.ts}
                                             (WebdriverIO-Pakete in apps/desktop/package.json, kein eigenes package.json)
```

---

### Task 1: DESIGN.md

**Files:**
- Create: `DESIGN.md` (Repository-Wurzel, Englisch wie README, weil es Entwickler- und Beitragenden-Doku neben dem Code ist; Tokens sind sprachneutral)

**Interfaces:**
- Produces: Token-Namen, die Task 3 als CSS-Variablen anlegt und alle UI-Tasks benutzen: `--bg`, `--surface`, `--surface-raised`, `--border`, `--border-strong`, `--text`, `--text-muted`, `--accent`, `--accent-hover`, `--accent-pressed`, `--on-accent`, `--focus`, `--danger`, `--on-danger`, `--warning`, `--success`, `--log-error`, `--log-warn`, `--log-info`, `--log-debug`; Abstände `--space-1..7`; Radien `--radius-control`, `--radius-panel`, `--radius-pill`; Dauern `--dur-press`, `--dur-popover-in`, `--dur-popover-out`, `--dur-dialog-in`, `--dur-dialog-out`; Kurve `--ease-out`.

Vorgaben, die DESIGN.md festschreibt (Werte sind Entscheidungen dieses Plans; die Kontrastprüfung in Task 3 kann einzelne Werte nachschärfen, dann wird DESIGN.md im selben Commit angepasst):

Richtung: ruhiges Werkzeug, das man täglich mehrmals öffnet. Eine Stelle darf auffallen: Startseite mit Instanzname in großer Schrift und dem Spielen-Knopf (4b). Alles andere still: Flächen durch Hintergrundstufen und Linien getrennt, keine Karten-Raster mit Schatten, keine Verläufe, keine Großbuchstaben-Labels, keine nummerierten Marker, kein "→" an Knöpfen. Nach `frontend-design` geprüft gegen die fünf Standardmuster (Creme + Terrakotta, Fast-Schwarz + Neon, Zeitungsraster, SaaS-Karten, Template-Chrome): keins davon.

| Token | Hell | Dunkel |
|---|---|---|
| `--bg` | `#F3F4F6` | `#121417` |
| `--surface` | `#FAFAFB` | `#1A1D21` |
| `--surface-raised` | `#FDFDFE` | `#22262B` |
| `--border` | `#D9DCE1` | `#2E333A` |
| `--border-strong` | `#B9BEC6` | `#454C55` |
| `--text` | `#1A1D21` | `#E7E9EC` |
| `--text-muted` | `#59616D` | `#9AA3AE` |
| `--accent` | `#1D6F80` | `#4FB0C2` |
| `--accent-hover` | `#185E6D` | `#69BECD` |
| `--accent-pressed` | `#134E5B` | `#86CCD8` |
| `--on-accent` | `#F6FBFC` | `#0B1A1E` |
| `--focus` | `#1D6F80` | `#69BECD` |
| `--danger` | `#B42318` | `#F0645A` |
| `--on-danger` | `#FFF7F6` | `#1F0A08` |
| `--warning` | `#8A5A00` | `#E0A93B` |
| `--success` | `#2D7A4E` | `#5BB683` |
| `--log-error` / `-warn` / `-info` / `-debug` | `#B42318` / `#8A5A00` / `#1A1D21` / `#59616D` | `#F0645A` / `#E0A93B` / `#E7E9EC` / `#9AA3AE` |

- Typografie: Systemschrift, weil die App nur unter Windows läuft und sich wie ein Windows-Werkzeug anfühlen soll (keine Schriftdatei im Bundle): `"Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif`, Überschriften `"Segoe UI Variable Display"`, Log und Codes `"Cascadia Mono", Consolas, monospace`. Skala (px/Zeilenhöhe): Text 16/24 (auch in Eingabefeldern, FO15), sekundär 14/20, Fußnote 13/18, h2 18/26 Semibold, h1 24/32 Semibold, Startseiten-Titel 32/40 Semibold (4b), Log 13/20 Mono. Zeilenlänge Fließtext höchstens 72 Zeichen.
- Abstände: 4-px-Raster, `--space-1..7` = 4, 8, 12, 16, 24, 32, 48.
- Radien: 6 px Bedienelemente, 10 px Panels und Dialoge, 999 px Chips. Bewusst zwei Stufen, nicht ein Radius für alles.
- Höhe: Ebene 0 flach (Seiten, Listen), Ebene 1 nur für Popover, Menüs, Dialoge: hell `0 8px 24px rgb(16 18 20 / 0.14)`, dunkel Rand `--border-strong` plus `0 8px 24px rgb(0 0 0 / 0.45)`.
- Trefferflächen: Knöpfe 40 px hoch, Icon-Knöpfe 40 × 40, Listenzeilen 48 px, Spielen-Knopf 56 px (4b). Begründung für check mode: Maus-App, 40 px liegt über der Untergrenze 24 × 24 aus T1 und nahe an 44; als Entscheidung in DESIGN.md vermerkt.
- Bewegung: `--ease-out: cubic-bezier(0.2, 0, 0, 1)` für alles, nie `ease-in`; Dauern: Drücken/Farbwechsel 120 ms, Popover/Menü rein 160 ms, raus 120 ms, Dialog rein 220 ms, raus 160 ms, Fortschrittsbalken Breite 200 ms linear (4b). Keine Bewegung bei Ansichtswechsel, Tastaturaktionen und Listenaktualisierung (häufig). `prefers-reduced-motion: reduce`: nur Deckkraft, Dauer 0 bis 80 ms. Gedrückt-Zustand ≤ 100 ms per Farbe (`--accent-pressed`), Skalierung nur am Spielen-Knopf (0,98).
- Fokus: 2 px Ring `--focus` mit 2 px Abstand über `:focus-visible`, nie entfernt.
- Fenster und Raster: native Titelleiste; links Seitenleiste 216 px (unter 1100 px Fensterbreite 64-px-Leiste nur mit Icons, Namen als `aria-label` und Tooltip), oben im Inhalt eine 56-px-Kopfzeile mit Seitentitel links und Konto-Chip rechts; Inhalt mit 32 px Rand (24 px unter 1100 px), Inhaltsbreite höchstens 1120 px, linksbündig.
- Komponenten-Inventar mit shadcn-Teil: Button (`primary`, `secondary`, `ghost`, `destructive`; Größen `default` 40, `icon` 40, `lg` 56), Dialog und AlertDialog (Bestätigen), DropdownMenu (Zeilenaktionen, Kontomenü), Popover + Command (Versions-Combobox), Select (Loader-Version), RadioGroup als Segmente (Loader), Checkbox (Versionsfilter), Input + Label, Skeleton, Tooltip, Sonner (Toasts mit Rückgängig nicht nötig, weil Papierkorb), Separator, ScrollArea, Badge (Status "Empfohlen", "Instabil", "Entwicklungs-Build"), Progress (4b), Switch (Einstellungen), Kbd nicht.
- Platzhalter-Zeichen: stilisiertes "H" aus zwei Balken und Querbalken in `--accent` auf abgerundetem Quadrat, SVG unter `apps/desktop/brand/mark.svg`; einzige Tauschstelle ist der Ordner `apps/desktop/brand/` (Anleitung in `brand/README.md`, Task 3).
- Zustände je Ansicht (Text sinngemäß, endgültig in den i18n-Dateien): Instanzen leer ("Noch keine Instanz", Hauptaktion "Instanz anlegen"), laden (drei Skeleton-Zeilen), Fehler (ErrorPanel mit "Erneut versuchen"); Konten leer ("Kein Konto angemeldet", Hauptaktion "Mit Microsoft anmelden"), Anmeldung wartet (Text plus "Browser erneut öffnen", "Code stattdessen verwenden", "Abbrechen"), Anmeldung nicht freigegeben (eigene Erklärung, siehe Task 8); Einstellungen laden (Skeleton), Fehler beim Speichern (Inline unter dem Feld); Startseite und Log in 4b.

- [ ] **Step 1: DESIGN.md schreiben** mit den Abschnitten Direction, Tokens (Tabellen hell/dunkel), Typography, Spacing, Radii, Elevation, Motion, Focus, Window and layout (ASCII-Skizze 1120 × 720 und 960 × 600), Components (Inventar mit shadcn-Zuordnung), Brand mark, States per screen, Decisions (Trefferfläche 40 px, Systemschrift, keine Karten).
- [ ] **Step 2: Gegenprüfung** gegen `frontend-design` (fünf Standardmuster, "remove one accessory") und `produkt-regelwerk` Kurzfassung (WD9: kein reines Schwarz/Weiß als Textfläche, M2/M3 Dauern und Kurven, A1 Fokus). Ergebnis als Abschnitt "Review notes" (drei bis fünf Zeilen) in DESIGN.md.
- [ ] **Step 3: Commit**

```bash
git add DESIGN.md
git commit -m "docs: add DESIGN.md for the desktop app"
```

---

### Task 2: Walking Skeleton (Fenster, ein Command, ein Ereignisstrom)

**Files:**
- Modify: `Cargo.toml` (Member `apps/desktop/src-tauri`), `crates/hengst-core/src/manifest.rs` (+ `load_cached`), `.gitignore` (+ `apps/desktop/node_modules/`, `apps/desktop/dist/`, `apps/desktop/e2e/.driver/`)
- Create: `apps/desktop/package.json`, `index.html`, `vite.config.ts`, `tsconfig.json`, `tsconfig.app.json`, `tsconfig.node.json`, `.prettierrc`, `components.json` (shadcn init), `src/main.tsx`, `src/app/App.tsx`, `src/features/about/AboutPanel.tsx`, `src/ipc/client.ts`, `src/ipc/events.ts`, `src/ipc/bindings/*.ts`, `src/lib/utils.ts`, `src/styles/index.css`
- Create: `apps/desktop/src-tauri/Cargo.toml`, `build.rs`, `tauri.conf.json`, `capabilities/main.json`, `icons/` (Platzhalter aus `pnpm tauri icon`), `src/main.rs`, `src/lib.rs`, `src/command_list.rs`, `src/state.rs`, `src/events.rs`, `src/error.rs`, `src/startup.rs`, `src/dto/{mod.rs,app.rs,versions.rs,error.rs}`, `src/commands/{mod.rs,app.rs,versions.rs}`, `tests/bindings.rs`
- Test: `apps/desktop/src-tauri/src/commands/versions.rs` (`#[cfg(test)]`), `apps/desktop/src-tauri/tests/bindings.rs`, `apps/desktop/src/ipc/client.test.ts`

**Interfaces:**
- Consumes: `manifest::{fetch_manifest, VersionManifest, VersionKind}`, `paths::Paths::{default_root, manifest_file}`, `launch::Endpoints`.
- Produces (Kern): `manifest::load_cached(paths: &Paths) -> Result<Option<VersionManifest>>` (fehlende Datei `Ok(None)`, kaputte Datei `Err(Json)`).
- Produces (Desktop, Rust):
  - Crate `hengst-desktop` (`[lib] name = "hengst_desktop_lib"`), `rust-version = "1.90"`, Features `dev-offline = ["hengst-core/dev-offline"]`, `e2e = []`; dev-dependency `tauri` mit Feature `test`.
  - `pub struct AppState { pub paths: Paths, pub endpoints: Endpoints, pub events: EventHub, pub versions: VersionsCache }` mit `AppState::new(paths: Paths, endpoints: Endpoints) -> AppState`; spätere Tasks fügen Felder hinzu.
  - `pub struct EventHub` mit `fn subscribe(&self, sink: EventSink<AppEvent>)` (ersetzt einen früheren Abonnenten) und `fn emit(&self, e: AppEvent)` (ohne Abonnent: verworfen). `pub type EventSink<T> = Arc<dyn Fn(T) + Send + Sync>`; Commands wickeln `Channel<T>` in einen `EventSink`.
  - `AppEvent` (`#[serde(tag = "event", content = "data", rename_all = "camelCase")]`): `DataChanged { scope: DataScope }`; `DataScope`: `Versions`, `Settings`, `Accounts`, `Instances`.
  - DTOs (alle `#[derive(Serialize, Deserialize, TS)]`, `#[serde(rename_all = "camelCase")]`): `AppInfo { version: String, data_dir: String, dev_build: bool, dev_offline: bool }`; `VersionFilter { release: bool, snapshot: bool, beta: bool, alpha: bool }`; `GameVersionKind` (`release`, `snapshot`, `beta`, `alpha`, `other`); `GameVersionDto { id: String, kind: GameVersionKind, release_time: String }`; `GameVersionsDto { versions: Vec<GameVersionDto>, latest_release: String, latest_snapshot: String, from_cache: bool }`.
  - `ErrorDto { code: ErrorCode, params: BTreeMap<String, String>, action: Option<ErrorAction>, detail: String }` mit vorerst `ErrorCode::{Internal, Network}` und `ErrorAction` (`#[serde(tag = "kind", rename_all = "camelCase")]`) mit nur `Retry`, also JSON `{"kind":"retry"}`; Task 4 vervollständigt beide.
  - Commands: `app_info() -> Result<AppInfo, ErrorDto>`, `subscribe_events(channel: Channel<AppEvent>) -> Result<(), ErrorDto>`, `game_versions(filter: VersionFilter) -> Result<GameVersionsDto, ErrorDto>`. Logik in einfachen Funktionen `commands::versions::game_versions_impl(state: &AppState, filter: VersionFilter) -> Result<GameVersionsDto, ErrorDto>`, die `#[tauri::command]`-Hüllen rufen nur diese auf.
  - `command_list.rs`: `pub const COMMANDS: &[&str] = &["app_info", "subscribe_events", "game_versions"];`
- Produces (Frontend): `src/ipc/client.ts` mit `export const api = { appInfo(): Promise<AppInfo>, gameVersions(filter: VersionFilter): Promise<GameVersionsDto> }`, `subscribeEvents(onEvent: (e: AppEvent) => void): Promise<void>` in `events.ts`; `export class IpcError extends Error { constructor(readonly dto: ErrorDto) }`; jede Ablehnung von `invoke` wird zu `IpcError` (ist der Ablehnungswert kein `ErrorDto`, entsteht `{ code: "internal", params: {}, action: null, detail: String(value) }`).

Verhalten `game_versions`: Liegt ein Cache vor, kommt sofort die gefilterte Liste daraus (`from_cache: true`), und einmal je App-Lauf startet im Hintergrund `fetch_manifest`; unterscheidet sich das Ergebnis (`latest` oder Anzahl), sendet der Hub `DataChanged { scope: Versions }`. Ohne Cache wartet der Aufruf auf `fetch_manifest` (`from_cache: false`). Reihenfolge wie im Manifest (neueste zuerst).

Konfiguration:
- `tauri.conf.json`: `productName` "Hengst Client", `identifier` "io.github.clemensjl.hengst-client", `version` "0.1.0", `build.devUrl` `http://localhost:1420`, `build.frontendDist` `../dist`, `beforeDevCommand` `pnpm dev`, `beforeBuildCommand` `pnpm build`, `app.windows: []` (Fenster entsteht in `setup`), `app.withGlobalTauri: false`, `app.security.freezePrototype: true`, `app.security.csp`: `default-src 'self'; script-src 'self'; style-src 'self'; style-src-attr 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src ipc: http://ipc.localhost; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'`; `app.security.devCsp` wie `csp`, aber `connect-src ipc: http://ipc.localhost ws://localhost:1420` und `script-src 'self' 'unsafe-inline'` (Vite-Preamble von React-Refresh); `bundle.active: false` in 4a.
- `build.rs`: `tauri_build::try_build(Attributes::new().app_manifest(AppManifest::new().commands(COMMANDS)))` mit `include!("src/command_list.rs")`; legt `../dist/index.html` mit einem Satz "Front end not built: run pnpm build in apps/desktop" an, wenn `../dist` fehlt (damit `cargo test --workspace` ohne pnpm baut).
- `capabilities/main.json`: `identifier` "main", `windows` ["main"], `permissions`: die erzeugten `allow-<cmd>` für jeden Eintrag in `COMMANDS` und keine Plugin- oder `core:`-Permission außer denen, die der Kanal nachweislich braucht (Step 5 ermittelt sie).
- `vite.config.ts`: Port 1420 `strictPort`, `server.watch.ignored: ["**/src-tauri/**"]`, Plugins `react()`, `tailwindcss()`, Alias `@` → `./src`, `test` mit `environment: "jsdom"`.
- `package.json` Skripte: `dev` (vite), `build` (`tsc -b && vite build`), `typecheck` (`tsc -b --noEmit`), `test` (`vitest run`), `format`/`format:check` (prettier), `tauri` (tauri), `e2e` (Task 12). Versionen exakt wie im Tech-Stack, übrige per `pnpm add` mit exakter Version (`save-exact`).
- shadcn: `pnpm dlx shadcn@4.23.0 init -t vite --base base` in `apps/desktop`; danach muss `components.json` und das erzeugte `button` Base UI importieren (`@base-ui-components/react` o. ä.). Erzeugt die CLI Radix-Importe, `--base` mit dem Wert wiederholen, den `pnpm dlx shadcn@4.23.0 init --help` für Base UI nennt.

- [ ] **Step 1: Failing Tests schreiben**

Kern (`manifest.rs`):
```rust
#[test]
fn load_cached_reads_what_fetch_manifest_saved_and_none_without_file() {
    // leerer Paths -> Ok(None); Datei mit gültigem Manifest -> Some(m) mit m.latest.release == "1.21.11"; "{" -> Err(Error::Json(_))
}
```

Desktop (`commands/versions.rs`, wiremock-Manifest mit 1.21.11 release, 25w45a snapshot, b1.7.3 old_beta, a1.2.6 old_alpha):
```rust
#[tokio::test]
async fn filter_selects_kinds_in_manifest_order() {
    let r = game_versions_impl(&state, VersionFilter { release: true, snapshot: false, beta: true, alpha: false }).await.unwrap();
    assert_eq!(ids(&r), ["1.21.11", "b1.7.3"]);
    assert_eq!(r.latest_release, "1.21.11");
    assert!(!r.from_cache);
}
#[tokio::test]
async fn cached_list_answers_first_and_refresh_emits_versions_changed() {
    // Cache mit nur 1.21.10 schreiben, Server liefert 1.21.11
    // erster Aufruf: ids == ["1.21.10"], from_cache == true
    // innerhalb 2 s ein AppEvent::DataChanged { scope: Versions } am Hub
    // zweiter Aufruf: ids beginnt mit "1.21.11"
}
#[tokio::test]
async fn unreachable_server_without_cache_is_a_network_error() {
    // Endpunkt auf geschlossenen Port -> Err(dto) mit dto.code == ErrorCode::Network
}
#[test]
fn hub_delivers_to_latest_subscriber_only() {
    // zwei subscribe, ein emit -> nur der zweite Sink sieht das Ereignis
}
#[test]
fn app_info_answers_over_ipc() {
    // tauri::test::mock_builder() mit invoke_handler + AppState, get_ipc_response("app_info")
    // -> JSON mit "version" == env!("CARGO_PKG_VERSION") und "devBuild" == cfg!(debug_assertions)
}
```

`tests/bindings.rs`:
```rust
#[test]
fn bindings_are_current() {
    // rendert jeden DTO-Typ aus dto::ALL_TYPES (Liste in dto/mod.rs) mit ts-rs in einen Tempordner
    // und vergleicht jede Datei byte-gleich mit apps/desktop/src/ipc/bindings/;
    // mit HENGST_UPDATE_BINDINGS=1 schreibt der Test stattdessen und besteht
}
#[test]
fn ts_client_covers_every_command() {
    // liest apps/desktop/src/ipc/client.ts und events.ts, sammelt invoke("<name>"
    // assert_eq!(gesammelt als Menge, COMMANDS als Menge)
}
```

Frontend (`client.test.ts`, `mockIPC`, `clearMocks` in `afterEach`):
```ts
it("calls game_versions with camelCase args", async () => {
  // mockIPC((cmd, args) => { seen = [cmd, args]; return { versions: [], latestRelease: "1.21.11", latestSnapshot: "x", fromCache: false } })
  await api.gameVersions({ release: true, snapshot: false, beta: false, alpha: false });
  expect(seen).toEqual(["game_versions", { filter: { release: true, snapshot: false, beta: false, alpha: false } }]);
});
it("turns a rejected ErrorDto into IpcError", async () => {
  // mockIPC wirft { code: "network", params: {}, action: { kind: "retry" }, detail: "x" }
  await expect(api.appInfo()).rejects.toMatchObject({ dto: { code: "network" } });
});
it("wraps a non-DTO rejection as internal", async () => { /* wirft "boom" -> dto.code "internal", detail "boom" */ });
```

- [ ] **Step 2: Tests laufen lassen, sie scheitern**

Run: `cargo test -p hengst-core load_cached` und `cargo test -p hengst-desktop`, `pnpm -C apps/desktop test`
Expected: FAIL (Funktionen, Crate, Client fehlen)

- [ ] **Step 3: Implementieren**

Gerüst wie in Interfaces und Konfiguration. `startup.rs`: `pub fn run()` registriert in dieser Reihenfolge `tauri_plugin_single_instance::init` (Callback: Fenster `main` zeigen, entminimieren, fokussieren), `tauri_plugin_window_state`, `tauri_plugin_opener`, `.manage(AppState::new(...))`, `.invoke_handler(generate_handler![...])`, `setup` erzeugt das Fenster `main` (Titel "Hengst Client", plus " (Development build)" bei `dev_build`, min 960 × 600, Start 1120 × 720, Hintergrund `--bg` hell, bis Task 3 das Theme liest). Debug-Argumente `--home`, `--window-size` in `startup.rs` (nur `cfg(debug_assertions)`). `AboutPanel` zeigt Version, Datenordner, Anzahl Versionen und neueste Release/Snapshot mit Lade-, Leer- und Fehlerzustand und abonniert `DataChanged(Versions)` zum Nachladen. Bindings erzeugen mit `HENGST_UPDATE_BINDINGS=1 cargo test -p hengst-desktop --test bindings`.

- [ ] **Step 4: Tests grün**

Run: `cargo test --workspace --all-features` und `pnpm -C apps/desktop typecheck && pnpm -C apps/desktop test`
Expected: PASS

- [ ] **Step 5: App starten, Capability und CSP prüfen**

Run: `pnpm -C apps/desktop tauri build --debug --no-bundle` und dann `target/debug/hengst-desktop.exe --home %TEMP%\hengst-4a-t2`
Expected: Fenster öffnet, AboutPanel zeigt echte Versionszahlen. Mit agentfenster (`open_app` auf die Exe mit den Argumenten, `read_desktop`): Panel-Text lesen, Screenshot unter `evidence/task-2/`. Zweiter Start der Exe fokussiert das erste Fenster, kein zweites Fenster (`list_windows`). Für den Kanal nötige `core:`-Permission: wenn `subscribe_events` ohne Zusatz abgelehnt wird, die in der Fehlermeldung genannte Permission einzeln eintragen und hier notieren. In der Web-Konsole (Debug-Build, Devtools) keine CSP-Verletzung; meldet Tauris Nonce-Ergänzung einen Konflikt mit `style-src-attr`, `dangerousDisableAssetCspModification: ["style-src-attr"]` setzen. Ein Aufruf `invoke("plugin:opener|open_url", ...)` aus der Konsole wird abgelehnt.

- [ ] **Step 6: Lint, Format, Commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace --all-features && pnpm -C apps/desktop format:check`
```bash
git add Cargo.toml Cargo.lock .gitignore crates/hengst-core/src/manifest.rs apps/desktop/package.json apps/desktop/pnpm-lock.yaml apps/desktop/index.html apps/desktop/vite.config.ts apps/desktop/tsconfig.json apps/desktop/tsconfig.app.json apps/desktop/tsconfig.node.json apps/desktop/.prettierrc apps/desktop/components.json apps/desktop/src apps/desktop/src-tauri
git commit -m "feat(desktop): walking skeleton with versions command and event channel"
```

---

### Task 3: App-Rahmen (Layout, Theme, Sprache, Einstellungen, Zeichen)

**Files:**
- Create: `apps/desktop/src-tauri/src/settings.rs`, `src/dto/settings.rs`, `src/commands/settings.rs`
- Create: `apps/desktop/src/app/{Shell.tsx,Sidebar.tsx,views.ts}`, `src/stores/app.ts`, `src/theme/theme.ts`, `src/i18n/{index.ts,de.json,en.json}`, `src/features/settings/SettingsView.tsx`, `brand/mark.svg`, `brand/README.md`, `src/styles/tokens.test.ts`, `src/i18n/i18n.test.ts`, `src/theme/theme.test.ts`
- Modify: `src/styles/index.css` (Tokens aus DESIGN.md), `src/app/App.tsx`, `src-tauri/src/{state.rs,startup.rs,command_list.rs,dto/mod.rs}`, `src-tauri/icons/` (aus `brand/mark.svg`)

**Interfaces:**
- Consumes: `AppState`, `EventHub`, `ErrorDto` (Task 2), Tokens (Task 1).
- Produces (Rust):
  - `settings.rs`: `pub const SETTINGS_FORMAT: u32 = 1;` `#[serde(rename_all = "camelCase")] pub struct Settings { pub format: u32, pub language: Option<Language>, pub theme: Theme, pub memory_mb: u32, pub java_override: Option<PathBuf>, pub minimize_on_launch: bool, pub selected_instance: Option<String>, pub dev_account: bool }`, Standard: `language None`, `theme System`, `memory_mb 2048`, `java_override None`, `minimize_on_launch true`, `selected_instance None`, `dev_account false`. `Language`: `De`, `En` (serde `"de"`, `"en"`); `Theme`: `System`, `Light`, `Dark`.
  - `pub fn load_settings(paths: &Paths, now: u64) -> (Settings, Option<SettingsNotice>)` (fehlend: Standard; kaputt oder `format` > 1: Datei nach `settings.json.broken-<now>` verschieben, Standard, `SettingsNotice::Reset`), `pub fn save_settings(paths: &Paths, s: &Settings) -> hengst_core::Result<()>` (Temp + Rename).
  - `SettingsPatch` (alle Felder `Option`, `java_override: Option<Option<String>>`), `SettingsDto` (wie `Settings` ohne `format`, Pfade als String, dazu `notice: Option<SettingsNotice>`).
  - Commands `settings_get() -> Result<SettingsDto, ErrorDto>`, `settings_update(patch: SettingsPatch) -> Result<SettingsDto, ErrorDto>` (prüft `memory_mb` in 512..=65536, sonst `ErrorCode::InvalidInput` mit `params.field = "memoryMb"`; sendet `DataChanged(Settings)`). `ErrorCode::InvalidInput` kommt hier dazu.
  - `AppState.settings: Mutex<Settings>`; `startup.rs` erzeugt das Fenster mit `theme` (`Light`/`Dark`, bei `System` keins) und Hintergrund aus `--bg` des aufgelösten Themes.
- Produces (Frontend):
  - `stores/app.ts`: `type Resource<T> = { status: "idle" | "loading" | "ready" | "error"; data?: T; error?: ErrorDto }`; Store `useApp` mit `view: View`, `settings: Resource<SettingsDto>`, `versions: Resource<GameVersionsDto>`, Aktionen `setView(v)`, `loadSettings()`, `updateSettings(p)`, `loadVersions(filter)`; `startEventLoop()` abonniert `subscribeEvents` und lädt bei `DataChanged` die betroffene Ressource neu.
  - `views.ts`: `type View = "instances" | "settings"` (4b ergänzt `"play"`, `"log"`); Startansicht in 4a `instances`.
  - `theme.ts`: `resolveTheme(setting: "system" | "light" | "dark", prefersDark: boolean): "light" | "dark"`, `applyTheme(resolved)` setzt/entfernt `.dark` an `<html>`; bei `system` Listener auf `matchMedia("(prefers-color-scheme: dark)")`.
  - `i18n/index.ts`: `initI18n(language: "de" | "en" | null, navigatorLanguage: string)`, `pickLanguage(setting, navigatorLanguage): "de" | "en"`; Namensräume flach mit Präfixen `nav.`, `settings.`, `about.`, `errors.`, `accounts.`, `instances.`, `common.`.
  - `SettingsView` in 4a: Sprache (RadioGroup System/Deutsch/English), Design (RadioGroup System/Hell/Dunkel), Datenordner (Text plus "Ordner öffnen" kommt in Task 10), AboutPanel aus Task 2, Hinweis "Not an official Minecraft product." in beiden Sprachen gleich (Rechtsformel, nicht übersetzt).

- [ ] **Step 1: Failing Tests schreiben**

Rust (`settings.rs`):
```rust
#[test] fn missing_settings_give_defaults() { /* memory_mb 2048, theme System, minimize_on_launch true, kein Notice */ }
#[test] fn settings_round_trip_through_the_file() { /* save, load -> gleich; Datei enthält "format": 1 und "memoryMb" */ }
#[test] fn broken_settings_are_moved_aside() {
    // "{" schreiben; load_settings(paths, 1700) -> Standard, Some(Reset); settings.json.broken-1700 existiert mit Inhalt "{"; settings.json fehlt danach
}
#[test] fn too_new_settings_are_moved_aside_not_overwritten() { /* format 2 -> wie oben */ }
#[test] fn memory_outside_range_is_invalid_input() { /* patch memory_mb 100 -> code InvalidInput, params["field"] == "memoryMb", Datei unverändert */ }
#[test] fn update_emits_settings_changed() { /* Hub-Sink sieht DataChanged { scope: Settings } */ }
```

Frontend:
```ts
// tokens.test.ts: liest src/styles/index.css, löst --text/--bg, --text-muted/--bg, --text-muted/--surface,
// --on-accent/--accent, --on-danger/--danger in :root und .dark auf
it.each(textPairs)("%s on %s has contrast >= 4.5 in both themes", ...)
it.each([["--border-strong", "--bg"], ["--focus", "--bg"], ["--accent", "--bg"]])("%s against %s >= 3", ...)
// i18n.test.ts
it("de and en have the same keys", ...)          // flache Schlüsselmengen gleich
it("no translation is empty", ...)
it("interpolation params match", ...)            // {{x}} je Schlüssel in de und en gleich
it("picks German for de-AT and English otherwise", () => { expect(pickLanguage(null, "de-AT")).toBe("de"); expect(pickLanguage(null, "fr-FR")).toBe("en"); expect(pickLanguage("en", "de-DE")).toBe("en"); });
// theme.test.ts
it("system follows prefersDark, explicit wins", () => { expect(resolveTheme("system", true)).toBe("dark"); expect(resolveTheme("light", true)).toBe("light"); });
```

- [ ] **Step 2: Tests laufen lassen, sie scheitern**

Run: `cargo test -p hengst-desktop settings` und `pnpm -C apps/desktop test`
Expected: FAIL

- [ ] **Step 3: Zustandsliste (produkt-regelwerk BAUEN Schritt 2)** für Shell und Einstellungen in den Task-Bericht: Hauptaktion je Ansicht, Laden/Leer/Fehler, Mutationen (Einstellung speichern: sofort beim Wählen, Fehler inline, Rückfall auf alten Wert), Extremwerte (längster deutscher Text "Einstellungen" in der 64-px-Leiste nur als Tooltip).

- [ ] **Step 4: Implementieren**

Tokens als CSS-Variablen in `:root` und `.dark`, Tailwind-v4-`@theme inline` bildet sie auf Utility-Namen ab, shadcn-Variablen (`--background`, `--primary` usw.) zeigen auf die Tokens, Dark-Variante per `@custom-variant dark`. Shell nach DESIGN.md (Seitenleiste mit Zeichen oben, Navigation als `<nav aria-label>` mit Buttons und `aria-current="page"`, Kopfzeile mit Seitentitel als `<h1>`, rechts Platz für den Konto-Chip ab Task 8). `brand/README.md` (Englisch): Zeichen ersetzen = `brand/mark.svg` tauschen, `pnpm tauri icon brand/mark.svg` ausführen, nichts sonst; das Frontend importiert das Zeichen nur aus `brand/mark.svg`. `main.tsx` lädt zuerst `settings_get`, dann i18n und Theme, dann rendert es (Ladefläche in `--bg`, kein Text-Aufblitzen). Notice `Reset` zeigt einmal einen Sonner-Toast "Einstellungen waren beschädigt und wurden zurückgesetzt".

- [ ] **Step 5: Tests grün**

Run: `cargo test --workspace --all-features` und `pnpm -C apps/desktop typecheck && pnpm -C apps/desktop test`
Expected: PASS

- [ ] **Step 6: Sichtprüfung**

Run: `pnpm -C apps/desktop tauri build --debug --no-bundle`, Start mit `--home %TEMP%\hengst-4a-t3 --window-size 960x600`, dann `--window-size 1920x1080`.
Expected (agentfenster `open_app`, `read_desktop`, `press_key tab`): Leiste bei 960 px als Icon-Leiste mit Namen im Elementbaum, bei 1920 px mit Text; Wechsel Hell/Dunkel und Deutsch/Englisch in den Einstellungen wirkt sofort und überlebt einen Neustart; Fokusring an jedem Tab-Schritt sichtbar, Reihenfolge Leiste → Kopfzeile → Inhalt. Kaputte `settings.json` vorab in den Home-Ordner legen: Toast erscheint einmal. Vier Screenshots (hell/dunkel × de/en) je Größe nach `evidence/task-3/`. Gegenprüfung C1 bis C27 (C8, C12, C14, C16, C18 sind hier relevant) mit Ergebnis im Bericht.

- [ ] **Step 7: Lint, Format, Commit**

```bash
git add apps/desktop/src apps/desktop/brand apps/desktop/src-tauri/src apps/desktop/src-tauri/icons
git commit -m "feat(desktop): app shell, settings file, theme and German/English UI"
```

---

### Task 4: Typisierte Fehler in Kern und App

**Files:**
- Modify: `crates/hengst-core/src/error.rs`, `crates/hengst-core/src/loaders/mod.rs` (`no_versions`, `LoaderHint`), `crates/hengst-core/src/loaders/forge/mod.rs` (`lock_file`), `apps/cli/src/main.rs` (nur falls Fehlertexte sich ändern: sie ändern sich nicht)
- Modify: `apps/desktop/src-tauri/src/{error.rs,dto/error.rs}`
- Create: `apps/desktop/src/features/errors/{ErrorPanel.tsx,errorCopy.ts,errorCopy.test.ts,ErrorPanel.test.tsx}`
- Modify: `apps/desktop/src/i18n/{de.json,en.json}`

**Interfaces:**
- Produces (Kern):
  - `Error::LockTimeout { label: String, waited_secs: u64, lock: PathBuf }` mit exakt dem bisherigen Text `"another install of {label} is still running after {waited_secs} s (lock {lock}); try again later"` (statt `Error::Io(TimedOut)`); `lock` ist `Display` über `.display()`.
  - `Error::Cancelled` mit Text `"the operation was cancelled"`.
  - `pub enum LoaderHint { NeoForgeStartsAt1201, FabricStartsAt114, LegacyFabricServesOld }` mit `Display` = bisherige Texte (`"NeoForge starts at 1.20.1"`, `"Fabric starts at 1.14; try --loader legacy-fabric"`, `"Legacy Fabric serves 1.13.2 and older; try --loader fabric"`); `Error::NoLoaderVersions.hint: Option<LoaderHint>`. Die Fehlermeldung bleibt zeichengleich.
- Produces (Desktop):
  - `ErrorCode` (serde `camelCase`, ts-rs Union): `network`, `offline`, `serverError`, `checksumMismatch`, `unsafeData`, `unknownVersion`, `noJavaRuntime`, `offlineAccountBlocked`, `noLoaderVersions`, `unknownLoaderVersion`, `loaderUnsupported`, `installerInvalid`, `installFailed`, `installStepFailed`, `nativesInUse`, `lockTimeout`, `credentialStore`, `missingClientId`, `appMisconfigured`, `appNotApproved`, `loginDeclined`, `loginExpired`, `deviceCodeExpired`, `noXboxAccount`, `xboxRegionBlocked`, `xboxAdultVerification`, `xboxChildAccount`, `xboxRefused`, `gameNotOwned`, `noProfile`, `accountNotFound`, `noAccount`, `cancelled`, `invalidInput`, `fileSystem`, `internal`. Task 5 ergänzt `browserLoginTimedOut`, `loopbackUnavailable`, `browserLoginFailed`; Task 9 `instanceNotFound`, `instanceExists`, `instanceFormatTooNew`, `invalidInstanceName`; Task 10 `busy`.
  - `ErrorAction` (`#[serde(tag = "kind", rename_all = "camelCase")]`): `Retry`, `SignIn`, `OpenLog { path: String }`, `OpenUrl { target: UrlTarget }`, `OpenSettings`. `UrlTarget` (Task 10 nutzt sie zum Öffnen): `Repository`, `SignInHelp`, `XboxSignup`, `MinecraftProfile`.
  - `pub fn error_dto(e: &hengst_core::Error) -> ErrorDto`, erschöpfend über `Error` und `AuthError` ohne `_`-Zweig; `detail` = `e.to_string()`.
  - Abbildung (Code, Aktion, Params): `Http` → network, Retry, `url`; `Status` → serverError, Retry, `status`; `Offline` → offline, Retry; `Checksum` → checksumMismatch, Retry; `UnsafePath`, `UnknownPlaceholder`, `ParentMismatch` → unsafeData; `UnknownVersion` → unknownVersion, `version`; `NoRuntime` → noJavaRuntime, OpenSettings; `OfflineLaunchDisabled` → offlineAccountBlocked; `NoLoaderVersions` → noLoaderVersions, `loader`, `game`, `hint` (Variantenname in camelCase, z. B. `fabricStartsAt114`); `UnknownLoaderVersion` → unknownLoaderVersion, `loader`, `game`, `version`; `LoaderUnsupported` → loaderUnsupported, `loader`, `game`, `reason`; `TooLarge`, `InvalidChecksumFile`, `NoChecksum` → serverError; `Installer`, `Zip` → installerInvalid; `InstallFailed` → installFailed, Retry; `Processor` → installStepFailed, OpenLog(log), `step`, `code`; `NativesInUse` → nativesInUse; `LockTimeout` → lockTimeout, Retry; `CredentialStore` → credentialStore; `Cancelled` → cancelled; `Io` → fileSystem; `Json` → internal; Auth: `MissingClientId` → missingClientId; `DeviceCodeExpired` → deviceCodeExpired, SignIn; `LoginDeclined` → loginDeclined, SignIn; `AppMisconfigured` → appMisconfigured; `LoginExpired` → loginExpired, SignIn, `name`; `NoXboxAccount` → noXboxAccount, OpenUrl(XboxSignup); `XboxRegionBlocked`, `XboxAdultVerification`, `XboxChildAccount` je eigener Code; `XboxRefused` → xboxRefused, `code`; `AppNotApproved` → appNotApproved, OpenUrl(SignInHelp); `GameNotOwned` → gameNotOwned; `NoProfile` → noProfile, OpenUrl(MinecraftProfile); `AccountNotFound` → accountNotFound; `NoAccount` → noAccount, SignIn; `BadResponse` → serverError, Retry.
- Produces (Frontend): `errorCopy.ts`: `export const errorKeys: Record<ErrorCode, { title: string; body: string }>` (i18n-Schlüssel; `Record` macht fehlende Codes zum Typfehler), `describeError(dto: ErrorDto, t: TFunction): { title: string; body: string; actionLabel?: string }`; `ErrorPanel` Props `{ error: ErrorDto; onRetry?: () => void; onAction?: (a: ErrorAction) => void }` mit "Details" (aufklappbar, `detail` in Mono, kopierbar) und `role="alert"`.

- [ ] **Step 1: Failing Tests schreiben**

Kern:
```rust
// loaders/forge/mod.rs, bestehender Lock-Test angepasst:
assert!(matches!(&err, Error::LockTimeout { waited_secs, .. } if *waited_secs == timing.limit.as_secs()));
assert!(err.to_string().contains("another install of forge for Minecraft 1.20.1 is still running after"));
// loaders/mod.rs
#[test] fn hints_are_typed_and_keep_their_text() {
    let e = no_versions(LoaderKind::Fabric, "1.8.9");
    assert!(matches!(e, Error::NoLoaderVersions { hint: Some(LoaderHint::FabricStartsAt114), .. }));
    assert_eq!(e.to_string(), "no fabric versions available for Minecraft 1.8.9 (Fabric starts at 1.14; try --loader legacy-fabric)");
}
```

Desktop (`error.rs`):
```rust
#[test] fn every_error_maps_to_a_code() {
    // Tabelle: je Variante ein Beispielwert -> erwarteter ErrorCode und ErrorAction
    // z. B. Error::Processor { jar: "binarypatcher".into(), code: 1, log: "C:/h/logs/x.log".into() }
    //   -> code InstallStepFailed, action OpenLog { path: "C:/h/logs/x.log" }, params["step"] == "binarypatcher", params["code"] == "1"
    // Error::Auth(AppNotApproved) -> AppNotApproved, OpenUrl { target: SignInHelp }
    // Error::NoLoaderVersions { hint: Some(FabricStartsAt114), .. } -> params["hint"] == "fabricStartsAt114"
}
#[test] fn error_detail_never_contains_secret() {
    // alle Beispielwerte der Tabelle aus every_error_maps_to_a_code serialisieren;
    // kein JSON enthält "Secret(", "<redacted>" oder "accessToken"; params enthalten nur die je Variante genannten Schlüssel
}
#[test] fn serialized_shape_is_stable() {
    // JSON von error_dto(&Error::Cancelled) == {"code":"cancelled","params":{},"action":null,"detail":"the operation was cancelled"}
}
```

Frontend:
```ts
it("every error code has German and English title and body", ...)   // über Object.keys(errorKeys) und beide Sprachen
it("noLoaderVersions with hint shows the hint sentence", ...)        // params.hint "fabricStartsAt114" -> Text enthält "1.14"
it("ErrorPanel shows retry only when onRetry is given and calls it once on double click", ...)
it("details are collapsed and contain the detail text", ...)
```

- [ ] **Step 2: Tests laufen lassen, sie scheitern**

Run: `cargo test -p hengst-core hints_are_typed` und `cargo test -p hengst-desktop error`, `pnpm -C apps/desktop test errorCopy`
Expected: FAIL

- [ ] **Step 3: Implementieren** wie in Interfaces; Texte der Fehler nach `frontend-design` ("was ist passiert, was tun", keine Entschuldigung). Beispiel `appNotApproved` siehe Task 8.

- [ ] **Step 4: Tests grün**

Run: `cargo test --workspace --all-features` und `pnpm -C apps/desktop typecheck && pnpm -C apps/desktop test`
Expected: PASS, CLI-Tests unverändert grün (Texte gleich).

- [ ] **Step 5: Lint, Format, Commit**

```bash
git add crates/hengst-core/src/error.rs crates/hengst-core/src/loaders/mod.rs crates/hengst-core/src/loaders/forge/mod.rs apps/desktop/src-tauri/src apps/desktop/src/features/errors apps/desktop/src/i18n apps/desktop/src/ipc/bindings
git commit -m "feat: typed lock timeout, cancel and loader hints; error DTO mapping for the desktop app"
```

---

### Task 5: Kern: Microsoft-Login per Browser (Auth Code + PKCE)

**Files:**
- Create: `crates/hengst-core/src/auth/browser.rs`
- Modify: `crates/hengst-core/src/auth/mod.rs` (`pub mod browser;`), `crates/hengst-core/src/auth/error.rs`, `crates/hengst-core/Cargo.toml` + `Cargo.toml` (`sha2`, `base64`, `getrandom` per `cargo add`, Versionen im Lockfile)
- Modify: `apps/desktop/src-tauri/src/error.rs` (neue Codes)

**Interfaces:**
- Consumes: `MicrosoftAuth { http, ep, client_id }`, `AuthEndpoints.msa` (`.../consumers/oauth2/v2.0`), `SCOPE`, `msa::MsaTokens`, `parse_tokens`, `error_code`, `map_client_error` (bestehende private Helfer in `msa.rs`, für `browser.rs` auf `pub(super)` heben).
- Produces:
  - `pub struct BrowserLoginPages { pub success_title: String, pub success_body: String, pub failure_title: String, pub failure_body: String }`
  - `pub struct BrowserLogin` (nicht `Clone`; `Debug` zeigt nur `redirect_uri`) mit `pub fn authorize_url(&self) -> &str` und `pub fn redirect_uri(&self) -> &str`.
  - `impl MicrosoftAuth { pub async fn start_browser_login(&self) -> Result<BrowserLogin>; pub async fn finish_browser_login(&self, login: BrowserLogin, pages: &BrowserLoginPages, timeout: Duration) -> Result<MsaTokens>; }`
  - `pub const BROWSER_LOGIN_TIMEOUT: Duration = Duration::from_secs(300);`
  - `AuthError::{BrowserLoginTimedOut, LoopbackUnavailable, BrowserLoginFailed { code: String }}` mit Texten `"the browser sign-in was not completed within 5 minutes"`, `"the launcher cannot listen on localhost for the sign-in answer"`, `"the browser sign-in failed ({code})"`.

Ablauf (festgelegt): `start_browser_login` bindet `127.0.0.1:0`, dann versucht es `[::1]:<derselbe Port>` (Fehler dort ignoriert); Bindefehler auf IPv4 → `LoopbackUnavailable`. Verifier = 32 Zufallsbytes (`getrandom`) Base64url ohne Padding (43 Zeichen), Challenge = Base64url(SHA-256(Verifier)), `state` = 16 Zufallsbytes Base64url. Authorize-URL `{msa}/authorize` mit `client_id`, `response_type=code`, `redirect_uri=http://localhost:<port>`, `response_mode=query`, `scope=XboxLive.signin offline_access`, `code_challenge`, `code_challenge_method=S256`, `state`, `prompt=select_account` (mit `reqwest::Url` gebaut, korrekt kodiert). `finish_browser_login` nimmt Verbindungen bis `timeout` an: je Verbindung höchstens 8 KiB bis `\r\n\r\n`, 5 s Lesefrist; nur `GET /?...` zählt, andere Pfade → 404 und weiter warten; falsches oder fehlendes `state` → 400 und weiter warten; `error=access_denied` → Seite mit `failure_*`, `LoginDeclined`; anderes `error=` → `failure_*`, `BrowserLoginFailed { code }` (nur der Code, nie `error_description`); `code` + richtiges `state` → Seite mit `success_*`, dann POST `{msa}/token` mit `grant_type=authorization_code`, `client_id`, `code`, `redirect_uri` (gleich wie oben), `code_verifier`, `scope`; Antwort wie beim Device Code ausgewertet (`map_client_error`). Seiten: `text/html; charset=utf-8`, `Cache-Control: no-store`, Titel und Text HTML-escaped, kein Script, kein externes CSS.

- [ ] **Step 1: Failing Tests schreiben** (`browser.rs`, wiremock für `/token`, Redirect per `reqwest::get` auf die Loopback-URI)

```rust
#[tokio::test] async fn authorize_url_carries_pkce_and_consumers_scope() {
    // URL parsen: response_type == "code", scope == "XboxLive.signin offline_access", code_challenge_method == "S256",
    // redirect_uri beginnt mit "http://localhost:", Host von authorize == Mock-Host, Pfad endet auf "/authorize",
    // code_challenge == b64url(sha256(login.verifier_for_test())), state.len() >= 22, prompt == "select_account"
}
#[tokio::test] async fn redirect_with_code_exchanges_it_with_the_verifier() {
    // Mock /token erwartet body_string_contains "grant_type=authorization_code", "code=abc", "code_verifier=<v>", "redirect_uri=http%3A%2F%2Flocalhost%3A<port>"
    // GET <redirect>?code=abc&state=<state> -> 200, Body enthält success_title; Ergebnis: access_token.expose() == "at", refresh == "rt"
}
#[tokio::test] async fn wrong_state_is_ignored_and_login_keeps_waiting() { /* erst state=x -> 400, dann richtig -> Ok */ }
#[tokio::test] async fn other_paths_get_404() { /* GET /favicon.ico -> 404, danach richtiger Redirect -> Ok */ }
#[tokio::test] async fn access_denied_maps_to_login_declined() { /* ?error=access_denied&error_description=secret+text&state=<s> -> Body enthält failure_title, nicht "secret text"; Err(Auth(LoginDeclined)) */ }
#[tokio::test] async fn other_error_keeps_only_the_code() { /* ?error=server_error&state=<s> -> BrowserLoginFailed { code } mit code == "server_error" */ }
#[tokio::test] async fn times_out_without_redirect() { /* timeout 200 ms -> Err(Auth(BrowserLoginTimedOut)) */ }
#[tokio::test] async fn invalid_client_on_exchange_maps_to_app_misconfigured() { /* /token 400 {"error":"invalid_client"} -> AppMisconfigured */ }
#[tokio::test] async fn page_texts_are_escaped() { /* success_title "<b>&" -> Body enthält "&lt;b&gt;&amp;" */ }
#[tokio::test] async fn ipv6_localhost_reaches_the_listener_when_available() { /* GET http://[::1]:<port>/?code&state -> Ok; überspringt mit eprintln, falls ::1 nicht bindbar */ }
#[test] fn debug_never_shows_verifier_or_state() { /* format!("{login:?}") enthält weder Verifier noch state */ }
#[tokio::test] async fn oversized_request_is_dropped() { /* 9 KiB Header ohne Ende -> Verbindung geschlossen, danach richtiger Redirect -> Ok */ }
```
`verifier_for_test()` und `state_for_test()` sind `#[cfg(test)]`.

- [ ] **Step 2: Tests laufen lassen, sie scheitern**

Run: `cargo test -p hengst-core browser`
Expected: FAIL

- [ ] **Step 3: Implementieren** nach "Ablauf"; Desktop-`error_dto` um die drei Codes ergänzen (`browserLoginTimedOut` → SignIn, `loopbackUnavailable` → keine Aktion, `browserLoginFailed` → SignIn, `params.code`), Übersetzungen ergänzen.

- [ ] **Step 4: Tests grün**

Run: `cargo test --workspace --all-features`
Expected: PASS

- [ ] **Step 5: Lint, Format, Commit**

```bash
git add Cargo.toml Cargo.lock crates/hengst-core/Cargo.toml crates/hengst-core/src/auth apps/desktop/src-tauri/src/error.rs apps/desktop/src-tauri/src/dto/error.rs apps/desktop/src/ipc/bindings apps/desktop/src/i18n apps/desktop/src/features/errors/errorCopy.ts
git commit -m "feat(core): Microsoft sign-in through the system browser with PKCE and a loopback redirect"
```

---

### Task 6: Testkit-Crate mit Mock-Diensten

**Files:**
- Create: `crates/hengst-testkit/Cargo.toml` (`publish = false`, Abhängigkeiten `wiremock`, `serde_json`, `tokio`, `hengst-core`, `zip`, `reqwest`), `src/lib.rs`, `src/auth.rs`, `src/mojang.rs`, `src/bin/hengst-mock.rs`
- Modify: `Cargo.toml` (Member)

**Interfaces:**
- Consumes: `AuthEndpoints`, `launch::Endpoints`, `loaders::LoaderEndpoints`, `download::sha1_hex`.
- Produces:
  - `pub struct MockAccount { pub name: &'static str, pub uuid: &'static str, pub minecraft_token: &'static str, pub refresh_token: &'static str }` und `pub const PLAYER_A: MockAccount` (`"E2EPlayer"`, `"0123456789abcdef0123456789abcdef"`, `"mc-token-a"`, `"rt-a"`), `PLAYER_B` (`"SecondPlayer"`, `"fedcba9876543210fedcba9876543210"`, `"mc-token-b"`, `"rt-b"`).
  - `pub enum AuthScenario { Ok, NotApproved, NoXbox, Declined }`
  - `pub async fn mount_auth(server: &MockServer, scenario: AuthScenario, accounts: &[MockAccount])` mountet `/consumers/oauth2/v2.0/authorize` (302 auf `redirect_uri?code=code-<n>&state=<state>`, bei `Declined` `error=access_denied`), `/token` (je Code das passende Konto, nacheinander `accounts[0]`, `accounts[1]`, ...; Refresh-Grant liefert dasselbe Konto), `/devicecode` und `/token` mit Device-Grant (sofort Erfolg), Xbox `user/authenticate`, XSTS `xsts/authorize` (bei `NoXbox` `XErr 2148916233`), Minecraft `login_with_xbox` (bei `NotApproved` HTTP 403), `entitlements/mcstore`, `minecraft/profile`.
  - `pub fn auth_endpoints(base: &str) -> AuthEndpoints` (alle vier Basen unter `base`, `msa` = `{base}/consumers/oauth2/v2.0`).
  - `pub async fn mount_mojang(server: &MockServer) -> ()` mit Manifest (`9.9` release, `9.9-snap` snapshot, `b9.9` old_beta, `a9.9` old_alpha, alles mit gültigen SHA-1 wie in `crates/hengst-core/tests/launch_e2e.rs`) und Fabric-Meta (`/fabric/versions/loader/9.9`: `0.99.0` stabil, `0.100.0-beta.1` instabil; leer für `b9.9`).
  - `pub fn endpoints(base: &str) -> Endpoints` (Manifest, Ressourcen, Laufzeit-Index, Loader-Basen unter `base`).
  - Binary `hengst-mock`: startet einen `MockServer`, mountet `mount_mojang` und `mount_auth` (Szenario aus `HENGST_MOCK_SCENARIO`: `ok`, `not-approved`, `no-xbox`, `declined`; Konten A, B), schreibt eine Zeile `{"url":"http://127.0.0.1:<port>"}` auf stdout und läuft, bis stdin schließt.

Bestehende Tests in `crates/hengst-core/tests/` bleiben unverändert (keine Umstellung auf das Testkit in dieser Phase).

- [ ] **Step 1: Failing Tests schreiben** (`crates/hengst-testkit/src/lib.rs`, `#[cfg(test)]`)

```rust
#[tokio::test] async fn device_flow_against_mock_signs_in_player_a() {
    // MicrosoftAuth::new("e2e-client", auth_endpoints(&uri)); start_device_login, poll_device_login, complete_login
    // -> session.profile.name == "E2EPlayer"
}
#[tokio::test] async fn browser_flow_against_mock_signs_in_a_then_b() {
    // start_browser_login; reqwest mit Redirect-Folgen auf authorize_url(); finish_browser_login -> "E2EPlayer"; zweiter Durchlauf -> "SecondPlayer"
}
#[tokio::test] async fn not_approved_scenario_yields_app_not_approved() { /* -> Err(Auth(AppNotApproved)) */ }
#[tokio::test] async fn mojang_mock_lists_all_kinds_and_fabric_versions() {
    // fetch_manifest -> 4 Versionen; loaders::list_versions(Fabric, "9.9") -> ids ["0.100.0-beta.1", "0.99.0"] oder in der Reihenfolge, die der Fabric-Leser liefert; pick_version(None) == "0.99.0"
}
#[test] fn mock_binary_prints_its_url() {
    // std::process::Command auf env!("CARGO_BIN_EXE_hengst-mock"), stdin piped; erste stdout-Zeile parst als {"url": ...}; stdin schließen -> Prozess endet binnen 5 s
}
```

- [ ] **Step 2: Tests laufen lassen, sie scheitern**

Run: `cargo test -p hengst-testkit`
Expected: FAIL

- [ ] **Step 3: Implementieren** wie in Interfaces.

- [ ] **Step 4: Tests grün**

Run: `cargo test --workspace --all-features`
Expected: PASS

- [ ] **Step 5: Lint, Format, Commit**

```bash
git add Cargo.toml Cargo.lock crates/hengst-testkit
git commit -m "test: add hengst-testkit with mock Microsoft, Xbox, Mojang and Fabric services"
```

---

### Task 7: Konten im Backend (Liste, Wechsel, Abmelden, Login-Vorgänge)

**Files:**
- Create: `apps/desktop/src-tauri/src/accounts.rs`, `src/dto/accounts.rs`, `src/commands/accounts.rs`
- Modify: `crates/hengst-core/src/auth/store.rs` (`impl<S: SecretStore + ?Sized> SecretStore for Box<S>`), `apps/desktop/src-tauri/src/{state.rs,startup.rs,command_list.rs,dto/mod.rs}`, `apps/desktop/src-tauri/Cargo.toml` (dev-dependency `hengst-testkit`)

**Interfaces:**
- Consumes: `AccountStore`, `load_accounts_file`, `KeyringSecretStore`, `MemorySecretStore`, `MicrosoftAuth::{start_browser_login, finish_browser_login, start_device_login, poll_device_login, complete_login}`, `client_id`, `unix_now`, `BrowserLoginPages`, `BROWSER_LOGIN_TIMEOUT`, `error_dto`, `EventHub`, `Settings.dev_account`, Testkit (Task 6).
- Produces:
  - DTOs: `AccountKindDto` (`microsoft`, `offlineDev`); `AccountDto { id: String, name: String, kind: AccountKindDto, selected: bool, sign_in_required: bool }` (`id` = UUID bzw. `"offline-dev"`); `AccountsDto { accounts: Vec<AccountDto>, can_sign_in: bool }` (`can_sign_in` false, wenn keine Client-ID); `LoginMethod` (`browser`, `deviceCode`); `LoginPagesDto { success_title, success_body, failure_title, failure_body }`; `OperationId(u64)` (TS `number`); `LoginEvent` (`#[serde(tag = "event", content = "data", rename_all = "camelCase")]`): `BrowserOpened`, `DeviceCode { user_code: String, verification_uri: String, expires_at: u64 }`, `Completing`, `SignedIn { account: AccountDto }`, `Failed { error: ErrorDto }`, `Cancelled`.
  - `pub struct Accounts` in `accounts.rs` mit `fn new(paths: &Paths, secrets: Box<dyn SecretStore>, auth: Option<MicrosoftAuth>, opener: BrowserOpener) -> Accounts`; `fn list(&self, settings: &Settings) -> Result<AccountsDto>` (liest nur `accounts.json`, kein Schlüsselspeicher); `fn select(&self, id: &str, settings: &mut Settings) -> Result<()>` (`"offline-dev"` setzt `dev_account = true` nur mit `dev-offline`, sonst `AccountNotFound`; Microsoft-Konto: `set_default`, `dev_account = false`); `fn remove(&self, id: &str) -> Result<()>`; `async fn login(&self, method: LoginMethod, pages: BrowserLoginPages, sink: EventSink<LoginEvent>) -> Result<AccountDto>`; `fn mark_sign_in_required(&self, uuid: &str)` (für 4b, wenn `launch_account` `LoginExpired` meldet).
  - `pub type BrowserOpener = Arc<dyn Fn(&str) -> Result<()> + Send + Sync>`: in Release/Debug `opener.open_url(url)`, mit Feature `e2e` ein HTTP-Abruf der URL mit Redirect-Folgen in einem eigenen Task.
  - `pub struct Operations` mit `fn start(&self, kind: OpKind, handle: AbortHandle, reopen: Option<String>) -> OperationId` (ein laufender Vorgang derselben Art wird abgebrochen und meldet `Cancelled`), `fn cancel(&self, id: OperationId) -> bool`, `fn reopen_url(&self, id: OperationId) -> Option<String>` (nur intern; verlässt Rust nie).
  - Commands: `accounts_list() -> Result<AccountsDto, ErrorDto>`, `account_select(id: String)`, `account_remove(id: String)`, `login_start(method: LoginMethod, pages: LoginPagesDto, channel: Channel<LoginEvent>) -> Result<OperationId, ErrorDto>` (kehrt sofort zurück, Arbeit im Task), `login_cancel(op: OperationId) -> Result<bool, ErrorDto>`, `login_reopen_browser(op: OperationId) -> Result<(), ErrorDto>`. Erfolgreiche Mutation sendet `DataChanged(Accounts)`.
  - Neues Konto wird gewählt (`set_default`), `dev_account` wird dabei `false`.
  - Mit `e2e`: `MemorySecretStore`, Client-ID `"e2e-client"`, `auth_endpoints(HENGST_E2E_BASE_URL)`. Ohne: `KeyringSecretStore::new()`; schlägt das fehl, bleibt die Liste nutzbar und `login_start` meldet `credentialStore`.

- [ ] **Step 1: Failing Tests schreiben** (`accounts.rs`, mit Testkit-Mock und `MemorySecretStore`)

```rust
#[tokio::test] async fn browser_login_saves_and_selects_the_account() {
    // Events in Reihenfolge: BrowserOpened, Completing, SignedIn { account.name == "E2EPlayer", selected == true }
    // list(): 1 Konto, selected
}
#[tokio::test] async fn device_login_reports_the_code_then_signs_in() { /* erstes Event DeviceCode mit user_code nicht leer */ }
#[tokio::test] async fn second_account_becomes_selected_and_switch_works() {
    // zwei Logins (A, B) -> B selected; select(A.id) -> A selected; accounts.json default == A.uuid
}
#[tokio::test] async fn remove_deletes_tokens_and_moves_selection() { /* remove(B) -> Liste [A], A selected, Secret für B fehlt */ }
#[tokio::test] async fn login_events_never_contain_tokens() {
    // alle Events eines Browser- und eines Device-Logins serialisieren;
    // kein String enthält "mc-token-a", "rt-a", "code-", "state=", "code_verifier"
}
#[tokio::test] async fn not_approved_login_maps_to_app_not_approved() { /* Szenario NotApproved -> Failed { error.code == AppNotApproved } und Ergebnis Err */ }
#[tokio::test] async fn second_login_cancels_the_first() {
    // Login 1 per Browser starten ohne Redirect (Opener tut nichts), Login 2 starten -> Login 1 meldet Cancelled, Login 2 läuft
}
#[tokio::test] async fn cancel_ends_a_waiting_login() { /* cancel(op) == true; Event Cancelled; kein Konto gespeichert */ }
#[test] fn accounts_list_works_without_secret_store() { /* Secret-Store, dessen get/set immer Err(CredentialStore) liefert; accounts.json mit A -> list() Ok */ }
#[cfg(feature = "dev-offline")] #[test] fn dev_build_lists_and_selects_the_offline_account() { /* id "offline-dev" in der Liste; select -> dev_account true, selected */ }
#[cfg(not(feature = "dev-offline"))] #[test] fn release_build_never_lists_dev_account() { /* dev_account = true in Settings -> kein "offline-dev"; select("offline-dev") -> AccountNotFound */ }
```

In `store.rs`:
```rust
#[test] fn boxed_secret_store_delegates() { /* Box<dyn SecretStore> aus MemorySecretStore; AccountStore damit; save_session + secret() funktionieren */ }
```

- [ ] **Step 2: Tests laufen lassen, sie scheitern**

Run: `cargo test -p hengst-desktop accounts` und `cargo test -p hengst-core boxed_secret_store`
Expected: FAIL

- [ ] **Step 3: Implementieren** wie in Interfaces. Login-Task: `start_browser_login` → Opener → `BrowserOpened` → `finish_browser_login(..., BROWSER_LOGIN_TIMEOUT)` → `Completing` → `complete_login` → `save_session` → `set_default` → `SignedIn`; Fehler → `Failed { error_dto }`. Die Seite im Browser bekommt die Texte aus `LoginPagesDto`.

- [ ] **Step 4: Tests grün**

Run: `cargo test --workspace --all-features` und `cargo test -p hengst-desktop` (ohne Features, prüft den Release-Zweig)
Expected: PASS

- [ ] **Step 5: Lint, Format, Commit**

```bash
git add crates/hengst-core/src/auth/store.rs apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/src apps/desktop/src/ipc/bindings Cargo.lock
git commit -m "feat(desktop): accounts backend with browser and device code sign-in"
```

---

### Task 8: Konten in der Oberfläche

**Files:**
- Create: `apps/desktop/src/features/accounts/{AccountChip.tsx,AccountMenu.tsx,SignInDialog.tsx,RemoveAccountDialog.tsx,signIn.ts,signIn.test.ts,AccountChip.test.tsx,SignInDialog.test.tsx}`
- Modify: `apps/desktop/src/ipc/{client.ts,events.ts}`, `src/stores/app.ts` (`accounts: Resource<AccountsDto>`), `src/app/Shell.tsx` (Chip in der Kopfzeile), `src/i18n/{de.json,en.json}`; shadcn-Komponenten `dialog`, `alert-dialog`, `dropdown-menu`, `badge`, `sonner` per `pnpm dlx shadcn@4.23.0 add`

**Interfaces:**
- Consumes: Commands aus Task 7, `ErrorPanel` (Task 4), Store (Task 3).
- Produces:
  - `api.accountsList()`, `api.accountSelect(id)`, `api.accountRemove(id)`, `api.loginStart(method, pages, onEvent): Promise<number>`, `api.loginCancel(op)`, `api.loginReopenBrowser(op)`.
  - `signIn.ts`: Reducer `signInReducer(state: SignInState, e: LoginEvent | { event: "start"; method: LoginMethod } | { event: "switchToCode" }): SignInState` mit `SignInState = { phase: "idle" } | { phase: "waitingBrowser"; op: number } | { phase: "showingCode"; op: number; userCode: string; verificationUri: string; expiresAt: number } | { phase: "completing" } | { phase: "done"; account: AccountDto } | { phase: "failed"; error: ErrorDto } | { phase: "cancelled" }`.
  - `AccountChip`: Knopf mit Name des gewählten Kontos (oder "Anmelden"), öffnet `AccountMenu` (DropdownMenu): Konten mit Häkchen am gewählten, Klick wählt; "Konto hinzufügen"; je Konto "Abmelden" (öffnet `RemoveAccountDialog`); `sign_in_required` zeigt Badge "Erneut anmelden" und Klick startet Login.
  - `SignInDialog`: Hauptaktion "Mit Microsoft anmelden" (Browser); während des Wartens Text "Melde dich im Browser an. Danach geht es hier automatisch weiter.", Nebenaktionen "Browser erneut öffnen", "Code stattdessen verwenden" (bricht Browser-Vorgang ab, startet Device Code), "Abbrechen"; Code-Ansicht: Code groß in Mono, "Code kopieren" (Clipboard per `navigator.clipboard.writeText`), "microsoft.com/link öffnen" (über `open_url` mit `UrlTarget::DeviceLogin`, siehe Task 10; bis dahin Text mit Adresse), Restzeit; Fehler über `ErrorPanel`; Erfolg schließt den Dialog, Toast "Angemeldet als {{name}}".
  - Sonderfall `appNotApproved` (eigener Block im Dialog statt generischem Fehler). Englisch: Titel "Sign-in is not available yet", Text "Mojang has not approved Hengst Client's Microsoft sign-in yet. Until then, accounts cannot be added. Your account is fine, nothing needs to change on your side.", Aktion "Learn more" (`SignInHelp`). Deutsch: "Anmeldung noch nicht verfügbar", "Mojang hat die Microsoft-Anmeldung von Hengst Client noch nicht freigegeben. Bis dahin lassen sich keine Konten hinzufügen. Mit deinem Konto ist alles in Ordnung, du musst nichts ändern.", "Mehr erfahren". In Builds mit `devOffline` zusätzlich "Development builds can use the offline development account." / "Entwicklungs-Builds können das Offline-Entwicklungskonto verwenden."
  - `RemoveAccountDialog`: Titel "{{name}} abmelden?", Text "Die gespeicherte Anmeldung wird von diesem PC entfernt. Dein Microsoft-Konto bleibt unverändert.", Knöpfe "Abmelden" (destructive) und "Behalten".
  - Leerzustand: Chip "Anmelden"; ohne Client-ID (`can_sign_in` false) ist der Knopf deaktiviert mit sichtbarer Erklärung im Menü.

- [ ] **Step 1: Zustandsliste** (produkt-regelwerk) für Chip, Menü, beide Dialoge in den Bericht: Hauptaktion je Zustand, alle sechs Phasen, Doppelklick auf "Mit Microsoft anmelden", Abbruch mit Escape (bricht Vorgang ab, Fokus zurück auf Chip), längster Name 16 Zeichen (Minecraft-Namen) plus Badge.

- [ ] **Step 2: Failing Tests schreiben**

```ts
// signIn.test.ts
it("browserOpened -> waitingBrowser with op", ...);
it("deviceCode event -> showingCode with code and expiry", ...);
it("failed keeps the error and allows restart", ...);
it("a late event after cancelled is ignored", ...);    // signedIn nach cancelled ändert nichts
// SignInDialog.test.tsx (mockIPC mit Channel-Simulation: Handler ruft args.channel.onmessage o. ä. nach Tauri-Mock-Doku)
it("double click on sign in starts one login", ...);    // login_start genau einmal
it("appNotApproved shows the explanation, not the generic error", ...);
it("escape cancels the running login", ...);            // login_cancel mit der op aufgerufen
// AccountChip.test.tsx
it("shows the selected account name and switches on click", ...);   // account_select mit id von B
it("remove asks first and names the account", ...);                  // Dialogtitel enthält Namen, account_remove erst nach Bestätigung
```

- [ ] **Step 3: Tests laufen lassen, sie scheitern**

Run: `pnpm -C apps/desktop test accounts`
Expected: FAIL

- [ ] **Step 4: Implementieren** nach DESIGN.md und Interfaces; Gegenprüfung C1 bis C27, besonders C4 (Sperre), C11 (eine Hauptaktion), C12/C3 (Icon-Knöpfe mit Namen), C17 (Dialogtexte konkret).

- [ ] **Step 5: Tests grün**

Run: `pnpm -C apps/desktop typecheck && pnpm -C apps/desktop test && cargo test --workspace --all-features`
Expected: PASS

- [ ] **Step 6: Sichtprüfung gegen den Mock**

Run: `cargo run -p hengst-testkit --bin hengst-mock` (URL notieren), `pnpm -C apps/desktop tauri build --debug --no-bundle --features e2e,dev-offline`, App per agentfenster `open_app` mit `--home %TEMP%\hengst-4a-t8 --e2e-base-url <url> --window-size 960x600` starten.
Expected: Leerzustand, Anmelden (Browser-Weg läuft im e2e-Build ohne Browser durch), zweites Konto, Wechsel, Abmelden mit Dialog; Mock mit `HENGST_MOCK_SCENARIO=not-approved`: Erklärung erscheint. Dev-Konto in der Liste sichtbar. Hell/dunkel, de/en, 960 × 600 und 1920 × 1080, Tastatur-Durchlauf durch Menü und Dialog (Escape, Fokus zurück). Screenshots nach `evidence/task-8/`.

- [ ] **Step 7: Lint, Format, Commit**

```bash
git add apps/desktop/src apps/desktop/components.json apps/desktop/package.json apps/desktop/pnpm-lock.yaml
git commit -m "feat(desktop): account chip, sign-in dialog with browser and code flow, sign out"
```

---

### Task 9: Kern: Instanzen-Modul

**Files:**
- Create: `crates/hengst-core/src/instances.rs`
- Modify: `crates/hengst-core/src/lib.rs` (`pub mod instances;`), `crates/hengst-core/src/error.rs`, `crates/hengst-core/src/loaders/mod.rs` (`LoaderKind`: serde über Slug, `display_name`), `apps/cli/src/main.rs` (`loader_name` ruft `LoaderKind::display_name`), `apps/desktop/src-tauri/src/error.rs` (neue Codes)

**Interfaces:**
- Consumes: `Paths::instance`, `validate_id`, `LoaderKind`, `ResolvedLoader`, `LaunchOptions`, `LoaderRequest`, `args::Account`.
- Produces:
  - `LoaderKind`: `Serialize`/`Deserialize` als `slug()` (`"fabric"`, `"quilt"`, `"legacy-fabric"`, `"neoforge"`, `"forge"`), `pub fn display_name(self) -> &'static str` (`"Fabric"`, `"Quilt"`, `"Legacy Fabric"`, `"NeoForge"`, `"Forge"`).
  - `Error::{InstanceNotFound { id: String }, InstanceExists { id: String }, InstanceFormatTooNew { id: String, format: u32 }, InvalidInstanceName(String)}` mit Texten `"instance {id} does not exist"`, `"instance folder {id} already exists"`, `"instance {id} was saved by a newer Hengst Client (format {format})"`, `"invalid instance name: {0}"`.
  - `pub const INSTANCE_FILE: &str = "instance.json"; pub const INSTANCE_FORMAT: u32 = 1;`
  - `#[serde(rename_all = "camelCase")] pub struct InstanceConfig { pub format: u32, pub name: String, pub game: String, pub loader: Option<InstanceLoader>, #[serde(default)] pub memory_mb: Option<u32>, #[serde(default)] pub java: Option<PathBuf>, pub created_at: u64, #[serde(default)] pub last_played: Option<u64>, #[serde(flatten)] pub extra: serde_json::Map<String, serde_json::Value> }`
  - `pub struct InstanceLoader { pub kind: LoaderKind, pub version: Option<String> }`
  - `pub struct Instance { pub id: String, pub dir: PathBuf, pub config: InstanceConfig }`
  - `pub enum Listed { Ready(Instance), TooNew { id: String, format: u32 }, Broken { id: String, reason: String } }`
  - `pub struct NewInstance { pub name: String, pub game: String, pub loader: Option<InstanceLoader> }`
  - `pub fn default_id(game: &str, loader: Option<LoaderKind>) -> String` (gleich wie `prepare`: `<game>` bzw. `<game>-<slug>`)
  - `pub fn parse_default_id(id: &str, is_game: impl Fn(&str) -> bool) -> Option<(String, Option<LoaderKind>)>` (erst ganze Id als Spielversion; sonst längstes passendes Loader-Suffix in der Reihenfolge `-legacy-fabric`, `-neoforge`, `-fabric`, `-quilt`, `-forge`, Rest muss Spielversion sein)
  - `pub fn validate_name(name: &str) -> Result<String>` (getrimmt, 1 bis 64 Zeichen, keine Steuerzeichen)
  - `pub fn list(paths: &Paths) -> Result<Vec<Listed>>` (nur Unterordner mit `instance.json`, sortiert: `last_played` absteigend, dann `name` ohne Groß-/Kleinschreibung; fehlender `instances/`-Ordner = leer)
  - `pub fn load(paths: &Paths, id: &str) -> Result<Instance>`
  - `pub fn create(paths: &Paths, new: &NewInstance, now: u64) -> Result<Instance>` (`game` darf nicht `latest`/`latest-release`/`latest-snapshot` sein → `Error::UnknownVersion(game)`; ob `game` im Manifest steht, prüft der Aufrufer; Id = `default_id`, bei belegtem Ordner `-2`, `-3`, ... bis 999; legt Ordner und Datei an; Ordner, die schon existieren, werden nie benutzt)
  - `pub fn save(paths: &Paths, inst: &Instance) -> Result<()>` (Temp + Rename; liest vorher das vorhandene `format` und verweigert mit `InstanceFormatTooNew`, wenn es größer als `INSTANCE_FORMAT` ist)
  - `pub fn adopt(paths: &Paths, is_game: impl Fn(&str) -> bool, now: u64) -> Result<Vec<String>>` (für jeden Ordner ohne `instance.json`, dessen Name `parse_default_id` erkennt: Datei mit Name `"<game> <display_name>"` bzw. `"<game>"`, `loader.version: None`, `created_at: now`; gibt die übernommenen Ids zurück; andere Ordner bleiben unberührt)
  - `pub fn launch_options(inst: &Instance, account: Account, default_memory_mb: u32, default_java: Option<PathBuf>) -> LaunchOptions` (`version` = `game`, `instance` = `Some(id)`, `memory_mb` = Instanz oder Standard, `java` = Instanz oder Standard, `loader` aus `InstanceLoader`)
  - `pub fn record_launch(paths: &Paths, id: &str, now: u64, resolved: Option<&ResolvedLoader>) -> Result<()>` (setzt `last_played`; ist `loader.version` `None` und `resolved` gegeben, wird `resolved.version.id` eingetragen; für 4b)
- Migrationsregel (als Doc-Kommentar über `INSTANCE_FORMAT`): Lesen erfolgt über `serde_json::Value`; `format` fehlt → `Broken`; `format` > `INSTANCE_FORMAT` → `TooNew`, Datei wird nie geschrieben; jede künftige Erhöhung bringt eine Funktion `v<n>_to_v<n+1>(Value) -> Value`, eine Fixture-Datei je Altformat und einen Test; die Datei wird erst beim nächsten `save` im neuen Format geschrieben; unbekannte Felder bleiben über `extra` erhalten.

- [ ] **Step 1: Failing Tests schreiben** (`instances.rs`, `tempfile`)

```rust
#[test] fn default_id_matches_prepare() { assert_eq!(default_id("1.20.1", Some(LoaderKind::Forge)), "1.20.1-forge"); assert_eq!(default_id("1.21.11", None), "1.21.11"); }
#[test] fn parses_default_ids_including_dashed_snapshots() {
    let known = |g: &str| ["1.20.1", "1.8.9", "26.4-snapshot-6", "1.21.11", "1.14 Pre-Release 1"].contains(&g);
    assert_eq!(parse_default_id("1.20.1-forge", known), Some(("1.20.1".into(), Some(LoaderKind::Forge))));
    assert_eq!(parse_default_id("1.8.9-legacy-fabric", known), Some(("1.8.9".into(), Some(LoaderKind::LegacyFabric))));
    assert_eq!(parse_default_id("26.4-snapshot-6-fabric", known), Some(("26.4-snapshot-6".into(), Some(LoaderKind::Fabric))));
    assert_eq!(parse_default_id("1.14 Pre-Release 1", known), Some(("1.14 Pre-Release 1".into(), None)));
    assert_eq!(parse_default_id("my-pack", known), None);
}
#[test] fn create_writes_format_1_and_uses_the_cli_folder_name() {
    // create("1.20.1 Forge", "1.20.1", forge 47.4.28) -> id "1.20.1-forge"; instance.json hat "format": 1, "game": "1.20.1", "loader": {"kind": "forge", "version": "47.4.28"}
}
#[test] fn create_never_reuses_an_existing_folder() {
    // instances/1.20.1-forge/saves/world/level.dat vorab anlegen (ohne instance.json)
    // create(...) -> id "1.20.1-forge-2"; level.dat unverändert; instances/1.20.1-forge/instance.json existiert nicht
}
#[test] fn adopts_cli_folders_by_default_name() {
    // Ordner "1.20.1-forge" und "random" ohne Datei; adopt(known) -> ["1.20.1-forge"]; load("1.20.1-forge").config.name == "1.20.1 Forge", loader.version == None; "random" ohne instance.json
}
#[test] fn too_new_instance_is_listed_and_never_written() {
    // instance.json {"format": 2, ...}; list -> TooNew { format: 2 }; save(...) -> Err(InstanceFormatTooNew); Datei byte-gleich
}
#[test] fn broken_instance_is_listed_not_deleted() { /* "{" -> Broken; Ordner und Datei bleiben */ }
#[test] fn unknown_fields_survive_a_save() { /* "notes": "x" bleibt nach load + save erhalten */ }
#[test] fn list_sorts_by_last_played_then_name() { /* a (nie), b (100), c (200), A2 (nie) -> c, b, a, A2 in dieser Logik: nie gespielte nach Namen */ }
#[test] fn names_are_validated() { /* "" und "   " und 65 Zeichen und "a\nb" -> InvalidInstanceName; "  Welt  " -> "Welt"; "Überleben 1.21" ok */ }
#[test] fn latest_is_not_a_game_version_for_instances() { /* create mit game "latest" -> Err(UnknownVersion) */ }
#[test] fn launch_options_use_instance_values_before_defaults() { /* memory 4096 aus Instanz, sonst 2048; instance == Some(id); loader request kind/version */ }
#[test] fn record_launch_pins_the_resolved_loader_once() { /* version None + resolved 0.17.3 -> "0.17.3", last_played gesetzt; zweiter Aufruf mit 0.18.0 ändert version nicht */ }
#[test] fn loader_kind_serializes_as_slug() { /* serde_json::to_string(&LoaderKind::NeoForge) == "\"neoforge\""; LegacyFabric == "\"legacy-fabric\""; Rundreise */ }
```

- [ ] **Step 2: Tests laufen lassen, sie scheitern**

Run: `cargo test -p hengst-core instances`
Expected: FAIL

- [ ] **Step 3: Implementieren** wie in Interfaces; Desktop-`error_dto` ergänzt `instanceNotFound` (Retry), `instanceExists`, `instanceFormatTooNew` (`params.format`), `invalidInstanceName`, Übersetzungen ergänzen.

- [ ] **Step 4: Tests grün**

Run: `cargo test --workspace --all-features`
Expected: PASS, CLI-Ausgaben unverändert (`loader_name` liefert dieselben Texte).

- [ ] **Step 5: Lint, Format, Commit**

```bash
git add crates/hengst-core/src/instances.rs crates/hengst-core/src/lib.rs crates/hengst-core/src/error.rs crates/hengst-core/src/loaders/mod.rs apps/cli/src/main.rs apps/desktop/src-tauri/src/error.rs apps/desktop/src-tauri/src/dto/error.rs apps/desktop/src/ipc/bindings apps/desktop/src/i18n apps/desktop/src/features/errors/errorCopy.ts
git commit -m "feat(core): instances module with instance.json format 1 and CLI folder adoption"
```

---

### Task 10: Instanzen im Backend, Ordner und Links öffnen

**Files:**
- Create: `apps/desktop/src-tauri/src/{dto/instances.rs,commands/instances.rs,commands/open.rs,open.rs}`
- Modify: `apps/desktop/src-tauri/src/{state.rs,command_list.rs,dto/mod.rs,dto/versions.rs,commands/versions.rs,error.rs}`, `apps/desktop/src-tauri/Cargo.toml` (`trash` per `cargo add`)

**Interfaces:**
- Consumes: `instances::*` (Task 9), `loaders::{list_versions, pick_version, LoaderKind}`, `download::http_client`, `VersionsCache` (Task 2), `Settings.selected_instance`, `UrlTarget` (Task 4).
- Produces:
  - DTOs: `LoaderKindDto` (`fabric`, `quilt`, `legacyFabric`, `neoForge`, `forge`; Abbildung auf `LoaderKind` in beide Richtungen), `LoaderVersionDto { id: String, stable: bool }`, `LoaderVersionsDto { versions: Vec<LoaderVersionDto>, recommended: String }`, `InstanceLoaderDto { kind: LoaderKindDto, version: Option<String> }`, `InstanceStatus` (`ready`, `tooNew`, `broken`), `InstanceDto { id: String, name: String, game: String, loader: Option<InstanceLoaderDto>, memory_mb: Option<u32>, last_played: Option<u64>, status: InstanceStatus, selected: bool }` (bei `tooNew`/`broken` sind `name` = Id und `game` leer), `CreateInstanceRequest { name: String, game: String, loader: Option<InstanceLoaderDto> }`, `OpenTarget` (`#[serde(tag = "kind")]`: `DataDir`, `Instance { id: String }`), `UrlTarget` ergänzt um `DeviceLogin`.
  - Commands: `loader_versions(kind: LoaderKindDto, game: String) -> Result<LoaderVersionsDto, ErrorDto>` (`recommended` = `pick_version(&list, None, kind, &game)`); `instances_list() -> Result<Vec<InstanceDto>, ErrorDto>` (ruft zuerst `adopt` mit den Ids aus dem gecachten Manifest; ohne Manifest-Cache keine Übernahme); `instance_create(req: CreateInstanceRequest) -> Result<InstanceDto, ErrorDto>` (prüft `game` gegen die Versionsliste und, mit Loader, die Version gegen `list_versions`; wählt die neue Instanz in den Einstellungen; `DataChanged(Instances)`); `instance_select(id: String) -> Result<(), ErrorDto>`; `instance_delete(id: String) -> Result<(), ErrorDto>` (Ordner per `trash::delete` in den Papierkorb; war sie gewählt, wird `selected_instance` `None`; `DataChanged(Instances)`); `open_location(target: OpenTarget) -> Result<(), ErrorDto>`; `open_url(target: UrlTarget) -> Result<(), ErrorDto>`.
  - Serialisierung: `AppState.instance_lock: tokio::sync::Mutex<()>` umschließt `instance_create` und `instance_delete`; ein zweiter Aufruf, während einer läuft, wartet nicht, sondern bekommt `ErrorCode::Busy` (neu).
  - `open.rs`: `fn resolve_location(paths: &Paths, t: &OpenTarget) -> Result<PathBuf>` (`Instance`: `validate_id`, Ordner muss existieren, sonst `InstanceNotFound`), `fn resolve_url(t: &UrlTarget) -> &'static str` mit `Repository` = `https://github.com/clemensjl/hengst-client`, `SignInHelp` = `https://github.com/clemensjl/hengst-client#readme`, `XboxSignup` = `https://www.xbox.com/`, `MinecraftProfile` = `https://www.minecraft.net/msaprofile/mygames/editprofile`, `DeviceLogin` = `https://www.microsoft.com/link`. Öffnen über `tauri_plugin_opener::OpenerExt` (`open_path`, `open_url`).

- [ ] **Step 1: Failing Tests schreiben** (Logik in `*_impl`-Funktionen; Testkit-Mojang-Mock)

```rust
#[tokio::test] async fn create_validates_game_and_loader_version() {
    // 9.9 + fabric 0.99.0 -> Ok, id "9.9-fabric", selected; 9.9 + fabric 0.0.1 -> UnknownLoaderVersion; "8.8" -> UnknownVersion; b9.9 + fabric -> NoLoaderVersions
}
#[tokio::test] async fn loader_versions_mark_the_recommended_build() { /* 9.9 fabric -> recommended "0.99.0", Liste enthält "0.100.0-beta.1" mit stable false */ }
#[tokio::test] async fn list_adopts_cli_folders_when_the_manifest_is_cached() { /* Ordner "9.9-fabric" ohne Datei, Manifest im Cache -> Liste enthält id "9.9-fabric", name "9.9 Fabric" */ }
#[tokio::test] async fn delete_moves_the_folder_away_and_clears_selection() { /* nach delete existiert der Ordner nicht mehr; selected_instance None; DataChanged(Instances) */ }
#[tokio::test] async fn create_and_delete_are_serialized() { /* zwei create gleichzeitig (join) -> eins Ok, eins Busy; danach genau ein Ordner */ }
#[test] fn open_location_refuses_unknown_and_unsafe_ids() { /* Instance { id: "..\\x" } -> UnsafeData; Instance { id: "fehlt" } -> InstanceNotFound */ }
#[test] fn url_targets_are_fixed_https_urls() { /* alle Varianten beginnen mit "https://" */ }
```
Hinweis zum Löschtest: `trash::delete` auf einem Temp-Ordner landet im echten Papierkorb; der Test benutzt einen kleinen Ordner unter `%TEMP%` und ist damit vertretbar. Wer den Papierkorb sauber halten will, setzt `HENGST_TEST_NO_TRASH=1`, dann löscht die Testvariante direkt (nur `cfg(test)`).

- [ ] **Step 2: Tests laufen lassen, sie scheitern**

Run: `cargo test -p hengst-desktop instances`
Expected: FAIL

- [ ] **Step 3: Implementieren** wie in Interfaces; `ErrorCode::Busy` mit Übersetzung ("Gerade läuft schon ein Vorgang. Warte einen Moment.").

- [ ] **Step 4: Tests grün**

Run: `cargo test --workspace --all-features`
Expected: PASS

- [ ] **Step 5: Lint, Format, Commit**

```bash
git add apps/desktop/src-tauri apps/desktop/src/ipc/bindings apps/desktop/src/i18n apps/desktop/src/features/errors/errorCopy.ts Cargo.lock
git commit -m "feat(desktop): instance commands, loader versions and validated open actions"
```

---

### Task 11: Instanzen in der Oberfläche

**Files:**
- Create: `apps/desktop/src/features/instances/{InstancesView.tsx,InstanceRow.tsx,CreateInstanceDialog.tsx,DeleteInstanceDialog.tsx,naming.ts,versionFilter.ts,naming.test.ts,versionFilter.test.ts,CreateInstanceDialog.test.tsx,InstancesView.test.tsx}`
- Modify: `apps/desktop/src/ipc/client.ts`, `src/stores/app.ts` (`instances: Resource<InstanceDto[]>`), `src/app/{Shell.tsx,views.ts}`, `src/features/settings/SettingsView.tsx` ("Ordner öffnen" für den Datenordner), `src/features/accounts/SignInDialog.tsx` ("microsoft.com/link öffnen" über `open_url`), `src/i18n/{de.json,en.json}`; shadcn `popover`, `command`, `select`, `radio-group`, `checkbox`, `input`, `label`, `skeleton`, `tooltip`

**Interfaces:**
- Consumes: Commands aus Task 10, `game_versions` (Task 2), `ErrorPanel`, Store.
- Produces:
  - `api.loaderVersions(kind, game)`, `api.instancesList()`, `api.instanceCreate(req)`, `api.instanceSelect(id)`, `api.instanceDelete(id)`, `api.openLocation(target)`, `api.openUrl(target)`.
  - `versionFilter.ts`: `filterVersions(list: GameVersionDto[], f: VersionFilter, query: string): GameVersionDto[]` (Suche als Teilstring ohne Groß-/Kleinschreibung auf `id`), `DEFAULT_FILTER: VersionFilter = { release: true, snapshot: false, beta: false, alpha: false }`.
  - `naming.ts`: `suggestName(game: string, loader: LoaderKindDto | null): string` (`"1.20.1 Forge"`, `"1.21.11"`), `LOADER_LABEL: Record<LoaderKindDto, string>` (Eigennamen, nicht übersetzt); der Name folgt der Auswahl, bis der Nutzer das Feld selbst ändert.
  - `InstancesView`: Kopf mit h1 "Instanzen" und Hauptaktion "Instanz anlegen"; Liste (`<ul>` mit Zeilen 48 px): Name, daneben zwei Spalten "Version" und "Loader" (z. B. "Forge 47.4.28") in sekundärer Schrift, keine mit Mittelpunkten verketteten Meta-Strings, rechts "Zuletzt gespielt" (`Intl.RelativeTimeFormat`, "Noch nie gespielt"); Klick auf Zeile wählt (`aria-pressed`/Häkchen), Zeilenmenü (DropdownMenu, Icon-Knopf mit Namen "Aktionen für {{name}}"): "Ordner öffnen", "Löschen"; `tooNew`/`broken` als Zeile mit Warn-Badge und nur "Ordner öffnen". Leer-, Lade- (drei Skeleton-Zeilen) und Fehlerzustand nach DESIGN.md.
  - `CreateInstanceDialog`: Felder in dieser Reihenfolge: Spielversion (Combobox aus Popover + Command, Suche, darunter Filter-Checkboxen Releases/Snapshots/Beta/Alpha), Loader (RadioGroup: Kein Loader, Fabric, Quilt, Legacy Fabric, Forge, NeoForge), Loader-Version (Select; Einträge mit Badge "Empfohlen" bzw. "Instabil"; Vorauswahl `recommended`; ist der Loader für die Version nicht verfügbar, steht dort der übersetzte Hinweis aus `errors.noLoaderVersions` mit `hint` und der Knopf bleibt aus mit dieser Erklärung), Name (Input, Label darüber, Hilfezeile mit fester Höhe). Hauptaktion "Instanz anlegen" (während des Aufrufs "Wird angelegt…", gesperrt), Nebenaktion "Abbrechen". Erfolg: Dialog zu, Toast "{{name}} angelegt", neue Zeile gewählt.
  - `DeleteInstanceDialog`: Titel "{{name}} löschen?", Text "Der Ordner mit Welten, Einstellungen und Screenshots wird in den Papierkorb verschoben.", Knöpfe "In den Papierkorb" (destructive) und "Behalten".

- [ ] **Step 1: Zustandsliste** (produkt-regelwerk) für Liste, Anlegen, Löschen in den Bericht; Extremwerte: 0, 1, 40 Instanzen; Name mit 64 Zeichen ("Aleksandra Wiśniewska-Kowalczyks Überlebenswelt mit Freunden 26.3"); Versions-Id `1.14 Pre-Release 1`; Loader-Liste leer; Versionsliste aus Cache ohne Netz.

- [ ] **Step 2: Failing Tests schreiben**

```ts
// versionFilter.test.ts
it("default filter keeps releases only", ...);
it("query matches case-insensitively and keeps manifest order", ...);   // "PRE" findet "1.14 Pre-Release 1"
// naming.test.ts
it("suggests '<game> <Loader>'", () => { expect(suggestName("1.20.1", "forge")).toBe("1.20.1 Forge"); expect(suggestName("1.21.11", null)).toBe("1.21.11"); });
// CreateInstanceDialog.test.tsx (mockIPC)
it("double_submit_creates_one_instance", ...);                // zwei Klicks -> instance_create genau einmal, Knopf zeigt Ladezustand
it("preselects the recommended loader version", ...);
it("explains why create is disabled when the loader has no builds", ...);  // Text mit Hinweis sichtbar, Knopf aria-disabled mit aria-describedby
it("keeps a typed name when the version changes", ...);
it("keeps inputs after a failed create and shows the error at the form", ...);
// InstancesView.test.tsx
it("shows the empty state with one primary action", ...);
it("delete asks first and calls instance_delete after confirm only", ...);
it("too new instance offers only open folder", ...);
```

- [ ] **Step 3: Tests laufen lassen, sie scheitern**

Run: `pnpm -C apps/desktop test instances`
Expected: FAIL

- [ ] **Step 4: Implementieren** nach DESIGN.md; Gegenprüfung C1 bis C27 (C1 Labels, C4 Sperre, C6 Leer/Fehler, C10 Fehler verschiebt nichts, C13 erklärter Sperrzustand, C14 Textlängen, C17 Dialogtexte).

- [ ] **Step 5: Tests grün**

Run: `pnpm -C apps/desktop typecheck && pnpm -C apps/desktop test && cargo test --workspace --all-features`
Expected: PASS

- [ ] **Step 6: Sichtprüfung gegen den Mock und echt**

Gegen `hengst-mock` (wie Task 8): leer, anlegen (Filter umschalten, Suche, Fabric mit Empfohlen, Beta-Version ohne Fabric zeigt Hinweis), Liste, Ordner öffnen (Explorer-Fenster erscheint, `list_windows`), löschen mit Dialog. Zusätzlich ein Debug-Build ohne `e2e` mit echtem Netz: Versionsliste von Mojang, Fabric- und Forge-Versionen für 1.21.11 laden (keine Spielstarts). Vorab per CLI `hengst --home %TEMP%\hengst-4a-t11 launch 1.21.11 --offline Dev --dry-run` einen CLI-Ordner erzeugen: erscheint als "1.21.11" in der Liste. Hell/dunkel, de/en, 960 × 600 und 1920 × 1080 mit 40 Instanzen (per Skript angelegte `instance.json`-Dateien), Tastatur-Durchlauf durch Liste, Menü, Dialog, Combobox. Screenshots nach `evidence/task-11/`.

- [ ] **Step 7: Lint, Format, Commit**

```bash
git add apps/desktop/src apps/desktop/package.json apps/desktop/pnpm-lock.yaml
git commit -m "feat(desktop): instances view with create and delete dialogs"
```

---

### Task 12: Ende-zu-Ende-Tests über WebDriver

**Files:**
- Create: `apps/desktop/e2e/{wdio.conf.ts,setup-driver.ps1,specs/accounts.e2e.ts,specs/instances.e2e.ts,support/app.ts}`
- Modify: `apps/desktop/package.json` (devDependencies `@wdio/cli`, `@wdio/local-runner`, `@wdio/mocha-framework`, `@wdio/spec-reporter`, `tsx`; Skript `e2e`), `apps/desktop/src-tauri/src/startup.rs` (nur falls nötig: `e2e`-Build überspringt `window-state`, damit jede Sitzung mit derselben Größe startet)
- Modify: `CONTRIBUTING.md` (Abschnitt "Desktop app tests", Englisch)

Warum WebDriver und nicht nur agentfenster: Die E2E-Läufe sollen bei jedem größeren Schritt in 4b ohne Menschen und ohne einen zweiten Desktop wiederholbar sein und später im Windows-CI laufen können; `tauri-driver` ist der offiziell unterstützte Weg unter Windows und auf diesem Rechner machbar (WebView2 157.0.4341.60 vorhanden, Treiber lassen sich installieren). agentfenster bleibt das Werkzeug für die Sichtprüfung (Aussehen, Themes, Größen), weil es echte Fenster und Screenshots liefert, aber keine Assertions in einer Testsuite.

**Interfaces:**
- Consumes: Debug-Build mit `--features e2e,dev-offline`, `hengst-mock` (Task 6), alle Commands aus 4a.
- Produces: `pnpm -C apps/desktop e2e` (baut nicht selbst; erwartet `target/debug/hengst-desktop.exe` und `target/debug/hengst-mock.exe`).
  - `setup-driver.ps1`: liest `pv` aus `HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}` (sonst HKCU), lädt `edgedriver_win64.zip` dieser Version von der offiziellen Microsoft-Edge-WebDriver-Downloadseite (URL-Muster beim Ausführen auf https://developer.microsoft.com/en-us/microsoft-edge/tools/webdriver/ nachsehen und im Skript als Konstante mit Kommentar ablegen), entpackt nach `apps/desktop/e2e/.driver/msedgedriver.exe`, prüft `tauri-driver --version` und gibt sonst `cargo install tauri-driver --locked` als Hinweis aus. Passt der Treiber nicht (Sitzung startet nicht), probiert das Skript die installierte Edge-Version (`{56EB18F8-B008-4CBD-B6D2-8C97FE7E9062}`) und notiert, welche passte.
  - `wdio.conf.ts`: je Spec-Datei ein frischer Temp-Home-Ordner mit `settings.json` (`language: "en"`, `theme: "light"`), `hengst-mock` als Kindprozess (Szenario je Spec über eine Hilfsfunktion), `tauri-driver --native-driver e2e/.driver/msedgedriver.exe` mit Umgebung `HENGST_HOME`, `HENGST_E2E_BASE_URL`; Capabilities `{ "tauri:options": { application: "../../target/debug/hengst-desktop.exe" } }`, `maxInstances: 1`, Mocha, Timeout 60 s.
  - Selektoren nur über Rolle und Namen (`$("aria/Sign in with Microsoft")`, `browser.$('button=Create instance')`), keine Test-IDs.

- [ ] **Step 1: Werkzeuge einrichten**

Run: `cargo install tauri-driver --locked` und `powershell -File apps/desktop/e2e/setup-driver.ps1`
Expected: `tauri-driver --version` meldet 2.1.0, `apps/desktop/e2e/.driver/msedgedriver.exe --version` meldet 157.0.4341.60 (oder die notierte passende Version).

- [ ] **Step 2: Specs schreiben**

```ts
// accounts.e2e.ts
it("signs in two accounts, switches and signs out", async () => {
  // leerer Zustand: Knopf "Sign in" in der Kopfzeile
  // Sign in -> Dialog -> "Sign in with Microsoft" -> Chip zeigt "E2EPlayer"
  // Menü -> "Add account" -> Chip zeigt "SecondPlayer"
  // Menü -> "E2EPlayer" wählen -> Chip "E2EPlayer"
  // Menü -> "Sign out" bei E2EPlayer -> Dialog "Sign out E2EPlayer?" -> "Sign out" -> Chip "SecondPlayer"
});
it("uses the code flow on request", async () => { /* "Use a code instead" -> Code sichtbar -> nach Mock-Erfolg Chip mit Namen */ });
it("shows_not_approved_explanation", async () => { /* Szenario not-approved -> Text "Sign-in is not available yet" sichtbar, kein Konto angelegt */ });
it("offers the development account in development builds", async () => { /* Menü enthält "Development account" */ });
// instances.e2e.ts
it("creates, lists and deletes an instance", async () => {
  // "No instances yet" sichtbar -> "Create instance" -> Version "9.9" -> Loader "Fabric" -> Loader-Version zeigt "0.99.0" mit "Recommended"
  // Name-Feld == "9.9 Fabric" -> "Create instance" -> Zeile "9.9 Fabric" in der Liste
  // Aktionen -> "Delete" -> "Move to Recycle Bin" -> Leerzustand
});
it("explains a loader that has no builds for the version", async () => { /* Filter Beta an, "b9.9", Fabric -> Hinweistext, Knopf deaktiviert mit Erklärung */ });
```

- [ ] **Step 3: Bauen und laufen lassen**

Run: `pnpm -C apps/desktop tauri build --debug --no-bundle --features e2e,dev-offline && cargo build -p hengst-testkit --bin hengst-mock && pnpm -C apps/desktop e2e`
Expected: alle Specs PASS; kein Fenster bleibt offen, kein `hengst-mock`-Prozess bleibt übrig (`Get-Process hengst-mock` leer).

- [ ] **Step 4: Gegenprobe, dass die Suite Fehler findet**

Den Dialogtext "Sign out {{name}}?" in `en.json` vorübergehend ändern, `pnpm build` und E2E erneut: genau der Abmelden-Test scheitert. Änderung zurücknehmen.

- [ ] **Step 5: CONTRIBUTING.md ergänzen** (Englisch): Werkzeuge, Befehle aus Step 1 und 3, Hinweis, dass E2E nur Debug-Builds mit `e2e` testet.

- [ ] **Step 6: Abschlussprüfung 4a**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --all-features && pnpm -C apps/desktop typecheck && pnpm -C apps/desktop test && pnpm -C apps/desktop format:check`
Expected: alles grün. `code-review` auf dem 4a-Diff, Befunde nach der Gegenprüf-Regel der globalen Vorgaben behandeln.

- [ ] **Step 7: Commit**

```bash
git add apps/desktop/e2e apps/desktop/package.json apps/desktop/pnpm-lock.yaml CONTRIBUTING.md
git commit -m "test(desktop): WebDriver end-to-end tests for accounts and instances"
```

---

## Selbstprüfung 4a

- Abdeckung: Konten (Browser-Login mit PKCE und Loopback, Device Code als Ausweg, mehrere Konten, Wechsel, Abmelden, Token nur im Kern, "nicht freigegeben" erklärt, Dev-Konto nur mit `dev-offline`): Tasks 5 bis 8. Instanzen (anlegen mit Version, Filter, Loader, Loader-Version; listen; löschen; Ordner öffnen; `instance.json` mit Format und Migrationsregel; CLI-Namen): Tasks 9 bis 11; Starten und Stoppen in 4b. Fehler typisiert mit Aktion: Task 4, ergänzt in 5, 9, 10. Sprache, Theme, Datenordner: Task 3 (Rest der Einstellungen 4b). Architekturvorgaben (dünne Schicht, eigene DTOs, Capability, CSP, Single Instance, kleine Zustandshaltung, Typen mit Drift-Test, Barrierefreiheit): Tasks 2, 3, 8, 11. Tests (Rust-Unit, Command-Ebene gegen Mock, Vitest, E2E, Sichtprüfung): je Task, E2E in 12. DESIGN.md vor jedem Screen: Task 1.
- Folgepunkte aus den Reviews: Auth Code + PKCE (Task 5), typisierte Lock-Fehler (Task 4; `NativesInUse` gibt es schon), Install-Log in der UI (4b Task 16), Job-Objekt (4b Task 13), engere API (4b Task 21), `--resolve-only` (4b Task 21), Wiederverwendung von Prüfergebnissen (ausgeschlossen).
- Typen: `ErrorDto`, `ErrorCode`, `ErrorAction`, `UrlTarget`, `OpenTarget`, `EventSink`, `AppEvent`/`DataScope`, `OperationId`, `LoaderKindDto` werden in den genannten Tasks eingeführt und später gleich benannt benutzt.

---

## Phase 4b: Gliederung (wird nach 4a als eigene Plandatei ausgeschrieben)

### Task 13: Kern: Windows-Prozesse (Job-Objekt, kein Konsolenfenster, Abbruch)
- Files: `crates/hengst-core/src/process/{mod.rs,job.rs}`, `loaders/forge/processors.rs` (`SystemRunner`), `Cargo.toml` (`windows-sys` mit `Win32_System_JobObjects`, `Win32_System_Threading`, `Win32_Foundation`, `Win32_UI_WindowsAndMessaging`).
- Interfaces: `process::Job::new_kill_on_close() -> Result<Job>`, `Job::assign(&self, child: &std::process::Child) -> Result<()>`, `Job::terminate(&self, code: u32)`; `pub struct CancelFlag(Arc<AtomicBool>)`; `pub struct JobRunner { cancel: CancelFlag }` implementiert `ProcessRunner` (Kind ins Job-Objekt, `CREATE_NO_WINDOW`, prüft `cancel` alle 100 ms, killt und meldet `Err(Interrupted)`, was `run_processors` als `Error::Cancelled` weiterreicht). `SystemRunner` bekommt ebenfalls Job-Objekt und `CREATE_NO_WINDOW` (CLI-Verhalten sonst gleich).
- Tests: `processor_child_dies_with_the_job` (Kind-Prozess `powershell -c Start-Sleep 60`, Job droppen, Prozess weg), `cancel_stops_a_running_processor_and_writes_no_marker`, `no_console_window_flag_is_set` (Command-Flags geprüft über einen Recorder).

### Task 14: Kern: Spielprozess mit Ausgabe, Schwärzung, Stopp
- Files: `crates/hengst-core/src/game.rs`, `crates/hengst-core/src/gamelog.rs`, `crates/hengst-testkit/src/bin/fake-java.rs`.
- Interfaces: `pub async fn spawn_game(p: &Prepared, sink: LineSink) -> Result<GameProcess>` (prüft `ensure_launch_allowed`, `CREATE_NO_WINDOW`, stdout/stderr gepipt, eigenes Job-Objekt ohne kill-on-close); `GameProcess::{pid, wait() -> Result<i32>, request_stop() (WM_SYSCOMMAND SC_CLOSE an die Fenster des Prozesses), force_stop() (TerminateJobObject)}`; `pub struct Redactor` (ersetzt das Access-Token, gebaut aus `Prepared`, sieht nie die Web-Ansicht); `gamelog::Parser` für log4j-XML-Ereignisse (`<log4j:Event ... level=... thread=...>` mit `CDATA`-Nachricht und `Throwable`) und Textzeilen `[hh:mm:ss] [thread/LEVEL]: msg`, Ergebnis `LogLine { seq, time, level: Option<Level>, thread, message, stream }`; Zeilen über 8 KiB gekürzt mit Marker. Launcher-Logdatei `logs/game/<instance>-<yyyymmdd-hhmmss>.log` (geschwärzt), höchstens 10 je Instanz.
- Tests: `token_never_reaches_the_sink_or_the_file` (fake-java gibt seine Argumente aus), `parses_log4j_xml_events_across_lines` (Fixture aus einem echten 1.21.11-Start, im Task aufgenommen), `parses_plain_lines_of_old_versions`, `graceful_stop_then_force_stop`, `offline_account_cannot_spawn_in_release`.

### Task 15: Spielen im Backend
- Files: `apps/desktop/src-tauri/src/{play.rs,commands/play.rs,dto/play.rs}`.
- Interfaces: Commands `play(instance_id, channel: Channel<PlayEvent>) -> OperationId`, `play_cancel(op)`, `game_stop()`, `game_force_stop()`, `game_state() -> GameStateDto`; `PlayEvent` bildet `Progress` 1:1 ab (`ResolvingLoader`, `DownloadingInstaller`, `Downloading { what, count }`, `Downloaded`, `InstallStarted`, `InstallLog { path }`, `InstallStep { index, total, name }`, `VerifyingInstall`, `InstallFinished { elapsed_ms }`, `InstallReused`, `Notice { message }`), dazu `Preparing`, `Started { run_id }`, `Failed { error }`, `Cancelled`; `AppEvent::GameState { state: Idle | Preparing { instance } | Running { instance, run_id, since } | Stopping }`. Ein Spiel zur Zeit (Ruling: RAM, einfacher Zustandsautomat); Konto über `launch_account` (Refresh), bei `LoginExpired` `mark_sign_in_required`; nach Erfolg `instances::record_launch`; `java.exe` mit `CREATE_NO_WINDOW` (kein Wechsel auf `javaw.exe`, Ausgabe bleibt lesbar).
- Tests: Ablauf gegen Testkit-Mojang mit fake-java (Events in Reihenfolge, Zustand Running, Stopp), `second_play_is_busy`, `cancel_during_downloads`, `expired_login_marks_account`.

### Task 16: Startseite
- Files: `apps/desktop/src/features/play/{PlayView.tsx,PlayButton.tsx,ProgressPanel.tsx,progress.ts}`.
- Inhalt: gewählte Instanz mit Name groß (32/40), Version/Loader, "Zuletzt gespielt", Konto-Chip in der Kopfzeile, ein Spielen-Knopf (56 px) mit Zuständen Bereit / Wird vorbereitet (Abbrechen) / Läuft (Beenden, nach 10 s zusätzlich "Sofort beenden") / Kein Konto (Anmelden) / Keine Instanz (Instanz anlegen); Fortschrittsbereich mit Phase, Zählern ("3.402 Dateien"), Installer-Schritt "Schritt 3 von 6: jarsplitter", Link "Installationsprotokoll öffnen" (`OpenTarget::InstallLog { path }`, Pfad muss unter `<root>/logs` liegen und auf `.log` enden). Animation nur am Fortschrittsbalken und Gedrückt-Zustand, Prüfung mit `review-animations`.
- Tests: `progress.ts`-Reducer (Events → Anzeige), Knopf-Zustände, Doppelklick startet einmal.

### Task 17: Tray und Fensterverhalten
- Files: `apps/desktop/src-tauri/src/tray.rs`, `tauri` Feature `tray-icon`.
- Entscheidung: Tray kommt in 4b, weil Tauri ihn mit `TrayIconBuilder` direkt mitbringt (https://v2.tauri.app/learn/system-tray/). Beim Spielstart minimiert die App in den Tray, wenn `minimize_on_launch` gilt; nach Spielende kehrt das Fenster zurück; Tray-Menü "Hengst Client öffnen", "Spiel beenden", "Beenden"; Schließen des Fensters bei laufendem Spiel versteckt in den Tray, "Beenden" bei laufendem Spiel fragt "Spiel beenden und Hengst Client schließen?" mit "Spiel beenden und schließen" / "Weiterspielen".
- Tests: Zustandslogik in Rust (Fenster sichtbar je Zustand), E2E-Teil in Task 22.

### Task 18: Live-Log im Backend
- Files: `apps/desktop/src-tauri/src/{log.rs,commands/log.rs}`.
- Interfaces: Ringpuffer 10.000 Zeilen je Lauf (aktueller und letzter Lauf), `log_attach(run_id, since_seq, channel: Channel<LogBatch>)` (Rückfüllung, dann live, gebündelt alle 100 ms oder 200 Zeilen), `log_detach`, `log_copy(run_id, filter) -> String` (geschwärzt, gefiltert), `open_location(OpenTarget::GameLog { run_id })`.
- Tests: Puffergrenze, Reihenfolge, Rückfüllung nach Neuladen, kein Token in Batches.

### Task 19: Live-Log in der Oberfläche
- Files: `apps/desktop/src/features/log/{LogView.tsx,LogToolbar.tsx,logStore.ts}`, Bibliothek Virtuoso (`pick-ui-library`).
- Inhalt: virtualisierte Liste (höchstens 10.000 Zeilen im Speicher), Stufenfilter (Fehler, Warnung, Info, Debug, Sonstige), Suche mit Trefferzahl, "Ans Ende folgen", Kopieren (gefiltert), "Logdatei öffnen", Mono 13/20, Farben `--log-*`.
- Tests: Filter und Suche im Store, Grenze, Folgen-Logik; Sichtprüfung mit 10.000 Zeilen.

### Task 20: Einstellungen vollständig und Über
- Files: `apps/desktop/src/features/settings/*`, `apps/desktop/src-tauri/src/commands/settings.rs`, Dialog-Plugin nur aus Rust (`tauri-plugin-dialog`, ohne JS-Permission) für die Java-Auswahl.
- Inhalt: Arbeitsspeicher (Zahlfeld mit `inputmode="numeric"`, Schritte 512 MB, Bereich 512 bis 65.536, Hinweis auf installierten RAM), Java-Überschreibung (Rust-Dialog, Prüfung `java.exe -version` mit Zeitlimit), Datenordner (Anzeige, Öffnen), Sprache, Design, "Beim Spielstart in den Infobereich", Über: Version, "Development build"-Badge, "Not an official Minecraft product.", Lizenz GPL-3.0, Drittlizenzen (generiert mit `cargo about` und `pnpm licenses list --json`, als statische Datei ins Bundle), Link zum Repository.
- Tests: Validierung, Java-Prüfung gegen fake-java, Lizenzliste nicht leer.

### Task 21: Kern-API verengen, `--resolve-only`, CI
- Files: `crates/hengst-core/src/{lib.rs,auth/mod.rs,loaders/mod.rs}`, `apps/cli/src/main.rs`, `.github/workflows/ci.yml`.
- Inhalt: öffentliche Oberfläche = was CLI, Desktop und Integrationstests nutzen; `auth::{msa, xbox, minecraft}` und `loaders::{fabric, merge}` werden privat mit Re-Exports (`DeviceCode`, `MsaTokens`, `Profile`, `ProcessRunner`, `SystemRunner`, `JobRunner`); `hengst launch --resolve-only` löst Version und Loader auf und gibt sie aus, ohne Bibliotheken zu laden (für CI-Matrix auf Pull Requests); CI-Job für die Desktop-App (pnpm install, typecheck, Vitest, `bindings_are_current`, `tauri build --debug --no-bundle`); E2E vorerst nur lokal (Treiberversion an den Runner gebunden, Entscheidung in 4b nach einem Versuchslauf).

### Task 22: E2E für Spielen, Log, Einstellungen und Abschluss
- Specs: Spielen mit fake-java (Fortschritt, Läuft, Log-Zeilen sichtbar ohne Token, Beenden), Einstellungen (Sprache und Design wechseln, Neustart behält), Tray-Verhalten soweit über WebDriver prüfbar.
- Abschluss: `produkt-regelwerk` im PRÜFEN-Modus über alle Ansichten (Befundtabelle), `break-ui`-Extremdaten, `accessibility`-Durchgang, `review-animations` über den Diff, `code-review` und `security-review` (Capability, CSP, Pfadprüfung, Schwärzung), echte Spielstarts aus der App (Debug-Build mit `dev-offline`, stumme `options.txt`): Vanilla 1.21.11, Fabric 1.21.11, Forge 1.20.1 bis Titelbildschirm; README (Desktop-Abschnitt, "Not an official Minecraft product"), `docs/PLAN.md` Abschnitt 11 Stand Phase 4.

## Beim Menschen (Clemens)

- **Azure-App (Auth Code + PKCE):** im Entra-Portal bei der App `7cad9d53-b0ce-44b1-9ef3-afa798da99c2` unter "Authentication" die Plattform "Mobile and desktop applications" hinzufügen und dort die Redirect-URI `http://localhost` eintragen (genau so, ohne Port, ohne Pfad, ohne Query); "Allow public client flows" bleibt auf "Yes"; unterstützte Kontotypen müssen persönliche Microsoft-Konten einschließen; kein Client-Secret. Ohne diesen Eintrag antwortet Microsoft beim Browser-Login mit einem Redirect-Fehler; der Device-Code-Weg funktioniert weiter.
- **Mojang-Freigabe:** weiter offen; bis dahin zeigt die App die Erklärung aus Task 8, echte Anmeldung bleibt ungeprüft.
- **Branches:** Phase 2 und 3 sind lokal fertig und nicht gemergt; 4a baut auf `phase-3-loader` auf. Wann gepusht und gemergt wird, entscheidet Clemens.
- **Logo:** Platzhalter unter `apps/desktop/brand/mark.svg`; Tausch nach `brand/README.md`.
- **Code-Signing:** nicht in Phase 4; ungesignierte Builds lösen SmartScreen-Warnungen aus (PLAN.md 12).
