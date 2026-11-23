# Hengst Client Phase 4a (Desktop-App: Gerüst, Konten, Instanzen) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eine Windows-Desktop-App `apps/desktop` (Tauri 2, React, TypeScript, Tailwind, shadcn/ui) startet, zeigt echte Daten aus dem Kern, meldet Microsoft-Konten per Browser-Login (Auth Code + PKCE, Device Code als Ausweg) an, wechselt und entfernt sie und legt Instanzen (Spielversion, Loader, Loader-Version) an, listet und löscht sie. Phase 4b (Gliederung am Ende) ergänzt Startseite, Spielstart, Live-Log, Einstellungen und Abschluss zu v0.1.

**Architecture:** Alle Launcher-Logik bleibt in `hengst-core`; neu sind dort die Module `instances` und `auth::browser`. Die Tauri-Crate `apps/desktop/src-tauri` ist eine dünne Schicht aus Commands, eigenen serialisierbaren DTOs und Event-Kanälen; Kerntypen erreichen die Web-Ansicht nie direkt. Die Oberfläche spricht nur über einen typisierten Client (`src/ipc/client.ts`) mit dem Backend; die TypeScript-Typen erzeugt `ts-rs` aus den DTOs, eingecheckt und per Test gegen Drift geprüft.

**Tech Stack:** Tauri 2.12 (`tauri` 2.12.7, `tauri-build` 2.7.6, `@tauri-apps/api` 2.12.5, `@tauri-apps/cli` 2.12.6), Plugins `single-instance` 2.5.5, `opener` 2.7.3 (Größe und Position des Fensters merkt ein kleines eigenes Modul `window_state.rs`, nicht das Plugin `window-state`: dessen Datei liegt im App-Konfigurationsordner statt im Datenordner `--home`, siehe Task 2); Vite 8.3.8, React 19.3.1, TypeScript, Tailwind CSS 4.3.5 mit `@tailwindcss/vite` 4.3.5, shadcn CLI 4.23.0 (Basis Base UI), i18next 27.2.2 + react-i18next 18.0.3, zustand 5.1.0, Sonner, lucide-react; `ts-rs` 12.0.1; Tests Vitest 5.1.1 + Testing Library + jsdom, `@tauri-apps/api/mocks`, `tauri::test` (Feature `test`), wiremock, `static_assertions` (Dev), WebdriverIO + `tauri-driver` 2.1.0. Neue Kern-Abhängigkeiten nur für PKCE: `sha2`, `base64`, `getrandom`.

**Spec:** `docs/PLAN.md` (Abschnitte 2, 3, 5.3, 7 mit den M-Punkten, 8, 9, 10, 11, 12) und der Phase-4-Auftrag vom 21.11.2026 (Umfang, Architektur-, Test- und Design-Vorgaben). Offene Punkte aus den Abschlussreviews: `.superpowers/sdd/2026-11-20-phase-2-login/progress.md` (F10, Auth Code + PKCE) und `.superpowers/sdd/2026-11-20-phase-3-loader/progress.md` (F17, Job-Objekt, typisierte Lock-Fehler, Install-Log in der UI, `--resolve-only`).

**Aufteilung:** Der Auftrag ist für eine Plandatei zu groß. Diese Datei enthält **Phase 4a vollständig (Tasks 1 bis 12)** und am Ende eine **Gliederung von Phase 4b (Tasks 13 bis 22)**. 4b wird nach Abschluss von 4a als eigene Plandatei `docs/superpowers/plans/<Datum>-phase-4b-desktop-mvp.md` ausgeschrieben, gegen die dann echten Schnittstellen aus 4a.

## Ausgangslage

- Branch `phase-4-desktop` von `phase-3-loader` (Phasen 2 und 3 sind lokal fertig, aber nicht gemergt; siehe "Beim Menschen"). Ledger unter `.superpowers/sdd/2026-11-21-phase-4-desktop-mvp/` (dort auch `evidence/`).
- Kern-API, auf die sich dieser Plan stützt (Stand Commit 9f4c0ac): `launch::{prepare_with_progress, Endpoints, LaunchOptions, Prepared, ensure_launch_allowed, DEV_OFFLINE}`, `progress::{Progress, ProgressSink, Transfer}`, `auth::{MicrosoftAuth, AuthEndpoints, client_id, unix_now, AuthError}`, `auth::msa::{DeviceCode, MsaTokens}` mit `start_device_login`/`poll_device_login`, `auth::flow::{launch_account, LoginSession}` mit `complete_login`, `auth::store::{AccountStore, SecretStore, KeyringSecretStore, MemorySecretStore, StoredAccount, AccountsFile, load_accounts_file}`, `loaders::{LoaderKind, LoaderVersion, LoaderRequest, LoaderEndpoints, list_versions, pick_version}`, `manifest::{fetch_manifest, VersionManifest, VersionEntry, VersionKind}`, `paths::{Paths, validate_id}`, `Error`.
- `launch::run` erbt die Konsole und kann nicht gestoppt werden; Spielstart mit Ausgabe-Stream kommt in 4b (Task 14).

### Werkzeuge auf dem Rechner (geprüft 21.11.2026)

| Werkzeug | Befund | Folge |
|---|---|---|
| Rust | `rustc 1.97.1`, `stable-x86_64-pc-windows-msvc` | reicht; `[workspace.package] rust-version` bleibt 1.89 für Kern und CLI, `tauri` 2.12.7 verlangt 1.90 (https://crates.io/api/v1/crates/tauri): die Desktop-Crate setzt eigenes `rust-version = "1.90"` (nicht `.workspace = true`). Folge: `cargo test --workspace` und `cargo clippy --workspace` brauchen eine Toolchain ab 1.90 (CI nutzt `stable`, hier 1.97.1); die 1.89-Zusage für Kern und CLI prüft ein eigener CI-Job (`cargo test -p hengst-core -p hengst-cli` auf 1.89, Task 2) |
| Node | `v22.19.0` | Vite 8.3.8 verlangt `^20.19.0 \|\| >=22.12.0` (https://registry.npmjs.org/vite/latest): passt; festgehalten in `apps/desktop/.nvmrc` (`22.19.0`) und `engines.node` (`>=22.12.0`), Task 2 |
| pnpm / npm | `11.9.0` / `10.9.3` | pnpm ist der Paketmanager der App, festgehalten über `packageManager: "pnpm@11.9.0"` in `apps/desktop/package.json` (Task 2) |
| Tauri CLI | `cargo tauri` fehlt | kein Cargo-Install nötig: `@tauri-apps/cli` 2.12.6 als devDependency, Aufruf `pnpm tauri ...` |
| WebView2 Runtime | `157.0.4341.60` (HKLM `...\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}`, Wert `pv`) | vorhanden; Prüfweg laut https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution |
| tauri-driver | fehlt | Task 12: `cargo install tauri-driver --locked` (2.1.0, https://v2.tauri.app/develop/tests/webdriver/manual-setup/) |
| msedgedriver | fehlt | Task 12: Skript lädt den Treiber passend zur WebView2-Version 157.0.4341.60 |
| agentfenster | als MCP-Werkzeug vorhanden (`desktop='agent'`, unsichtbarer zweiter Desktop) | Sichtprüfung jeder UI-Task; `agentshot` und jede Aufnahme des Nutzer-Desktops sind dafür tabu (sie können private Fenster erfassen) |

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

- **Typen zwischen Rust und TypeScript:** `ts-rs` 12 statt `tauri-specta`. Grund: stabil, keine RC-Abhängigkeit im Build-Pfad der App, erzeugt nur Typen und lässt die Command-Schicht unverändert. Die Dateien liegen eingecheckt unter `apps/desktop/src/ipc/bindings/`; der Rust-Test `bindings_are_current` erzeugt alle Typen neu und vergleicht sie zeichengenau mit den eingecheckten Dateien (Neuschreiben nur mit `HENGST_UPDATE_BINDINGS=1`).
- **Eine Liste für Commands:** `src-tauri/src/command_list.rs` enthält die einzige Liste als Makro `with_commands!` mit Einträgen `modul::name` (z. B. `app::app_info`). `build.rs` (Permissions über `AppManifest`) und `lib.rs` (Konstante `COMMANDS` und `generate_handler!`) binden die Datei per `include!` ein und rufen das Makro mit je einem eigenen Callback-Makro auf: Registrierung, Namensliste und erzeugte `allow-<cmd>`-Permissions stammen aus derselben Zeile, ein Command kann nicht registriert und zugleich unbeschränkt sein. Tests: `capability_allows_exactly_the_commands` (Task 2: `capabilities/main.json` hat für jeden Eintrag in `COMMANDS` genau eine `allow-<cmd>`-Permission und keine für einen nicht vorhandenen Command) und `ts_client_covers_every_command` (`src/ipc/client.ts` und `events.ts` rufen genau diese Namen auf). Jeder Task, der Commands hinzufügt, trägt sie nur in `command_list.rs` ein und führt in seinem Files-Block und in seinem `git add` zusätzlich `src-tauri/capabilities/main.json` und `src/ipc/client.ts` (bei Kanälen `events.ts`) auf.
- **Kanäle:** Ein globaler Kanal `subscribe_events(channel: Channel<AppEvent>)` für Zustandsänderungen, die das Backend selbst auslöst (`DataChanged`, in 4b Spielzustand); je Vorgang ein eigener `Channel` (Login, in 4b Startfortschritt und Log). Keine String-Events per `Emitter`.
- **UI-Bibliothek:** shadcn/ui mit Base UI als Primitiv-Schicht (`pick-ui-library`: base-ui für zugängliche Primitive; shadcn bietet sie als `--base`). Weitere Wahl nach `pick-ui-library`: Sonner (Toasts), cmdk (Combobox für Versionen, kommt mit shadcn `command`), zustand (Zustand), clsx + cva (von shadcn), in 4b Virtuoso (Log). Kein next-themes: Quelle des Themes sind die Einstellungen im Backend, das Fenster wird mit passender Hintergrundfarbe erzeugt, also kein Aufblitzen. Kein TanStack Query, kein Router: vier Ansichten über einen zustand-Wert.
- **Fenster:** native Windows-Titelleiste (sie folgt dem aufgelösten Theme beim Erzeugen und bei jeder Änderung zur Laufzeit, DESIGN.md 7.1), ein Fenster mit Label `main`. Größen sind Innengrößen in logischen Pixeln und werden an den Arbeitsbereich des Monitors geklemmt (DESIGN.md 7.1): Mindestgröße `min(960 × 600, Arbeitsbereich minus Rahmen)`, Startgröße je Achse `max(Mindestgröße, min(1120 × 720, 90 % des Arbeitsbereichs minus Rahmen))`, also nie unter der Mindestgröße (DESIGN.md 7.1), damit die App auf 1366 × 768 bei 125 % passt (Arbeitsbereich 1093 × 580 logisch). Die Klemmung steckt in reinen Funktionen in `window_state.rs` (Task 2, mit Tests). Größe und Position merkt `window_state.rs` in `<root>/window.json` (Datenordner, damit `--home` und `HENGST_HOME` sie mitnehmen; das Plugin `window-state` kann seine Datei nicht dorthin legen). Die gemerkte Größe wird beim Wiederherstellen mit derselben Klemmung begrenzt. Die Wiederherstellung entfällt, wenn `--window-size` gegeben ist und im Feature `e2e`; `--window-size` ersetzt dann die Startgröße und wird ebenfalls geklemmt (nicht unter die Mindestgröße, nicht über den Arbeitsbereich), das tatsächliche Maß steht im Beleg. In diesen Läufen wird auch nichts gespeichert, damit Beleg- und Testläufe nie vom Zustand eines früheren Laufs abhängen. Das Fenster wird in `setup` per `WebviewWindowBuilder` erzeugt (nicht in `tauri.conf.json`), damit Theme und Hintergrundfarbe aus den Einstellungen gesetzt sind, bevor die Web-Ansicht lädt. Zweiter App-Start fokussiert das erste Fenster.
- **Debug-Startargumente:** Nur in Debug-Builds (`cfg(debug_assertions)`) versteht die App `--home <dir>` und `--window-size <B>x<H>` (Innengröße, an den Arbeitsbereich geklemmt), im Feature `e2e` zusätzlich `--e2e-base-url <url>` (gleichwertig zu `HENGST_E2E_BASE_URL`); damit startet agentfenster die App per `open_app` mit Argumenten, Testdaten, Mock und fester Größe, ohne Umgebungsvariablen. Release-Builds ignorieren alle drei (Datenordner dort weiter über `HENGST_HOME`). Task 2 liest alle drei Argumente in eine Struktur `StartupArgs`; die Verdrahtung von `--e2e-base-url` in die Endpunkte folgt in Task 7.
- **Entwicklungs-Features der Desktop-Crate:** `dev-offline` (reicht an `hengst-core/dev-offline` durch, zeigt das Offline-Entwicklungskonto) und `e2e` (alle Endpunkte aus `HENGST_E2E_BASE_URL`, `MemorySecretStore` statt Anmeldeinformationsspeicher, Browser-Login per HTTP-Abruf statt echtem Browser). `hengst-testkit` ist eine **optionale** Abhängigkeit der Desktop-Crate, aktiviert nur durch `e2e = ["dep:hengst-testkit"]` (Task 7; zusätzlich Dev-Abhängigkeit für Tests); ein Build ohne `e2e` enthält weder Testkit noch wiremock. `e2e` außerhalb von Debug-Builds ist ein `compile_error!`. Eine einzige Definition entscheidet, ob es ein Entwicklungs-Build ist: `pub const fn dev_build() -> bool { cfg!(debug_assertions) || cfg!(feature = "dev-offline") || cfg!(feature = "e2e") }` in `lib.rs`; `AppInfo.dev_build`, Fenstertitel und Über-Ansicht ("Development build") lesen nur sie. Weder `dev-offline` noch `e2e` sind in einem Release-Build für Nutzer.
- **Browser-Login:** Redirect-URI `http://localhost:<Port>` ohne Pfad; der Kern lauscht ausschließlich auf `127.0.0.1:<Port>` und, falls möglich, `[::1]:<Port>` (Browser lösen `localhost` unterschiedlich auf), nie auf einer anderen Adresse. Eigener Mini-HTTP-Parser (eine GET-Zeile, höchstens 8 KiB, 5 s Lesefrist), kein neues HTTP-Server-Crate. Jede Verbindung läuft in einer eigenen Aufgabe (höchstens 32 gleichzeitig; bei Überschreitung wird die älteste noch lesende Verbindung geschlossen), damit eine hängende Verbindung die echte Weiterleitung nicht aufhält. Genau ein gültiger Rückruf (passendes `state`) wird angenommen; jede andere Anfrage bekommt 404 oder 400, ohne das Warten zu beenden. Die Erfolgsseite geht erst nach dem erfolgreichen Token-Tausch an den Browser (scheitert der Tausch, die Fehlerseite). Der Port wird bei Erfolg, Fehler, Zeitablauf und Abbruch freigegeben. Wartezeit 5 Minuten. Der PKCE-Verifier liegt im Kern-Typ `Secret`. Die Web-Ansicht bekommt die Authorize-URL nie (sie enthält `state`); "Browser erneut öffnen" ist ein Command. Die Texte der Antwortseite im Browser übergibt die Oberfläche beim Start (übersetzt), der Kern setzt sie HTML-escaped ein. Die Redirect-URI `http://localhost` muss der Eigentümer vorab in Azure eintragen (siehe "Beim Menschen"); bis dahin laufen alle Tests gegen die Mock-Dienste, die echte Anmeldung ist ein offener manueller Schritt.
- **Auswahl des Kontos:** Das gewählte Microsoft-Konto ist `AccountsFile.default` des Kerns, also dasselbe wie in der CLI. Neu angemeldete Konten werden gewählt. Das Offline-Entwicklungskonto (nur `dev-offline`) wird über die Einstellung `devAccount: true` gewählt; Release-Builds lesen diese Einstellung nie.
- **Instanzen:** Ordner `<root>/instances/<id>/`, darin `instance.json`; der Ordner ist zugleich Spielordner wie heute (CLI-Kompatibilität, keine Unterordner). `id` ist der Ordnername (geprüft mit `validate_id`), der Anzeigename steht in `instance.json`. Eine neue Instanz bekommt als `id` den CLI-Standardnamen `<game>` bzw. `<game>-<loader-slug>`, wenn er frei ist, sonst `<...>-2`, `-3` usw.; so nutzt `hengst launch 1.20.1 --loader forge` denselben Ordner wie die erste App-Instanz "1.20.1 Forge". Ordner ohne `instance.json` (von der CLI angelegt) übernimmt die App, wenn sich der Name als Standardname einer bekannten Spielversion lesen lässt; alle anderen bleiben unangetastet und unsichtbar.
- **Loader-Version:** wird beim Anlegen festgeschrieben (Vorauswahl: empfohlene Version laut `pick_version`). Übernommene CLI-Ordner haben `version: null` und bekommen beim ersten erfolgreichen Start in 4b die aufgelöste Version eingetragen.
- **Löschen von Instanzen:** in den Windows-Papierkorb (Crate `trash`), damit es umkehrbar ist; der Dialog nennt Welten und Papierkorb ausdrücklich.
- **Einstellungen:** Datei `<root>/settings.json` mit eigenem `format`-Feld, gehört der Desktop-Crate (die CLI liest sie nicht). Eine kaputte Datei wird nach `settings.json.broken-<unix>` verschoben, die App startet mit Standardwerten; die Meldung dazu liefert genau der erste `settings_get` nach dem Start aus (danach nie wieder). Schreibzugriffe laufen alle über `SettingsHandle::update` (Sperre, auf Kopie ändern, speichern, erst dann übernehmen). Die Web-Ansicht darf per `settings_update` nur Felder ändern, die ein Nutzer in den Einstellungen ändert (Sprache, Design, Arbeitsspeicher, Java-Pfad, Minimieren beim Start), jedes in Rust geprüft; `dev_account` und `selected_instance` ändern nur `account_select`, die Login-Vorgänge und die Instanz-Commands, der Datenordner wird nur angezeigt.
- **Fehler:** Jeder Command liefert `Result<T, ErrorDto>`. `ErrorDto` trägt einen `code` (Union-Typ im Frontend), `params` (Strings für die Übersetzung), eine optionale `action` und `detail` (englischer Kerntext für "Details"). Die Abbildung `error_dto(&hengst_core::Error)` matcht erschöpfend ohne `_`-Zweig, damit jede neue Kernvariante den Build bricht, bis sie abgebildet ist.
- **Ordner und URLs öffnen:** nur über `open_url(target: UrlTarget)` (Task 4, Enum mit festen https-Adressen) und `open_location(target: OpenTarget)` (Task 10); freie Pfade oder URLs aus JavaScript gibt es nicht. Logdateien öffnet erst 4b (Task 16) über einen geprüften Pfad. Das `opener`-Plugin wird nur aus Rust benutzt, die Capability gewährt keine `opener:`-Permission.
- **Sprache und Ton:** Deutsch und Englisch ab Task 3 über i18next; Deutsch duzt, Satzanfang groß, Rest klein; Englisch in US-Schreibweise; keine Emojis. Wortlaut, Satzstil, Anführungszeichen, Zahlen und Einheiten nach DESIGN.md Abschnitt 12 und 3.3. Beispielnamen in Fixtures und Tests sind neutrale Spielernamen (`Mira_K`; die Mock-Spieler `E2EPlayer` und `SecondPlayer`), nie die Standardfigur des Spiels. Ohne Einstellung folgt die Sprache `navigator.language` (de* = Deutsch, sonst Englisch).
- **Ende-zu-Ende-Tests:** WebdriverIO über `tauri-driver` gegen den Debug-Build mit `--features e2e,dev-offline` und einen Mock-Dienst aus der neuen Crate `crates/hengst-testkit`; Selektoren über Rolle und zugänglichen Namen (prüft nebenbei die Beschriftung). Sichtprüfung getrennt davon mit agentfenster. Begründung in Task 12.
- **Branch und Arbeitsweise:** streng nacheinander 1 → 12, keine parallelen Builds (RAM), kein Spielstart in 4a.

## Global Constraints

- Plattform: Windows 10/11 x64.
- "Minecraft" nie im Produktnamen; Hinweis "Not an official Minecraft product" bleibt in README und kommt in die App (Über-Ansicht, 4b Task 20; in 4a schon im Fußbereich der Einstellungen).
- Kein Spielstart mit Offline-Konto in Release-Builds: nur mit Feature `dev-offline`.
- Code, Kommentare, Fehlertexte im Rust-Code, README: Englisch. `docs/` und dieser Plan: Deutsch mit echten Umlauten. UI-Texte nur über i18n, Deutsch und Englisch vollständig (Englisch in US-Schreibweise, Wortlaut nach DESIGN.md Abschnitt 12).
- Tokens nie loggen, ausgeben, serialisieren oder an die Web-Ansicht geben; DTOs enthalten keine `Secret`-Felder. **Invariante (F20):** kein Token-Typ (`Secret`, `MsaTokens`, `LoginSession`) implementiert `Serialize`; zwei Tests in der Desktop-Crate halten das fest (`tests/no_tokens.rs`, angelegt in Task 7, erweitert in Task 10): `token_types_do_not_implement_serialize` (`static_assertions::assert_not_impl_any!`) und `no_dto_or_event_contains_a_token` (jedes DTO und Ereignis wird mit einem bekannten falschen Token `FAKE-ACCESS-TOKEN-7f3a` in den Quelldaten gebaut und serialisiert; der Token-String darf nirgends vorkommen). Jeder neue DTO-Typ wird in diesen Test aufgenommen.
- Keine Telemetrie, keine Netzaufrufe aus JavaScript (`connect-src` nur IPC).
- Keine Emojis in Code, UI, Doku, Commits. Icons (lucide) sind erlaubt.
- Keine Minecraft-Assets, -Schriften oder -Logos.
- Alle Basis-URLs injizierbar; Standardtests ohne Live-Aufrufe; kein Kontakt zu Microsoft-Servern in Tests.
- Rust: jeder Task endet mit `cargo test --workspace --all-features`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all` ohne Diff. Mit der Desktop-Crate als Workspace-Mitglied brauchen diese Befehle weder pnpm noch WebView2: `build.rs` legt ein Platzhalter-`apps/desktop/dist/index.html` an, wenn `dist` fehlt (das Verzeichnis ist in `.gitignore`, `generate_context!` bettet es beim Kompilieren ein; `beforeBuildCommand` läuft nur unter `tauri build`/`tauri dev`, nie unter `cargo`), `icons/icon.ico` ist eingecheckt, und die Command-Tests laufen mit `tauri::test::mock_builder` (Mock-Laufzeit, kein Webview). Das bleibt so: nichts in `build.rs` oder in den Tests darf ein gebautes Frontend voraussetzen. Die Toolchain muss ab 1.89 für Kern und CLI, ab 1.90 für alles mit der Desktop-Crate reichen (siehe Werkzeuge).
- Frontend (ab Task 2): `pnpm -C apps/desktop typecheck`, `pnpm -C apps/desktop test`, `pnpm -C apps/desktop format:check` grün; dieselben Befehle laufen in CI (Job `desktop-frontend`, angelegt in Task 2, weil dort die ersten Frontend-Tests entstehen). Node und pnpm sind über `.nvmrc`, `engines` und `packageManager` festgelegt.
- UI-Tasks folgen `DESIGN.md` (Werte nur von dort, nicht aus diesem Plan): `produkt-regelwerk` im BAUEN-Modus (Zustandsliste vor dem Code, Gegenprüfung C1 bis C27 nach dem Code), danach Sichtprüfung der laufenden App in hell und dunkel, Deutsch und Englisch, bei 960 × 600, bei 1200 × 800 (Grenze der breiten Seitenleiste) und bei großer Größe (1920 × 1080), dazu mit WebView-Zoom 150 % (Strg+Plus) bei 960 × 600, wo das schmale Layout unter 880 CSS-Pixeln greift und nichts abgeschnitten wird oder waagerecht scrollt (DESIGN.md 7.1, 7.3, 13). Alle Maße sind Innengrößen, die nach Task 2 an den Arbeitsbereich geklemmt werden; der Beleg nennt das tatsächliche Maß. Die Windows-Skalierung (125 %, 150 %) ändert das Layout nicht (gleiche logische Pixel, DESIGN.md 7.1); die Platzierung auf 1366 × 768 bei 125 % deckt der Klemm-Test aus Task 2 ab. Die Belege (Screenshots, Elementbaum-Auszüge) liegen unter `.superpowers/sdd/2026-11-21-phase-4-desktop-mvp/evidence/task-<n>/`. Aufnahmen nur mit den Agent-Desktop-Werkzeugen (agentfenster auf dem unsichtbaren zweiten Desktop) oder den Screenshots der App über WebDriver, nie vom Bildschirm des Nutzers (kann private Fenster zeigen; also kein `agentshot`, keine Aufnahme von `desktop='user'`). Jede neue Animation nach `motion-design`, geprüft nach `~/.claude/skills/review-animations/SKILL.md`. Neue Komponenten erst nach `~/.claude/skills/pick-ui-library/SKILL.md`, sonst shadcn.
- **CSP- und Prototyp-Prüfung (F2b):** Jeder Task, der eine Frontend-Bibliothek hinzufügt, die Styles oder Skripte einschleusen kann (Task 3: shadcn-Basis, Base UI, Sonner; Task 8: Dialog, AlertDialog, DropdownMenu; Task 11: Popover, Command/cmdk, Select, Tooltip; in 4b Task 19: Virtuoso), wiederholt nach dem Bauen: Debug-Build der App starten, die neuen Zustände durchklicken und die Web-Konsole lesen (keine `Content-Security-Policy`-Meldung), und in der Konsole `Object.isFrozen(Object.prototype)` auswerten (muss `true` sein, `freezePrototype`). Der Rust-Test `csp_forbids_unsafe_inline_for_scripts_and_style_elements` (Task 2) läuft in jedem Task mit. Braucht eine Bibliothek ein Inline-`<style>` oder -Skript: nie `'unsafe-inline'` für `script-src` oder `style-src` setzen; stattdessen erst eine Nonce-Option der Bibliothek nutzen (Tauri ergänzt Nonces und Hashes für die gebaute Seite über seine CSP-Unterstützung; Elemente, die erst zur Laufzeit entstehen, muss die Bibliothek selbst mit dem Nonce versehen), sonst den SHA-256-Hash des festen Style-Texts in `app.security.csp` eintragen, sonst die Bibliothek wechseln. `style-src-attr 'unsafe-inline'` bleibt nur für `style="..."`-Attribute. `devCsp` gilt nur für `pnpm tauri dev`; geprüft wird immer der Build.
- **Arbeitsweise:** höchstens ein Schreiber im Repository zur Zeit (Reviews dürfen parallel lesen). Jeder Task endet mit grünen Rust- und Frontend-Prüfungen (siehe oben) und einem Commit, dessen Nachricht mit `Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>` endet. Dateien werden namentlich gestaged (Verzeichnisse nur für erzeugte Mengen wie `src/ipc/bindings`, `src/components/ui`, `icons` und für Verzeichnisse, die nur Dateien dieses Tasks enthalten; nie `git add -A` oder `.`); vor dem Commit zeigt `git diff --cached --name-only` genau die Files-Liste des Tasks. Kein Push.
- **Commands (F2/F3):** Commands stehen nur in `command_list.rs` (siehe Entscheidungen); jeder Task, der welche hinzufügt, nimmt `src-tauri/capabilities/main.json` und `src/ipc/client.ts` (bzw. `events.ts`) in Files und `git add` auf.
- **Azure und echte Anmeldung:** Die Redirect-URI `http://localhost` muss der Eigentümer in Azure eintragen, bevor die Browser-Anmeldung real funktionieren kann (siehe "Beim Menschen"). Alle Tests der Anmeldung laufen deshalb gegen die Mock-Dienste; die echte Anmeldung ist ein manueller Schritt und bleibt in 4a als offen aufgeführt.

## Review Focus

1. **Token erreicht die Web-Ansicht oder einen sichtbaren Text** (Login-Events, Fehlerdetails, Kontolisten, später Spielargumente): kein Token-String in irgendeinem serialisierten DTO. Tests `login_events_never_contain_tokens` (Task 7), `error_detail_never_contains_secret` (Task 4), `no_dto_or_event_contains_a_token` und `token_types_do_not_implement_serialize` (Task 7, erweitert in Task 10).
2. **CLI-Ordner trifft App-Instanz** (`instances/1.20.1-forge` ohne `instance.json` existiert, Nutzer legt "1.20.1 Forge" an; Snapshot-Ids mit Bindestrichen wie `26.4-snapshot-6-fabric`): Übernahme statt Überschreiben, neue Instanz bekommt `-2`, Spielstände bleiben. Tests `create_never_reuses_an_existing_folder`, `adopts_cli_folders_by_default_name` (Task 9).
3. **Mojang hat die App noch nicht freigegeben, Netz weg, Anmeldeinformationsspeicher verweigert**: verständliche Meldung mit passender Aktion statt Absturz oder Rohtext; die Kontoliste funktioniert ohne Schlüsselspeicher. Tests `not_approved_login_maps_to_app_not_approved` (Task 7), `accounts_list_works_without_secret_store` (Task 7), E2E `shows_not_approved_explanation` (Task 12).
4. **Kaputte oder zu neue Dateien** (`instance.json` mit `format: 2`, abgeschnittenes JSON, kaputte `settings.json`): Instanz erscheint als "nicht lesbar" bzw. "neuere Version", wird nie überschrieben; Einstellungen fallen mit Sicherungskopie auf Standard zurück. Tests `too_new_instance_is_listed_and_never_written`, `broken_instance_is_listed_not_deleted` (Task 9), `broken_settings_are_moved_aside` (Task 3).
5. **Doppelklick und gleichzeitige Vorgänge** (zweimal "Instanz anlegen", zweiter Login während der erste wartet, Löschen während Anlegen): genau ein Vorgang, Knopf gesperrt mit Ladezustand. Tests `double_submit_creates_one_instance` (Task 11, Vitest), `second_login_cancels_the_first` (Task 7), `create_and_delete_are_serialized` (Task 10).
6. **Der lokale Port des Browser-Logins** (hängende Verbindung, zweiter Rückruf, falsches `state`, Abbruch): der Port ist danach frei, nur ein gültiger Rückruf zählt, die Erfolgsseite erscheint erst nach dem Token-Tausch. Tests `stalled_connection_does_not_block_the_real_callback`, `second_callback_is_refused`, `wrong_state_is_rejected_and_the_wait_continues_until_timeout`, `dropping_finish_frees_the_port`, `success_page_is_sent_only_after_the_exchange` (alle Task 5).

---

## Dateistruktur (4a)

```
Cargo.toml                                   + members apps/desktop/src-tauri, crates/hengst-testkit
DESIGN.md                                    Design-Tokens, Komponenten, Layout, Zustände (Task 1)
crates/hengst-core/src/
  manifest.rs                                + load_cached, fetch_manifest schreibt über Temp + Rename
  error.rs                                   + LockTimeout, Cancelled, Instance*, LoaderHint
  loaders/mod.rs                             LoaderKind: display_name (Task 4), serde als Slug (Task 9); LoaderHint
  loaders/forge/mod.rs                       lock_file liefert Error::LockTimeout
  auth/browser.rs                            Auth Code + PKCE mit Loopback (Task 5)
  auth/error.rs                              + BrowserLoginTimedOut, LoopbackUnavailable, BrowserLoginFailed
  auth/store.rs                              SecretStore für Box<dyn SecretStore>
  instances.rs                               instance.json, anlegen, listen, übernehmen (Task 9)
crates/hengst-testkit/                       publish = false: Mock-Dienste für Tests und E2E
  src/lib.rs, src/auth.rs, src/mojang.rs, src/bin/hengst-mock.rs, tests/mock_binary.rs
.github/workflows/ci.yml                     + Jobs msrv und desktop-frontend (Task 2)
apps/desktop/
  package.json (packageManager, engines), .nvmrc, pnpm-lock.yaml, index.html, vite.config.ts, tsconfig*.json, components.json, .prettierrc
  brand/mark.svg, brand/README.md            Platzhalter-Zeichen, einzige Tauschstelle
  src/main.tsx, src/styles/{index.css,tokens.test.ts,accent.test.ts}
  src/app/{App.tsx,Shell.tsx,Sidebar.tsx,views.ts}
  src/ipc/{client.ts,events.ts,errors.ts}, src/ipc/bindings/*.ts (erzeugt)
  src/i18n/{index.ts,de.json,en.json}
  src/stores/app.ts
  src/theme/{theme.ts,inputMode.ts}
  src/components/Segments.tsx                Loader-, Sprach- und Design-Segmente (DESIGN.md 10.6), Task 3
  src/features/about/AboutPanel.tsx
  src/features/settings/SettingsView.tsx
  src/features/errors/{ErrorPanel.tsx,errorCopy.ts}
  src/dev/ErrorGallery.tsx                   nur im Vite-Dev-Server (Task 4)
  src/features/accounts/{AccountChip.tsx,AccountMenu.tsx,SignInDialog.tsx,RemoveAccountDialog.tsx}
  src/features/instances/{InstancesView.tsx,InstanceRow.tsx,CreateInstanceDialog.tsx,DeleteInstanceDialog.tsx,naming.ts,versionFilter.ts,selection.ts}
  src/components/ui/*                        von shadcn erzeugt
  src/lib/utils.ts
  src-tauri/Cargo.toml, build.rs, tauri.conf.json, capabilities/main.json, icons/ (eingecheckt)
  src-tauri/src/{main.rs,lib.rs,command_list.rs,state.rs,events.rs,error.rs,settings.rs,open.rs,startup.rs,window_state.rs}
  src-tauri/src/dto/{mod.rs,app.rs,versions.rs,accounts.rs,instances.rs,settings.rs,error.rs}
  src-tauri/src/commands/{mod.rs,app.rs,versions.rs,settings.rs,accounts.rs,instances.rs,open.rs}
  src-tauri/src/accounts.rs                  Kontodienst, Login-Vorgänge
  src-tauri/tests/{bindings.rs,capabilities.rs,config.rs,no_tokens.rs}
  e2e/wdio.conf.ts, e2e/setup-driver.ps1, e2e/support/app.ts, e2e/specs/{accounts.e2e.ts,instances.e2e.ts}
                                             (WebdriverIO-Pakete in apps/desktop/package.json, kein eigenes package.json)
```

---

### Task 1: DESIGN.md

**Files:**
- Create: `DESIGN.md` (Repository-Wurzel, Englisch wie README, weil es Entwickler- und Beitragenden-Doku neben dem Code ist; Tokens sind sprachneutral)

**Interfaces:**
- Produces: `DESIGN.md` mit den Token-Namen unten; Task 3 legt sie als CSS-Variablen an, alle UI-Tasks benutzen sie.

Die Vorgaben stehen vollständig in `DESIGN.md` (Commit 05fe9ff, überarbeitet in 69b0d54). Dieser Plan wiederholt keine Werte daraus: Farben (Abschnitt 2, mit den berechneten Kontrasten in 2.3 und der shadcn-Zuordnung in 2.4), Schrift (3), Abstände (4), Form und Höhe (5), Icons (6), Fenster, Rahmen und Breakpoints (7), Bewegung (8), Fokus (9), Komponenten (10), Ansichten mit Zuständen (11), Texte (12), Barrierefreiheit (13), Zeichen (14), Abgrenzung (15), Entscheidungen (16). Maßgeblich ist immer `DESIGN.md`; ein Wert, der sich ändern muss, ändert sich dort im selben Commit wie der Code. Die Kontrastprüfung in Task 3 kann Werte nachschärfen, dann wird DESIGN.md im selben Commit angepasst. Werte stehen in diesem Plan nur dort, wo ein Test des Plans sie prüft, und müssen dann mit DESIGN.md übereinstimmen.

Richtung (Kurzfassung, Begründung in DESIGN.md Abschnitt 1): ruhiges Werkzeug, das man täglich mehrmals öffnet; eine Stelle darf auffallen (Startseite, 4b); der Akzent markiert nur die nächste Aktion (Abschnitt 2.2: Hauptaktion, aktives Navigationssymbol, Fokusring); Flächen durch Hintergrundstufen und Linien getrennt, keine Karten-Raster mit Schatten, keine Verläufe, keine Großbuchstaben-Labels, keine Marker, kein "→" an Knöpfen.

- Token-Namen (Quelle: DESIGN.md 2.1), die Task 3 als CSS-Variablen anlegt und alle UI-Tasks benutzen: Flächen `--bg`, `--surface`, `--surface-raised`; Linien `--border`, `--border-strong` (dekorativ) und `--border-control` (einzige Begrenzung eines bedienbaren Elements); Text `--text`, `--text-muted`, `--text-disabled`; Akzent `--accent`, `--accent-hover`, `--accent-pressed`, `--on-accent`, `--focus`; Gefahr `--danger`, `--danger-hover`, `--danger-pressed`, `--on-danger`; Status `--warning`, `--success`; Zustandsflächen `--hover`, `--selected`, `--selection`, `--highlight`, `--overlay`; Höhe `--elevation-1`; Log `--log-error`, `--log-warn`, `--log-info`, `--log-debug`; Abstände `--space-1..7`; Radien `--radius-control`, `--radius-panel`, `--radius-pill`; Dauern `--dur-press`, `--dur-popover-in`, `--dur-popover-out`, `--dur-dialog-in`, `--dur-dialog-out`, `--dur-progress`; Kurve `--ease-out`.
- Rollen der Linien-Token (DESIGN.md 2.1): `--border` und `--border-strong` sind dekorativ und haben keinen Mindestkontrast; jede Linie, die die einzige Begrenzung eines bedienbaren Elements ist, nutzt `--border-control` (mindestens 3:1 gegen `--bg`, `--surface` und `--surface-raised`, WCAG 1.4.11); shadcn-`--input` zeigt auf `--border-control`.
- Komponenten-Inventar mit shadcn-Zuordnung: DESIGN.md Abschnitt 10 (Button, Play-Knopf, Konto-Chip, Instanzzeile, Versionswahl, Loader-Segmente, Formularfeld, Checkbox und Switch, Dialoge, Toast, ErrorPanel, Fortschritt, Log, Badge, Übrige). Nicht benutzt: Card, Sheet, Tabs, Kbd, Avatar, der Sidebar-Block, Accordion, Carousel.
- Zustände je Ansicht (Leer, Laden, Fehler, Mutationen, Tastatur): DESIGN.md Abschnitt 11, Texte endgültig in den i18n-Dateien nach Abschnitt 12.
- Platzhalter-Zeichen: DESIGN.md Abschnitt 14; einzige Tauschstelle ist der Ordner `apps/desktop/brand/` (Anleitung in `brand/README.md`, Task 3).

- [ ] **Step 1: DESIGN.md schreiben** mit den Abschnitten 1 bis 17 und Anhang A (Direction, Color tokens, Typography, Spacing, Shape, Icons, Window and layout, Motion, Focus, Components, Screens, Writing, Accessibility, Brand mark, Not like this, Decisions, Review notes).
- [ ] **Step 2: Gegenprüfung** gegen `frontend-design` (fünf Standardmuster, "remove one accessory") und `produkt-regelwerk` Kurzfassung (WD9: kein reines Schwarz/Weiß als Textfläche, M2/M3 Dauern und Kurven, A1 Fokus). Ergebnis als Abschnitt "Review notes" (drei bis fünf Zeilen) in DESIGN.md.
- [ ] **Step 3: Commit**

```bash
git add DESIGN.md
git commit -m "docs: add DESIGN.md for the desktop app"
```

---

### Task 2: Walking Skeleton (Fenster, ein Command, ein Ereignisstrom)

**Files:**
- Modify: `Cargo.toml` (Member `apps/desktop/src-tauri`), `crates/hengst-core/src/manifest.rs` (+ `load_cached`, atomares Schreiben in `fetch_manifest`), `.gitignore` (+ `apps/desktop/node_modules/`, `apps/desktop/dist/`, `apps/desktop/e2e/.driver/`, `apps/desktop/src-tauri/gen/`), `.github/workflows/ci.yml` (Jobs `msrv` und `desktop-frontend`)
- Create: `apps/desktop/package.json` (mit `packageManager` und `engines`), `.nvmrc`, `index.html`, `vite.config.ts`, `tsconfig.json`, `tsconfig.app.json`, `tsconfig.node.json`, `.prettierrc`, `components.json` (shadcn init), `src/main.tsx`, `src/app/App.tsx`, `src/features/about/AboutPanel.tsx`, `src/ipc/client.ts`, `src/ipc/events.ts`, `src/ipc/bindings/*.ts`, `src/lib/utils.ts`, `src/styles/index.css`
- Create: `apps/desktop/src-tauri/Cargo.toml`, `build.rs`, `tauri.conf.json`, `capabilities/main.json`, `icons/` (Platzhalter aus `pnpm tauri icon`, eingecheckt), `src/main.rs`, `src/lib.rs`, `src/command_list.rs`, `src/state.rs`, `src/events.rs`, `src/error.rs`, `src/startup.rs`, `src/window_state.rs`, `src/dto/{mod.rs,app.rs,versions.rs,error.rs}`, `src/commands/{mod.rs,app.rs,versions.rs}`, `tests/bindings.rs`, `tests/capabilities.rs`, `tests/config.rs`
- Test: `crates/hengst-core/src/manifest.rs` (`#[cfg(test)]`), `apps/desktop/src-tauri/src/commands/versions.rs` und `src/window_state.rs` (`#[cfg(test)]`), `apps/desktop/src-tauri/tests/{bindings.rs,capabilities.rs,config.rs}`, `apps/desktop/src/ipc/client.test.ts`

**Interfaces:**
- Consumes: `manifest::{fetch_manifest, VersionManifest, VersionKind}`, `paths::Paths::{default_root, manifest_file}`, `launch::Endpoints`.
- Produces (Kern): `manifest::load_cached(paths: &Paths) -> Result<Option<VersionManifest>>` (fehlende **oder abgeschnittene/unlesbare** Datei `Ok(None)`: ein halb geschriebener Cache zählt als nicht vorhanden, nur ein Lesefehler des Dateisystems ist `Err(Io)`). `fetch_manifest` schreibt den Cache über eine Temp-Datei (`paths::unique_temp`) und `rename`, nie direkt, damit ein paralleles `load_cached` nie eine halbe Datei sieht.
- Produces (Desktop, Rust):
  - Crate `hengst-desktop` (`[lib] name = "hengst_desktop_lib"`), eigenes `rust-version = "1.90"` (nicht `rust-version.workspace`), Features `dev-offline = ["hengst-core/dev-offline"]`, `e2e = []` (Task 7 ergänzt `dep:hengst-testkit`); dev-dependency `tauri` mit Feature `test`, `tempfile`. `lib.rs` enthält `#[cfg(all(feature = "e2e", not(debug_assertions)))] compile_error!(...)` und `pub const fn dev_build() -> bool { cfg!(debug_assertions) || cfg!(feature = "dev-offline") || cfg!(feature = "e2e") }`, die einzige Definition.
  - `pub struct AppState { pub paths: Paths, pub endpoints: Endpoints, pub events: EventHub, pub versions: VersionsCache }` mit `AppState::new(paths: Paths, endpoints: Endpoints) -> AppState`; spätere Tasks fügen Felder hinzu.
  - `pub struct VersionsCache { memory: Mutex<Option<VersionManifest>>, refresh_started: AtomicBool }` mit `fn get(&self, paths: &Paths) -> Result<Option<VersionManifest>>` (Speicher, sonst `load_cached`), `fn put(&self, m: VersionManifest)`, `fn claim_refresh(&self) -> bool` (liefert genau einmal je App-Lauf `true`).
  - `window_state.rs`: `pub struct WindowState { pub format: u32, pub width: u32, pub height: u32, pub x: i32, pub y: i32, pub maximized: bool }`, `pub fn load(paths: &Paths) -> Option<WindowState>` (Datei `<root>/window.json`; fehlend, kaputt oder `format` ≠ 1 gleich `None`), `pub fn save(paths: &Paths, s: &WindowState) -> Result<()>` (Temp + Rename), `pub fn should_restore(size_arg_given: bool, e2e: bool) -> bool` (`!size_arg_given && !e2e`), `pub fn fits_on_a_monitor(s: &WindowState, monitors: &[(i32, i32, u32, u32)]) -> bool` (Fenster liegt zu mindestens 100 × 50 px in einem Monitor). `width` und `height` sind die logische Innengröße. Dazu die reinen Klemmfunktionen nach DESIGN.md 7.1: `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub struct Size { pub width: u32, pub height: u32 }`, `pub const MIN_SIZE: Size = Size { width: 960, height: 600 }`, `pub const DEFAULT_SIZE: Size = Size { width: 1120, height: 720 }`, `pub fn min_size(work: Size, frame: Size) -> Size` (je Achse `min(MIN_SIZE, work - frame)`, Subtraktion sättigend), `pub fn default_size(work: Size, frame: Size) -> Size` (je Achse `max(min_size, min(DEFAULT_SIZE, (work - frame) * 9 / 10))`, ganzzahlig, nie unter `min_size`, DESIGN.md 7.1), `pub fn clamp_size(requested: Size, work: Size, frame: Size) -> Size` (je Achse zwischen `min_size` und `work - frame`), `pub fn startup_size(arg: Option<Size>, saved: Option<Size>, restore: bool, work: Size, frame: Size) -> Size` (`--window-size` gewinnt und wird geklemmt; sonst die gemerkte Größe, geklemmt, nur bei `restore`; sonst `default_size`). `work` ist der Arbeitsbereich des Monitors in logischen Pixeln (physische Größe geteilt durch den Skalierungsfaktor; Tauri 2.12: `Monitor::work_area()`, Namen prüfen), `frame` der Rahmen (Außen- minus Innengröße, zur Laufzeit gelesen). Der Monitor ist der der gemerkten Position, sonst der Hauptmonitor. `startup.rs` ruft `load` nur bei `should_restore`, erzeugt das Fenster zunächst unsichtbar, liest den Rahmen, setzt Mindestgröße `min_size` und Größe `startup_size` und die Position nur bei `fits_on_a_monitor` (sonst Standard), zeigt es dann und speichert beim Schließen des Fensters (`CloseRequested`) nur bei `should_restore`.
  - `pub struct EventHub` mit `fn subscribe(&self, sink: EventSink<AppEvent>)` (ersetzt einen früheren Abonnenten) und `fn emit(&self, e: AppEvent)` (ohne Abonnent: verworfen). `pub type EventSink<T> = Arc<dyn Fn(T) + Send + Sync>`; Commands wickeln `Channel<T>` in einen `EventSink`.
  - `AppEvent` (`#[serde(tag = "event", content = "data", rename_all = "camelCase")]`): `DataChanged { scope: DataScope }`, `Notice { kind: NoticeKind }`; `DataScope`: `Versions`, `Settings`, `Accounts`, `Instances`; `NoticeKind`: `VersionsRefreshFailed` und `VersionsRefreshOffline` (die Aktualisierung scheiterte; `Offline`, wenn `error_dto` den Code `network` oder `offline` liefert, DESIGN.md 10.5; beides nicht blockierende Hinweise, kein Fehlerzustand).
  - DTOs (alle `#[derive(Serialize, Deserialize, TS)]`, `#[serde(rename_all = "camelCase")]`): `AppInfo { version: String, data_dir: String, dev_build: bool, dev_offline: bool }`; `VersionFilter { release: bool, snapshot: bool, beta: bool, alpha: bool }`; `GameVersionKind` (`release`, `snapshot`, `beta`, `alpha`, `other`); `GameVersionDto { id: String, kind: GameVersionKind, release_time: String }`; `GameVersionsDto { versions: Vec<GameVersionDto>, latest_release: String, latest_snapshot: String, from_cache: bool }`.
  - `ErrorDto { code: ErrorCode, params: BTreeMap<String, String>, action: Option<ErrorAction>, detail: String }` mit vorerst `ErrorCode::{Internal, Network}` und `ErrorAction` (`#[serde(tag = "kind", rename_all = "camelCase")]`) mit nur `Retry`, also JSON `{"kind":"retry"}`; Task 4 vervollständigt beide.
  - Commands: `app_info() -> Result<AppInfo, ErrorDto>`, `subscribe_events(channel: Channel<AppEvent>) -> Result<(), ErrorDto>`, `game_versions(filter: VersionFilter) -> Result<GameVersionsDto, ErrorDto>`. Logik in einfachen Funktionen `commands::versions::game_versions_impl(state: &AppState, filter: VersionFilter) -> Result<GameVersionsDto, ErrorDto>`, die `#[tauri::command]`-Hüllen rufen nur diese auf.
  - `command_list.rs`: die einzige Liste, `macro_rules! with_commands { ($callback:ident) => { $callback!(app::app_info, app::subscribe_events, versions::game_versions) }; }`. `lib.rs` definiert `macro_rules! names { ($($m:ident :: $n:ident),* $(,)?) => { &[$(stringify!($n)),*] }; }` und `pub const COMMANDS: &[&str] = with_commands!(names);`, dazu `macro_rules! handler { ($($m:ident :: $n:ident),* $(,)?) => { tauri::generate_handler![$(commands::$m::$n),*] }; }` und `.invoke_handler(with_commands!(handler))`; `build.rs` bindet dieselbe Datei per `include!` ein und baut mit einem eigenen `names!` die Permission-Liste. Spätere Tasks ändern nur diese Datei.
- Produces (Frontend): `src/ipc/client.ts` mit `export const api = { appInfo(): Promise<AppInfo>, gameVersions(filter: VersionFilter): Promise<GameVersionsDto> }`, `subscribeEvents(onEvent: (e: AppEvent) => void): Promise<void>` in `events.ts`; `export class IpcError extends Error { constructor(readonly dto: ErrorDto) }`; jede Ablehnung von `invoke` wird zu `IpcError` (ist der Ablehnungswert kein `ErrorDto`, entsteht `{ code: "internal", params: {}, action: null, detail: String(value) }`).

Verhalten `game_versions`: Liegt ein Cache vor (`VersionsCache::get`), kommt sofort die gefilterte Liste daraus (`from_cache: true`), und einmal je App-Lauf (`claim_refresh`) startet im Hintergrund `fetch_manifest`; unterscheidet sich das Ergebnis (`latest` oder Anzahl), sendet der Hub `DataChanged { scope: Versions }`. Scheitert die Aktualisierung, wird der Fehler geloggt (`eprintln!`, ohne Query-Teil der URL) und der Hub sendet `AppEvent::Notice { kind: VersionsRefreshOffline }` (Code `network` oder `offline`) bzw. `{ kind: VersionsRefreshFailed }` (jede andere Ursache); die Oberfläche zeigt daraufhin einen nicht blockierenden Hinweis (Toast, Task 3) und die Zeile unter der Versionswahl (Task 3 und 11), die gespeicherte Liste bleibt in Gebrauch. Ohne Cache wartet der Aufruf auf `fetch_manifest` (`from_cache: false`). Reihenfolge wie im Manifest (neueste zuerst).

Konfiguration:
- `tauri.conf.json`: `productName` "Hengst Client", `identifier` "io.github.clemensjl.hengst-client", `version` "0.1.0", `build.devUrl` `http://localhost:1420`, `build.frontendDist` `../dist`, `beforeDevCommand` `pnpm dev`, `beforeBuildCommand` `pnpm build`, `app.windows: []` (Fenster entsteht in `setup`), `app.withGlobalTauri: false`, `app.security.freezePrototype: true`, `app.security.csp`: `default-src 'self'; script-src 'self'; style-src 'self'; style-src-attr 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src ipc: http://ipc.localhost; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'`; `app.security.devCsp` wie `csp`, aber `connect-src ipc: http://ipc.localhost ws://localhost:1420` und `script-src 'self' 'unsafe-inline'` (Vite-Preamble von React-Refresh); `bundle.active: false` in 4a.
- `build.rs`: bindet `src/command_list.rs` per `include!` ein, legt zuerst `../dist/index.html` mit einem Satz "Front end not built: run pnpm build in apps/desktop" an, wenn `../dist` fehlt, und ruft dann `tauri_build::try_build(Attributes::new().app_manifest(AppManifest::new().commands(with_commands!(names))))`. Mechanismus dafür, dass `cargo test --workspace` und `cargo clippy --workspace` weder pnpm noch WebView2 brauchen: `tauri::generate_context!` verlangt beim Kompilieren nur, dass `frontendDist` existiert (das Platzhalterverzeichnis ist in `.gitignore`, ein echter `pnpm build` ersetzt es); `beforeBuildCommand` läuft nur unter `pnpm tauri build`/`dev`; `icons/icon.ico` ist eingecheckt; Tests benutzen `tauri::test::mock_builder` und starten kein Webview.
- `capabilities/main.json`: `identifier` "main", `windows` ["main"], `permissions`: die erzeugten `allow-<cmd>`-Permissions für jeden Eintrag in `COMMANDS` (Tauri bildet den Namen mit Bindestrichen statt Unterstrichen, z. B. `allow-app-info`; in Step 5 am erzeugten Manifest bestätigen) und keine Plugin- oder `core:`-Permission außer denen, die der Kanal nachweislich braucht (Step 5 ermittelt sie und trägt sie in die Konstante `EXTRA_PERMISSIONS` des Tests ein). Der Test `capability_allows_exactly_the_commands` hält das fest.
- `vite.config.ts`: Port 1420 `strictPort`, `server.watch.ignored: ["**/src-tauri/**"]`, Plugins `react()`, `tailwindcss()`, Alias `@` → `./src`, `test` mit `environment: "jsdom"`.
- `package.json` Skripte: `dev` (vite), `build` (`tsc -b && vite build`), `typecheck` (`tsc -b --noEmit`), `test` (`vitest run`), `format`/`format:check` (prettier), `tauri` (tauri), `e2e` (Task 12). Versionen exakt wie im Tech-Stack, übrige per `pnpm add` mit exakter Version (`save-exact`). `"packageManager": "pnpm@11.9.0"`, `"engines": { "node": ">=22.12.0" }`; `.nvmrc` enthält `22.19.0`.
- `ci.yml` (Änderung in diesem Task): Job `msrv` (`dtolnay/rust-toolchain@1.89`, `cargo test -p hengst-core -p hengst-cli`) und Job `desktop-frontend` (`windows-latest`, `pnpm/action-setup` liest `packageManager`, `actions/setup-node` mit `node-version-file: apps/desktop/.nvmrc`, dann `pnpm -C apps/desktop install --frozen-lockfile`, `typecheck`, `test`, `format:check`). Der bestehende Job `test` läuft mit `stable` und deckt `cargo test --workspace --all-features` samt `bindings_are_current` ab.
- shadcn: `pnpm dlx shadcn@4.23.0 init -t vite --base base` in `apps/desktop`; danach muss `components.json` und das erzeugte `button` Base UI importieren (`@base-ui-components/react` o. ä.). Erzeugt die CLI Radix-Importe, `--base` mit dem Wert wiederholen, den `pnpm dlx shadcn@4.23.0 init --help` für Base UI nennt.

- [ ] **Step 1: Failing Tests schreiben**

Kern (`manifest.rs`):
```rust
#[test]
fn load_cached_reads_what_fetch_manifest_saved_and_none_without_file() {
    // leerer Paths -> Ok(None); Datei mit gültigem Manifest -> Some(m) mit m.latest.release == "1.21.11"; "{" (abgeschnitten) -> Ok(None)
}

#[tokio::test]
async fn fetch_manifest_replaces_the_cache_through_a_temp_file() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(200).set_body_json(sample())).mount(&server).await;
    fetch_manifest(&reqwest::Client::new(), &paths, &server.uri()).await.unwrap();
    let names: Vec<String> = std::fs::read_dir(paths.manifest_file().parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names.len(), 1, "no temp file may stay behind: {names:?}");
    assert_eq!(load_cached(&paths).unwrap().unwrap().latest.release, "1.21.11");
}

#[tokio::test]
async fn load_cached_never_sees_a_partial_file_while_the_cache_is_rewritten() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(200).set_body_json(sample())).mount(&server).await;
    let client = reqwest::Client::new();
    fetch_manifest(&client, &paths, &server.uri()).await.unwrap();
    let reader_paths = paths.clone();
    let reader = tokio::task::spawn_blocking(move || {
        for _ in 0..300 {
            assert!(load_cached(&reader_paths).unwrap().is_some(), "a rewrite must never expose a truncated cache");
        }
    });
    for _ in 0..30 {
        fetch_manifest(&client, &paths, &server.uri()).await.unwrap();
    }
    reader.await.unwrap();
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
    // -> JSON mit "version" == env!("CARGO_PKG_VERSION") und "devBuild" == dev_build()
}
#[test]
fn dev_build_is_debug_or_dev_offline_or_e2e() {
    assert_eq!(
        hengst_desktop_lib::dev_build(),
        cfg!(debug_assertions) || cfg!(feature = "dev-offline") || cfg!(feature = "e2e")
    );
}
#[tokio::test]
async fn failed_background_refresh_is_logged_and_surfaced_as_a_notice() {
    // Cache mit 1.21.10 schreiben, Server antwortet auf den Abruf mit 200 und dem Body "{" (kein gültiges Manifest)
    let (state, events) = test_state_with_event_log(&server);
    let first = game_versions_impl(&state, VersionFilter { release: true, snapshot: false, beta: false, alpha: false }).await.unwrap();
    assert_eq!(ids(&first), ["1.21.10"]);
    wait_until(|| events.lock().unwrap().contains(&AppEvent::Notice { kind: NoticeKind::VersionsRefreshFailed })).await;
    let again = game_versions_impl(&state, VersionFilter { release: true, snapshot: false, beta: false, alpha: false }).await.unwrap();
    assert_eq!(ids(&again), ["1.21.10"]);
    assert!(events.lock().unwrap().iter().all(|e| !matches!(e, AppEvent::DataChanged { scope: DataScope::Versions })));
}
#[tokio::test]
async fn refresh_failing_because_the_server_is_unreachable_is_marked_offline() {
    // Cache mit 1.21.10 schreiben, Endpunkt auf einen geschlossenen Port
    let (state, events) = test_state_with_event_log_and_closed_port();
    let first = game_versions_impl(&state, VersionFilter { release: true, snapshot: false, beta: false, alpha: false }).await.unwrap();
    assert_eq!(ids(&first), ["1.21.10"]);
    wait_until(|| events.lock().unwrap().contains(&AppEvent::Notice { kind: NoticeKind::VersionsRefreshOffline })).await;
    assert!(events.lock().unwrap().iter().all(|e| !matches!(e, AppEvent::Notice { kind: NoticeKind::VersionsRefreshFailed })));
}
#[tokio::test]
async fn refresh_runs_once_per_app_run() {
    // Server zählt Abrufe: drei game_versions_impl-Aufrufe mit Cache -> genau ein Abruf von fetch_manifest
}
```

`src/window_state.rs`:
```rust
#[test]
fn window_state_round_trips_and_a_broken_file_counts_as_absent() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let s = WindowState { format: 1, width: 1120, height: 720, x: 40, y: 30, maximized: false };
    save(&paths, &s).unwrap();
    assert_eq!(load(&paths), Some(s));
    std::fs::write(paths.root().join("window.json"), "{").unwrap();
    assert_eq!(load(&paths), None);
}
#[test]
fn window_file_lives_in_the_data_directory() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    save(&paths, &WindowState { format: 1, width: 960, height: 600, x: 0, y: 0, maximized: false }).unwrap();
    assert!(dir.path().join("window.json").is_file());
}
#[test]
fn restore_is_skipped_with_a_size_argument_and_in_e2e() {
    assert!(should_restore(false, false));
    assert!(!should_restore(true, false));
    assert!(!should_restore(false, true));
}
#[test]
fn a_window_that_is_off_every_monitor_is_not_restored() {
    let monitors = [(0, 0, 1920, 1080)];
    let on = WindowState { format: 1, width: 1120, height: 720, x: 100, y: 100, maximized: false };
    let off = WindowState { x: 5000, ..on.clone() };
    assert!(fits_on_a_monitor(&on, &monitors));
    assert!(!fits_on_a_monitor(&off, &monitors));
}

// Arbeitsbereiche in logischen Pixeln; Rahmen = Titelleiste bei 125 %, die Ränder zählen nicht
const LARGE: Size = Size { width: 1920, height: 1040 };   // 1920 × 1080 minus Taskleiste
const LAPTOP: Size = Size { width: 1093, height: 580 };   // 1366 × 768 bei 125 % (DESIGN.md 7.1)
const TINY: Size = Size { width: 800, height: 500 };
const FRAME: Size = Size { width: 0, height: 31 };   // angenommener Rahmen der Tests: nur die Titelleiste, 31 logische Pixel

#[test]
fn a_large_screen_keeps_the_nominal_sizes() {
    assert_eq!(min_size(LARGE, FRAME), MIN_SIZE);
    assert_eq!(default_size(LARGE, FRAME), DEFAULT_SIZE);
    assert_eq!(clamp_size(Size { width: 1920, height: 1080 }, LARGE, FRAME), Size { width: 1920, height: 1009 });
    assert_eq!(clamp_size(Size { width: 500, height: 300 }, LARGE, FRAME), MIN_SIZE);
}
#[test]
fn a_1366_by_768_screen_at_125_percent_still_fits_the_window() {
    assert_eq!(min_size(LAPTOP, FRAME), Size { width: 960, height: 549 });
    // DESIGN.md 7.1: (Arbeitsbereich minus Rahmen) * 90 % wären 983 × 494, je Achse max mit der Mindestgröße ergibt 983 × 549
    assert_eq!(default_size(LAPTOP, FRAME), Size { width: 983, height: 549 });
    assert_eq!(clamp_size(Size { width: 1120, height: 720 }, LAPTOP, FRAME), Size { width: 1093, height: 549 });
    assert_eq!(clamp_size(Size { width: 960, height: 600 }, LAPTOP, FRAME), Size { width: 960, height: 549 });
}
#[test]
fn a_tiny_work_area_wins_over_the_nominal_minimum() {
    assert_eq!(min_size(TINY, FRAME), Size { width: 800, height: 469 });
    assert_eq!(default_size(TINY, FRAME), Size { width: 800, height: 469 });
    assert_eq!(clamp_size(Size { width: 1920, height: 1080 }, TINY, FRAME), Size { width: 800, height: 469 });
}
#[test]
fn startup_size_prefers_the_argument_then_the_saved_size_then_the_default() {
    let arg = Some(Size { width: 1920, height: 1080 });
    let saved = Some(Size { width: 1000, height: 650 });
    // --window-size gewinnt, wird geklemmt, die gemerkte Größe bleibt unbeachtet (restore ist dann false)
    assert_eq!(startup_size(arg, saved, false, LAPTOP, FRAME), Size { width: 1093, height: 549 });
    assert_eq!(startup_size(None, saved, true, LARGE, FRAME), Size { width: 1000, height: 650 });
    // e2e: nichts wiederherstellen
    assert_eq!(startup_size(None, saved, false, LARGE, FRAME), DEFAULT_SIZE);
    // eine auf großem Bildschirm gemerkte Größe wird auf dem kleinen geklemmt
    assert_eq!(startup_size(None, Some(Size { width: 1800, height: 1000 }), true, LAPTOP, FRAME), Size { width: 1093, height: 549 });
    assert_eq!(startup_size(None, None, true, LAPTOP, FRAME), Size { width: 983, height: 549 });
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

`tests/capabilities.rs`:
```rust
use std::collections::BTreeSet;

const EXTRA_PERMISSIONS: &[&str] = &[]; // Step 5 trägt hier die nachweislich nötigen core:-Permissions ein

fn allow_name(cmd: &str) -> String {
    format!("allow-{}", cmd.replace('_', "-"))
}

#[test]
fn capability_allows_exactly_the_commands() {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/capabilities/main.json")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    let granted: BTreeSet<String> = json["permissions"].as_array().unwrap().iter().map(|p| p.as_str().unwrap().to_string()).collect();
    let expected: BTreeSet<String> = hengst_desktop_lib::COMMANDS
        .iter()
        .map(|c| allow_name(c))
        .chain(EXTRA_PERMISSIONS.iter().map(|p| p.to_string()))
        .collect();
    assert_eq!(granted, expected, "every command needs one allow entry and no entry may name a missing command");
    assert!(granted.iter().all(|p| !p.starts_with("opener:")), "opener is used from Rust only");
}

#[test]
fn command_names_are_unique() {
    let set: BTreeSet<&&str> = hengst_desktop_lib::COMMANDS.iter().collect();
    assert_eq!(set.len(), hengst_desktop_lib::COMMANDS.len());
}
```

`tests/config.rs`:
```rust
#[test]
fn csp_forbids_unsafe_inline_for_scripts_and_style_elements() {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tauri.conf.json")).unwrap();
    let conf: serde_json::Value = serde_json::from_str(&text).unwrap();
    let csp = conf["app"]["security"]["csp"].as_str().unwrap();
    let directive = |name: &str| csp.split(';').map(str::trim).find(|d| d.starts_with(&format!("{name} "))).unwrap_or("").to_string();
    assert!(!directive("script-src").contains("'unsafe-inline'"));
    assert!(!directive("script-src").contains("'unsafe-eval'"));
    assert!(!directive("style-src").contains("'unsafe-inline'"));
    assert!(csp.contains("connect-src ipc: http://ipc.localhost"));
    assert_eq!(conf["app"]["security"]["freezePrototype"], true);
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

Run: `cargo test -p hengst-core manifest` und `cargo test -p hengst-desktop`, `pnpm -C apps/desktop test`
Expected: FAIL (Funktionen, Crate, Client fehlen)

- [ ] **Step 3: Implementieren**

Gerüst wie in Interfaces und Konfiguration. `startup.rs`: `pub fn run()` registriert in dieser Reihenfolge `tauri_plugin_single_instance::init` (Callback: Fenster `main` zeigen, entminimieren, fokussieren), `tauri_plugin_opener`, `.manage(AppState::new(...))`, `.invoke_handler(with_commands!(handler))`, `setup` erzeugt das Fenster `main` (Titel "Hengst Client", plus " (Development build)" bei `dev_build()`, Mindestgröße `window_state::min_size` und Startgröße `window_state::startup_size` (`--window-size`, sonst der gespeicherte Zustand nach `should_restore`, sonst `default_size`; alles an den Arbeitsbereich geklemmt, siehe Interfaces), Hintergrund `--bg` hell, bis Task 3 das Theme liest) und speichert den Fensterzustand beim Schließen. Es gibt kein Plugin `window-state`. Debug-Argumente `--home`, `--window-size`, `--e2e-base-url` werden in `startup.rs` in `StartupArgs` gelesen (nur `cfg(debug_assertions)`). `AboutPanel` zeigt Version, Datenordner, Anzahl Versionen und neueste Release/Snapshot mit Lade-, Leer- und Fehlerzustand und abonniert `DataChanged(Versions)` zum Nachladen. Bindings erzeugen mit `HENGST_UPDATE_BINDINGS=1 cargo test -p hengst-desktop --test bindings`. Die CI-Jobs `msrv` und `desktop-frontend` kommen in diesem Task in `ci.yml`.

- [ ] **Step 4: Tests grün**

Run: `cargo test --workspace --all-features` und `pnpm -C apps/desktop typecheck && pnpm -C apps/desktop test`
Expected: PASS

- [ ] **Step 5: App starten, Capability und CSP prüfen, Sichtprüfung**

Run: `pnpm -C apps/desktop tauri build --debug --no-bundle` und dann `target/debug/hengst-desktop.exe --home %TEMP%\hengst-4a-t2 --window-size 960x600`
Expected (Sichtprüfung mit agentfenster, `open_app` auf die Exe mit den Argumenten, `read_desktop`, `list_windows`): Das Fenster öffnet in der angegebenen Innengröße (Außenmaß aus `list_windows` entspricht 960 × 600 plus Rahmen und Titelleiste, höchstens 40 px Abweichung; ein zweiter Start mit `--window-size 1120x720` ergibt das größere Maß, und kein Lauf liest oder schreibt `window.json`). Ist der Arbeitsbereich des Rechners kleiner als die Angabe, gilt die geklemmte Größe nach `clamp_size`; der Beleg nennt Arbeitsbereich und tatsächliche Größe, und das AboutPanel zeigt echte Daten (Versionszahl der App, Datenordner `%TEMP%\hengst-4a-t2`, Anzahl der Versionen aus dem Mojang-Manifest und die neueste Release-Id), nicht Platzhalter. Panel-Text per `read_desktop` festhalten, Screenshot (nur vom Agent-Desktop) unter `evidence/task-2/`. Zweiter Start der Exe fokussiert das erste Fenster, kein zweites Fenster (`list_windows`). Ohne `--window-size`: Fenster verschieben, schließen, erneut starten, es erscheint an der gemerkten Stelle, und `%TEMP%\hengst-4a-t2\window.json` existiert. Für den Kanal nötige `core:`-Permission: wenn `subscribe_events` ohne Zusatz abgelehnt wird, die in der Fehlermeldung genannte Permission einzeln in `capabilities/main.json` und in `EXTRA_PERMISSIONS` (`tests/capabilities.rs`) eintragen und hier notieren; das Permission-Namensschema (`allow-app-info`) am erzeugten Manifest unter `src-tauri/gen/schemas` bestätigen. In der Web-Konsole (Debug-Build, Devtools) keine CSP-Verletzung und `Object.isFrozen(Object.prototype)` ist `true`; meldet Tauris Nonce-Ergänzung einen Konflikt mit `style-src-attr`, `dangerousDisableAssetCspModification: ["style-src-attr"]` setzen (der Test `csp_forbids_unsafe_inline_for_scripts_and_style_elements` bleibt grün). Ein Aufruf `invoke("plugin:opener|open_url", ...)` aus der Konsole wird abgelehnt.

- [ ] **Step 6: Lint, Format, Commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace --all-features && pnpm -C apps/desktop format:check`
```bash
git add Cargo.toml Cargo.lock .gitignore .github/workflows/ci.yml crates/hengst-core/src/manifest.rs apps/desktop/package.json apps/desktop/.nvmrc apps/desktop/pnpm-lock.yaml apps/desktop/index.html apps/desktop/vite.config.ts apps/desktop/tsconfig.json apps/desktop/tsconfig.app.json apps/desktop/tsconfig.node.json apps/desktop/.prettierrc apps/desktop/components.json apps/desktop/src apps/desktop/src-tauri
git commit -m "feat(desktop): walking skeleton with versions command and event channel"
```

---

### Task 3: App-Rahmen (Layout, Theme, Sprache, Einstellungen, Zeichen)

**Files:**
- Create: `apps/desktop/src-tauri/src/settings.rs`, `src/dto/settings.rs`, `src/commands/settings.rs`
- Create: `apps/desktop/src/app/{Shell.tsx,Sidebar.tsx,views.ts}`, `src/stores/app.ts`, `src/theme/theme.ts`, `src/i18n/{index.ts,de.json,en.json}`, `src/features/settings/SettingsView.tsx`, `brand/mark.svg`, `brand/README.md`, `src/styles/tokens.test.ts`, `src/styles/accent.test.ts`, `src/i18n/i18n.test.ts`, `src/theme/theme.test.ts`, `src/theme/inputMode.ts`, `src/theme/inputMode.test.ts`, `src/components/Segments.tsx`, `src/app/Sidebar.test.tsx`, `src/lib/webviewDefaults.ts`, `src/lib/webviewDefaults.test.ts`; shadcn-Komponenten `button`, `radio-group`, `sonner`, `skeleton`, `tooltip`, `separator`, `collapsible` per `pnpm dlx shadcn@4.23.0 add` (erzeugt unter `src/components/ui/`)
- Modify: `src/styles/index.css` (Tokens aus DESIGN.md), `src/features/about/AboutPanel.tsx` (Aufbau nach DESIGN.md 11.7), `src/app/App.tsx`, `src/main.tsx`, `src/ipc/client.ts` (`settingsGet`, `settingsUpdate`), `src-tauri/capabilities/main.json` (`allow-settings-get`, `allow-settings-update`), `src-tauri/src/{state.rs,startup.rs,command_list.rs,dto/mod.rs}`, `src-tauri/icons/` (aus `brand/mark.svg`), `src/ipc/bindings/` (erzeugt), `apps/desktop/package.json`, `pnpm-lock.yaml`, `components.json`

**Interfaces:**
- Consumes: `AppState`, `EventHub`, `ErrorDto` (Task 2), Tokens (Task 1).
- Produces (Rust):
  - `settings.rs`: `pub const SETTINGS_FORMAT: u32 = 1;` `#[serde(rename_all = "camelCase")] pub struct Settings { pub format: u32, pub language: Option<Language>, pub theme: Theme, pub memory_mb: u32, pub java_override: Option<PathBuf>, pub minimize_on_launch: bool, pub selected_instance: Option<String>, pub dev_account: bool }`, Standard: `language None`, `theme System`, `memory_mb 2048`, `java_override None`, `minimize_on_launch true`, `selected_instance None`, `dev_account false`. `Language`: `De`, `En` (serde `"de"`, `"en"`); `Theme`: `System`, `Light`, `Dark`.
  - `pub fn load_settings(paths: &Paths, now: u64) -> (Settings, Option<SettingsNotice>)` (fehlend: Standard; kaputt oder `format` > 1: Datei nach `settings.json.broken-<now>` verschieben, Standard, `SettingsNotice::Reset`), `pub fn save_settings(paths: &Paths, s: &Settings) -> hengst_core::Result<()>` (Temp + Rename).
  - `pub enum Patch<T> { Keep, Set(T), Clear }` (`Debug, Clone, PartialEq`, `Default` ist `Keep`; `#[serde(tag = "op", content = "value", rename_all = "camelCase")]`, JSON `{"op":"keep"}`, `{"op":"set","value":...}`, `{"op":"clear"}`; ts-rs-Typ im Frontend) ersetzt jedes verschachtelte `Option<Option<_>>`: "setzen", "löschen" und "unverändert lassen" sind drei verschiedene Fälle.
  - `#[serde(deny_unknown_fields, rename_all = "camelCase")] pub struct SettingsPatch { #[serde(default)] pub language: Patch<Language>, #[serde(default)] pub theme: Option<Theme>, #[serde(default)] pub memory_mb: Option<u32>, #[serde(default)] pub java_override: Patch<String>, #[serde(default)] pub minimize_on_launch: Option<bool> }` (`Patch::default()` ist `Keep`). Der Patch enthält nur Felder, die ein Nutzer in den Einstellungen ändern kann; `dev_account`, `selected_instance`, `format` und der Datenordner fehlen absichtlich (unbekannte Felder werden abgelehnt). `SettingsDto` (wie `Settings` ohne `format`, Pfade als String, dazu `notice: Option<SettingsNotice>`).
  - Prüfung in Rust vor dem Speichern, Fehler `ErrorCode::InvalidInput` mit `params.field`: `memory_mb` in 512..=65536 (`"memoryMb"`); `java_override` `Set(pfad)`: der Pfad muss absolut sein, als Datei existieren (kein Ordner) und `java.exe` oder `javaw.exe` heißen (ohne Rücksicht auf Groß-/Kleinschreibung), sonst `"javaOverride"` (in 4b Task 20 kommt der Pfad aus einem Rust-Dialog; die Prüfung gilt trotzdem für jeden Weg); `Clear` löscht ihn.
  - `pub struct SettingsHandle` (`Clone`, intern `Arc`): `fn new(paths: Paths, settings: Settings) -> SettingsHandle`, `fn get(&self) -> Settings` (Kopie), `fn update(&self, f: impl FnOnce(&mut Settings) -> hengst_core::Result<()>) -> hengst_core::Result<Settings>` (nimmt die Sperre, ändert eine Kopie, ruft `save_settings`, übernimmt die Kopie erst nach erfolgreichem Speichern, gibt sie zurück; ein Fehler ändert weder Datei noch Speicher). Alle Schreiber (`settings_update`, Konten und Instanzen in Task 7 und 10) gehen durch `update`, es gibt keinen zweiten Schreibweg.
  - Commands `settings_get() -> Result<SettingsDto, ErrorDto>` (nimmt `AppState.pending_notice` mit `Option::take`: die Meldung `SettingsNotice::Reset` kommt in genau einer Antwort, jede weitere hat `notice: None`, auch das Nachladen nach `DataChanged(Settings)`), `settings_update(patch: SettingsPatch) -> Result<SettingsDto, ErrorDto>` (prüft, speichert über `SettingsHandle::update`, sendet `DataChanged(Settings)`). `ErrorCode::InvalidInput` kommt hier dazu.
  - `AppState.settings: SettingsHandle` und `AppState.pending_notice: Mutex<Option<SettingsNotice>>`; `AppState::new` ruft `load_settings(&paths, unix_now())` auf und befüllt beides; `startup.rs` erzeugt das Fenster mit `theme` (`Light`/`Dark`, bei `System` keins) und Hintergrund aus `--bg` des aufgelösten Themes.
- Produces (Frontend):
  - `stores/app.ts`: `type Resource<T> = { status: "idle" | "loading" | "ready" | "error"; data?: T; error?: ErrorDto }`; Store `useApp` mit `view: View`, `settings: Resource<SettingsDto>`, `versions: Resource<GameVersionsDto>`, Aktionen `setView(v)`, `loadSettings()`, `updateSettings(p: SettingsPatch)`, `loadVersions(filter)`; `versionsStale: null | "offline" | "other"`, `startEventLoop()` abonniert `subscribeEvents`, lädt bei `DataChanged` die betroffene Ressource neu und zeigt bei `Notice { kind: "versionsRefreshFailed" | "versionsRefreshOffline" }` einen Toast ("Die Versionsliste konnte nicht aktualisiert werden. Die gespeicherte Liste wird verwendet.") und setzt `versionsStale` auf `"other"` bzw. `"offline"` (die Zeile unter der Versionswahl, DESIGN.md 10.5, Task 11); `DataChanged(Versions)` setzt `versionsStale` zurück. Der Toast zur beschädigten Einstellungsdatei kommt nur aus der ersten `settings_get`-Antwort mit `notice` (in `main.tsx`), nie aus dem Nachladen.
  - `views.ts`: `type View = "instances" | "settings"` (4b ergänzt `"play"`, `"log"`); Startansicht in 4a `instances`.
  - `theme.ts`: `resolveTheme(setting: "system" | "light" | "dark", prefersDark: boolean): "light" | "dark"`, `applyTheme(resolved)` setzt/entfernt `.dark` an `<html>`; bei `system` Listener auf `matchMedia("(prefers-color-scheme: dark)")`. Die native Titelleiste folgt dem Theme (DESIGN.md 7.1): beim Erzeugen über `startup.rs`, zur Laufzeit über den Fenster-Theme-Setter von Tauri (Name in 2.12 prüfen), den `settings_update` nach einer Theme-Änderung aufruft; bei `System` wird das Fenster-Theme auf "folgt dem System" zurückgesetzt. Reine Funktion `window_theme(setting: Theme) -> Option<tauri::Theme>` (`System` gleich `None`). `inputMode.ts`: setzt `data-input="pointer"` bzw. `"keyboard"` an `<html>` (Pointer-Down bzw. Keydown); mit `keyboard` entfallen alle Übergänge (DESIGN.md Abschnitt 8 und Anhang A).
  - `i18n/index.ts`: `initI18n(language: "de" | "en" | null, navigatorLanguage: string)`, `pickLanguage(setting, navigatorLanguage): "de" | "en"`; Namensräume flach mit Präfixen `nav.`, `settings.`, `about.`, `errors.`, `accounts.`, `instances.`, `common.`.
  - `Segments.tsx` (DESIGN.md 10.6): eine Segment-Komponente auf shadcn `radio-group` für Sprache, Design (hier) und Loader (Task 11), ein Tab-Stopp je Gruppe, Pfeiltasten wählen.
  - `SettingsView` in 4a nach DESIGN.md 11.7 (eine Spalte, Abschnitte, Speichern beim Wählen, Fehler inline mit Rückfall, Skeletons beim Laden): Sprache (Segmente System/Deutsch/English), Design (Segmente System/Hell/Dunkel), Datenordner (Pfadanzeige, "Ordner öffnen" kommt in Task 11 über Task 10), "Über Hengst Client" mit AboutPanel aus Task 2 (Aufbau nach 11.7, ohne die 4b-Teile), Hinweis "Not an official Minecraft product." in beiden Sprachen gleich (Rechtsformel, nicht übersetzt).

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
#[test]
fn notice_is_delivered_once() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("settings.json"), "{").unwrap();
    let state = AppState::new(Paths::new(dir.path()), test_endpoints());
    let first = settings_get_impl(&state).unwrap();
    assert_eq!(first.notice, Some(SettingsNotice::Reset));
    assert_eq!(settings_get_impl(&state).unwrap().notice, None);
    assert_eq!(settings_get_impl(&state).unwrap().notice, None);
}
#[test]
fn patch_distinguishes_keep_set_and_clear() {
    let keep: SettingsPatch = serde_json::from_str("{}").unwrap();
    assert_eq!(keep.language, Patch::Keep);
    let set: SettingsPatch = serde_json::from_str(r#"{"language":{"op":"set","value":"de"}}"#).unwrap();
    assert_eq!(set.language, Patch::Set(Language::De));
    let clear: SettingsPatch = serde_json::from_str(r#"{"language":{"op":"clear"}}"#).unwrap();
    assert_eq!(clear.language, Patch::Clear);
    // gespeichert: Clear setzt language auf None, Keep lässt "de" stehen, Set("de") dann Keep -> weiter "de"
}
#[test]
fn patch_cannot_touch_dev_account_or_selected_instance() {
    assert!(serde_json::from_str::<SettingsPatch>(r#"{"devAccount":true}"#).is_err());
    assert!(serde_json::from_str::<SettingsPatch>(r#"{"selectedInstance":"x"}"#).is_err());
    assert!(serde_json::from_str::<SettingsPatch>(r#"{"format":2}"#).is_err());
}
#[test]
fn java_override_must_be_an_existing_java_file() {
    let dir = tempfile::tempdir().unwrap();
    let java = dir.path().join("java.exe");
    let javaw = dir.path().join("JAVAW.EXE");
    let other = dir.path().join("notepad.exe");
    let folder = dir.path().join("java.exe.d");
    for f in [&java, &javaw, &other] { std::fs::write(f, b"").unwrap(); }
    std::fs::create_dir(&folder).unwrap();
    let state = state_in(dir.path());
    let set = |p: &std::path::Path| settings_update_impl(&state, SettingsPatch { java_override: Patch::Set(p.display().to_string()), ..Default::default() });
    assert!(set(&java).is_ok());
    assert!(set(&javaw).is_ok());
    for bad in [other.clone(), folder.clone(), dir.path().join("missing").join("java.exe"), std::path::PathBuf::from("java.exe")] {
        let e = set(&bad).unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidInput);
        assert_eq!(e.params["field"], "javaOverride");
    }
    let cleared = settings_update_impl(&state, SettingsPatch { java_override: Patch::Clear, ..Default::default() }).unwrap();
    assert_eq!(cleared.java_override, None);
}
#[test]
fn a_failed_save_changes_neither_file_nor_memory() {
    // settings.json als Ordner anlegen (Speichern schlägt fehl) -> settings_update_impl Err; state.settings.get().theme unverändert
}
#[test]
fn window_theme_follows_the_setting() {
    assert_eq!(window_theme(Theme::System), None);
    assert_eq!(window_theme(Theme::Light), Some(tauri::Theme::Light));
    assert_eq!(window_theme(Theme::Dark), Some(tauri::Theme::Dark));
}
```

Frontend:
```ts
// tokens.test.ts: liest src/styles/index.css und DESIGN.md; die CSS-Werte sind die geprüften, DESIGN.md nennt die Anforderungen
it("index.css holds the token values of DESIGN.md 2.1", ...);   // Zeilen der Tabelle 2.1 (`--name` | hell | dunkel) gegen :root und .dark, Hex und rgb() zeichengleich
it.each(contrastRowsOf("DESIGN.md", "2.3"))("%s on %s meets its requirement in both themes", ...);
// contrastRowsOf: liest die Tabelle 2.3, nimmt je Zeile das erste `--token` der Spalten Foreground und Background und die Spalte Requirement (4.5 oder 3);
// Zeilen mit "exempt" und Zeilen ohne Token (Markenzeichen) entfallen; mindestens 40 Zeilen gelesen, sonst schlägt der Test fehl.
// Berechnung aus den CSS-Werten, kein Hex im Test; so bleiben Plan, Tabelle und CSS nicht stehen, wenn DESIGN.md ein Token ändert.
const interactiveBorders = ["--border-control"];   // einzige Begrenzung eines bedienbaren Elements (Input, Select- und Combobox-Auslöser, Checkbox, Segment, Secondary-Button, Switch aus, Konto-Chip)
const decorativeBorders = ["--border", "--border-strong"];   // Trennlinien und Panelränder ohne Information, ausgenommen
it.each(interactiveBorders.flatMap((b) => ["--bg", "--surface", "--surface-raised"].map((s) => [b, s])))("%s against %s >= 3 in both themes", ...);
it("decorative borders are exempt, but shadcn --input points at --border-control", () => {
  for (const theme of [":root", ".dark"]) {
    expect(cssVar(theme, "--input")).toBe(cssVar(theme, "--border-control"));
    for (const b of decorativeBorders) expect(cssVar(theme, "--input")).not.toBe(cssVar(theme, b));
  }
});
it("each theme sets its color-scheme", () => {
  expect(cssProp(":root", "color-scheme")).toBe("light");
  expect(cssProp(".dark", "color-scheme")).toBe("dark");
});
it("shadcn variables are declared on :root and point at our tokens (DESIGN.md 2.4)", () => {
  // je Zeile der Tabelle 2.4: :root enthält `--background: var(--bg)` usw.; --accent bleibt unser Markentoken (kein shadcn-Hover-Wert),
  // `--accent-foreground` ist deklariert, `--color-accent` in @theme inline ist `var(--hover)`, --input ist var(--border-control), --radius ist var(--radius-panel)
});
it("@theme inline carries the radius, shadow, font, ease and breakpoint scale of DESIGN.md 2.4", () => {
  // --radius-sm bis --radius-4xl, --shadow-xs bis --shadow-xl (xs und sm gleich none), --font-sans, --font-display, --font-mono, --ease-out, --breakpoint-wide
});
it("the generated components in src/components/ui pass the gate of DESIGN.md 2.4", () => {
  // liest alle Dateien unter src/components/ui/**; jedes var(--x) ist in index.css deklariert oder eine dokumentierte Base-UI-Laufzeitvariable (--transform-origin, ...);
  // nicht vorhanden: "ring-ring/", "outline-ring/", "dark:", "shadow-xs", "shadow-sm", "rounded-2xl", "rounded-3xl", "rounded-4xl", "var(--accent)";
  // `primary`-Utilities (bg-, text-, border-, ring-, accent-, fill-, stroke-) nur in button.tsx und nur an der Variante primary; Checkbox, Switch, Progress, Radio, Badge, Tooltip und die Variante link dürfen keine tragen (sie würden im Akzent erscheinen, DESIGN.md 2.2 und 2.5); button.tsx hat genau fünf Varianten und drei Größen, Standardvariante secondary
});
// accent.test.ts: der Akzent ist auf drei Verwendungen beschränkt (DESIGN.md 2.2)
it("accent is used only for the primary action, the active nav icon and the focus ring", () => {
  // scannt src/**/*.{ts,tsx} ohne src/components/ui/, ohne *.test.*, ohne src/styles/:
  // 1. verboten außerhalb von src/components/ui/: bg-accent, text-accent, border-accent (shadcns Hover-Rolle, gemeint wäre die Marke)
  // 2. bg-primary, text-primary, border-primary, ring-primary, var(--accent), accent-hover, accent-pressed kommen nur in der Allowlist vor:
  //    Hauptaktion = <Button variant="primary"> (kein eigenes bg-primary), aktives Nav-Symbol = text-primary in src/app/Sidebar.tsx, Fokusring = Wert --focus aus index.css
});
// i18n.test.ts
it("de and en have the same keys", ...)          // flache Schlüsselmengen gleich
it("no translation is empty", ...)
it("interpolation params match", ...)            // {{x}} je Schlüssel in de und en gleich
it("picks German for de-AT and English otherwise", () => { expect(pickLanguage(null, "de-AT")).toBe("de"); expect(pickLanguage(null, "fr-FR")).toBe("en"); expect(pickLanguage("en", "de-DE")).toBe("en"); });
it("en.json uses US spelling", () => { /* kein Treffer für /\b(colour|behaviour|centre|licence|organis\w*|cancelled|favourite|grey|catalogue|programme|initialis\w*|recognis\w*|customis\w*)\b/i in en.json */ });
it("no string has an exclamation mark or an emoji", ...);   // DESIGN.md 12
// theme.test.ts
it("system follows prefersDark, explicit wins", () => { expect(resolveTheme("system", true)).toBe("dark"); expect(resolveTheme("light", true)).toBe("light"); });
// inputMode.test.ts
it("pointer-down sets data-input to pointer and keydown to keyboard", ...);
// Sidebar.test.tsx
it("the nav is a landmark with a label, exactly one item has aria-current page, every item has an accessible name", ...);   // Namen bleiben im Elementbaum, auch wenn die Leiste unter 1200 px nur Icons zeigt
// webviewDefaults.test.ts (DESIGN.md Anhang A: kein Standard-Kontextmenü außer in Texteingaben und im Log, in Release-Builds)
it("context menu is allowed only in text inputs and the log area", () => {
  // allowsNativeContextMenu(<input type="text">) und (<textarea>) und ein Element im Log-Bereich ([data-native-menu]) -> true; ein <div>, <button>, <a> -> false
});
```

- [ ] **Step 2: Tests laufen lassen, sie scheitern**

Run: `cargo test -p hengst-desktop settings` und `pnpm -C apps/desktop test`
Expected: FAIL

- [ ] **Step 3: Zustandsliste (produkt-regelwerk BAUEN Schritt 2)** für Shell und Einstellungen in den Task-Bericht: Hauptaktion je Ansicht, Laden/Leer/Fehler, Mutationen (Einstellung speichern: sofort beim Wählen, Fehler inline, Rückfall auf alten Wert), Extremwerte (längster deutscher Text "Einstellungen" in der 64-px-Leiste nur als Tooltip).

- [ ] **Step 4: Implementieren**

Tokens (Werte aus `DESIGN.md` 2.1, nicht aus diesem Plan) als CSS-Variablen in `:root` und `.dark`, dazu `color-scheme: light` bzw. `dark`, Abstände, Radien, Dauern und `--ease-out` aus den Abschnitten 4, 5 und 8, Dark-Variante per `@custom-variant dark`. Zuordnung auf shadcn nach DESIGN.md 2.4: die shadcn-Variablen (`--background`, `--primary`, `--input` auf `--border-control` und nie auf `--border` oder `--border-strong` usw.) werden auf `:root` deklariert und zeigen auf unsere Tokens (`--accent` bleibt unser Markentoken, shadcns Hover-Rolle gibt es nur als Utility `--color-accent: var(--hover)`), `@theme inline` bildet Farben, Radius-, Schatten- und Schriftskala, `--ease-out`, die elf Schriftgrößen-Variablen (`--text-display-lg` bis `--text-code`, DESIGN.md 2.5) und die Breakpoints `--breakpoint-medium` (880 px), `--breakpoint-wide` (1200 px) und `--breakpoint-full` (1280 px) ab; Tailwinds Standard-Breakpoints `sm` bis `2xl` entfallen per `--breakpoint-*: initial`. Die Farbtokens `--text`, `--text-muted`, `--text-disabled` liegen auf `:root`, nie in `@theme`. `button.tsx` behält genau die Varianten `primary`, `secondary`, `ghost`, `destructive`, `link` und die Größen `default` 40, `icon` 40 × 40, `lg` 56 (shadcn `default` wird `primary`, `outline` wird `secondary` mit `--border-control`, shadcns eigenes `secondary` entfällt); Standardvariante ist `secondary`, damit ein Knopf ohne Angabe nie ein zweiter Akzentknopf wird (DESIGN.md 2.5) Die Pflichten dieses Tasks aus DESIGN.md 2.4: (1) jede von `shadcn add` erzeugte Komponente gegen die dort genannten Standardwerte prüfen und patchen (vor allem Checkbox, Switch, Progress, Radio, Badge, Tooltip und die Variante `link`: `*-primary`-Utilities ersetzen durch `--text`, `--bg`, `--border-strong` usw. nach DESIGN.md 10.8, 10.12, 10.14; Fokusring und Rand, `hover:bg-primary/90`, `hover:bg-destructive/90`, `dark:bg-input/30`, `text-white` am Destructive-Knopf, `rounded-md` an schwebenden Ebenen, `animate-pulse` am Skeleton, Sonner-Variablen); (2) die Gate-Prüfung über `src/components/ui/**` in `tokens.test.ts` (siehe Tests); (3) die Akzent-Regel als Test `accent.test.ts`. Zusätzlich aus DESIGN.md Anhang A: Standard-Kontextmenü und Browser-Tastenkürzel der WebView sind in Release-Builds aus (kein Kontextmenü außer in Texteingaben und im Log-Bereich, F5, Strg+R, Strg+P, F12 und die Suchleiste gesperrt; Strg+F gehört später dem Log), die Zoom-Tasten (Strg+Plus, Strg+Minus, Strg+0) bleiben an (DESIGN.md 13); die Optionsnamen in Tauri 2.12 und WebView2 vor dem Einbau prüfen (`src/lib/webviewDefaults.ts` für die Seite, `startup.rs` für die WebView-Einstellungen). Shell nach DESIGN.md 7.2 und 7.3 (Seitenleiste mit Zeichen oben, Navigation als `<nav aria-label>` mit Buttons und `aria-current="page"`, Kopfzeile mit Seitentitel als `<h1>`, rechts Platz für den Konto-Chip ab Task 8, Kopfzeilenlinie per IntersectionObserver, Seitenleiste ab Fensterbreite 1200 (`wide:`) breit und darunter als Icon-Leiste, schmales Layout unter 880 CSS-Pixeln, einzeiliger Log-Werkzeugstreifen ab 1280). `brand/README.md` (Englisch): Zeichen ersetzen = `brand/mark.svg` tauschen, `pnpm tauri icon brand/mark.svg` ausführen, nichts sonst; das Frontend importiert das Zeichen nur aus `brand/mark.svg`. `main.tsx` lädt zuerst `settings_get`, dann i18n und Theme, dann rendert es (Ladefläche in `--bg`, kein Text-Aufblitzen; Zustände von Start und Erststart nach DESIGN.md 11.0). Notice `Reset` zeigt einmal einen Sonner-Toast "Einstellungen waren beschädigt und wurden zurückgesetzt" (aus der ersten `settings_get`-Antwort; Sonner und `radio-group` werden in diesem Task per shadcn hinzugefügt).

- [ ] **Step 5: Tests grün**

Run: `cargo test --workspace --all-features` und `pnpm -C apps/desktop typecheck && pnpm -C apps/desktop test`
Expected: PASS

- [ ] **Step 6: Sichtprüfung**

Run: `pnpm -C apps/desktop tauri build --debug --no-bundle`, Start mit `--home %TEMP%\hengst-4a-t3 --window-size 960x600`, dann `--window-size 1199x700`, `--window-size 1200x700` und `--window-size 1920x1080` (Maße geklemmt, siehe Task 2). Danach ein Release-Build (`pnpm -C apps/desktop tauri build --no-bundle`) ohne Argumente für die WebView-Vorgaben.
Expected (agentfenster `open_app`, `read_desktop`, `press_key tab`): Leiste bei 960 und 1199 px als Icon-Leiste mit Namen im Elementbaum und Tooltip, ab 1200 px mit Text; WebView-Zoom 150 % bei 960 × 600 (Strg+Plus): schmales Layout, nichts abgeschnitten, kein waagerechtes Scrollen; die Titelleiste folgt dem Theme sofort nach dem Wechsel in den Einstellungen (Elementbaum und Screenshot der Titelleiste); im Release-Build kein Standard-Kontextmenü auf der Seitenleiste, F5, Strg+R, Strg+P und F12 bleiben ohne Wirkung, Strg+Plus zoomt; Wechsel Hell/Dunkel und Deutsch/Englisch in den Einstellungen wirkt sofort und überlebt einen Neustart; Fokusring an jedem Tab-Schritt sichtbar, Reihenfolge Leiste → Kopfzeile → Inhalt. Kaputte `settings.json` vorab in den Home-Ordner legen: Toast erscheint einmal. Vier Screenshots (hell/dunkel × de/en) je Größe nach `evidence/task-3/`. Gegenprüfung C1 bis C27 (C8, C12, C14, C16, C18 sind hier relevant) mit Ergebnis im Bericht. **CSP- und Prototyp-Prüfung** (neu: shadcn-Basis mit Base UI, Sonner, RadioGroup, Tooltip): Web-Konsole beim Durchklicken lesen, keine CSP-Meldung, `Object.isFrozen(Object.prototype)` ist `true`; Sonner fügt zur Laufzeit ein `<style>`-Element ein: erst prüfen, ob die installierte Version eine Nonce-Option kennt, sonst den Hash des festen Style-Textes in `app.security.csp` eintragen, nie `'unsafe-inline'` für `style-src` oder `script-src` (Regel in Global Constraints).

- [ ] **Step 7: Lint, Format, Commit**

```bash
git add apps/desktop/src apps/desktop/brand apps/desktop/src-tauri/src apps/desktop/src-tauri/icons apps/desktop/src-tauri/capabilities/main.json apps/desktop/src-tauri/tauri.conf.json apps/desktop/components.json apps/desktop/package.json apps/desktop/pnpm-lock.yaml
git commit -m "feat(desktop): app shell, settings file, theme and German/English UI"
```

---

### Task 4: Typisierte Fehler in Kern und App

**Files:**
- Modify: `crates/hengst-core/src/error.rs`, `crates/hengst-core/src/loaders/mod.rs` (`no_versions`, `LoaderHint`, `LoaderKind::display_name`), `crates/hengst-core/src/loaders/forge/mod.rs` (`lock_file`), `apps/cli/src/main.rs` (`loader_name` ruft `LoaderKind::display_name`, Ausgaben unverändert)
- Modify: `apps/desktop/src-tauri/src/{error.rs,dto/error.rs,dto/mod.rs,command_list.rs,commands/mod.rs}`, `src-tauri/capabilities/main.json` (`allow-open-url`), `apps/desktop/src/ipc/client.ts` (`openUrl`), `src/ipc/bindings/` (erzeugt), `src/main.tsx` (nur im Dev-Server: Hash `#error-gallery`)
- Create: `apps/desktop/src-tauri/src/{open.rs,commands/open.rs}`
- Create: `apps/desktop/src/features/errors/{ErrorPanel.tsx,errorCopy.ts,errorCopy.test.ts,ErrorPanel.test.tsx}`, `apps/desktop/src/dev/ErrorGallery.tsx`
- Modify: `apps/desktop/src/i18n/{de.json,en.json}`

**Interfaces:**
- Produces (Kern):
  - `Error::LockTimeout { label: String, waited_secs: u64, lock: PathBuf }` mit exakt dem bisherigen Text `"another install of {label} is still running after {waited_secs} s (lock {lock}); try again later"` (statt `Error::Io(TimedOut)`); `lock` ist `Display` über `.display()`.
  - `Error::Cancelled` mit Text `"the operation was cancelled"`.
  - `LoaderKind::display_name(self) -> &'static str` (`"Fabric"`, `"Quilt"`, `"Legacy Fabric"`, `"NeoForge"`, `"Forge"`; die Quelle für jeden Loader-Namen, den ein Nutzer sieht; ersetzt `loader_name` in der CLI).
  - `pub enum LoaderHint { NeoForgeStartsAt1201, FabricStartsAt114, LegacyFabricServesOld }` mit `Display` = bisherige Texte (`"NeoForge starts at 1.20.1"`, `"Fabric starts at 1.14; try --loader legacy-fabric"`, `"Legacy Fabric serves 1.13.2 and older; try --loader fabric"`); `Error::NoLoaderVersions.hint: Option<LoaderHint>`. Die Fehlermeldung bleibt zeichengleich.
- Produces (Desktop):
  - `ErrorCode` (serde `camelCase`, ts-rs Union): `network`, `offline`, `serverError`, `checksumMismatch`, `unsafeData`, `unknownVersion`, `noJavaRuntime`, `offlineAccountBlocked`, `noLoaderVersions`, `unknownLoaderVersion`, `loaderUnsupported`, `installerInvalid`, `installFailed`, `installStepFailed`, `nativesInUse`, `lockTimeout`, `credentialStore`, `missingClientId`, `appMisconfigured`, `appNotApproved`, `loginDeclined`, `loginExpired`, `deviceCodeExpired`, `noXboxAccount`, `xboxRegionBlocked`, `xboxAdultVerification`, `xboxChildAccount`, `xboxRefused`, `gameNotOwned`, `noProfile`, `accountNotFound`, `noAccount`, `cancelled`, `invalidInput`, `fileSystem`, `internal`. Task 5 ergänzt `browserLoginTimedOut`, `loopbackUnavailable`, `browserLoginFailed`; Task 9 `instanceNotFound`, `instanceExists`, `instanceFormatTooNew`, `invalidInstanceName`; Task 10 `busy`, `recycleBinFailed`, `instanceRunning`.
  - `ErrorAction` (`#[serde(tag = "kind", rename_all = "camelCase")]`): `Retry`, `SignIn`, `OpenUrl { target: UrlTarget }`, `OpenSettings`. Es gibt bewusst keine Aktion mit einem Pfad: `OpenLog { path }` hat in 4a keinen Verbraucher und kommt in 4b Task 16 zusammen mit der Pfadprüfung. `UrlTarget` (`#[serde(rename_all = "camelCase")]`, einfaches Enum aus festen https-Adressen, JSON z. B. `"signInHelp"`): `Repository`, `SignInHelp`, `XboxSignup`, `MinecraftProfile`, `DeviceLogin`, `BuildHelp` (alle hier, damit Task 8 sie ohne Vorgriff nutzen kann).
  - `open.rs`: `pub fn resolve_url(t: &UrlTarget) -> &'static str` (`Repository` = `https://github.com/clemensjl/hengst-client`, `SignInHelp` = `https://github.com/clemensjl/hengst-client#readme`, `XboxSignup` = `https://www.xbox.com/`, `MinecraftProfile` = `https://www.minecraft.net/msaprofile/mygames/editprofile`, `DeviceLogin` = `https://www.microsoft.com/link`, `BuildHelp` = `https://github.com/clemensjl/hengst-client#build`, der Abschnitt zum Bauen der README; dort muss stehen, wie die Client-ID beim Bauen gesetzt wird). Command `open_url(target: UrlTarget) -> Result<(), ErrorDto>` in `commands/open.rs` öffnet nur `resolve_url(target)` über `tauri_plugin_opener::OpenerExt::opener().open_url`; Eintrag `open::open_url` in `command_list.rs`, `allow-open-url` in `capabilities/main.json`, `api.openUrl(target)` in `client.ts`. (`OpenTarget` und `open_location` bleiben Task 10.)
  - `pub fn error_dto(e: &hengst_core::Error) -> ErrorDto`, erschöpfend über `Error` und `AuthError` ohne `_`-Zweig; `detail` = `e.to_string()`.
  - Abbildung (Code, Aktion, Params): `Http` → network, Retry, `url`; `Status` → serverError, Retry, `status`; `Offline` → offline, Retry; `Checksum` → checksumMismatch, Retry; `UnsafePath`, `UnknownPlaceholder`, `ParentMismatch` → unsafeData; `UnknownVersion` → unknownVersion, `version`; `NoRuntime` → noJavaRuntime, OpenSettings; `OfflineLaunchDisabled` → offlineAccountBlocked; `NoLoaderVersions` → noLoaderVersions, `loader`, `loaderSlug`, `game`, `hint` (Variantenname in camelCase, z. B. `fabricStartsAt114`); `UnknownLoaderVersion` → unknownLoaderVersion, `loader`, `loaderSlug`, `game`, `version`; `LoaderUnsupported` → loaderUnsupported, `loader`, `loaderSlug`, `game`, `reason`; dabei ist `loaderSlug` der Wert aus dem Kernfehler (der Slug, z. B. `legacy-fabric`) und `loader` der Anzeigename, gewonnen aus `LoaderKind::from_str(slug).display_name()` (bei einem unbekannten Slug der Slug selbst), nie der Slug als Anzeigetext; `TooLarge`, `InvalidChecksumFile`, `NoChecksum` → serverError; `Installer`, `Zip` → installerInvalid; `InstallFailed` → installFailed, Retry; `Processor` → installStepFailed, keine Aktion, `step`, `code` (der Logpfad `log` bleibt im Kern und geht nicht an die Web-Ansicht); `NativesInUse` → nativesInUse; `LockTimeout` → lockTimeout, Retry; `CredentialStore` → credentialStore; `Cancelled` → cancelled; `Io` → fileSystem; `Json` → internal; Auth: `MissingClientId` → missingClientId; `DeviceCodeExpired` → deviceCodeExpired, SignIn; `LoginDeclined` → loginDeclined, SignIn; `AppMisconfigured` → appMisconfigured; `LoginExpired` → loginExpired, SignIn, `name`; `NoXboxAccount` → noXboxAccount, OpenUrl(XboxSignup); `XboxRegionBlocked`, `XboxAdultVerification`, `XboxChildAccount` je eigener Code; `XboxRefused` → xboxRefused, `code`; `AppNotApproved` → appNotApproved, OpenUrl(SignInHelp); `GameNotOwned` → gameNotOwned; `NoProfile` → noProfile, OpenUrl(MinecraftProfile); `AccountNotFound` → accountNotFound; `NoAccount` → noAccount, SignIn; `BadResponse` → serverError, Retry.
- Produces (Frontend): `errorCopy.ts`: `export const errorKeys: Record<ErrorCode, { title: string; body: string }>` (i18n-Schlüssel; `Record` macht fehlende Codes zum Typfehler), `describeError(dto: ErrorDto, t: TFunction): { title: string; body: string; actionLabel?: string }`; `ErrorPanel` Props `{ error: ErrorDto; onRetry?: () => void; onAction?: (a: ErrorAction) => void; extraAction?: { label: string; onClick: () => void } }`, Aufbau nach DESIGN.md 10.11 (`role="alert"`; Titel ohne Punkt, Text darunter; Aktionszeile mit "Erneut versuchen" als `secondary`-Knopf mit `refresh-cw`, der Aktion aus dem Fehler und `extraAction` als weiteren `secondary`-Knöpfen; "Details" als Collapsible mit `detail` in Mono in einer `--bg`-Box und "Kopieren"). `extraAction` nimmt Aktionen, die nicht aus dem Fehler stammen (z. B. "Ordner öffnen" im Löschdialog, Task 11, "Code stattdessen verwenden" im Anmeldedialog, Task 8). `detail` bleibt der englische Kerntext in beiden Sprachen (DESIGN.md 10.11).

- [ ] **Step 1: Failing Tests schreiben**

Kern:
```rust
// loaders/forge/mod.rs, bestehender Lock-Test angepasst:
assert!(matches!(&err, Error::LockTimeout { waited_secs, .. } if *waited_secs == timing.limit.as_secs()));
assert!(err.to_string().contains("another install of forge for Minecraft 1.20.1 is still running after"));
// loaders/mod.rs
#[test] fn display_names_are_the_user_facing_names() {
    assert_eq!(LoaderKind::LegacyFabric.display_name(), "Legacy Fabric");
    assert_eq!(LoaderKind::NeoForge.display_name(), "NeoForge");
    assert_eq!(LoaderKind::Forge.display_name(), "Forge");
}
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
    //   -> code InstallStepFailed, action None, params["step"] == "binarypatcher", params["code"] == "1"
    // Error::Auth(AppNotApproved) -> AppNotApproved, OpenUrl { target: SignInHelp }
    // Error::NoLoaderVersions { loader: "legacy-fabric", hint: Some(FabricStartsAt114), .. } -> params["hint"] == "fabricStartsAt114", params["loader"] == "Legacy Fabric", params["loaderSlug"] == "legacy-fabric"
}
#[test] fn processor_error_carries_no_path_to_the_web_view() {
    let dto = error_dto(&Error::Processor { jar: "binarypatcher".into(), code: 1, log: "C:/h/logs/x.log".into() });
    assert_eq!(dto.code, ErrorCode::InstallStepFailed);
    assert!(dto.action.is_none());
    assert!(!serde_json::to_string(&dto.params).unwrap().contains("x.log"));
}
#[test] fn error_detail_never_contains_secret() {
    // alle Beispielwerte der Tabelle aus every_error_maps_to_a_code serialisieren;
    // kein JSON enthält "Secret(", "<redacted>" oder "accessToken"; params enthalten nur die je Variante genannten Schlüssel
}
#[test] fn url_targets_are_fixed_https_urls() {
    for t in [UrlTarget::Repository, UrlTarget::SignInHelp, UrlTarget::XboxSignup, UrlTarget::MinecraftProfile, UrlTarget::DeviceLogin, UrlTarget::BuildHelp] {
        assert!(resolve_url(&t).starts_with("https://"));
    }
    assert_eq!(resolve_url(&UrlTarget::DeviceLogin), "https://www.microsoft.com/link");
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
it("detail stays English in the German UI and has a copy button", ...)
it("extraAction renders as an additional secondary button and calls its handler once", ...)
it("titles end without a full stop and bodies end with one, in de and en", ...)   // über errorKeys (DESIGN.md 12)
it("noLoaderVersions shows the display name, never the slug", ...)   // params.loader "Legacy Fabric" erscheint, "legacy-fabric" nicht
it("onAction with openUrl calls api.openUrl with the target and nothing else", ...)
```

- [ ] **Step 2: Tests laufen lassen, sie scheitern**

Run: `cargo test -p hengst-core hints_are_typed`, `cargo test -p hengst-core display_names` und `cargo test -p hengst-desktop error`, `pnpm -C apps/desktop test errorCopy`
Expected: FAIL

- [ ] **Step 3: Implementieren** wie in Interfaces; Texte der Fehler nach `frontend-design` ("was ist passiert, was tun", keine Entschuldigung) und DESIGN.md Abschnitt 12 (Titel ohne Punkt, Text als ganze Sätze); für die Anmeldefehler gilt der Wortlaut aus DESIGN.md 11.5 (`network`, `offline`, `loginDeclined`, `deviceCodeExpired`, `appMisconfigured`, `missingClientId`, `noXboxAccount`, `gameNotOwned`, `noProfile`, `credentialStore`; Task 5 ergänzt drei, Task 8 prüft alle gegen die Tabelle). Beispiel `appNotApproved` siehe Task 8.

- [ ] **Step 4: Tests grün**

Run: `cargo test --workspace --all-features` und `pnpm -C apps/desktop typecheck && pnpm -C apps/desktop test`
Expected: PASS, CLI-Tests unverändert grün (Texte gleich).

- [ ] **Step 4b: Sichtprüfung der Fehlerzustände**

`ErrorPanel` ist reine Darstellung; gerendert wird er im Vite-Dev-Server, nicht in der Tauri-App. `src/dev/ErrorGallery.tsx` (nur geladen, wenn `import.meta.env.DEV` und `location.hash === "#error-gallery"`, per dynamischem Import, damit der Produktionsbuild sie nicht enthält) zeigt `ErrorPanel` für feste Beispiel-DTOs: `network` mit Retry, `appNotApproved` mit `openUrl`, `noLoaderVersions` mit Hinweis und `params.loader` "Legacy Fabric", `credentialStore`, `internal` mit langem `detail` (300 Zeichen ohne Leerzeichen), jeweils mit eingeklappten und aufgeklappten Details.
Run: `pnpm -C apps/desktop dev`, dann mit agentfenster (`browser` auf `http://localhost:1420/#error-gallery`, Agent-Desktop) Screenshots in hell und dunkel (Theme aus Task 3, über die Browser-Einstellung für die Farbvorliebe oder die Einstellungen der App) und Deutsch und Englisch, bei 960 × 600 und bei 1920 × 1080, nach `evidence/task-4/`.
Expected: jeder Zustand zeigt Titel, Text und passende Aktion in beiden Sprachen und Themes; das lange `detail` bricht um oder scrollt und sprengt weder Panel noch Fenster; Fokusring auf "Erneut versuchen" und "Details" sichtbar. Die Funktionsprüfung in der laufenden App folgt in Task 8 (`appNotApproved`).

- [ ] **Step 5: Lint, Format, Commit**

```bash
git add crates/hengst-core/src/error.rs crates/hengst-core/src/loaders/mod.rs crates/hengst-core/src/loaders/forge/mod.rs apps/cli/src/main.rs apps/desktop/src-tauri/src apps/desktop/src-tauri/capabilities/main.json apps/desktop/src/features/errors apps/desktop/src/dev apps/desktop/src/main.tsx apps/desktop/src/ipc/client.ts apps/desktop/src/i18n apps/desktop/src/ipc/bindings
git commit -m "feat: typed lock timeout, cancel and loader hints; error DTO mapping for the desktop app"
```

---

### Task 5: Kern: Microsoft-Login per Browser (Auth Code + PKCE)

**Files:**
- Create: `crates/hengst-core/src/auth/browser.rs`
- Modify: `crates/hengst-core/src/auth/mod.rs` (`pub mod browser;`), `crates/hengst-core/src/auth/msa.rs` (`parse_tokens`, `error_code`, `map_client_error` auf `pub(super)`), `crates/hengst-core/src/auth/error.rs`, `crates/hengst-core/Cargo.toml` + `Cargo.toml` (`sha2`, `base64`, `getrandom` per `cargo add`, Versionen im Lockfile), `Cargo.lock`
- Modify: `apps/desktop/src-tauri/src/error.rs`, `apps/desktop/src-tauri/src/dto/error.rs` (neue Codes), `apps/desktop/src/ipc/bindings/` (erzeugt), `apps/desktop/src/i18n/{de.json,en.json}`, `apps/desktop/src/features/errors/errorCopy.ts`

**Interfaces:**
- Consumes: `MicrosoftAuth { http, ep, client_id }`, `AuthEndpoints.msa` (`.../consumers/oauth2/v2.0`), `SCOPE`, `msa::MsaTokens`, `parse_tokens`, `error_code`, `map_client_error` (bestehende private Helfer in `msa.rs`, für `browser.rs` auf `pub(super)` heben).
- Produces:
  - `pub struct BrowserLoginPages { pub success_title: String, pub success_body: String, pub failure_title: String, pub failure_body: String }`
  - `pub struct BrowserLogin` (nicht `Clone`; `Debug` zeigt nur `redirect_uri`; besitzt die lauschenden Sockets, ein Drop gibt den Port frei; der PKCE-Verifier liegt als `Secret`, `state` als `String`) mit `pub fn authorize_url(&self) -> &str` und `pub fn redirect_uri(&self) -> &str`.
  - `impl MicrosoftAuth { pub async fn start_browser_login(&self) -> Result<BrowserLogin>; pub async fn finish_browser_login(&self, login: BrowserLogin, pages: &BrowserLoginPages, timeout: Duration) -> Result<MsaTokens>; }`
  - `pub const BROWSER_LOGIN_TIMEOUT: Duration = Duration::from_secs(300);`
  - `AuthError::{BrowserLoginTimedOut, LoopbackUnavailable, BrowserLoginFailed { code: String }}` mit Texten `"the browser sign-in was not completed within 5 minutes"`, `"the launcher cannot listen on localhost for the sign-in answer"`, `"the browser sign-in failed ({code})"`. `code` stammt aus der URL und ist deshalb höchstens 64 Zeichen lang und enthält nur `[A-Za-z0-9_.-]` (andere Zeichen werden verworfen; bleibt nichts übrig, ist es `unknown`); er erreicht über `ErrorDto.params` die Oberfläche.

Ablauf (festgelegt): `start_browser_login` bindet `127.0.0.1:0`, dann versucht es `[::1]:<derselbe Port>` (Fehler dort ignoriert); Bindefehler auf IPv4 → `LoopbackUnavailable`. Gebunden wird nie auf einer anderen Adresse als Loopback. Verifier = 32 Zufallsbytes (`getrandom`) Base64url ohne Padding (43 Zeichen, als `Secret`), Challenge = Base64url(SHA-256(Verifier)), `state` = 16 Zufallsbytes Base64url. Authorize-URL `{msa}/authorize` mit `client_id`, `response_type=code`, `redirect_uri=http://localhost:<port>`, `response_mode=query`, `scope=XboxLive.signin offline_access`, `code_challenge`, `code_challenge_method=S256`, `state`, `prompt=select_account` (mit `reqwest::Url` gebaut, korrekt kodiert).

`finish_browser_login(login, pages, timeout)` wartet in einer Schleife auf Rückrufe bis `timeout`. Jede angenommene Verbindung läuft in einer **eigenen Aufgabe** (`JoinSet`; höchstens 32 gleichzeitig, bei Überschreitung wird die älteste noch lesende Verbindung geschlossen), die höchstens 8 KiB bis `\r\n\r\n` liest (5 s Lesefrist) und die Anfrage einordnet: kein `GET /?...` (anderer Pfad oder Verb, kaputte Zeile) → die Aufgabe antwortet selbst mit 404 bzw. 400; sonst reicht sie die Parameter mit einem `oneshot`-Antwortkanal an die Schleife. Eine hängende Verbindung blockiert dadurch nur ihre eigene Aufgabe, nie die echte Weiterleitung. Die Schleife prüft `state`: falsch oder fehlend → Antwort 400 über den Antwortkanal, Warten geht weiter (bis `timeout`, dann `BrowserLoginTimedOut`); richtig → **das ist der einzige angenommene Rückruf**: `error=access_denied` → Fehlerseite (`failure_*`) und `LoginDeclined`; anderes `error=` → Fehlerseite und `BrowserLoginFailed { code }` (nur der bereinigte Code, nie `error_description`); `code` → POST `{msa}/token` mit `grant_type=authorization_code`, `client_id`, `code`, `redirect_uri` (gleich wie oben), `code_verifier`, `scope`, Antwort wie beim Device Code ausgewertet (`map_client_error`). **Die Erfolgsseite (`success_*`) geht erst an den Browser, nachdem der Tausch gelungen ist**; scheitert er, bekommt der Browser die Fehlerseite (`failure_*`). Während des Tauschs beantwortet die Schleife weitere Rückrufe weiter mit 400. Danach (Erfolg, Fehler, Zeitablauf) und bei jedem Abbruch (Drop des Futures) werden alle Aufgaben abgebrochen und beide Sockets geschlossen: der Port ist sofort wieder frei. Alle Antworten: `Cache-Control: no-store`, `Referrer-Policy: no-referrer`, `X-Content-Type-Options: nosniff`, `Connection: close`; Seiten `text/html; charset=utf-8`, Titel und Text HTML-escaped, kein Script, kein externes CSS; 404 und 400 haben nur einen kurzen Klartext-Body.

- [ ] **Step 1: Failing Tests schreiben** (`browser.rs`, wiremock für `/token`, Redirect per `reqwest::get` auf die Loopback-URI; Hilfen im Testmodul: `redirect_port(&login) -> u16`, `callback(port, query) -> reqwest::Response` (GET `http://127.0.0.1:<port>/?<query>`), `assert_port_free(port)` (bindet `127.0.0.1:<port>` innerhalb von 2 s, Wiederholung alle 50 ms, sonst Fehlschlag))

```rust
#[tokio::test] async fn authorize_url_carries_pkce_and_consumers_scope() {
    // URL parsen: response_type == "code", scope == "XboxLive.signin offline_access", code_challenge_method == "S256",
    // redirect_uri beginnt mit "http://localhost:", Host von authorize == Mock-Host, Pfad endet auf "/authorize",
    // code_challenge == b64url(sha256(login.verifier_for_test())), state.len() >= 22, prompt == "select_account"
}
#[tokio::test] async fn redirect_with_code_exchanges_it_with_the_verifier() {
    // Mock /token erwartet body_string_contains "grant_type=authorization_code", "code=abc", "code_verifier=<v>", "redirect_uri=http%3A%2F%2Flocalhost%3A<port>"
    // GET <redirect>?code=abc&state=<state> -> 200, Body enthält success_title; Ergebnis: access_token.expose() == "at", refresh == "rt"
    // danach assert_port_free(port)
}
#[tokio::test] async fn wrong_state_is_ignored_and_login_keeps_waiting() { /* erst state=x -> 400, dann richtig -> Ok */ }
#[tokio::test] async fn other_paths_get_404() { /* GET /favicon.ico -> 404, danach richtiger Redirect -> Ok */ }
#[tokio::test] async fn access_denied_maps_to_login_declined() { /* ?error=access_denied&error_description=secret+text&state=<s> -> Body enthält failure_title, nicht "secret text"; Err(Auth(LoginDeclined)); assert_port_free(port) */ }
#[tokio::test] async fn other_error_keeps_only_the_code() { /* ?error=server_error&state=<s> -> BrowserLoginFailed { code } mit code == "server_error" */ }
#[tokio::test] async fn invalid_client_on_exchange_maps_to_app_misconfigured() { /* /token 400 {"error":"invalid_client"} -> AppMisconfigured */ }
#[tokio::test] async fn page_texts_are_escaped() { /* success_title "<b>&" -> Body enthält "&lt;b&gt;&amp;" */ }
#[tokio::test] async fn ipv6_localhost_reaches_the_listener_when_available() { /* GET http://[::1]:<port>/?code&state -> Ok; überspringt mit eprintln, falls ::1 nicht bindbar */ }
#[test] fn debug_never_shows_verifier_or_state() { /* format!("{login:?}") enthält weder Verifier noch state */ }
#[tokio::test] async fn oversized_request_is_dropped() { /* 9 KiB Header ohne Ende -> Verbindung geschlossen, danach richtiger Redirect -> Ok */ }

#[tokio::test]
async fn times_out_without_redirect() {
    let (auth, _server) = auth_with_token_endpoint(0).await;   // /token darf nie aufgerufen werden
    let login = auth.start_browser_login().await.unwrap();
    let port = redirect_port(&login);
    let err = auth.finish_browser_login(login, &pages(), Duration::from_millis(200)).await.unwrap_err();
    assert!(matches!(err, Error::Auth(AuthError::BrowserLoginTimedOut)));
    assert_port_free(port).await;
}

#[tokio::test]
async fn wrong_state_is_rejected_and_the_wait_continues_until_timeout() {
    let (auth, _server) = auth_with_token_endpoint(0).await;   // wiremock expect(0): kein Tausch mit falschem state
    let login = auth.start_browser_login().await.unwrap();
    let port = redirect_port(&login);
    let finish = tokio::spawn(async move { auth.finish_browser_login(login, &pages(), Duration::from_millis(600)).await });
    let first = callback(port, "code=abc&state=wrong").await;
    let second = callback(port, "code=abc").await;
    assert_eq!(first.status(), 400);
    assert_eq!(second.status(), 400);
    assert!(!finish.is_finished(), "a rejected callback must not end the wait");
    let err = finish.await.unwrap().unwrap_err();
    assert!(matches!(err, Error::Auth(AuthError::BrowserLoginTimedOut)));
    assert_port_free(port).await;
}

#[tokio::test]
async fn second_callback_is_refused() {
    // /token antwortet nach 300 ms (set_delay) mit gültigen Tokens und erwartet genau einen Aufruf (expect(1))
    let (auth, _server) = auth_with_slow_token_endpoint(Duration::from_millis(300), 1).await;
    let login = auth.start_browser_login().await.unwrap();
    let port = redirect_port(&login);
    let state = login.state_for_test().to_string();
    let finish = tokio::spawn(async move { auth.finish_browser_login(login, &pages(), Duration::from_secs(5)).await });
    let first = tokio::spawn({ let s = state.clone(); async move { callback(port, &format!("code=abc&state={s}")).await } });
    tokio::time::sleep(Duration::from_millis(100)).await;    // der Tausch läuft noch
    let second = callback(port, &format!("code=other&state={state}")).await;
    assert_eq!(second.status(), 400, "only one valid callback is accepted");
    assert_eq!(first.await.unwrap().status(), 200);
    assert!(finish.await.unwrap().is_ok());
    // wiremock prüft beim Drop des Servers: genau ein /token-Aufruf
}

#[tokio::test]
async fn success_page_is_sent_only_after_the_exchange() {
    // /token antwortet nach 300 ms mit 400 {"error":"invalid_grant"}
    let (auth, _server) = auth_with_failing_token_endpoint(Duration::from_millis(300)).await;
    let login = auth.start_browser_login().await.unwrap();
    let port = redirect_port(&login);
    let state = login.state_for_test().to_string();
    let finish = tokio::spawn(async move { auth.finish_browser_login(login, &pages(), Duration::from_secs(5)).await });
    let started = std::time::Instant::now();
    let body = callback(port, &format!("code=abc&state={state}")).await.text().await.unwrap();
    assert!(started.elapsed() >= Duration::from_millis(300), "the browser answer waits for the token exchange");
    assert!(body.contains("FAILURE-TITLE") && !body.contains("SUCCESS-TITLE"));   // pages() nutzt diese Titel
    assert!(finish.await.unwrap().is_err());
    assert_port_free(port).await;
}

#[tokio::test]
async fn stalled_connection_does_not_block_the_real_callback() {
    let (auth, _server) = auth_with_token_endpoint(1).await;
    let login = auth.start_browser_login().await.unwrap();
    let port = redirect_port(&login);
    let state = login.state_for_test().to_string();
    let finish = tokio::spawn(async move { auth.finish_browser_login(login, &pages(), Duration::from_secs(10)).await });
    // 40 Verbindungen öffnen und nichts senden (mehr als das Limit von 32)
    let mut stalled = Vec::new();
    for _ in 0..40 { stalled.push(tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap()); }
    let started = std::time::Instant::now();
    let r = callback(port, &format!("code=abc&state={state}")).await;
    assert_eq!(r.status(), 200);
    assert!(started.elapsed() < Duration::from_secs(2), "the 5 s read timeout of a stalled connection must not delay the redirect");
    assert!(finish.await.unwrap().is_ok());
    drop(stalled);
}

#[tokio::test]
async fn dropping_finish_frees_the_port() {
    let (auth, _server) = auth_with_token_endpoint(0).await;
    let login = auth.start_browser_login().await.unwrap();
    let port = redirect_port(&login);
    let finish = tokio::spawn(async move { auth.finish_browser_login(login, &pages(), Duration::from_secs(300)).await });
    tokio::time::sleep(Duration::from_millis(100)).await;
    finish.abort();                       // Abbruch wie bei login_cancel
    let _ = finish.await;
    assert_port_free(port).await;         // ein neuer Listener kann den Port sofort binden
}

#[tokio::test]
async fn error_code_is_capped_and_sanitized() {
    // state richtig, error=<80 Mal "a"> -> BrowserLoginFailed { code } mit code == "a" * 64
    // error=a%3Cb%3E%26x -> code == "abx"
    // error=%0D%0A -> code == "unknown"
    // jeweils ein eigener Durchlauf (start_browser_login, callback, finish)
}
```
`verifier_for_test()` (liefert `login.verifier.expose()`) und `state_for_test()` sind `#[cfg(test)]`; `pages()` setzt die Titel `SUCCESS-TITLE` und `FAILURE-TITLE`.

- [ ] **Step 2: Tests laufen lassen, sie scheitern**

Run: `cargo test -p hengst-core browser`
Expected: FAIL

- [ ] **Step 3: Implementieren** nach "Ablauf"; Desktop-`error_dto` um die drei Codes ergänzen (`browserLoginTimedOut` → SignIn, `loopbackUnavailable` → keine Aktion, `browserLoginFailed` → SignIn, `params.code`), Übersetzungen ergänzen (Wortlaut für `browserLoginTimedOut`, `loopbackUnavailable` und `browserLoginFailed` aus DESIGN.md 11.5; die Aktion "Code stattdessen verwenden" bei `loopbackUnavailable` gehört dem Anmeldedialog, Task 8, nicht dem Backend). Es gibt keinen echten Microsoft-Kontakt: die echte Anmeldung braucht die Redirect-URI aus "Beim Menschen" und bleibt ein offener manueller Schritt.

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
- Create: `crates/hengst-testkit/Cargo.toml` (`publish = false`, Abhängigkeiten `wiremock`, `serde_json`, `tokio`, `hengst-core`, `zip`, `reqwest`), `src/lib.rs`, `src/auth.rs`, `src/mojang.rs`, `src/bin/hengst-mock.rs`, `tests/mock_binary.rs`
- Modify: `Cargo.toml` (Member), `Cargo.lock`

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
```

`crates/hengst-testkit/tests/mock_binary.rs` (Integrationstest: `CARGO_BIN_EXE_<name>` gibt es nur dort, nicht in `#[cfg(test)]`-Modulen von `src/`):
```rust
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
git add Cargo.toml Cargo.lock crates/hengst-testkit/Cargo.toml crates/hengst-testkit/src crates/hengst-testkit/tests
git commit -m "test: add hengst-testkit with mock Microsoft, Xbox, Mojang and Fabric services"
```

---

### Task 7: Konten im Backend (Liste, Wechsel, Abmelden, Login-Vorgänge)

**Files:**
- Create: `apps/desktop/src-tauri/src/accounts.rs`, `src/dto/accounts.rs`, `src/commands/accounts.rs`, `src-tauri/tests/no_tokens.rs`
- Modify: `crates/hengst-core/src/auth/store.rs` (`impl<S: SecretStore + ?Sized> SecretStore for Box<S>`), `apps/desktop/src-tauri/src/{state.rs,startup.rs,command_list.rs,commands/mod.rs,dto/mod.rs,lib.rs}`, `apps/desktop/src-tauri/capabilities/main.json` (`allow-accounts-list`, `allow-account-select`, `allow-account-remove`, `allow-login-start`, `allow-login-cancel`, `allow-login-reopen-browser`), `apps/desktop/src-tauri/Cargo.toml` (`hengst-testkit` als **optionale** Abhängigkeit: `e2e = ["dep:hengst-testkit"]`, zusätzlich als Dev-Abhängigkeit; Dev-Abhängigkeit `static_assertions`), `apps/desktop/src/ipc/{client.ts,events.ts,client.test.ts}`, `apps/desktop/src/ipc/bindings/` (erzeugt), `Cargo.lock`

**Interfaces:**
- Consumes: `AccountStore`, `load_accounts_file`, `KeyringSecretStore`, `MemorySecretStore`, `MicrosoftAuth::{start_browser_login, finish_browser_login, start_device_login, poll_device_login, complete_login}`, `client_id`, `unix_now`, `BrowserLoginPages`, `BROWSER_LOGIN_TIMEOUT`, `error_dto`, `EventHub`, `Settings.dev_account`, `SettingsHandle` (Task 3), `UrlTarget` (Task 4), Testkit (Task 6).
- Produces:
  - DTOs: `AccountKindDto` (`microsoft`, `offlineDev`); `AccountDto { id: String, name: String, kind: AccountKindDto, selected: bool, sign_in_required: bool }` (`id` = UUID bzw. `"offline-dev"`); `AccountsDto { accounts: Vec<AccountDto>, can_sign_in: bool }` (`can_sign_in` false, wenn keine Client-ID); `LoginMethod` (`browser`, `deviceCode`); `LoginPagesDto { success_title, success_body, failure_title, failure_body }`; `OperationId(u64)` (TS `number`); `LoginEvent` (`#[serde(tag = "event", content = "data", rename_all = "camelCase")]`): **`Started { op: OperationId }` (immer das erste Ereignis, noch vor `login_start`-Rückgabe)**, `BrowserOpened`, `DeviceCode { user_code: String, verification_uri: String, expires_at: u64 }`, `Completing`, `SignedIn { account: AccountDto }`, `Failed { error: ErrorDto }`, `Cancelled`. Ereignisse eines Vorgangs gehen nur an dessen eigenen Kanal.
  - `pub struct Accounts` in `accounts.rs` mit `fn new(paths: &Paths, secrets: Box<dyn SecretStore>, auth: Option<MicrosoftAuth>, opener: BrowserOpener) -> Accounts`; intern eine `std::sync::Mutex<()>` (`io_lock`), die **jeden Schreibzugriff auf `accounts.json`** (`save_session`, `set_default`, `remove`, `mark_sign_in_required`) serialisiert (nie über ein `await` gehalten). `fn list(&self, settings: &Settings) -> Result<AccountsDto>` (liest nur `accounts.json`, kein Schlüsselspeicher); `fn select(&self, id: &str, settings: &SettingsHandle) -> Result<()>` (`"offline-dev"` setzt `dev_account = true` nur mit `dev-offline`, sonst `AccountNotFound`; Microsoft-Konto: `set_default` und `dev_account = false`; die Einstellung wird im selben Aufruf über `SettingsHandle::update` gespeichert, nicht erst später); `fn remove(&self, id: &str) -> Result<()>`; `async fn login(&self, method: LoginMethod, pages: BrowserLoginPages, settings: SettingsHandle, op: OperationId, sink: EventSink<LoginEvent>) -> Result<AccountDto>` (sendet als Erstes `Started { op }`; nach `save_session` und `set_default` setzt es `dev_account = false` über `SettingsHandle::update`); `fn mark_sign_in_required(&self, uuid: &str)` (für 4b, wenn `launch_account` `LoginExpired` meldet).
  - `pub type BrowserOpener = Arc<dyn Fn(&str) -> Result<()> + Send + Sync>`: in Release/Debug `opener.open_url(url)`, mit Feature `e2e` ein HTTP-Abruf der URL mit Redirect-Folgen in einem eigenen Task.
  - `pub enum OpKind { Login }` (Browser- **und** Device-Code-Anmeldung teilen sich diese eine Art, damit "Code stattdessen verwenden" den wartenden Browser-Vorgang beendet und dessen Port freigibt; 4b ergänzt `Play`). `pub struct Operations` mit `fn start(&self, kind: OpKind, on_cancel: Box<dyn Fn() + Send + Sync>, spawn: impl FnOnce(OperationId) -> AbortHandle) -> OperationId` (vergibt die Id, bricht einen laufenden Vorgang derselben Art ab, ruft `spawn(id)` und merkt sich Handle und `on_cancel`), `fn cancel(&self, id: OperationId) -> bool` (bricht den Task ab, der Drop gibt den Port frei, und ruft `on_cancel`, das `Cancelled` an den Kanal des Vorgangs sendet), `fn set_reopen(&self, id: OperationId, url: String)`, `fn reopen_url(&self, id: OperationId) -> Option<String>` (nur intern; verlässt Rust nie).
  - Commands: `accounts_list() -> Result<AccountsDto, ErrorDto>`, `account_select(id: String)` (ruft `Accounts::select` mit `AppState.settings`, sendet `DataChanged(Accounts)` und `DataChanged(Settings)`), `account_remove(id: String)`, `login_start(method: LoginMethod, pages: LoginPagesDto, channel: Channel<LoginEvent>) -> Result<OperationId, ErrorDto>` (kehrt sofort zurück, Arbeit im Task; die Oberfläche nimmt die `op` aus dem `Started`-Ereignis, weil Ereignisse vor der Rückgabe eintreffen können), `login_cancel(op: OperationId) -> Result<bool, ErrorDto>`, `login_reopen_browser(op: OperationId) -> Result<(), ErrorDto>`. Erfolgreiche Mutation sendet `DataChanged(Accounts)`. Einträge in `command_list.rs`: `accounts::accounts_list`, `accounts::account_select`, `accounts::account_remove`, `accounts::login_start`, `accounts::login_cancel`, `accounts::login_reopen_browser`.
  - Neues Konto wird gewählt (`set_default`), `dev_account` wird dabei `false` (gespeichert, siehe oben).
  - Frontend (`client.ts`, hier statt in Task 8, damit `ts_client_covers_every_command` grün bleibt): `api.accountsList()`, `api.accountSelect(id)`, `api.accountRemove(id)`, `api.loginStart(method, pages, onEvent): Promise<number>` (legt den `Channel<LoginEvent>` an), `api.loginCancel(op)`, `api.loginReopenBrowser(op)`.
  - Mit `e2e`: `MemorySecretStore`, Client-ID `"e2e-client"`, `auth_endpoints(<Basis>)` und `endpoints(<Basis>)` aus `hengst-testkit` mit der Basis aus `--e2e-base-url` bzw. `HENGST_E2E_BASE_URL` (`startup.rs` verdrahtet beides hier). Ohne `e2e`: `KeyringSecretStore::new()`; schlägt das fehl, bleibt die Liste nutzbar und `login_start` meldet `credentialStore`.

- [ ] **Step 1: Failing Tests schreiben** (`accounts.rs`, mit Testkit-Mock und `MemorySecretStore`)

```rust
#[tokio::test] async fn browser_login_saves_and_selects_the_account() {
    // Events in Reihenfolge: Started { op }, BrowserOpened, Completing, SignedIn { account.name == "E2EPlayer", selected == true }
    // list(): 1 Konto, selected
}
#[tokio::test] async fn device_login_reports_the_code_then_signs_in() { /* Events: Started { op }, dann DeviceCode mit user_code nicht leer, Completing, SignedIn */ }
#[tokio::test] async fn started_is_always_the_first_event_and_carries_the_op_returned_by_login_start() { /* Browser- und Device-Login: events[0] == Started { op } und op == Rückgabe von login_start */ }
#[tokio::test] async fn second_account_becomes_selected_and_switch_works() {
    // zwei Logins (A, B) -> B selected; select(A.id) -> A selected; accounts.json default == A.uuid
}
#[tokio::test] async fn remove_deletes_tokens_and_moves_selection() { /* remove(B) -> Liste [A], A selected, Secret für B fehlt */ }
#[tokio::test] async fn login_events_never_contain_tokens() {
    // alle Events eines Browser- und eines Device-Logins serialisieren;
    // Geheimnisse kommen aus dem Testkit, nicht aus Literalen: `auth.issued_secrets()` (Handle von `mount_auth`/`Services::auth`) liefert alle bis dahin
    // ausgegebenen Werte (Minecraft- und Refresh-Tokens der Spieler, abgeleitete Microsoft-, Xbox- und XSTS-Tokens, rotierte Refresh-Tokens,
    // Autorisierungscodes, Device Codes, `state`, Verifier); der Test prüft zuerst, dass die Liste nicht leer ist und `PLAYER_A.minecraft_token`
    // und `PLAYER_A.refresh_token` enthält, und dann, dass kein serialisierter String einen davon enthält; zusätzlich nicht "state=" und "code_verifier"
}
#[tokio::test] async fn not_approved_login_maps_to_app_not_approved() { /* Szenario NotApproved -> Failed { error.code == AppNotApproved } und Ergebnis Err */ }
#[tokio::test] async fn second_login_cancels_the_first() {
    // Login 1 per Browser starten ohne Redirect (Opener tut nichts), Login 2 per Device Code starten (gleiche OpKind::Login)
    // -> Login 1 meldet Cancelled, Login 2 läuft; der Port aus Login 1 ist danach frei (TcpListener::bind auf 127.0.0.1:<port> gelingt binnen 2 s)
}
#[tokio::test] async fn cancel_ends_a_waiting_login() { /* cancel(op) == true; Event Cancelled; kein Konto gespeichert; Port frei */ }
#[tokio::test]
async fn concurrent_account_writes_leave_a_consistent_file() {
    // zwei Konten A und B vorhanden; je 20 Mal parallel select(A) und select(B) (tokio::spawn) und remove/save_session eines dritten Kontos C
    let accounts = Arc::new(test_accounts_with(&[PLAYER_A, PLAYER_B]));
    let tasks: Vec<_> = (0..40).map(|i| { let a = accounts.clone(); let s = settings.clone(); tokio::spawn(async move { a.select(if i % 2 == 0 { A_ID } else { B_ID }, &s) }) }).collect();
    for t in tasks { t.await.unwrap().unwrap(); }
    let file = load_accounts_file(&paths).unwrap();     // parst: nie eine halb geschriebene Datei
    assert!(file.default == Some(A_ID.into()) || file.default == Some(B_ID.into()));
    assert_eq!(file.accounts.len(), 2);
}
#[cfg(feature = "dev-offline")] #[test]
fn selecting_the_dev_account_saves_the_setting_in_the_same_call() {
    // select("offline-dev", &settings) -> settings.json enthält "devAccount": true, ohne dass ein weiterer Aufruf nötig ist;
    // select(<Microsoft-Konto>) -> "devAccount": false in der Datei
}
#[tokio::test] async fn a_new_login_clears_the_dev_account_flag_in_the_file() { /* settings.json mit devAccount true (nur dev-offline) -> nach SignedIn steht devAccount false in der Datei */ }
#[test] fn accounts_list_works_without_secret_store() { /* Secret-Store, dessen get/set immer Err(CredentialStore) liefert; accounts.json mit A -> list() Ok */ }
#[cfg(feature = "dev-offline")] #[test] fn dev_build_lists_and_selects_the_offline_account() { /* id "offline-dev" in der Liste; select -> dev_account true, selected */ }
#[cfg(not(feature = "dev-offline"))] #[test] fn release_build_never_lists_dev_account() { /* dev_account = true in Settings -> kein "offline-dev"; select("offline-dev") -> AccountNotFound */ }
```

`src-tauri/tests/no_tokens.rs` (Invariante aus den Global Constraints; Task 10 ergänzt die Instanz-DTOs):
```rust
use static_assertions::assert_not_impl_any;
use hengst_core::secret::Secret;
use hengst_core::auth::msa::MsaTokens;
use hengst_core::auth::flow::LoginSession;

const FAKE: &str = "FAKE-ACCESS-TOKEN-7f3a";

#[test]
fn token_types_do_not_implement_serialize() {
    assert_not_impl_any!(Secret: serde::Serialize);
    assert_not_impl_any!(MsaTokens: serde::Serialize);
    assert_not_impl_any!(LoginSession: serde::Serialize);
}

#[tokio::test]
async fn no_dto_or_event_contains_a_token() {
    // MemorySecretStore und accounts.json mit einem Konto, dessen access_token, refresh_token und minecraft_token den Wert FAKE enthalten
    let mut seen: Vec<String> = Vec::new();
    seen.push(serde_json::to_string(&accounts.list(&settings.get()).unwrap()).unwrap());   // AccountsDto
    seen.push(serde_json::to_string(&app_info_dto()).unwrap());                            // AppInfo
    seen.push(serde_json::to_string(&settings_dto(&settings.get(), None)).unwrap());       // SettingsDto
    for e in run_mock_login_with_token(FAKE, LoginMethod::Browser).await { seen.push(serde_json::to_string(&e).unwrap()); }      // jedes LoginEvent
    for e in run_mock_login_with_token(FAKE, LoginMethod::DeviceCode).await { seen.push(serde_json::to_string(&e).unwrap()); }
    for err in every_auth_error_with_message_containing(FAKE) { seen.push(serde_json::to_string(&error_dto(&err)).unwrap()); }   // ErrorDto
    for s in &seen { assert!(!s.contains(FAKE), "token leaked into: {s}"); }
    assert!(!seen.is_empty());
}
```

In `store.rs`:
```rust
#[test] fn boxed_secret_store_delegates() { /* Box<dyn SecretStore> aus MemorySecretStore; AccountStore damit; save_session + secret() funktionieren */ }
```

- [ ] **Step 2: Tests laufen lassen, sie scheitern**

Frontend (`client.test.ts`, `mockIPC`):
```ts
it("loginStart passes the channel and the pages and returns the op", async () => {
  // mockIPC((cmd, args) => { seen = [cmd, Object.keys(args)]; args.channel.onmessage?.({ event: "started", data: { op: 7 } }); return 7; })
  const events: LoginEvent[] = [];
  const op = await api.loginStart("browser", pages, (e) => events.push(e));
  expect(seen).toEqual(["login_start", ["method", "pages", "channel"]]);
  expect(op).toBe(7);
  expect(events[0]).toEqual({ event: "started", data: { op: 7 } });
});
it("accounts commands use camelCase args", async () => { /* account_select -> { id }, login_cancel -> { op } */ });
```

Run: `cargo test -p hengst-desktop accounts`, `cargo test -p hengst-desktop --test no_tokens`, `cargo test -p hengst-core boxed_secret_store` und `pnpm -C apps/desktop test client`
Expected: FAIL

- [ ] **Step 3: Implementieren** wie in Interfaces. Login-Task: `Started { op }` → `start_browser_login` → `set_reopen` → Opener → `BrowserOpened` → `finish_browser_login(..., BROWSER_LOGIN_TIMEOUT)` → `Completing` → `complete_login` → `save_session` → `set_default` → `dev_account = false` (gespeichert) → `SignedIn`; Fehler → `Failed { error_dto }`. Die Seite im Browser bekommt die Texte aus `LoginPagesDto`.

- [ ] **Step 4: Tests grün**

Run: `cargo test --workspace --all-features`, `cargo test -p hengst-desktop` (ohne Features: prüft den Zweig ohne `dev-offline`; `debug_assertions` ist im Testprofil an, der Lauf sagt also nichts über `dev_build`), `cargo check -p hengst-desktop --release` (Release-Zweig übersetzt) und `cargo tree -p hengst-desktop -e normal` (die Ausgabe enthält weder `hengst-testkit` noch `wiremock`: beide hängen nur an `e2e` und an den Dev-Abhängigkeiten)
Expected: PASS

- [ ] **Step 5: Lint, Format, Commit**

```bash
git add crates/hengst-core/src/auth/store.rs apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/src apps/desktop/src-tauri/tests/no_tokens.rs apps/desktop/src-tauri/capabilities/main.json apps/desktop/src/ipc/client.ts apps/desktop/src/ipc/events.ts apps/desktop/src/ipc/client.test.ts apps/desktop/src/ipc/bindings Cargo.lock
git commit -m "feat(desktop): accounts backend with browser and device code sign-in"
```

---

### Task 8: Konten in der Oberfläche

**Files:**
- Create: `apps/desktop/src/features/accounts/{AccountChip.tsx,AccountMenu.tsx,SignInDialog.tsx,RemoveAccountDialog.tsx,signIn.ts,signIn.test.ts,AccountChip.test.tsx,SignInDialog.test.tsx}`
- Modify: `src/stores/app.ts` (`accounts: Resource<AccountsDto>`), `src/app/Shell.tsx` (Chip in der Kopfzeile), `src/i18n/{de.json,en.json}`, `apps/desktop/package.json`, `pnpm-lock.yaml`, `components.json`; shadcn-Komponenten `dialog`, `alert-dialog`, `dropdown-menu`, `badge` per `pnpm dlx shadcn@4.23.0 add` (Sonner kam in Task 3). Die Client-Funktionen für Konten und `api.openUrl` existieren schon (Task 7 und Task 4); dieser Task fügt keine Commands hinzu

**Interfaces:**
- Consumes: Commands und `api.*` aus Task 7 (`accountsList`, `accountSelect`, `accountRemove`, `loginStart`, `loginCancel`, `loginReopenBrowser`), `api.openUrl` und `UrlTarget` (Task 4), `ErrorPanel` (Task 4), Store und Sonner (Task 3).
- Produces:
  - `signIn.ts`: Reducer `signInReducer(state: SignInState, e: LoginEvent | { event: "start"; method: LoginMethod } | { event: "switchToCode" } | { event: "reopenFailed" } | { event: "reopened" }): SignInState` mit `SignInState = { phase: "idle" } | { phase: "starting"; method: LoginMethod } | { phase: "waitingBrowser"; op: number; reopenFailed: boolean } | { phase: "showingCode"; op: number; userCode: string; verificationUri: string; expiresAt: number } | { phase: "completing"; op: number } | { phase: "done"; account: AccountDto } | { phase: "failed"; error: ErrorDto; method: LoginMethod } | { phase: "appNotApproved" } | { phase: "cancelled" }` (die Phasen der Tabelle in DESIGN.md 11.5; `Failed` mit Code `appNotApproved` führt zu `appNotApproved`, jeder andere Fehler zu `failed`). `start` setzt `starting`; die `op` kommt aus dem ersten Kanal-Ereignis `Started { op }` (nicht aus der Rückgabe von `loginStart`, weil Ereignisse vor ihr eintreffen können), `BrowserOpened` führt von `starting` zu `waitingBrowser { op }`, `DeviceCode` zu `showingCode { op, ... }`. Der Dialog hält je Anmeldeversuch einen eigenen Kanal und verwirft Ereignisse eines abgelösten Versuchs (Zähler im Dialog). `switchToCode` ruft `login_cancel(op)` und startet danach `login_start("deviceCode", ...)`: beide Wege sind dieselbe Vorgangsart, der Browser-Vorgang endet und gibt seinen Port frei. Dazu `signInErrorAction(code: ErrorCode): { kind: "retry" } | { kind: "useCode" } | { kind: "openUrl"; target: UrlTarget; labelKey: string } | { kind: "none" }` nach der Spalte "Action" der Fehlertabelle in DESIGN.md 11.5 (`network`, `offline`, `loginDeclined`, `browserLoginTimedOut`, `deviceCodeExpired`, `browserLoginFailed`, `gameNotOwned`, `credentialStore`: retry, das den Vorgang mit derselben Methode neu startet; `loopbackUnavailable`: useCode; `noXboxAccount`: openUrl `xboxSignup`; `noProfile`: openUrl `minecraftProfile`; `appMisconfigured`, `missingClientId`: none, nur Details) und `signedInMessage(account: AccountDto, resign: { id: string; name: string } | null): { key: string; params: Record<string, string> }` (normal "Angemeldet als {{name}}", beim erneuten Anmelden mit einem anderen Konto "Angemeldet als {{new}}. {{old}} ist weiterhin abgemeldet.").
  - `AccountChip` und `AccountMenu` nach DESIGN.md 10.3 und 11.4: Chip als 40-px-Pille mit Initialkreis, Name (Ellipsis und Tooltip ab 208 px), Chevron und Warn-Badge "Erneut anmelden", Name "Konto: {{name}}"; ohne Konto ein `secondary`-Knopf "Anmelden", der den Dialog öffnet; ist `can_sign_in` false (keine Client-ID), öffnet der Chip nicht den Anmeldedialog, sondern zeigt den Block "Build ohne Client-ID" aus DESIGN.md 11.6 als Hinweisdialog (440 px, Text unten); derselbe Hinweisdialog erscheint bei "Konto hinzufügen" im Menü (bleibt aktiv); Menü: Konten als Radio-Gruppe (Häkchen am gewählten, Klick wählt, Konto mit `sign_in_required` startet die Anmeldung für dieses Konto, Dev-Konto mit Badge "Entwicklungs-Build"), Trenner, "Konto hinzufügen", Untermenü "Abmelden" mit einem Eintrag je Konto (öffnet `RemoveAccountDialog`, ausgeblendet ohne Konten); Laden: 40 × 200 Skeleton; Fehler: Knopf "Konten nicht verfügbar" mit Menü aus Fehlertext und "Erneut versuchen". Unter 880 CSS-Pixeln zeigt der Chip nur Initialkreis und Chevron.
  - `SignInDialog` (Breite 560, DESIGN.md 11.5): Titel "Konto hinzufügen", bei `resign` "{{name}} erneut anmelden"; Phasen und Fußzeilen (höchstens zwei Knöpfe, Hauptaktion zuletzt) exakt nach der Tabelle: `idle` (Text, darunter der Link-Knopf "Code stattdessen verwenden"; Abbrechen, primary "Mit Microsoft anmelden" mit `external-link`), `starting` ("Wird geöffnet…" mit Spinner, träge, Abbrechen bleibt aktiv), `waitingBrowser` (Spinner und "Melde dich im Browser an. Danach geht es hier automatisch weiter.", Zeile "Wartet höchstens 5 Minuten.", der Link "Code stattdessen verwenden"; Abbrechen und `secondary` "Browser erneut öffnen", kein primary; scheitert das Öffnen, steht darunter inline "Der Browser ließ sich nicht öffnen. Verwende stattdessen den Code."), `showingCode` (Anweisung, Codebox in Mono mit `secondary` "Code kopieren", das 2 s "Kopiert" zeigt; Restzeit "Noch 14:32 Min. gültig" jede Sekunde aktualisiert und nicht angesagt; Abbrechen, primary "microsoft.com/link öffnen" über `api.openUrl("deviceLogin")`), `completing` (Spinner, "Wird angemeldet…", beide Knöpfe träge), `failed` (`ErrorPanel` mit "Erneut versuchen" bzw. der Aktion aus `signInErrorAction` als `extraAction`, Wortlaut je Code aus DESIGN.md 11.5; Fußzeile `secondary` "Schließen"), `appNotApproved` (Block unten; `secondary` "Schließen", primary "Mehr erfahren"), `done` (Dialog zu, Toast aus `signedInMessage`), `cancelled` (Dialog zu, Fokus auf den Chip bzw. den Spielen-Knopf). Erste Fokusposition: der primary-Knopf (`idle`) bzw. "Code kopieren" (`showingCode`); Escape bricht den laufenden Vorgang ab und schließt; Klick außerhalb schließt den Dialog nie. Ohne Client-ID (`can_sign_in` false) wird der Anmeldedialog gar nicht geöffnet: Spielen-Knopf, Chip und "Konto hinzufügen" zeigen den Hinweisdialog mit dem Block "Build ohne Client-ID" aus 11.6. Meldet `login_start` trotzdem `missingClientId`, zeigt die `failed`-Phase dessen Text, der in DESIGN.md 11.5 diesem Block gleicht.
  - Sonderfall `appNotApproved` (eigener Block im Dialog statt generischem Fehler, DESIGN.md 11.6: Symbol `info` in `--text-muted`, kein Fehlerstil). Englisch: Titel "Sign-in is not available yet", Text "Mojang has not approved Hengst Client's Microsoft sign-in yet. Until then, accounts cannot be added. Your account is fine, nothing needs to change on your side.", Aktion "Learn more" (`SignInHelp`). Deutsch: "Anmeldung noch nicht verfügbar", "Mojang hat die Microsoft-Anmeldung von Hengst Client noch nicht freigegeben. Bis dahin lassen sich keine Konten hinzufügen. Mit deinem Konto ist alles in Ordnung, du musst nichts ändern.", "Mehr erfahren". In Builds mit `devOffline` zusätzlich "Development builds can use the offline development account." / "Entwicklungs-Builds können das Offline-Entwicklungskonto verwenden."
  - Zweiter Block "Build ohne Client-ID" (`can_sign_in` false, DESIGN.md 11.6; ein eigener Hinweisdialog mit 440 px, gleicher Stil wie der Block oben, nie der Anmeldedialog): Titel "Dieser Build kann sich nicht anmelden" / "This build cannot sign in", Text "Er wurde ohne Microsoft-Client-ID gebaut. Verwende ein offizielles Release oder trage beim eigenen Bauen die Client-ID ein." / "It was built without a Microsoft client id. Use an official release, or set the client id when building it yourself."; Knöpfe `secondary` "Schließen" / "Close" (erste Fokusposition) und primary "Anleitung zum Bauen" / "Build instructions" über `api.openUrl("buildHelp")`. i18n-Schlüssel getrennt von `appNotApproved`: `signIn.noClientId.{title,body,help}` und `signIn.notApproved.{title,body,help}`; `errors.missingClientId.{title,body}` übernimmt den Text von `signIn.noClientId` (ein Wortlaut, eine Quelle).
  - `RemoveAccountDialog` (Breite 440): Titel "{{name}} abmelden?", Text "Die gespeicherte Anmeldung wird von diesem PC entfernt. Dein Microsoft-Konto bleibt unverändert.", Knöpfe "Behalten" (`secondary`, erste Fokusposition) und "Abmelden" (destructive); Erfolg: Toast "{{name}} abgemeldet", Fokus auf den Chip.

- [ ] **Step 1: Zustandsliste** (produkt-regelwerk) für Chip, Menü, beide Dialoge in den Bericht: Hauptaktion je Zustand, alle Phasen (`idle`, `starting`, `waitingBrowser` mit Wartehinweis und Zeitgrenze, `showingCode`, `completing`, `done`, `failed` je Fehlercode aus DESIGN.md 11.5, `appNotApproved`, `cancelled`), erneutes Anmelden mit eigenem Titel und Konto-Abweichung, Chip ohne Konto, ohne Client-ID und mit Kontenfehler, Doppelklick auf "Mit Microsoft anmelden", Abbruch mit Escape (bricht Vorgang ab, Fokus zurück auf Chip), längster Name 16 Zeichen (Minecraft-Namen) plus Badge.

- [ ] **Step 2: Failing Tests schreiben**

```ts
// signIn.test.ts
it("started then browserOpened -> waitingBrowser with the op from started", ...);
it("started arrives before login_start resolves and the op is still known", ...);   // Reducer bekommt Started zuerst, loginStart-Promise offen
it("switchToCode cancels the browser login with its op and ignores that channel's late events", ...);
it("deviceCode event -> showingCode with code and expiry", ...);
it("failed keeps the error and the method so retry restarts the same method", ...);
it("appNotApproved error leads to the appNotApproved phase, not failed", ...);
it("reopenFailed in waitingBrowser sets the inline error and keeps the wait", ...);
it("signInErrorAction follows the action column of DESIGN.md 11.5", () => {
  // loopbackUnavailable -> useCode; noXboxAccount -> openUrl xboxSignup; noProfile -> openUrl minecraftProfile;
  // appMisconfigured und missingClientId -> none; network, offline, loginDeclined, browserLoginTimedOut, deviceCodeExpired, browserLoginFailed, gameNotOwned, credentialStore -> retry
});
it("signedInMessage names both accounts when another account signed in again", () => {
  // signedInMessage({ id: "b", name: "SecondPlayer", ... }, { id: "a", name: "E2EPlayer" }) -> Schlüssel "accounts.signedInOther", params { new: "SecondPlayer", old: "E2EPlayer" }
  // gleiche id oder resign null -> "accounts.signedIn" mit { name }
});
it("login error copy matches DESIGN.md 11.5 in de and en", () => {
  // liest die Tabelle "Error copy in the failed phase" aus DESIGN.md (Spalten Code, Titel, Text je "de / en") und vergleicht errors.<code>.title und .body
  // für network, offline, loginDeclined, browserLoginTimedOut, deviceCodeExpired, loopbackUnavailable, browserLoginFailed, appMisconfigured, missingClientId, noXboxAccount, gameNotOwned, noProfile, credentialStore
  // missingClientId: Titel und Text sind gleich Titel und Text des Blocks "Build ohne Client-ID" in 11.6 und verschieden vom Block appNotApproved (eigene i18n-Schlüssel)
});
it("a late event after cancelled is ignored", ...);    // signedIn nach cancelled ändert nichts
// SignInDialog.test.tsx (mockIPC mit Channel-Simulation: Handler ruft args.channel.onmessage o. ä. nach Tauri-Mock-Doku)
it("double click on sign in starts one login", ...);    // login_start genau einmal
it("appNotApproved shows the explanation, not the generic error", ...);
it("escape cancels the running login", ...);            // login_cancel mit der op aus Started aufgerufen
it("no client id never opens the sign-in dialog: Play button, chip and add-account entry show the no-client-id dialog", ...);
it("build instructions in the no-client-id dialog open the BuildHelp target", ...);   // open_url mit "buildHelp" als einzigem Argument
it("learn more on appNotApproved opens the SignInHelp target", ...);   // open_url mit "signInHelp" als einzigem Argument
it("every phase has at most two footer buttons and the primary one is last", ...);   // idle, starting, waitingBrowser (kein primary), showingCode, completing, failed, appNotApproved
it("waitingBrowser shows the 5 minute caption, the use-a-code link and the reopen button", ...);
it("showingCode counts down in whole seconds and the copy button reads Copied for 2 seconds", ...);   // Fake-Timer
it("re-signing in an account uses the resign title and names the old account when another one signed in", ...);
it("outside pointer-down does not close the dialog", ...);
it("loopbackUnavailable offers the code flow as the action", ...);   // Klick startet login_start("deviceCode", ...)
// AccountChip.test.tsx
it("shows the selected account name and switches on click", ...);   // account_select mit id von B
it("without an account the chip is a Sign in button that opens the dialog", ...);
it("without a client id the chip stays active and opens the menu with the explanation and a disabled Add account", ...);
it("the account list failing shows Accounts unavailable with a retry in the menu", ...);
it("the dev account carries the Development build badge", ...);
it("remove keeps focus on Keep first, names the account and returns focus to the chip", ...);
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
Expected: Leerzustand, Anmelden (Browser-Weg läuft im e2e-Build ohne Browser durch), zweites Konto, Wechsel, Abmelden mit Dialog; "Code stattdessen verwenden" beendet den wartenden Browser-Vorgang (kein offener Port: ein zweiter Start desselben Ablaufs funktioniert); "microsoft.com/link öffnen" öffnet die feste Adresse (Browserfenster in `list_windows`). Mock mit `HENGST_MOCK_SCENARIO=not-approved`: Erklärung erscheint, "Mehr erfahren" öffnet das Repository. Fehlerzustand in der laufenden App: Mock beenden, dann Anmelden versuchen (Netzfehler im Dialog über `ErrorPanel`, in beiden Themes und Sprachen). Dev-Konto in der Liste sichtbar mit Badge. Browser-Warten mit Hinweis und Zeitgrenze, "Browser erneut öffnen" mit gestopptem Mock (Inline-Fehler), Codeansicht mit Restzeit. "Erneut anmelden" und die Konto-Abweichung sind in der App erst mit 4b erreichbar (`sign_in_required` entsteht beim Start) und stehen deshalb in den Vitest-Tests. Hell/dunkel, de/en, 960 × 600, 1200 × 800 und 1920 × 1080, dazu Zoom 150 % bei 960 × 600 (Chip nur mit Initialkreis), Tastatur-Durchlauf durch Menü und Dialog (Escape, Fokus zurück). Screenshots nach `evidence/task-8/`. **CSP- und Prototyp-Prüfung** (neu: Dialog, AlertDialog, DropdownMenu auf Base-UI-Basis): Web-Konsole beim Durchklicken lesen, keine CSP-Meldung, `Object.isFrozen(Object.prototype)` ist `true`; braucht eine Komponente ein Inline-`<style>`, gilt die Regel aus den Global Constraints (Nonce oder Hash, nie `'unsafe-inline'` für Skripte oder Style-Elemente). Die echte Microsoft-Anmeldung bleibt ein offener manueller Schritt (Redirect-URI, siehe "Beim Menschen").

- [ ] **Step 7: Lint, Format, Commit**

```bash
git add apps/desktop/src apps/desktop/src-tauri/tauri.conf.json apps/desktop/components.json apps/desktop/package.json apps/desktop/pnpm-lock.yaml
git commit -m "feat(desktop): account chip, sign-in dialog with browser and code flow, sign out"
```

---

### Task 9: Kern: Instanzen-Modul

**Files:**
- Create: `crates/hengst-core/src/instances.rs`
- Modify: `crates/hengst-core/src/lib.rs` (`pub mod instances;`), `crates/hengst-core/src/error.rs`, `crates/hengst-core/src/loaders/mod.rs` (`LoaderKind`: serde über Slug; `display_name` gibt es seit Task 4), `apps/desktop/src-tauri/src/error.rs`, `apps/desktop/src-tauri/src/dto/error.rs` (neue Codes), `apps/desktop/src/ipc/bindings/` (erzeugt), `apps/desktop/src/i18n/{de.json,en.json}`, `apps/desktop/src/features/errors/errorCopy.ts`

**Interfaces:**
- Consumes: `Paths::instance`, `validate_id`, `LoaderKind`, `ResolvedLoader`, `LaunchOptions`, `LoaderRequest`, `args::Account`.
- Produces:
  - `LoaderKind`: `Serialize`/`Deserialize` als `slug()` (`"fabric"`, `"quilt"`, `"legacy-fabric"`, `"neoforge"`, `"forge"`). Der Anzeigename kommt aus `LoaderKind::display_name` (Task 4); der Slug ist nie Anzeigetext.
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
  - `pub fn instance_dir(paths: &Paths, id: &str) -> Result<PathBuf>` (prüft `validate_id`, dann dass `instances/<id>/instance.json` eine Datei ist; sonst `InstanceNotFound`; der einzige Weg, auf dem ein Command eine Id aus JavaScript in einen Ordner verwandelt: ein CLI-Ordner ohne `instance.json` ist hier **nicht** gültig)
  - `pub fn load(paths: &Paths, id: &str) -> Result<Instance>` (ruft `instance_dir`; fehlende Datei `InstanceNotFound`)
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
#[test]
fn instance_dir_needs_a_valid_id_and_an_instance_json() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    std::fs::create_dir_all(paths.instance("1.20.1-forge").join("saves")).unwrap();      // CLI-Ordner ohne instance.json
    create(&paths, &NewInstance { name: "Welt".into(), game: "1.21.11".into(), loader: None }, 1).unwrap();
    assert!(instance_dir(&paths, "1.21.11").is_ok());
    assert!(matches!(instance_dir(&paths, "1.20.1-forge"), Err(Error::InstanceNotFound { .. })));
    assert!(matches!(instance_dir(&paths, "fehlt"), Err(Error::InstanceNotFound { .. })));
    for bad in ["..", "..\\x", "a/b", ""] {
        assert!(matches!(instance_dir(&paths, bad), Err(Error::UnsafePath(_))), "{bad:?}");
    }
}
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
Expected: PASS, CLI-Ausgaben unverändert.

- [ ] **Step 5: Lint, Format, Commit**

```bash
git add crates/hengst-core/src/instances.rs crates/hengst-core/src/lib.rs crates/hengst-core/src/error.rs crates/hengst-core/src/loaders/mod.rs apps/desktop/src-tauri/src/error.rs apps/desktop/src-tauri/src/dto/error.rs apps/desktop/src/ipc/bindings apps/desktop/src/i18n apps/desktop/src/features/errors/errorCopy.ts
git commit -m "feat(core): instances module with instance.json format 1 and CLI folder adoption"
```

---

### Task 10: Instanzen im Backend, Ordner und Links öffnen

**Files:**
- Create: `apps/desktop/src-tauri/src/{dto/instances.rs,commands/instances.rs}`
- Modify: `apps/desktop/src-tauri/src/{state.rs,command_list.rs,commands/mod.rs,dto/mod.rs,dto/error.rs,error.rs,open.rs,commands/open.rs}` (`open.rs` und `commands/open.rs` gibt es seit Task 4: hier kommen `OpenTarget`, `resolve_location` und `open_location` dazu), `src-tauri/capabilities/main.json` (`allow-loader-kinds`, `allow-loader-versions`, `allow-instances-list`, `allow-instance-create`, `allow-instance-select`, `allow-instance-delete`, `allow-open-location`), `src-tauri/tests/no_tokens.rs` (Instanz-DTOs aufnehmen), `apps/desktop/src-tauri/Cargo.toml` (`trash` per `cargo add`), `apps/desktop/src/ipc/client.ts`, `apps/desktop/src/ipc/bindings/` (erzeugt), `apps/desktop/src/i18n/{de.json,en.json}`, `apps/desktop/src/features/errors/errorCopy.ts`, `Cargo.lock`
- Test: `apps/desktop/src-tauri/src/commands/instances.rs` und `src/open.rs` (`#[cfg(test)]`), `apps/desktop/src/ipc/client.test.ts`

**Interfaces:**
- Consumes: `instances::*` (Task 9), `loaders::{list_versions, pick_version, LoaderKind}`, `download::http_client`, `VersionsCache` (Task 2), `SettingsHandle` (Task 3), `LoaderKind::display_name` (Task 4), `open.rs` und `UrlTarget` (Task 4).
- Produces:
  - DTOs: `LoaderKindDto` (serialisiert als der Kern-Slug: `"fabric"`, `"quilt"`, `"legacy-fabric"`, `"forge"`, `"neoforge"`; Abbildung auf `LoaderKind` in beide Richtungen, ts-rs-Union genau dieser Strings, kein eigenes camelCase), `LoaderInfoDto { kind: LoaderKindDto, display_name: String }` (Anzeigename aus `LoaderKind::display_name`, die einzige Quelle für Loader-Namen in der Oberfläche), `LoaderVersionDto { id: String, stable: bool }`, `LoaderVersionsDto { versions: Vec<LoaderVersionDto>, recommended: String }`, `InstanceLoaderDto { kind: LoaderKindDto, display_name: String, version: Option<String> }` (Slug und Anzeigename), `InstanceStatus` (`ready`, `tooNew`, `broken`), `InstanceDto { id: String, name: String, game: String, loader: Option<InstanceLoaderDto>, memory_mb: Option<u32>, last_played: Option<u64>, status: InstanceStatus, selected: bool }` (bei `tooNew`/`broken` sind `name` = Id und `game` leer), `CreateInstanceRequest { name: String, game: String, loader: Option<LoaderRequestDto> }` mit `LoaderRequestDto { kind: LoaderKindDto, version: Option<String> }` (die Oberfläche schickt nur Slug und Version, nie einen Anzeigenamen), `OpenTarget` (`#[serde(tag = "kind", rename_all = "camelCase")]`: `DataDir`, `Instance { id: String }`). `UrlTarget` gibt es seit Task 4 (mit `DeviceLogin`).
  - Commands (Einträge in `command_list.rs`: `instances::loader_kinds`, `instances::loader_versions`, `instances::instances_list`, `instances::instance_create`, `instances::instance_select`, `instances::instance_delete`, `open::open_location`): `loader_kinds() -> Result<Vec<LoaderInfoDto>, ErrorDto>` (Reihenfolge Fabric, Quilt, Legacy Fabric, Forge, NeoForge); `loader_versions(kind: LoaderKindDto, game: String) -> Result<LoaderVersionsDto, ErrorDto>` (`recommended` = `pick_version(&list, None, kind, &game)`); `instances_list() -> Result<Vec<InstanceDto>, ErrorDto>` (nimmt `instance_lock`, ruft zuerst `adopt` mit den Ids aus dem gecachten Manifest, ohne Manifest-Cache keine Übernahme, dann `list`); `instance_create(req: CreateInstanceRequest) -> Result<InstanceDto, ErrorDto>` (prüft `game` gegen die Versionsliste und, mit Loader, die Version gegen `list_versions`; wählt die neue Instanz in den Einstellungen über `SettingsHandle::update`; `DataChanged(Instances)` und `DataChanged(Settings)`); `instance_select(id: String) -> Result<(), ErrorDto>` (`instances::load(id)` muss `Ready` liefern, sonst `InstanceNotFound`; speichert `selected_instance` über `SettingsHandle::update`); `instance_delete(id: String) -> Result<(), ErrorDto>` (`instances::instance_dir(id)` muss gelingen: unbekannte Ids, unsichere Ids und CLI-Ordner ohne `instance.json` werden **abgelehnt, der Ordner bleibt unberührt**; ist `AppState.active_instance` (`Mutex<Option<String>>`, in 4a immer `None`, 4b Task 15 setzt sie für die vorbereitende und die laufende Instanz) diese Id, kommt `ErrorCode::InstanceRunning` und der Ordner bleibt unberührt; dann löscht `AppState.trasher` (`Arc<dyn Fn(&Path) -> hengst_core::Result<()> + Send + Sync>`, Standard `trash::delete`) in den Papierkorb; scheitert das (Ordner zu groß für den Papierkorb, Laufwerk ohne Papierkorb), kommt `ErrorCode::RecycleBinFailed` ohne Aktion, der Ordner und `selected_instance` bleiben unberührt und die App löscht nie endgültig (DESIGN.md 11.3); war sie gewählt, wird `selected_instance` `None`; `DataChanged(Instances)`); `open_location(target: OpenTarget) -> Result<(), ErrorDto>` (`Instance { id }`: `instance_dir`, `DataDir`: der Datenordner). Frontend `client.ts`: `api.loaderKinds()`, `api.loaderVersions(kind, game)`, `api.instancesList()`, `api.instanceCreate(req)`, `api.instanceSelect(id)`, `api.instanceDelete(id)`, `api.openLocation(target)` (`api.openUrl` gibt es seit Task 4).
  - Serialisierung: `AppState.instance_lock: tokio::sync::Mutex<()>` ist die **eine** Sperre für `instances_list` (einschließlich `adopt`), `instance_create` und `instance_delete`, damit `list`/`adopt` nie einen Ordner übernehmen, den `create` angelegt, aber noch nicht gefüllt hat. `instance_create` und `instance_delete` warten nicht: läuft schon einer von beiden, bekommt der zweite `ErrorCode::Busy` (neu; `try_lock`). `instances_list` wartet kurz auf die Sperre (`lock().await`), damit eine Aktualisierung der Liste nie als Fehler erscheint.
  - `open.rs` (kommt zu `resolve_url` aus Task 4 hinzu): `fn resolve_location(paths: &Paths, t: &OpenTarget) -> Result<PathBuf>` (`Instance`: `instances::instance_dir`, also `validate_id` und vorhandene `instance.json`, sonst `UnsafePath` bzw. `InstanceNotFound`; angezeigte `tooNew`- und `broken`-Instanzen haben eine `instance.json` und lassen sich öffnen). Öffnen über `tauri_plugin_opener::OpenerExt` (`open_path`).

- [ ] **Step 1: Failing Tests schreiben** (Logik in `*_impl`-Funktionen; Testkit-Mojang-Mock)

```rust
#[tokio::test] async fn create_validates_game_and_loader_version() {
    // 9.9 + fabric 0.99.0 -> Ok, id "9.9-fabric", selected; 9.9 + fabric 0.0.1 -> UnknownLoaderVersion; "8.8" -> UnknownVersion; b9.9 + fabric -> NoLoaderVersions
}
#[tokio::test] async fn loader_versions_mark_the_recommended_build() { /* 9.9 fabric -> recommended "0.99.0", Liste enthält "0.100.0-beta.1" mit stable false */ }
#[tokio::test] async fn list_adopts_cli_folders_when_the_manifest_is_cached() { /* Ordner "9.9-fabric" ohne Datei, Manifest im Cache -> Liste enthält id "9.9-fabric", name "9.9 Fabric" */ }
#[tokio::test] async fn delete_error_when_the_recycle_bin_refuses_leaves_everything_untouched() { /* Test-Trasher liefert Err -> code RecycleBinFailed, action None; Ordner samt saves/ unverändert, selected_instance unverändert, kein DataChanged, kein endgültiges Löschen */ }
#[tokio::test] async fn delete_is_refused_for_the_active_instance() { /* active_instance = Some("9.9") -> code InstanceRunning, Ordner unberührt; mit None gelingt es */ }
#[tokio::test] async fn delete_moves_the_folder_away_and_clears_selection() { /* nach delete existiert der Ordner nicht mehr; selected_instance None; DataChanged(Instances) */ }
#[tokio::test] async fn create_and_delete_are_serialized() { /* zwei create gleichzeitig (join) -> eins Ok, eins Busy; danach genau ein Ordner */ }
#[test] fn open_location_refuses_unknown_and_unsafe_ids() { /* Instance { id: "..\\x" } -> UnsafePath; Instance { id: "fehlt" } -> InstanceNotFound; CLI-Ordner ohne instance.json -> InstanceNotFound; DataDir -> Ok(root) */ }
#[tokio::test]
async fn delete_refuses_unknown_and_unlisted_ids() {
    // Temp-Home: Instanz "9.9" (mit instance.json) und CLI-Ordner "9.9-fabric" mit saves/world/level.dat (ohne instance.json)
    let r = instance_delete_impl(&state, "..\\x").await;            assert_eq!(r.unwrap_err().code, ErrorCode::UnsafeData);
    let r = instance_delete_impl(&state, "fehlt").await;            assert_eq!(r.unwrap_err().code, ErrorCode::InstanceNotFound);
    let r = instance_delete_impl(&state, "9.9-fabric").await;       assert_eq!(r.unwrap_err().code, ErrorCode::InstanceNotFound);
    assert!(paths.instance("9.9-fabric").join("saves/world/level.dat").is_file(), "an unlisted folder must stay untouched");
    assert!(instance_delete_impl(&state, "9.9").await.is_ok());
}
#[tokio::test]
async fn select_refuses_unknown_unreadable_and_unlisted_ids() {
    // "fehlt", "..\\x", CLI-Ordner ohne instance.json, Instanz mit "{" und Instanz mit format 2 -> Err; settings.selected_instance bleibt unverändert
}
#[tokio::test]
async fn list_waits_for_create_and_never_adopts_a_half_made_folder() {
    // instance_lock halten, instances_list starten -> nach 200 ms noch nicht fertig (is_finished() == false); Lock freigeben -> list endet Ok
}
#[tokio::test] async fn loader_kinds_carry_display_names() { /* Legacy Fabric -> kind "legacy-fabric", displayName "Legacy Fabric"; NeoForge -> "neoforge"/"NeoForge" */ }
#[tokio::test] async fn created_instance_dto_has_slug_and_display_name() { /* create 9.9 + fabric -> loader.kind "fabric", loader.displayName "Fabric" */ }
#[test] fn instance_dtos_never_contain_a_token() { /* InstanceDto, LoaderVersionsDto, CreateInstanceRequest in tests/no_tokens.rs aufnehmen; "notes" mit FAKE-ACCESS-TOKEN-7f3a in instance.json taucht in keinem DTO auf, weil `extra` nie serialisiert wird */ }
```
Hinweis zum Löschtest: Die Tests setzen einen eigenen `trasher` ein (verschiebt den Ordner in einen Temp-Ordner oder scheitert), der echte Papierkorb wird in Tests nicht benutzt; ein Test mit dem Standard-Trasher auf einem kleinen Ordner unter `%TEMP%` ist ein einmaliger Rauchtest in der Sichtprüfung.

- [ ] **Step 2: Tests laufen lassen, sie scheitern**

Run: `cargo test -p hengst-desktop instances`
Expected: FAIL

- [ ] **Step 3: Implementieren** wie in Interfaces; `ErrorCode::Busy` mit Übersetzung ("Gerade läuft schon ein Vorgang. Warte einen Moment."), `ErrorCode::RecycleBinFailed` (Titel "Ordner konnte nicht in den Papierkorb verschoben werden", Text "Es wurde nichts gelöscht. Öffne den Ordner, um ihn selbst zu verschieben oder zu löschen.", DESIGN.md 11.3) und `ErrorCode::InstanceRunning` ("Läuft gerade. Beende zuerst das Spiel.").

- [ ] **Step 4: Tests grün**

Run: `cargo test --workspace --all-features`
Expected: PASS

- [ ] **Step 5: Lint, Format, Commit**

```bash
git add apps/desktop/src-tauri/src apps/desktop/src-tauri/tests/no_tokens.rs apps/desktop/src-tauri/capabilities/main.json apps/desktop/src-tauri/Cargo.toml apps/desktop/src/ipc/client.ts apps/desktop/src/ipc/client.test.ts apps/desktop/src/ipc/bindings apps/desktop/src/i18n apps/desktop/src/features/errors/errorCopy.ts Cargo.lock
git commit -m "feat(desktop): instance commands, loader versions and validated open actions"
```

---

### Task 11: Instanzen in der Oberfläche

**Files:**
- Create: `apps/desktop/src/features/instances/{InstancesView.tsx,InstanceRow.tsx,CreateInstanceDialog.tsx,DeleteInstanceDialog.tsx,naming.ts,versionFilter.ts,naming.test.ts,versionFilter.test.ts,CreateInstanceDialog.test.tsx,InstancesView.test.tsx}`
- Modify: `apps/desktop/src/stores/app.ts` (`instances: Resource<InstanceDto[]>`), `src/app/{Shell.tsx,views.ts}`, `src/features/settings/SettingsView.tsx` ("Ordner öffnen" für den Datenordner über `api.openLocation({ kind: "dataDir" })`), `src/i18n/{de.json,en.json}`, `apps/desktop/package.json`, `pnpm-lock.yaml`, `components.json`; shadcn `popover`, `command`, `select`, `checkbox`, `input`, `label` per `pnpm dlx shadcn@4.23.0 add` (`radio-group`, `skeleton`, `tooltip`, `sonner` kamen in Task 3, `dialog`, `alert-dialog`, `dropdown-menu`, `badge` in Task 8). Die Client-Funktionen (`api.loaderKinds` bis `api.openLocation`) gibt es seit Task 10, `api.openUrl` seit Task 4: dieser Task fügt keine Commands hinzu

**Interfaces:**
- Consumes: Commands und `api.*` aus Task 10, `game_versions` (Task 2), `ErrorPanel`, Store.
- Produces:
  - `versionFilter.ts`: `filterVersions(list: GameVersionDto[], f: VersionFilter, query: string): GameVersionDto[]` (Suche als Teilstring ohne Groß-/Kleinschreibung auf `id`), `DEFAULT_FILTER: VersionFilter = { release: true, snapshot: false, beta: false, alpha: false }`.
  - `naming.ts`: `suggestName(game: string, loader: LoaderInfoDto | null): string` (`"1.20.1 Forge"`, `"1.21.11"`); es gibt keine eigene Tabelle mit Loader-Namen im Frontend: Anzeigenamen kommen ausschließlich aus `LoaderInfoDto.displayName` (`loader_kinds`, Quelle `LoaderKind::display_name` im Kern), geschickt wird nur der Slug `kind`. Der Name folgt der Auswahl, bis der Nutzer das Feld selbst ändert.
  - `selection.ts`: `effectiveSelection(list: InstanceDto[]): string | null` (die gewählte Instanz, wenn sie `ready` ist; sonst die erste `ready`-Instanz in Listenreihenfolge, also die zuletzt gespielte, sonst die erste; `null` ohne lesbare Instanz; DESIGN.md 10.2 "Selection fallback"). Wird nach dem Löschen und beim ersten Start benutzt; Task 16 benutzt dieselbe Funktion.
  - `InstancesView` nach DESIGN.md 10.4 und 11.1: Kopf mit h1 "Instanzen" und Hauptaktion "Instanz anlegen" (im Leerzustand ausgeblendet, dort steht sie im Block); Spaltenkopf (32 px, "Name", "Version", "Loader", "Zuletzt gespielt", Linie darunter) und Liste (`<ul>`, Zeilen 48 px, Spalten und Breiten nach 10.4; Schmal-Layout unter 880 CSS-Pixeln nach 7.3: ohne "Zuletzt gespielt", Version und Loader als zwei Spans unter dem Namen, Zeile 64 px); Name einzeilig mit Ellipsis und Tooltip, Version und Loader in sekundärer Schrift ohne Mittelpunkt-Ketten, "Zuletzt gespielt" per `Intl.RelativeTimeFormat` ("Noch nie gespielt"); die Zeile ist ein Knopf mit `aria-current="true"` und Häkchen, wenn gewählt, der Aktionsknopf ("Aktionen für {{name}}", `ghost`, `ellipsis`) ein Geschwister; Zeilenmenü: "Ordner öffnen", Trenner, "Löschen" (`--danger`), bei vorbereitender oder laufender Instanz deaktiviert mit der Zeile "Läuft gerade. Beende zuerst das Spiel." (Store-Feld `activeInstance`, in 4a immer `null`, in 4b aus dem Spielzustand); `tooNew`/`broken`-Zeilen: Name gedämpft, Version und Loader zu einem Warn-Badge ("Neuere Version" bzw. "Nicht lesbar") verschmolzen, nicht wählbar (`aria-disabled`, vom Badge beschrieben), Menü nur "Ordner öffnen". Entf auf einer fokussierten Zeile öffnet den Löschdialog (gleiche Regeln wie im Menü). Laden: Spaltenkopf plus drei Skeleton-Zeilen nach 200 ms; Leer: Block (max. 480 px, linksbündig) mit h2 "Noch keine Instanz", einem Satz und "Instanz anlegen"; Fehler: `ErrorPanel` statt der Liste.
  - `CreateInstanceDialog` (Breite 560, DESIGN.md 10.9, 11.2): Felder in dieser Reihenfolge: Spielversion (Combobox aus Popover + Command nach 10.5, Suche, darunter Filter-Checkboxen Releases/Snapshots/Beta/Alpha), Loader (Segmente aus `Segments.tsx`, Task 3: "Kein Loader" plus die Einträge aus `loader_kinds` in deren Reihenfolge, beschriftet mit `displayName`), Loader-Version (Select nach 10.6; Einträge mit Badge "Empfohlen" bzw. "Instabil"; Vorauswahl `recommended`; ist der Loader für die Version nicht verfügbar, steht dort der übersetzte Hinweis aus `errors.noLoaderVersions` mit `hint` und der Knopf bleibt aus, begründet über `aria-describedby`), Name (Input, 64 Zeichen, vorbelegt bis zur eigenen Änderung, Hilfezeile mit fester Höhe "So heißt die Instanz in der Liste."). Fußzeile: "Abbrechen" (`secondary`) und "Instanz anlegen" (primary; während des Aufrufs "Wird angelegt…", gesperrt). Zustände: Laden (Auslöser "Versionen werden geladen…", Loader-Version "Wird geladen…"); Versionsliste aus dem Cache bei fehlender Verbindung: Zeile unter dem Feld mit `wifi-off` "Offline: gespeicherte Liste" (`versionsStale` ist `"offline"`) bzw. "Gespeicherte Liste, nicht aktuell" (`"other"`); ohne Cache und ohne Verbindung (Fehlercode `offline` oder `network`): Inline-Fehler "Keine Verbindung. Die Versionsliste braucht beim ersten Mal Internet." mit "Erneut versuchen", der Anlegen-Knopf ist deaktiviert und durch diesen Text begründet; anderer Ladefehler: Inline-Fehler mit "Erneut versuchen"; Loader-Versionen ohne Verbindung: "Keine Verbindung. Loader-Versionen können gerade nicht geladen werden." mit "Erneut versuchen"; leere Suche: "Keine Version gefunden für „{{query}}“" mit "Filter zurücksetzen". Validierung: leer "Gib einen Namen ein.", über 64 "Höchstens 64 Zeichen.", Zeilenumbrüche beim Einfügen entfernt; Fehler beim Anlegen: `ErrorPanel` direkt über der Fußzeile, Eingaben bleiben, Fokus auf den Titel des Panels. Verhalten: erste Fokusposition der Versionsauslöser; Escape schließt und behält den Entwurf (Wiederöffnen in derselben Sitzung stellt ihn her, ein erfolgreiches Anlegen löscht ihn); ein Klick außerhalb schließt nie. Erfolg: Dialog zu, Toast "{{name}} angelegt", neue Zeile gewählt, in den Sichtbereich gescrollt und fokussiert.
  - `DeleteInstanceDialog` (Breite 440, DESIGN.md 11.3): Titel "{{name}} löschen?", Text "Der Ordner mit Welten, Einstellungen und Screenshots wird in den Papierkorb verschoben.", Knöpfe "Behalten" (`secondary`, erste Fokusposition) und "In den Papierkorb" (destructive, Laden "Wird verschoben…", beide Knöpfe träge). Fehler: `ErrorPanel` im Dialog über den Knöpfen, der Dialog bleibt offen; bei `recycleBinFailed` mit `extraAction` "Ordner öffnen" (`api.openLocation({ kind: "instance", id })`). Erfolg: Dialog zu, Zeile weg, Toast "{{name}} ist im Papierkorb", Auswahl nach `effectiveSelection`; Fokus danach auf den Aktionsknopf der nächsten Zeile, sonst der vorigen, sonst "Instanz anlegen".

- [ ] **Step 1: Zustandsliste** (produkt-regelwerk) für Liste, Anlegen, Löschen in den Bericht; Extremwerte: 0, 1, 40 Instanzen; Name mit 64 Zeichen ("Aleksandra Wiśniewska-Kowalczyks Überlebenswelt mit Freunden 26.3"); Versions-Id `1.14 Pre-Release 1`; Loader-Liste leer; Versionsliste aus Cache ohne Netz; Versionsliste ohne Cache und ohne Netz; Löschen der gewählten Instanz (Auswahl-Rückfall); Löschen scheitert am Papierkorb; Liste im schmalen Layout.

- [ ] **Step 2: Failing Tests schreiben**

```ts
// versionFilter.test.ts
it("default filter keeps releases only", ...);
it("query matches case-insensitively and keeps manifest order", ...);   // "PRE" findet "1.14 Pre-Release 1"
// naming.test.ts
it("suggests '<game> <Loader>' from the display name", () => { expect(suggestName("1.20.1", { kind: "forge", displayName: "Forge" })).toBe("1.20.1 Forge"); expect(suggestName("1.21.11", null)).toBe("1.21.11"); expect(suggestName("1.8.9", { kind: "legacy-fabric", displayName: "Legacy Fabric" })).toBe("1.8.9 Legacy Fabric"); });
// CreateInstanceDialog.test.tsx (mockIPC)
it("double_submit_creates_one_instance", ...);                // zwei Klicks -> instance_create genau einmal, Knopf zeigt Ladezustand
it("preselects the recommended loader version", ...);
it("sends only the slug and the version to instance_create", ...);   // loader: { kind: "legacy-fabric", version: "..." }, kein displayName
it("explains why create is disabled when the loader has no builds", ...);  // Text mit Hinweis sichtbar, Knopf aria-disabled mit aria-describedby
it("keeps a typed name when the version changes", ...);
it("keeps inputs after a failed create and shows the error at the form", ...);
it("escape closes and keeps the draft, reopening restores it, a successful create clears it", ...);
it("an outside pointer-down does not close the dialog and focus starts on the version trigger", ...);
it("validates the name: empty gives 'Enter a name.', 65 characters give 'At most 64 characters.', pasted line breaks are removed", ...);
it("offline with a cached list shows the offline caption, with another refresh failure the not-current caption", ...);   // versionsStale offline / other
it("offline without a cached list shows the connection error with Try again and a disabled create button described by it", ...);
it("loader versions offline show their own text with Try again", ...);
it("the created row is selected, scrolled into view and focused", ...);
// InstancesView.test.tsx
it("shows the empty state with one primary action", ...);
it("delete asks first and calls instance_delete after confirm only", ...);
it("too new instance offers only open folder, is aria-disabled and shows the Newer format badge", ...);
it("the selected row has aria-current true and a check", ...);
it("the delete key on a focused row opens the delete dialog", ...);
it("delete is disabled with its reason for the active instance", ...);   // activeInstance gesetzt
it("keep has initial focus in the delete dialog", ...);
it("a failed move to the Recycle Bin shows the panel with Open folder and keeps the dialog open", ...);   // instance_delete lehnt mit recycleBinFailed ab; open_location { kind: "instance", id }
it("after a delete focus moves to the next row's action button, else the previous, else Create instance", ...);
it("the empty state hides the header action and the column header", ...);
// selection.test.ts
it("effectiveSelection prefers the selected ready instance, else the first ready one, else null", ...);
```

- [ ] **Step 3: Tests laufen lassen, sie scheitern**

Run: `pnpm -C apps/desktop test instances`
Expected: FAIL

- [ ] **Step 4: Implementieren** nach DESIGN.md; Gegenprüfung C1 bis C27 (C1 Labels, C4 Sperre, C6 Leer/Fehler, C10 Fehler verschiebt nichts, C13 erklärter Sperrzustand, C14 Textlängen, C17 Dialogtexte).

- [ ] **Step 5: Tests grün**

Run: `pnpm -C apps/desktop typecheck && pnpm -C apps/desktop test && cargo test --workspace --all-features`
Expected: PASS

- [ ] **Step 6: Sichtprüfung gegen den Mock und echt**

Gegen `hengst-mock` (wie Task 8): leer, anlegen (Filter umschalten, Suche, Fabric mit Empfohlen, Beta-Version ohne Fabric zeigt Hinweis), Liste, Ordner öffnen (Explorer-Fenster erscheint, `list_windows`), löschen mit Dialog. Zusätzlich ein Debug-Build ohne `e2e` mit echtem Netz: Versionsliste von Mojang, Fabric- und Forge-Versionen für 1.21.11 laden (keine Spielstarts). Vorab per CLI `hengst --home %TEMP%\hengst-4a-t11 launch 1.21.11 --offline Dev --dry-run` einen CLI-Ordner erzeugen: erscheint als "1.21.11" in der Liste. Netz weg: Mock beenden, App neu starten (Cache vorhanden: Zeile "Offline: gespeicherte Liste"; frischer Home-Ordner: Fehler mit "Erneut versuchen" und gesperrtem Anlegen-Knopf). Namensspalte der Zeilen messen (nach DESIGN.md 7.3 bei 960 px Fensterbreite 248 px, bei 1200 px 320 px). Hell/dunkel, de/en, 960 × 600, 1200 × 800 und 1920 × 1080 mit 40 Instanzen (per Skript angelegte `instance.json`-Dateien), dazu Zoom 150 % bei 960 × 600 (schmale Zeilen mit zwei Spans, Zeile 64 px), Tastatur-Durchlauf durch Liste, Menü, Dialog, Combobox (Entf auf der Zeile, Fokus nach dem Löschen). Screenshots nach `evidence/task-11/`. **CSP- und Prototyp-Prüfung** (neu: Popover, Command/cmdk, Select, Checkbox, Input): Web-Konsole beim Durchklicken lesen, keine CSP-Meldung, `Object.isFrozen(Object.prototype)` ist `true`; cmdk und Popover-Positionierung setzen Styles per Attribut (erlaubt über `style-src-attr`), ein Inline-`<style>`-Element dagegen nur mit Nonce oder Hash, nie mit `'unsafe-inline'` (Regel in Global Constraints).

- [ ] **Step 7: Lint, Format, Commit**

```bash
git add apps/desktop/src apps/desktop/src-tauri/tauri.conf.json apps/desktop/components.json apps/desktop/package.json apps/desktop/pnpm-lock.yaml
git commit -m "feat(desktop): instances view with create and delete dialogs"
```

---

### Task 12: Ende-zu-Ende-Tests über WebDriver

**Files:**
- Create: `apps/desktop/e2e/{wdio.conf.ts,setup-driver.ps1,specs/accounts.e2e.ts,specs/instances.e2e.ts,support/app.ts}`
- Modify: `apps/desktop/package.json` (devDependencies `@wdio/cli`, `@wdio/local-runner`, `@wdio/mocha-framework`, `@wdio/spec-reporter`, `tsx`; Skript `e2e`; `window.json` wird im `e2e`-Build seit Task 2 weder gelesen noch geschrieben, damit jede Sitzung mit derselben Größe startet)
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
it("runs with a frozen prototype and without CSP violations", async () => {
  expect(await browser.execute(() => Object.isFrozen(Object.prototype))).toBe(true);
  // Anmelde-Dialog, Kontomenü und Fehlerpanel durchklicken; browser.getLogs("browser") enthält keine Meldung mit "Content Security Policy"
});
// instances.e2e.ts
it("creates, lists and deletes an instance", async () => {
  // "No instances yet" sichtbar -> "Create instance" -> Version "9.9" -> Loader "Fabric" -> Loader-Version zeigt "0.99.0" mit "Recommended"
  // Name-Feld == "9.9 Fabric" -> "Create instance" -> Zeile "9.9 Fabric" in der Liste
  // "Actions for 9.9 Fabric" -> "Delete" -> "Move to Recycle Bin" -> Leerzustand
});
it("explains a loader that has no builds for the version", async () => { /* Filter Beta an, "b9.9", Fabric -> Hinweistext, Knopf deaktiviert mit Erklärung */ });
```

- [ ] **Step 3: Bauen und laufen lassen**

Run: `pnpm -C apps/desktop tauri build --debug --no-bundle --features e2e,dev-offline && cargo build -p hengst-testkit --bin hengst-mock && pnpm -C apps/desktop e2e`
Expected: alle Specs PASS; kein Fenster bleibt offen, kein `hengst-mock`-Prozess bleibt übrig (`Get-Process hengst-mock` leer).

- [ ] **Step 4: Gegenprobe, dass die Suite Fehler findet**

Den Dialogtext "Sign out {{name}}?" in `en.json` vorübergehend ändern, `pnpm build` und E2E erneut: genau der Abmelden-Test scheitert. Änderung zurücknehmen.

- [ ] **Step 5: CONTRIBUTING.md ergänzen** (Englisch): Werkzeuge, Befehle aus Step 1 und 3, Hinweis, dass E2E nur Debug-Builds mit `e2e` testet; Toolchain: Kern und CLI bauen ab Rust 1.89, `cargo test --workspace` und `cargo clippy --workspace` brauchen wegen der Desktop-Crate Rust 1.90 oder neuer, brauchen aber weder pnpm noch WebView2 (Platzhalter-`dist`, Mock-Laufzeit); Node und pnpm kommen aus `.nvmrc` und `packageManager`.

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
- Typen: `ErrorDto`, `ErrorCode`, `ErrorAction`, `UrlTarget` (Task 4), `OpenTarget` (Task 10), `EventSink`, `AppEvent`/`DataScope`/`NoticeKind`, `OperationId`/`OpKind`, `Patch`/`SettingsPatch`/`SettingsHandle` (Task 3), `LoaderKindDto`/`LoaderInfoDto` werden in den genannten Tasks eingeführt und später gleich benannt benutzt.
- Offene Befunde der Vorabprüfung, die der Plan bewusst nicht in 4a erledigt: keine (alle Findings F1 bis F21 sind in den Tasks verankert; `OpenLog { path }` ist nach 4b Task 16 verschoben).

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
- Interfaces: Commands `play(instance_id, channel: Channel<PlayEvent>) -> OperationId`, `play_cancel(op)`, `game_stop()`, `game_force_stop()`, `game_state() -> GameStateDto`; `PlayEvent` bildet `Progress` 1:1 ab (`ResolvingLoader`, `DownloadingInstaller`, `Downloading { what, count }`, `Downloaded`, `InstallStarted`, `InstallLog { path }`, `InstallStep { index, total, name }`, `VerifyingInstall`, `InstallFinished { elapsed_ms }`, `InstallReused`, `Notice { message }`), dazu `Preparing`, `Started { run_id }`, `Failed { error }`, `Cancelled`; `AppEvent::GameState { state: GameStateDto }` mit `GameStateDto = Idle { last_exit: Option<LastExit> } | Preparing { instance } | Running { instance, run_id, since } | Stopping` und `LastExit { instance, run_id, code: i32, stop_requested: bool }`: der Exit-Code des zuletzt beendeten Laufs und ob ein Stopp angefordert war (`stop_requested` setzt das Backend, wenn es den Stopp selbst eingeleitet hat: `game_stop` und `game_force_stop`, auch für den Tray-Eintrag und den Zwangsweg des Schließen-Dialogs, Task 17) gehören zum Ereignis und zu `game_state()`, damit die Oberfläche nach einem Neuladen denselben Stand kennt; `last_exit` bleibt gesetzt, bis der nächste Start (`Preparing`) beginnt. Die Oberfläche zeigt bei `code != 0` und `!stop_requested` das ErrorPanel "Das Spiel wurde unerwartet beendet" mit "Log anzeigen" (DESIGN.md 10.2); bei `0` oder bei `stop_requested`, gleich welcher Code, kehrt sie still zu Bereit zurück. `AppState.active_instance` (Task 10) wird bei `Preparing` und `Running` gesetzt und bei `Idle` gelöscht. Ein Spiel zur Zeit (Ruling: RAM, einfacher Zustandsautomat); Konto über `launch_account` (Refresh), bei `LoginExpired` `mark_sign_in_required`; nach Erfolg `instances::record_launch`; `java.exe` mit `CREATE_NO_WINDOW` (kein Wechsel auf `javaw.exe`, Ausgabe bleibt lesbar).
- Tests: Ablauf gegen Testkit-Mojang mit fake-java (Events in Reihenfolge, Zustand Running, Stopp), `second_play_is_busy`, `cancel_during_downloads`, `expired_login_marks_account`, `exit_code_reaches_the_idle_state` (fake-java endet mit 0 bzw. 3: `Idle { last_exit: Some(code 0 bzw. 3, stop_requested false) }`, auch über `game_state()`), `stop_requested_is_set_when_the_backend_stopped_the_game` (nach `game_stop` und nach `game_force_stop`: `stop_requested` true), `last_exit_is_cleared_by_the_next_start`, `delete_is_refused_while_preparing_or_running` (`instance_delete` liefert `InstanceRunning`).

### Task 16: Startseite
- Files: `apps/desktop/src/features/play/{PlayView.tsx,PlayButton.tsx,ProgressPanel.tsx,progress.ts}`.
- Inhalt nach DESIGN.md 10.2, 10.12 und 11.8: Startseite mit Instanzname (Starttitel), Meta-Zeile aus drei Spans, "Instanz wechseln", Konto-Chip in der Kopfzeile und dem einen Spielen-Knopf. Zustandsrangfolge des Knopfs genau nach der Tabelle in 10.2, erste Übereinstimmung gewinnt: Wird beendet (nach 10 s zusätzlich "Sofort beenden") / Läuft (Beenden, Statuszeile mit Punkt und "Läuft seit {{time}}") / Wird gestartet (bis zum ersten Fortschrittsereignis, träge) / Wird vorbereitet (Abbrechen) / Keine lesbare Instanz (Instanz anlegen; mit unlesbaren Instanzen darüber "Keine Instanz lässt sich starten.") / Gewählte Instanz nicht lesbar (Instanz wählen, Zeile "{{name}} lässt sich nicht starten.") / Kein Konto (Anmelden; ohne Client-ID wird der Dialog nicht geöffnet, stattdessen erscheint der Hinweisdialog "Build ohne Client-ID" aus 11.6, nicht der Block `appNotApproved`) / Konto muss sich neu anmelden (Erneut anmelden) / Bereit (Spielen); die Instanzwahl nach `effectiveSelection` (Task 11). Klickschutz: nach dem Wechsel von Spielen zu Abbrechen ignoriert der Knopf 600 ms lang Klicks. Der Knopf ist nie `disabled`. Fortschrittsbereich (feste Höhe, nur beim Vorbereiten) mit Phase, Zählern ("3.402 von 4.118 Dateien"), Installer-Schritt "Schritt 3 von 6: jarsplitter", Link "Installationsprotokoll öffnen", ohne Verbindung und bei fehlenden Dateien das ErrorPanel "Keine Verbindung", bei Fehlern das ErrorPanel an Ort und Stelle. Nach dem Spiel: Exit-Code 0 oder angeforderter Stopp (`stop_requested`) still zurück zu Bereit, sonst ErrorPanel "Das Spiel wurde unerwartet beendet" (Task 15). Die Auswahl der Instanz zeigt `activeInstance` im Store für Task 11 an. Dafür kommt hier `ErrorAction::OpenLog { path: String }` zu `ErrorAction` (Task 4) und `OpenTarget::LogFile { path }` zu `OpenTarget` (Task 10); beide laufen durch dieselbe Prüfung `resolve_log_path(paths, path) -> Result<PathBuf>`: der Pfad wird mit `std::fs::canonicalize` aufgelöst (folgt Links und Junctions), das Ergebnis muss unter dem kanonischen `<root>/logs` oder unter `<root>/instances/<id>/logs` (mit gültiger `<id>` laut `instances::instance_dir`) liegen, die Endung muss `.log` sein (ohne Rücksicht auf Groß-/Kleinschreibung), und es muss eine Datei sein; ein Link, der aus diesen Ordnern hinausführt, wird abgelehnt, weil der aufgelöste Pfad draußen liegt. Die Web-Ansicht übergibt nie einen frei gewählten Pfad, sondern nur den, den der Kern in `InstallLog { path }` gemeldet hat. Tests `log_path_outside_the_logs_directories_is_refused`, `log_path_needs_the_log_extension`, `log_path_through_a_junction_out_of_logs_is_refused`, `log_path_traversal_is_refused`. Animation nur am Fortschrittsbalken und Gedrückt-Zustand, Prüfung mit `review-animations`.
- Tests: `progress.ts`-Reducer (Events → Anzeige), `play_button_state_precedence` (Tabelle aus 10.2, je Zeile ein Fall, auch zwei zugleich zutreffende Zustände), `cancel_ignores_clicks_for_600_ms_after_the_change` (Fake-Timer), `starting_is_inert`, `force_stop_appears_after_10_seconds_in_stopping`, `exit_code_zero_returns_silently_and_nonzero_shows_the_panel` (Code ungleich 0 ohne angeforderten Stopp zeigt das Panel), `stop_requested_with_exit_code_1_shows_no_error_panel`, `offline_with_missing_files_shows_no_connection`, Doppelklick startet einmal.

### Task 17: Tray und Fensterverhalten
- Files: `apps/desktop/src-tauri/src/tray.rs`, `tauri` Feature `tray-icon`.
- Entscheidung: Tray kommt in 4b, weil Tauri ihn mit `TrayIconBuilder` direkt mitbringt (https://v2.tauri.app/learn/system-tray/). Beim Spielstart minimiert die App in den Tray, wenn `minimize_on_launch` gilt; nach Spielende kehrt das Fenster zurück; Tray-Menü nach DESIGN.md 11.11: "Hengst Client öffnen", "Spiel beenden" (nur solange ein Spiel läuft; ein darauf folgendes Spielende gilt als angeforderter Stopp), Trenner, "Hengst Client beenden" / "Quit Hengst Client"; Tooltip "Hengst Client" bzw. "Hengst Client: {{instance}} läuft"; Linksklick öffnet das Fenster; nach Spielende wird das Fenster gezeigt und wiederhergestellt, aber nicht in den Vordergrund gezwungen. Schließen des Fensters bei laufendem Spiel versteckt in den Tray (ohne Dialog). "Hengst Client beenden" bei laufendem Spiel zeigt zuerst das Fenster und öffnet den Dialog (440 px) "Spiel beenden und Hengst Client schließen?" mit "Weiterspielen" (`secondary`, erste Fokusposition) und "Spiel beenden und schließen" (destructive, Laden "Wird beendet…", danach beendet sich die App); reagiert das Spiel nach 10 s nicht, zeigt der Dialog "Das Spiel reagiert nicht" mit "Sofort beenden und schließen".
- Tests: Zustandslogik in Rust (Fenster sichtbar je Zustand), `tray_menu_lists_stop_game_only_while_a_game_runs` (reine Funktion `tray_items(state) -> Vec<TrayItem>`), `close_hides_to_tray_while_a_game_runs`, `quit_while_running_needs_confirmation`, Dialogtests (Fokus, Laden, 10-s-Fehler) in Vitest, E2E-Teil in Task 22.

### Task 18: Live-Log im Backend
- Files: `apps/desktop/src-tauri/src/{log.rs,commands/log.rs}`.
- Interfaces: Ringpuffer 10.000 Zeilen je Lauf (aktueller und letzter Lauf), `log_attach(run_id, since_seq, channel: Channel<LogBatch>)` (Rückfüllung, dann live, gebündelt alle 100 ms oder 200 Zeilen), `log_detach`, `log_copy(run_id, filter) -> String` (geschwärzt, gefiltert), `open_location(OpenTarget::GameLog { run_id })`.
- Tests: Puffergrenze, Reihenfolge, Rückfüllung nach Neuladen, kein Token in Batches.

### Task 19: Live-Log in der Oberfläche
- Files: `apps/desktop/src/features/log/{LogView.tsx,LogToolbar.tsx,logStore.ts}`, Bibliothek Virtuoso (`pick-ui-library`); CSP- und Prototyp-Prüfung aus den Global Constraints (Virtuoso setzt Positionen per `style`-Attribut, kein `'unsafe-inline'` für Style-Elemente).
- Inhalt nach DESIGN.md 10.13 und 11.9: virtualisierte Liste (höchstens 10.000 Zeilen im Speicher) im Log-Bereich, Werkzeugleiste in zwei Zeilen (Zeile 1: Label "Im Log suchen", Suchfeld, Trefferzahl, Vorheriger/Nächster Treffer; Zeile 2: "Stufen: 4 von 5" mit Fehler, Warnung, Info, Debug, Sonstige, rechts "Kopieren" und "Logdatei öffnen"; ab Fensterbreite 1280 (`full:`) eine Zeile, unter 880 CSS-Pixeln drei Zeilen), Laufwahl ("Aktueller Lauf" / "Letzter Lauf"), "Zum Ende springen", Strg+F, Kopieren (gefiltert), Leer-, Lade- und Fehlerzustände aus 11.9, Mono, Farben `--log-*`.
- Tests: Filter und Suche im Store, Grenze, Folgen-Logik; Sichtprüfung mit 10.000 Zeilen.

### Task 20: Einstellungen vollständig und Über
- Files: `apps/desktop/src/features/settings/*`, `apps/desktop/src-tauri/src/commands/settings.rs`, Dialog-Plugin nur aus Rust (`tauri-plugin-dialog`, ohne JS-Permission) für die Java-Auswahl.
- Inhalt nach DESIGN.md 11.7 und 11.10: Arbeitsspeicher (Zahlfeld mit `inputmode="numeric"`, Bereich 512 bis 65.536 MB, Hilfezeile "Empfohlen: 2048 bis 4096 MB. Installiert: 16,0 GB." mit der echten Größe, Warnung statt Ablehnung oberhalb des installierten Speichers), Java-Überschreibung (Rust-Dialog, Prüfung `java.exe -version` mit Zeitlimit; zusätzlich gilt die Prüfung aus `settings_update` (Task 3): der Pfad muss existieren, eine Datei sein und `java.exe` oder `javaw.exe` heißen, auch wenn er nicht aus dem Dialog stammt), Datenordner (Anzeige, Öffnen), Sprache, Design, "Beim Spielstart in den Infobereich", Über: Version, "Development build"-Badge, "Not an official Minecraft product.", Lizenz GPL-3.0, Drittlizenzen (generiert mit `cargo about` und `pnpm licenses list --json`, als statische Datei ins Bundle; Dialog nach 11.10 mit nur einem `secondary`-Knopf "Schließen"), Link zum Repository.
- Tests: Validierung, Java-Prüfung gegen fake-java, Lizenzliste nicht leer.

### Task 21: Kern-API verengen, `--resolve-only`, CI
- Files: `crates/hengst-core/src/{lib.rs,auth/mod.rs,loaders/mod.rs}`, `apps/cli/src/main.rs`, `.github/workflows/ci.yml`.
- Inhalt: öffentliche Oberfläche = was CLI, Desktop und Integrationstests nutzen; `auth::{msa, xbox, minecraft}` und `loaders::{fabric, merge}` werden privat mit Re-Exports (`DeviceCode`, `MsaTokens`, `Profile`, `ProcessRunner`, `SystemRunner`, `JobRunner`); `hengst launch --resolve-only` löst Version und Loader auf und gibt sie aus, ohne Bibliotheken zu laden (für CI-Matrix auf Pull Requests); CI-Job für die Desktop-App: `desktop-frontend` (pnpm install, typecheck, Vitest, format) und `bindings_are_current` laufen seit 4a Task 2, hier kommt nur `tauri build --debug --no-bundle` dazu; E2E vorerst nur lokal (Treiberversion an den Runner gebunden, Entscheidung in 4b nach einem Versuchslauf).

### Task 22: E2E für Spielen, Log, Einstellungen und Abschluss
- Specs: Spielen mit fake-java (Fortschritt, Läuft, Log-Zeilen sichtbar ohne Token, Beenden), Einstellungen (Sprache und Design wechseln, Neustart behält), Tray-Verhalten soweit über WebDriver prüfbar.
- Abschluss: Platzierung und Layout bei 1366 × 768 und 125 % Skalierung sowie Zoom 150 % bei Mindestgröße über alle Ansichten (DESIGN.md 13), `produkt-regelwerk` im PRÜFEN-Modus über alle Ansichten (Befundtabelle), `break-ui`-Extremdaten, `accessibility`-Durchgang, `review-animations` über den Diff, `code-review` und `security-review` (Capability, CSP, Pfadprüfung, Schwärzung), echte Spielstarts aus der App (Debug-Build mit `dev-offline`, stumme `options.txt`): Vanilla 1.21.11, Fabric 1.21.11, Forge 1.20.1 bis Titelbildschirm; README (Desktop-Abschnitt, "Not an official Minecraft product"), `docs/PLAN.md` Abschnitt 11 Stand Phase 4.

## Beim Menschen (Clemens)

- **Azure-App (Auth Code + PKCE):** im Entra-Portal bei der App `7cad9d53-b0ce-44b1-9ef3-afa798da99c2` unter "Authentication" die Plattform "Mobile and desktop applications" hinzufügen und dort die Redirect-URI `http://localhost` eintragen (genau so, ohne Port, ohne Pfad, ohne Query); "Allow public client flows" bleibt auf "Yes"; unterstützte Kontotypen müssen persönliche Microsoft-Konten einschließen; kein Client-Secret. Ohne diesen Eintrag antwortet Microsoft beim Browser-Login mit einem Redirect-Fehler; der Device-Code-Weg funktioniert weiter.
- **Mojang-Freigabe:** weiter offen; bis dahin zeigt die App die Erklärung aus Task 8, echte Anmeldung bleibt ungeprüft.
- **Branches:** Phase 2 und 3 sind lokal fertig und nicht gemergt; 4a baut auf `phase-3-loader` auf. Wann gepusht und gemergt wird, entscheidet Clemens.
- **Logo:** Platzhalter unter `apps/desktop/brand/mark.svg`; Tausch nach `brand/README.md`.
- **Code-Signing:** nicht in Phase 4; ungesignierte Builds lösen SmartScreen-Warnungen aus (PLAN.md 12).
