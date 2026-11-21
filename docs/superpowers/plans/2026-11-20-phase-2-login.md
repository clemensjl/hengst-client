# Hengst Client Phase 2 (Login) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Die CLI `hengst` meldet echte Microsoft-Konten per Device-Code an, verwaltet mehrere Konten (Secrets nur im Windows-Anmeldeinformationsspeicher), erneuert Tokens vor dem Start und startet das Spiel mit `user_type` "msa"; Offline-Konten starten nur noch mit Feature `dev-offline`, und das erzwingt der Kern.

**Architecture:** Alles bleibt in `crates/hengst-core` (kein eigenes `auth`-Crate, abweichend von PLAN.md Abschnitt 3), neuer Modulbaum `auth/` mit `msa` (Microsoft-OAuth Device Code + Refresh), `xbox` (Xbox Live + XSTS), `minecraft` (minecraftservices), `flow` (Kette, Refresh, Startkonto) und `store` (Metadaten-JSON + `SecretStore`-Trait mit Keyring- und In-Memory-Implementierung). Jeder Endpunkt hängt an einer injizierbaren Basis-URL in `AuthEndpoints`, alle Tests laufen gegen wiremock. Auth Code + PKCE ist bewusst nicht Teil dieser Phase, er kommt mit der Desktop-App in Phase 4.

**Tech Stack:** Rust 1.97 (Edition 2021, MSRV wird 1.88), tokio, reqwest 0.12 (rustls, json, `.form()`), serde/serde_json, thiserror; neu: `keyring-core` 1.1 + `windows-native-keyring-store` 1.1 (nur `cfg(windows)`). Tests: wiremock 0.6, tempfile. CLI: clap 4.

**Spec:** `docs/PLAN.md` (Abschnitte 3, 7 "Konten", 8, 9, 11) und der Final-Review-Bericht zu Phase 1 (Subagent-Transkript vom 31.10.2026, nicht im Repo; die Phase-2-Punkte daraus sind unten unter "Übernahme aus Phase 1" aufgelistet).

## Global Constraints

- MVP nur Windows 10/11 x64. Keyring-Code nur unter `#[cfg(windows)]`; der Rest kompiliert plattformneutral.
- Kein Spielstart mit Offline-Konto ohne Cargo-Feature `dev-offline`, durchgesetzt in `hengst-core` (Feature `dev-offline` auf `hengst-core`, die CLI reicht es durch). `--dry-run` mit Offline-Konto bleibt in allen Builds erlaubt (CI-Matrix braucht das).
- Tokens nie loggen oder ausgeben: kein Token in `Debug`, `Display`, Fehlermeldungen, URLs (Query-Strings) oder Dry-Run-Ausgabe. Secrets (Refresh-Token, Minecraft-Access-Token) nur im Windows-Anmeldeinformationsspeicher, nie in `accounts.json`.
- Microsoft-Client-ID: genau eine Konstante `hengst_core::auth::CLIENT_ID: Option<&str>` (heute `None`), Override per Umgebungsvariable `HENGST_MS_CLIENT_ID`. Fehlt beides: klare Fehlermeldung. Die Client-ID ist kein Secret.
- Microsoft-Tenant `consumers`, Scope exakt `XboxLive.signin offline_access`, XSTS-Relying-Party exakt `rp://api.minecraftservices.com/`.
- Keine Live-Calls in Tests. Ein echter Credential-Manager-Test ist `#[ignore]` und läuft nur von Hand.
- "Minecraft" nie im Produktnamen. Code, Fehlertexte, CLI-Ausgaben, README auf Englisch; `docs/` auf Deutsch. Keine Emojis.
- Jede Task endet mit `cargo test --workspace --all-features`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all` (danach kein Diff) und einem Commit. Commits enden mit `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- User-Agent für alle Auth-Aufrufe: der bestehende `download::http_client()` (setzt `USER_AGENT`, Timeouts).

## Review Focus

1. **Redaktion darf Offline-Argumente nicht zerstören:** das Offline-Token ist `"0"`; ein naives Ersetzen von `"0"` würde `-Xmx2048M` verstümmeln. Erwartet: nur Microsoft-Tokens werden ersetzt, Offline-Argumente bleiben byte-gleich (Test in Task 2).
2. **Kaputte oder fremde `accounts.json`:** eine unlesbare Datei darf nicht still durch eine leere ersetzt werden, sonst sind alle Konten weg. Erwartet: `Error::Json`, Datei bleibt unverändert (Test in Task 8).
3. **Refresh-Token-Rotation:** Microsoft gibt bei jedem Refresh ein neues Refresh-Token aus. Erwartet: das neue wird gespeichert, ein zweiter Refresh nutzt es (Test in Task 9).
4. **Kein Netz kurz vor Ablauf:** Token läuft in 30 Minuten ab (innerhalb der Refresh-Marge), Netzwerk ist weg. Erwartet: Start mit dem noch gültigen Token statt Abbruch; nur ein wirklich abgelaufenes Token führt zum Fehler (Test in Task 9).
5. **Konto-Auswahl nach Name:** `hengst logout dev` muss "Dev" treffen; nach Entfernen des Standardkontos rückt das erste verbleibende nach, nach dem letzten gibt es keins (Test in Task 8).

---

## Übernahme aus Phase 1 (Final-Review, als "later-phase 2" markiert)

| Punkt | Task |
|---|---|
| I4: Offline-Sperre nur in der CLI und erst nach `prepare` | Task 3 |
| T7/T8: `Account`- und `Prepared`-`Debug` sowie Dry-Run zeigen `access_token` | Task 2 |
| T7: `user_type` muss "msa" sein | Task 1 |
| T7: kein Testvektor für die Offline-UUID (Notch) | Task 1 |

Alle anderen offenen Punkte des Berichts sind Phase 3, 4, 8 oder 9 zugeordnet und bleiben hier draußen.

## Dateistruktur

```
crates/hengst-core/
  Cargo.toml                 Feature dev-offline, keyring-core + windows-native-keyring-store (cfg windows)
  src/lib.rs                 + pub mod auth; pub mod secret;
  src/error.rs               + OfflineLaunchDisabled, CredentialStore, Auth(#[from] AuthError)
  src/secret.rs              Secret-Newtype (redigiertes Debug, kein Serialize)
  src/args.rs                Account: private Felder, AccountKind, user_type()
  src/launch.rs              Prepared mit privatem Konto, redacted_args(), ensure_launch_allowed, DEV_OFFLINE
  src/paths.rs               + accounts_file()
  src/auth/mod.rs            CLIENT_ID, client_id(), AuthEndpoints, MicrosoftAuth, unix_now()
  src/auth/error.rs          AuthError mit allen Klartext-Meldungen
  src/auth/msa.rs            Device Code, Polling, Refresh
  src/auth/xbox.rs           Xbox-Live-User-Auth, XSTS, XErr-Zuordnung
  src/auth/minecraft.rs      login_with_xbox, Entitlements, Profil, Besitz-Entscheidung
  src/auth/flow.rs           complete_login, refresh_session, needs_refresh, launch_account
  src/auth/store.rs          SecretStore, MemorySecretStore, KeyringSecretStore, AccountStore
apps/cli/
  Cargo.toml                 dev-offline = ["hengst-core/dev-offline"]
  src/main.rs                login, accounts [use], logout, launch --account/--offline
.github/workflows/ci.yml     Test-Schritt ohne Features, Dry-Run mit --offline Dev
README.md, docs/PLAN.md      Status, MSRV 1.88
```

## Task-Reihenfolge

Task 1 → 2 → 3 (Übernahmen, berühren `args.rs`/`launch.rs`/CLI nacheinander) → Task 4 → (Task 5, 6, 8 parallel möglich, eigene Dateien) → Task 7 (braucht 4 bis 6) → Task 9 (braucht 7 und 8) → Task 10 → Task 11.

---

### Task 1: AccountKind, `user_type` "msa" und gekapseltes `Account`

**Files:**
- Modify: `crates/hengst-core/src/args.rs:8-30` (Account), `:103-132` (`variables`), Tests am Dateiende
- Modify: `apps/cli/src/main.rs` nur falls ein Feldzugriff bricht (heute keiner)

**Interfaces:**
- Produces:
  - `pub enum AccountKind { Microsoft, Offline }` (`Clone, Copy, Debug, PartialEq, Eq`) in `args.rs`
  - `Account` mit **privaten** Feldern `name, uuid, access_token: String, kind: AccountKind, xuid`; das Feld `user_type` entfällt.
  - `pub fn Account::offline(name: &str) -> Account` (unverändert in Verhalten, `kind = Offline`)
  - `pub(crate) fn Account::microsoft(name: String, uuid: String, access_token: String, xuid: String) -> Account` (nur der Kern darf Microsoft-Konten bauen; Task 2 ändert den Token-Typ auf `Secret`)
  - Getter `pub fn name(&self) -> &str`, `pub fn uuid(&self) -> &str`, `pub fn kind(&self) -> AccountKind`, `pub fn xuid(&self) -> &str`, `pub fn user_type(&self) -> &'static str` ("msa" für Microsoft, "legacy" für Offline)

- [ ] **Step 1: Failing Tests schreiben** (in `args.rs`)

```rust
#[test]
fn offline_uuid_matches_java_name_uuid_vector() {
    assert_eq!(Account::offline("Notch").uuid(), "b50ad385829d3141a2167e7d7539ba7f");
    assert_eq!(Account::offline("Dev").uuid(), "380df991f603344ca090369bad2a924a");
}

#[test]
fn user_type_follows_account_kind() {
    let ms = Account::microsoft("Steve".into(), "a".repeat(32), "tok".into(), "123".into());
    assert_eq!(ms.kind(), AccountKind::Microsoft);
    assert_eq!(ms.user_type(), "msa");
    assert_eq!(Account::offline("Dev").user_type(), "legacy");
}

#[test]
fn microsoft_account_fills_user_type_and_xuid_args() {
    // modern_arguments-JSON mit "--userType", "${user_type}", "--xuid", "${auth_xuid}" im game-Array
    // ctx().account = Account::microsoft(...); erwartet: "--userType", "msa", "--xuid", "123"
}
```

Bestehende Tests (`offline_uuid_is_version_3_and_stable`, `legacy_arguments_get_default_jvm_args`) auf Getter umstellen.

- [ ] **Step 2: Tests laufen lassen, Fehlschlag prüfen**

Run: `cargo test -p hengst-core args::`
Expected: FAIL (Kompilierfehler: `microsoft`, `kind`, `user_type()` unbekannt)

- [ ] **Step 3: Implementieren** in `args.rs`: Felder privat, Konstruktoren und Getter wie oben; `variables()` liest `user_type` über `a.user_type()`.

- [ ] **Step 4: Alles grün**

Run: `cargo test --workspace --all-features` ; `cargo clippy --workspace --all-targets --all-features -- -D warnings` ; `cargo fmt --all`
Expected: PASS, keine Warnungen, kein fmt-Diff. `tests/launch_e2e.rs` kompiliert unverändert (nutzt nur `Account::offline`).

- [ ] **Step 5: Commit**

```bash
git add crates/hengst-core/src/args.rs
git commit -m "feat(core): account kind with user_type msa for Microsoft accounts"
```

---

### Task 2: `Secret` und Token-Schwärzung in Debug und Dry-Run

**Files:**
- Create: `crates/hengst-core/src/secret.rs`
- Modify: `crates/hengst-core/src/lib.rs` (`pub mod secret;`)
- Modify: `crates/hengst-core/src/args.rs` (`access_token: Secret`)
- Modify: `crates/hengst-core/src/launch.rs:42-48` (Prepared), `:131-136` (Konstruktion)
- Modify: `apps/cli/src/main.rs:97-102` (Dry-Run-Ausgabe)

**Interfaces:**
- Consumes: `Account`, `AccountKind` aus Task 1
- Produces:
  - `pub struct Secret(String)` in `secret.rs`: `Clone`, `PartialEq`, `Eq`, `serde::Deserialize` (transparent), **kein** `Serialize`, **kein** `Display`; manuelles `Debug` schreibt `Secret(<redacted>)`.
  - `pub fn Secret::new(value: impl Into<String>) -> Secret`, `pub fn Secret::expose(&self) -> &str`
  - `pub const REDACTED: &str = "<redacted>";` in `secret.rs`
  - `Account::microsoft(name: String, uuid: String, access_token: Secret, xuid: String)` (Typwechsel), `pub(crate) fn Account::access_token(&self) -> &Secret`
  - `Prepared` behält `pub java, pub args, pub game_dir, pub version_id` und bekommt das private Feld `account: Account` (damit kann außerhalb des Kerns niemand ein `Prepared` per Struct-Literal bauen; Task 3 nutzt das).
  - `pub fn Prepared::redacted_args(&self) -> Vec<String>`: bei `AccountKind::Microsoft` wird jedes Vorkommen des Tokens in jedem Argument durch `REDACTED` ersetzt (deckt `--accessToken <t>` und `token:<t>:<uuid>` ab); bei Offline unverändert.
  - Manuelles `impl Debug for Prepared`, das `redacted_args()` statt `args` ausgibt und `account` als dessen (redigiertes) Debug.

- [ ] **Step 1: Failing Tests**

In `secret.rs`:
```rust
#[test]
fn debug_never_shows_the_value() {
    let s = Secret::new("eyJhbGciOi.secret");
    assert_eq!(format!("{s:?}"), "Secret(<redacted>)");
    assert_eq!(s.expose(), "eyJhbGciOi.secret");
}
```
In `args.rs`:
```rust
#[test]
fn account_debug_hides_token() {
    let a = Account::microsoft("Steve".into(), "a".repeat(32), Secret::new("tok-123"), "1".into());
    assert!(!format!("{a:?}").contains("tok-123"));
}
```
In `launch.rs` (Unit-Test, baut `Prepared` direkt):
```rust
#[test]
fn redacts_microsoft_token_everywhere() {
    // args: ["-Xmx2048M", "--accessToken", "tok-123", "token:tok-123:uuid"], account = microsoft mit "tok-123"
    assert_eq!(p.redacted_args(), ["-Xmx2048M", "--accessToken", "<redacted>", "token:<redacted>:uuid"]);
    assert!(!format!("{p:?}").contains("tok-123"));
}

#[test]
fn offline_args_stay_untouched() {
    // args: ["-Xmx2048M", "-Xms512M", "--accessToken", "0"], account = Account::offline("Dev")
    assert_eq!(p.redacted_args(), p.args);
}
```

- [ ] **Step 2: Fehlschlag prüfen**

Run: `cargo test -p hengst-core secret:: args:: launch::`
Expected: FAIL (Modul `secret` fehlt)

- [ ] **Step 3: Implementieren** wie in Interfaces. `variables()` in `args.rs` nutzt `access_token().expose()`. CLI-Dry-Run druckt `prepared.redacted_args()` statt `prepared.args`.

- [ ] **Step 4: Alles grün** (Test, Clippy, fmt wie Global Constraints)

Zusätzlich: `cargo run -p hengst-cli -- --home <tmp> launch rd-132211 --username Dev --dry-run` gibt weiter `--accessToken`-freie Legacy-Zeile bzw. `0` aus (Offline unverändert).

- [ ] **Step 5: Commit**

```bash
git add crates/hengst-core/src/secret.rs crates/hengst-core/src/lib.rs crates/hengst-core/src/args.rs crates/hengst-core/src/launch.rs apps/cli/src/main.rs
git commit -m "feat(core): redact access tokens in debug and dry-run output"
```

---

### Task 3: Offline-Sperre im Kern

**Files:**
- Modify: `crates/hengst-core/Cargo.toml` (`[features] dev-offline = []`)
- Modify: `apps/cli/Cargo.toml` (`dev-offline = ["hengst-core/dev-offline"]`)
- Modify: `crates/hengst-core/src/error.rs` (neue Variante)
- Modify: `crates/hengst-core/src/launch.rs` (`run`, neue Funktion, Konstante)
- Modify: `apps/cli/src/main.rs:104-109` (Prüfung vor `prepare`)
- Modify: `.github/workflows/ci.yml` (Test-Schritt ohne Features)

**Interfaces:**
- Consumes: `AccountKind`, `Prepared.account` (Tasks 1, 2)
- Produces:
  - `Error::OfflineLaunchDisabled` mit Text exakt: `offline accounts can only start the game in development builds (cargo feature dev-offline)`
  - `pub const DEV_OFFLINE: bool = cfg!(feature = "dev-offline");` in `launch.rs`
  - `pub fn ensure_launch_allowed(kind: AccountKind, dev_offline: bool) -> Result<()>`: `Offline && !dev_offline` → `Err(OfflineLaunchDisabled)`, sonst `Ok`.
  - `run(p: &Prepared)` ruft zuerst `ensure_launch_allowed(p.account.kind(), DEV_OFFLINE)?` und startet erst danach den Prozess.

- [ ] **Step 1: Failing Tests** in `launch.rs`

```rust
#[test]
fn offline_needs_dev_offline() {
    assert!(matches!(ensure_launch_allowed(AccountKind::Offline, false), Err(Error::OfflineLaunchDisabled)));
    ensure_launch_allowed(AccountKind::Offline, true).unwrap();
    ensure_launch_allowed(AccountKind::Microsoft, false).unwrap();
}

#[cfg(not(feature = "dev-offline"))]
#[tokio::test]
async fn run_refuses_offline_account_in_release_builds() {
    // Prepared mit java = "C:/does-not-exist/java.exe", Account::offline("Dev")
    assert!(matches!(run(&p).await, Err(Error::OfflineLaunchDisabled)));  // nicht Io: die Sperre greift vor dem Prozessstart
}
```

- [ ] **Step 2: Fehlschlag prüfen**

Run: `cargo test -p hengst-core --lib launch::`
Expected: FAIL (Funktion fehlt). Hinweis: `--all-features` schaltet `dev-offline` ein, der zweite Test läuft deshalb nur ohne Features.

- [ ] **Step 3: Implementieren** wie in Interfaces. CLI: im Nicht-Dry-Run-Zweig `ensure_launch_allowed(account.kind(), DEV_OFFLINE)?` **vor** `prepare` aufrufen; der alte `cfg!`-`bail!` entfällt. CI, Job `test`, nach dem bestehenden Test-Schritt:

```yaml
      - run: cargo test -p hengst-core --lib
```

- [ ] **Step 4: Alles grün**

Run: Global-Constraints-Befehle plus `cargo test -p hengst-core --lib`
Expected: PASS in beiden Feature-Varianten.
Manuell: `cargo run -p hengst-cli -- --home <leerer tmp> launch 1.8.9 --username Dev` endet sofort mit der neuen Meldung, und `<tmp>/versions` existiert danach nicht (keine Downloads vor der Sperre).

- [ ] **Step 5: Commit**

```bash
git add crates/hengst-core apps/cli .github/workflows/ci.yml
git commit -m "feat(core): enforce the dev-offline lock in core before any download"
```

---

### Task 4: Auth-Grundgerüst und Microsoft Device Code Flow

**Files:**
- Create: `crates/hengst-core/src/auth/mod.rs`, `auth/error.rs`, `auth/msa.rs`
- Modify: `crates/hengst-core/src/lib.rs` (`pub mod auth;`), `crates/hengst-core/src/error.rs` (`#[error(transparent)] Auth(#[from] auth::AuthError)`)

**Interfaces:**
- Consumes: `Secret` (Task 2), `download::http_client()`
- Produces (in `auth/mod.rs`):
  - `pub const CLIENT_ID: Option<&str> = None;` mit Kommentar "set once the Azure app is approved by Mojang"
  - `pub const CLIENT_ID_ENV: &str = "HENGST_MS_CLIENT_ID";`
  - `pub fn resolve_client_id(env: Option<String>, builtin: Option<&str>) -> Result<String>`: Env (getrimmt, nicht leer) vor Konstante, sonst `AuthError::MissingClientId`
  - `pub fn client_id() -> Result<String>` = `resolve_client_id(std::env::var(CLIENT_ID_ENV).ok(), CLIENT_ID)`
  - `pub struct AuthEndpoints { pub msa: String, pub xbox_user: String, pub xsts: String, pub minecraft: String }` mit `Default`:
    - `msa = "https://login.microsoftonline.com/consumers/oauth2/v2.0"` (Pfade `/devicecode`, `/token`)
    - `xbox_user = "https://user.auth.xboxlive.com"` (Pfad `/user/authenticate`)
    - `xsts = "https://xsts.auth.xboxlive.com"` (Pfad `/xsts/authorize`)
    - `minecraft = "https://api.minecraftservices.com"` (Pfade `/authentication/login_with_xbox`, `/entitlements/mcstore`, `/minecraft/profile`)
  - `pub const SCOPE: &str = "XboxLive.signin offline_access";`
  - `pub struct MicrosoftAuth { http: reqwest::Client, ep: AuthEndpoints, client_id: String }`, `pub fn MicrosoftAuth::new(client_id: String, ep: AuthEndpoints) -> Self` (nutzt `http_client()`)
  - `pub fn unix_now() -> u64`
- Produces (in `auth/error.rs`): `pub enum AuthError` (thiserror). Texte exakt:
  - `MissingClientId`: `no Microsoft client id configured; set HENGST_MS_CLIENT_ID (the official id is not published yet)`
  - `DeviceCodeExpired`: `the sign-in code expired before it was used; run "hengst login" again`
  - `LoginDeclined`: `the sign-in was declined in the browser`
  - `AppMisconfigured { code: String }`: `Microsoft rejected the client id ({code}); the Azure app must allow personal accounts and public client flows`
  - `LoginExpired { name: String }`: `the saved login for {name} is no longer valid; run "hengst login" again`
  - `NoXboxAccount`, `XboxRegionBlocked`, `XboxAdultVerification`, `XboxChildAccount`, `XboxRefused { code: u64 }` (Texte in Task 5)
  - `AppNotApproved`, `GameNotOwned`, `NoProfile` (Texte in Task 6)
  - `AccountNotFound { selector: String }`, `NoAccount` (Texte in Task 8)
  - `BadResponse(&'static str)`: `unexpected response from {0}`
- Produces (in `auth/msa.rs`):
  - `#[derive(Debug, Deserialize)] pub struct DeviceCode { pub device_code: Secret, pub user_code: String, pub verification_uri: String, pub expires_in: u64, pub interval: u64 }`
  - `#[derive(Debug)] pub struct MsaTokens { pub access_token: Secret, pub refresh_token: Secret }`
  - `pub async fn MicrosoftAuth::start_device_login(&self) -> Result<DeviceCode>`: POST Form `client_id`, `scope=SCOPE` an `{msa}/devicecode`
  - `pub async fn MicrosoftAuth::poll_device_login(&self, code: &DeviceCode) -> Result<MsaTokens>`: Schleife `sleep(interval)` → POST Form `grant_type=urn:ietf:params:oauth:grant-type:device_code`, `client_id`, `device_code` an `{msa}/token`; Abbruch nach `expires_in` mit `DeviceCodeExpired`
  - `pub async fn MicrosoftAuth::refresh_msa(&self, refresh: &Secret) -> Result<MsaTokens>`: POST Form `grant_type=refresh_token`, `client_id`, `refresh_token`, `scope=SCOPE`
  - `pub(crate) fn next_interval(current: Duration, error: &str) -> Duration`: `"slow_down"` → `current + 5 s`, sonst `current`
  - Fehlerzuordnung der Token-Antwort (HTTP 400, JSON-Feld `error`): `authorization_pending` → weiter, `slow_down` → weiter mit `next_interval`, `authorization_declined` → `LoginDeclined`, `expired_token` → `DeviceCodeExpired`, `invalid_client`/`unauthorized_client` → `AppMisconfigured { code }`, `invalid_grant` beim Refresh → `LoginExpired { name: String::new() }` (Task 9 setzt den Namen); alles andere → `Error::Status`. `error_description` wird nie in Fehler übernommen.

- [ ] **Step 1: Failing Tests** (in `auth/mod.rs` und `auth/msa.rs`, wiremock, Device-Code-Antworten mit `"interval": 0`)

```rust
#[test]
fn client_id_env_wins_and_missing_is_a_clear_error() {
    assert_eq!(resolve_client_id(Some(" env-id ".into()), Some("built")).unwrap(), "env-id");
    assert_eq!(resolve_client_id(Some("".into()), Some("built")).unwrap(), "built");
    let e = resolve_client_id(None, None).unwrap_err();
    assert!(e.to_string().contains("HENGST_MS_CLIENT_ID"));
}

#[tokio::test]
async fn device_code_request_uses_consumers_scope() {
    // Mock POST /devicecode mit body_string_contains("scope=XboxLive.signin+offline_access") und client_id=test-id
    // → DeviceCode mit user_code "ABCD-EFGH", verification_uri "https://microsoft.com/link"
}

#[tokio::test]
async fn polling_waits_through_pending_then_returns_tokens() {
    // /token: up_to_n_times(2) 400 {"error":"authorization_pending"}, danach 200 {access_token, refresh_token, expires_in}
    assert_eq!(tokens.refresh_token.expose(), "rt-1");
}

#[tokio::test]
async fn declined_and_expired_codes_map_to_clear_errors() {
    // 400 {"error":"authorization_declined"} → Error::Auth(AuthError::LoginDeclined)
    // 400 {"error":"expired_token"}         → Error::Auth(AuthError::DeviceCodeExpired)
    // 400 {"error":"unauthorized_client"}   → Error::Auth(AuthError::AppMisconfigured { code }) mit code == "unauthorized_client"
}

#[test]
fn slow_down_adds_five_seconds() {
    assert_eq!(next_interval(Duration::from_secs(5), "slow_down"), Duration::from_secs(10));
    assert_eq!(next_interval(Duration::from_secs(5), "authorization_pending"), Duration::from_secs(5));
}

#[tokio::test]
async fn refresh_sends_refresh_grant_and_rejects_invalid_grant() {
    // body_string_contains("grant_type=refresh_token") → MsaTokens; 400 {"error":"invalid_grant"} → AuthError::LoginExpired
}
```

- [ ] **Step 2: Fehlschlag prüfen**

Run: `cargo test -p hengst-core auth::`
Expected: FAIL (Modul `auth` fehlt)

- [ ] **Step 3: Implementieren** wie in Interfaces. Ein privater Helfer `post_form(&self, url, form) -> Result<reqwest::Response>` mappt Transportfehler auf `Error::Http`. Kein Token in einer URL.

- [ ] **Step 4: Alles grün** (Global-Constraints-Befehle)

- [ ] **Step 5: Commit**

```bash
git add crates/hengst-core/src/auth crates/hengst-core/src/lib.rs crates/hengst-core/src/error.rs
git commit -m "feat(auth): Microsoft device code flow with configurable client id"
```

---

### Task 5: Xbox Live und XSTS

**Files:**
- Create: `crates/hengst-core/src/auth/xbox.rs`
- Modify: `crates/hengst-core/src/auth/error.rs` (Texte), `auth/mod.rs` (`mod xbox;`)

**Interfaces:**
- Consumes: `MicrosoftAuth`, `AuthEndpoints`, `Secret`, `AuthError` (Task 4)
- Produces:
  - `#[derive(Debug, Clone)] pub struct XboxToken { pub token: Secret, pub uhs: String, pub xid: Option<String> }`
  - `pub async fn MicrosoftAuth::xbox_user_token(&self, msa_access: &Secret) -> Result<XboxToken>`: POST JSON an `{xbox_user}/user/authenticate`
    `{"Properties":{"AuthMethod":"RPS","SiteName":"user.auth.xboxlive.com","RpsTicket":"d=<token>"},"RelyingParty":"http://auth.xboxlive.com","TokenType":"JWT"}`, Header `Accept: application/json`, `x-xbl-contract-version: 1`. Antwort: `Token`, `DisplayClaims.xui[0].uhs`.
  - `pub async fn MicrosoftAuth::xsts_token(&self, user: &XboxToken) -> Result<XboxToken>`: POST JSON an `{xsts}/xsts/authorize`
    `{"Properties":{"SandboxId":"RETAIL","UserTokens":["<user token>"]},"RelyingParty":"rp://api.minecraftservices.com/","TokenType":"JWT"}`. `xid` aus `DisplayClaims.xui[0].xid`, falls vorhanden (kein zusätzlicher Request dafür).
  - `pub(crate) fn xsts_error(xerr: u64) -> AuthError` für HTTP 401 mit JSON-Feld `XErr`. Zuordnung und Texte exakt:
    - `2148916233` → `NoXboxAccount`: `this Microsoft account has no Xbox profile yet; sign in once at https://www.xbox.com to create one, then try again`
    - `2148916235` → `XboxRegionBlocked`: `Xbox Live is not available in this account's country or region`
    - `2148916236`, `2148916237` → `XboxAdultVerification`: `this account needs adult verification on the Xbox website before it can sign in`
    - `2148916238` → `XboxChildAccount`: `this is a child account; an adult must add it to a Microsoft family group before it can sign in`
    - sonst → `XboxRefused { code }`: `Xbox Live refused the sign-in (XErr {code})`
  - Fehlende `uhs` oder `Token` in einer 200-Antwort → `BadResponse("Xbox Live")`.

- [ ] **Step 1: Failing Tests** in `xbox.rs`

```rust
#[tokio::test]
async fn user_token_sends_rps_ticket_and_reads_uhs() {
    // Mock POST /user/authenticate mit body_partial_json({"Properties":{"RpsTicket":"d=msa-token"},"RelyingParty":"http://auth.xboxlive.com"})
    // Antwort {"Token":"xbl","DisplayClaims":{"xui":[{"uhs":"uhs1"}]}}
    assert_eq!(t.uhs, "uhs1"); assert_eq!(t.token.expose(), "xbl");
}

#[tokio::test]
async fn xsts_targets_minecraft_relying_party() {
    // body_partial_json({"RelyingParty":"rp://api.minecraftservices.com/","Properties":{"SandboxId":"RETAIL","UserTokens":["xbl"]}})
    // Antwort mit xui[0] = {"uhs":"uhs1","xid":"2535"} → xid == Some("2535")
}

#[tokio::test]
async fn xsts_401_maps_known_xerr_codes() {
    // 401 {"Identity":"0","XErr":2148916233,"Message":"","Redirect":"https://start.ui.xboxlive.com/CreateAccount"}
    // → Error::Auth(AuthError::NoXboxAccount)
}

#[test]
fn xerr_table() {
    assert!(matches!(xsts_error(2148916235), AuthError::XboxRegionBlocked));
    assert!(matches!(xsts_error(2148916236), AuthError::XboxAdultVerification));
    assert!(matches!(xsts_error(2148916237), AuthError::XboxAdultVerification));
    assert!(matches!(xsts_error(2148916238), AuthError::XboxChildAccount));
    assert!(xsts_error(42).to_string().contains("XErr 42"));
}
```

- [ ] **Step 2: Fehlschlag prüfen** — Run: `cargo test -p hengst-core auth::xbox` — Expected: FAIL

- [ ] **Step 3: Implementieren** wie in Interfaces (serde-Structs mit `#[serde(rename_all = "PascalCase")]`).

- [ ] **Step 4: Alles grün** (Global-Constraints-Befehle)

- [ ] **Step 5: Commit**

```bash
git add crates/hengst-core/src/auth
git commit -m "feat(auth): Xbox Live and XSTS tokens with readable XErr messages"
```

---

### Task 6: Minecraft-Dienste (Login, Besitz, Profil)

**Files:**
- Create: `crates/hengst-core/src/auth/minecraft.rs`
- Modify: `crates/hengst-core/src/auth/error.rs` (Texte), `auth/mod.rs`

**Interfaces:**
- Consumes: `MicrosoftAuth`, `XboxToken` (Task 5), `Secret`
- Produces:
  - `#[derive(Debug, Clone)] pub struct MinecraftToken { pub token: Secret, pub expires_in: u64 }`
  - `#[derive(Debug, Clone, PartialEq, Eq, Deserialize)] pub struct Profile { pub id: String, pub name: String }`
  - `pub async fn MicrosoftAuth::minecraft_login(&self, xsts: &XboxToken) -> Result<MinecraftToken>`: POST JSON `{"identityToken":"XBL3.0 x=<uhs>;<xsts token>"}` an `{minecraft}/authentication/login_with_xbox`. HTTP 403 → `AppNotApproved`.
  - `pub async fn MicrosoftAuth::owns_game(&self, mc: &Secret) -> Result<bool>`: GET `{minecraft}/entitlements/mcstore` mit `Authorization: Bearer`; `true`, wenn ein `items[].name` gleich `game_minecraft` oder `product_minecraft` ist. Signaturen werden nicht geprüft.
  - `pub async fn MicrosoftAuth::profile(&self, mc: &Secret) -> Result<Option<Profile>>`: GET `{minecraft}/minecraft/profile`; 404 → `None`. `id` muss 32 Hex-Zeichen sein, `name` 1 bis 16 Zeichen aus `[A-Za-z0-9_]`, sonst `BadResponse("Minecraft profile")` (die UUID wird Schlüssel im Credential Manager, der Name landet in Startargumenten).
  - `pub(crate) fn ownership(owns: bool, profile: Option<Profile>) -> Result<Profile>`, Entscheidung:
    | owns | profile | Ergebnis |
    |---|---|---|
    | egal | Some(p) | `Ok(p)` (Game Pass liefert oft leere Entitlements) |
    | true | None | `NoProfile` |
    | false | None | `GameNotOwned` |
  - Texte exakt:
    - `AppNotApproved`: `the Minecraft services rejected this launcher's Azure app (HTTP 403): the client id is not approved by Mojang yet`
    - `GameNotOwned`: `this Microsoft account does not own Minecraft: Java Edition`
    - `NoProfile`: `this account owns the game but has no player name yet; choose one on minecraft.net, then try again`

- [ ] **Step 1: Failing Tests** in `minecraft.rs`

```rust
#[tokio::test]
async fn login_builds_xbl3_identity_token() {
    // body_json({"identityToken":"XBL3.0 x=uhs1;xsts"}) → {"access_token":"mc","expires_in":86400,"token_type":"Bearer","username":"x"}
    assert_eq!(t.expires_in, 86400);
}

#[tokio::test]
async fn login_403_means_app_not_approved() {
    // 403 {"path":"/authentication/login_with_xbox","errorMessage":"Invalid app registration..."} → AuthError::AppNotApproved
}

#[tokio::test]
async fn entitlements_and_profile() {
    // mcstore {"items":[{"name":"product_minecraft"},{"name":"game_minecraft"}]} → true; {"items":[]} → false
    // header("authorization","Bearer mc") geprüft
    // profile 200 {"id":"069a79f444e94726a5befca90e38aaf5","name":"Notch","skins":[],"capes":[]} → Some; 404 → None
    // profile mit name "../x" → BadResponse
}

#[test]
fn ownership_table() { /* drei Zeilen der Tabelle oben */ }
```

- [ ] **Step 2: Fehlschlag prüfen** — Run: `cargo test -p hengst-core auth::minecraft` — Expected: FAIL

- [ ] **Step 3: Implementieren** wie in Interfaces.

- [ ] **Step 4: Alles grün** (Global-Constraints-Befehle)

- [ ] **Step 5: Commit**

```bash
git add crates/hengst-core/src/auth
git commit -m "feat(auth): Minecraft services login, ownership and profile"
```

---

### Task 7: Login-Kette `complete_login`

**Files:**
- Create: `crates/hengst-core/src/auth/flow.rs`
- Create: `crates/hengst-core/tests/auth_e2e.rs`
- Modify: `crates/hengst-core/src/auth/mod.rs`

**Interfaces:**
- Consumes: Tasks 4 bis 6
- Produces:
  - `#[derive(Debug, Clone)] pub struct LoginSession { pub profile: Profile, pub xuid: String, pub minecraft_token: Secret, pub minecraft_expires_at: u64, pub refresh_token: Secret }`
  - `pub async fn MicrosoftAuth::complete_login(&self, msa: MsaTokens, now: u64) -> Result<LoginSession>`: `xbox_user_token` → `xsts_token` → `minecraft_login` → `owns_game` + `profile` → `ownership`. `xuid = xid` oder `"0"`; `minecraft_expires_at = now + expires_in`.
  - `pub async fn MicrosoftAuth::refresh_session(&self, refresh: &Secret, now: u64) -> Result<LoginSession>` = `refresh_msa` + `complete_login`.
  - `pub fn LoginSession::launch_account(&self) -> Account` (nutzt `Account::microsoft`, Task 2)

- [ ] **Step 1: Failing Tests** in `tests/auth_e2e.rs` (ein MockServer für alle vier Basen, `AuthEndpoints` zeigt auf `server.uri()` plus Präfix je Dienst)

```rust
#[tokio::test]
async fn device_login_runs_the_whole_chain() {
    // devicecode → token (einmal pending, dann ok) → user/authenticate → xsts → login_with_xbox → mcstore → profile
    let s = auth.complete_login(tokens, 1_000).await.unwrap();
    assert_eq!(s.profile.name, "Notch");
    assert_eq!(s.minecraft_expires_at, 1_000 + 86_400);
    assert_eq!(s.refresh_token.expose(), "rt-1");
    let a = s.launch_account();
    assert_eq!(a.user_type(), "msa");
    assert!(!format!("{s:?}").contains("mc-token"));
}

#[tokio::test]
async fn game_pass_profile_without_entitlement_is_accepted() { /* mcstore leer, profile 200 → Ok */ }

#[tokio::test]
async fn refresh_session_returns_rotated_refresh_token() { /* /token mit refresh_token-Grant liefert "rt-2" → s.refresh_token == "rt-2" */ }
```

- [ ] **Step 2: Fehlschlag prüfen** — Run: `cargo test -p hengst-core --test auth_e2e` — Expected: FAIL

- [ ] **Step 3: Implementieren** wie in Interfaces.

- [ ] **Step 4: Alles grün** (Global-Constraints-Befehle)

- [ ] **Step 5: Commit**

```bash
git add crates/hengst-core/src/auth crates/hengst-core/tests/auth_e2e.rs
git commit -m "feat(auth): full Microsoft to Minecraft login chain"
```

---

### Task 8: Kontenablage (Metadaten-JSON und Credential Manager)

**Files:**
- Create: `crates/hengst-core/src/auth/store.rs`
- Modify: `Cargo.toml` (workspace.dependencies `keyring-core = "1.1"`, `windows-native-keyring-store = "1.1"`), `crates/hengst-core/Cargo.toml` (`[target.'cfg(windows)'.dependencies]` beide)
- Modify: `crates/hengst-core/src/paths.rs` (`accounts_file`, Test `layout_is_stable` erweitern)
- Modify: `crates/hengst-core/src/error.rs` (`CredentialStore(String)`), `auth/error.rs` (Texte)

**Interfaces:**
- Consumes: `LoginSession` (Task 7), `Secret`, `Paths`
- Produces:
  - `pub fn Paths::accounts_file(&self) -> PathBuf` = `<root>/accounts.json`
  - `Error::CredentialStore(String)`: `Windows Credential Manager error: {0}` (nur Fehlertext der Bibliothek, nie ein Wert)
  - `pub trait SecretStore: Send + Sync { fn get(&self, key: &str) -> Result<Option<Secret>>; fn set(&self, key: &str, value: &Secret) -> Result<()>; fn delete(&self, key: &str) -> Result<()>; }` (`delete` auf fehlenden Schlüssel ist `Ok`)
  - `#[derive(Default)] pub struct MemorySecretStore` (Mutex<HashMap>), für Tests und als Vorlage
  - `#[cfg(windows)] pub struct KeyringSecretStore` mit `pub fn new() -> Result<Self>`: `windows_native_keyring_store::Store::new()`, Einträge über `keyring_core::Entry::new_in_store(&store, SERVICE, key)`; Werte als UTF-8 über `set_secret`/`get_secret` (Blob-Grenze 2560 Byte, bei `set_password` wären es wegen UTF-16 nur 1280 Zeichen); `keyring_core::Error::NoEntry` → `Ok(None)` bzw. `Ok(())` beim Löschen; sonstige Fehler → `Error::CredentialStore(e.to_string())`.
  - `pub const SERVICE: &str = "hengst-client";`
  - `pub enum SecretKind { Refresh, Minecraft }`, `pub fn secret_key(uuid: &str, kind: SecretKind) -> String` = `"{uuid}/refresh"` bzw. `"{uuid}/minecraft"`
  - `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)] pub struct StoredAccount { pub uuid: String, pub name: String, pub xuid: String, pub minecraft_expires_at: u64 }`
  - `#[derive(Debug, Default, Serialize, Deserialize)] pub struct AccountsFile { pub version: u32 /* 1 */, pub default: Option<String> /* uuid */, pub accounts: Vec<StoredAccount> }`
  - `pub struct AccountStore<S: SecretStore> { file: PathBuf, secrets: S }`, Methoden:
    - `pub fn new(file: PathBuf, secrets: S) -> Self`
    - `pub fn load(&self) -> Result<AccountsFile>`: fehlende Datei → leer; kaputtes JSON → `Error::Json`, Datei bleibt unberührt
    - `pub fn save_session(&self, s: &LoginSession) -> Result<StoredAccount>`: Secrets zuerst, dann Datei; Upsert nach `uuid` (Name kann sich geändert haben); erstes Konto wird Standard
    - `pub fn find(&self, selector: Option<&str>) -> Result<StoredAccount>`: Name ohne Groß-/Kleinschreibung oder exakte UUID; `None` → Standardkonto
    - `pub fn set_default(&self, selector: &str) -> Result<StoredAccount>`
    - `pub fn remove(&self, selector: &str) -> Result<StoredAccount>`: löscht beide Secrets; war es Standard, wird das erste verbleibende Standard, sonst `None`
    - `pub fn secret(&self, account: &StoredAccount, kind: SecretKind) -> Result<Secret>`: fehlt → `AuthError::LoginExpired { name }`
    - `pub fn update_tokens(&self, account: &StoredAccount, s: &LoginSession) -> Result<StoredAccount>` (für Task 9; identisch zu `save_session`, ohne Standardwechsel)
  - Schreiben atomar: `accounts.json.tmp` schreiben, dann `rename`. Verzeichnis anlegen, falls nötig.
  - Texte: `AccountNotFound { selector }`: `no account matches "{selector}"; run "hengst accounts" to list them`; `NoAccount`: `no Microsoft account signed in; run "hengst login" first`

- [ ] **Step 1: Failing Tests** in `store.rs` (tempfile + `MemorySecretStore`)

```rust
#[test]
fn first_login_becomes_default_and_secrets_stay_out_of_the_file() {
    store.save_session(&session("Steve", UUID_A, "mc-a", "rt-a")).unwrap();
    let raw = std::fs::read_to_string(&file).unwrap();
    assert!(!raw.contains("mc-a") && !raw.contains("rt-a"));
    assert_eq!(store.find(None).unwrap().name, "Steve");
    assert_eq!(store.secret(&acc, SecretKind::Refresh).unwrap().expose(), "rt-a");
}

#[test]
fn relogin_updates_name_without_duplicate() { /* gleiche uuid, Name "Steve2" → accounts.len() == 1 */ }

#[test]
fn selectors_are_case_insensitive_and_default_moves_on_remove() {
    // Dev (Standard), Alex; remove("dev") → Standard ist Alex; remove("alex") → default None, find(None) → AuthError::NoAccount
    // remove entfernt beide secret_keys aus dem MemorySecretStore
}

#[test]
fn unknown_selector_is_a_clear_error() { /* find(Some("Nobody")) → AccountNotFound mit "Nobody" im Text */ }

#[test]
fn corrupt_file_is_reported_not_replaced() {
    std::fs::write(&file, "{not json").unwrap();
    assert!(matches!(store.load(), Err(Error::Json(_))));
    assert!(store.save_session(&session(..)).is_err());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "{not json");
}

#[cfg(windows)]
#[test]
#[ignore = "touches the real Windows Credential Manager"]
fn keyring_round_trip() {
    // Schlüssel "hengst-test/<zufall>", 2000-Byte-Wert: set, get == Wert, delete, get == None
}
```

- [ ] **Step 2: Fehlschlag prüfen** — Run: `cargo test -p hengst-core auth::store` — Expected: FAIL

- [ ] **Step 3: Implementieren** wie in Interfaces. Keyring-Aufrufe sind blockierend, aber kurz; kein `spawn_blocking` in dieser Phase.

- [ ] **Step 4: Alles grün** (Global-Constraints-Befehle) und einmal von Hand:

Run: `cargo test -p hengst-core keyring_round_trip -- --ignored`
Expected: PASS; danach unter "Anmeldeinformationsverwaltung > Windows-Anmeldeinformationen" kein `hengst-test`-Eintrag mehr.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock crates/hengst-core
git commit -m "feat(auth): account store with secrets in Windows Credential Manager"
```

---

### Task 9: Token-Refresh vor dem Start

**Files:**
- Modify: `crates/hengst-core/src/auth/flow.rs`
- Test: `crates/hengst-core/tests/auth_e2e.rs`

**Interfaces:**
- Consumes: `AccountStore`, `StoredAccount`, `SecretKind` (Task 8), `refresh_session` (Task 7)
- Produces:
  - `pub const REFRESH_MARGIN_SECS: u64 = 3600;`
  - `pub fn needs_refresh(account: &StoredAccount, now: u64) -> bool` = `minecraft_expires_at <= now + REFRESH_MARGIN_SECS`
  - `pub async fn launch_account<S: SecretStore>(store: &AccountStore<S>, auth: Option<&MicrosoftAuth>, selector: Option<&str>, now: u64) -> Result<Account>`:
    1. `store.find(selector)`
    2. kein Refresh nötig → gespeichertes Minecraft-Token, **kein** Netzwerk, `auth` darf `None` sein
    3. Refresh nötig, `auth` ist `None` → `AuthError::MissingClientId`
    4. `refresh_session` ok → `store.update_tokens` (neues Refresh-Token wird gespeichert), Konto daraus
    5. `LoginExpired`/`invalid_grant` → `LoginExpired { name: account.name }` (Konto bleibt gespeichert)
    6. sonstiger Fehler (Netz, 5xx) und `minecraft_expires_at > now` → altes, noch gültiges Token verwenden; sonst den Fehler zurückgeben

- [ ] **Step 1: Failing Tests** in `tests/auth_e2e.rs` (MemorySecretStore, Mock-Server)

```rust
#[tokio::test]
async fn fresh_token_launches_without_network() {
    // expires_at = now + 7200, auth = None → Ok, user_type "msa"
}

#[tokio::test]
async fn near_expiry_refreshes_and_stores_rotated_token() {
    // expires_at = now + 600; Mock-Kette liefert refresh_token "rt-2", mc "mc-2"
    // danach store.secret(.., Refresh) == "rt-2", .., Minecraft) == "mc-2", minecraft_expires_at == now + 86400
}

#[tokio::test]
async fn rejected_refresh_asks_for_login_again() {
    // /token 400 invalid_grant → Err(Auth(LoginExpired { name: "Steve" })), Konto noch in store.load()
}

#[tokio::test]
async fn offline_network_near_expiry_uses_still_valid_token() {
    // expires_at = now + 1800, AuthEndpoints auf einen geschlossenen Port (Listener binden und droppen) → Ok mit altem Token
    // expires_at = now - 1 → Err
}
```

- [ ] **Step 2: Fehlschlag prüfen** — Run: `cargo test -p hengst-core --test auth_e2e` — Expected: FAIL

- [ ] **Step 3: Implementieren** wie in Interfaces.

- [ ] **Step 4: Alles grün** (Global-Constraints-Befehle)

- [ ] **Step 5: Commit**

```bash
git add crates/hengst-core/src/auth/flow.rs crates/hengst-core/tests/auth_e2e.rs
git commit -m "feat(auth): refresh the Minecraft token before launch"
```

---

### Task 10: CLI `login`, `accounts`, `logout`, `launch --account`

**Files:**
- Modify: `apps/cli/src/main.rs`
- Modify: `.github/workflows/ci.yml` (Dry-Run-Matrix)

**Interfaces:**
- Consumes: `client_id`, `MicrosoftAuth`, `AuthEndpoints`, `AccountStore`, `KeyringSecretStore`, `launch_account`, `unix_now` (Tasks 4 bis 9), `ensure_launch_allowed`, `DEV_OFFLINE` (Task 3)
- Produces (clap, an die bestehende `Cmd`-Struktur angehängt):
  - `Cmd::Login`: `client_id()?` → `start_device_login` → druckt `To sign in, open {verification_uri} and enter the code {user_code}` und `Waiting for sign-in...` → `poll_device_login` → `complete_login` → `save_session` → `Signed in as {name}` (bei erstem Konto zusätzlich ` (default account)`)
  - `Cmd::Accounts { #[command(subcommand)] action: Option<AccountsCmd> }`; ohne Unterbefehl Liste: je Konto `{marker} {name:<16} {uuid}` mit Marker `*` für Standard, sonst Leerzeichen; leer: `No accounts. Run "hengst login" to add one.`
  - `AccountsCmd::Use { name: String }` → `set_default` → `{name} is now the default account`
  - `Cmd::Logout { name: String }` → `remove` → `Removed {name}` (Hinweis im Hilfetext: entfernt nur lokal gespeicherte Tokens)
  - `Cmd::Launch`: `--username` entfällt; neu `--account <NAME>` und `--offline <NAME>` (`conflicts_with`). `--offline` → `Account::offline`; sonst `launch_account(&store, auth.as_ref(), account.as_deref(), unix_now())` mit `auth = client_id().ok().map(|id| MicrosoftAuth::new(id, AuthEndpoints::default()))`. Ohne `--dry-run` vor `prepare`: `ensure_launch_allowed(account.kind(), DEV_OFFLINE)?`.
  - Store-Konstruktion: `AccountStore::new(paths.accounts_file(), KeyringSecretStore::new()?)`; die CLI ist Windows-only, ein `#[cfg(not(windows))]`-Zweig bricht mit `account storage is only implemented on Windows` ab.
  - CI-Matrix: `... launch ${{ matrix.version }} --offline Dev --dry-run`

- [ ] **Step 1: Failing Check** — die Kontologik ist in Tasks 8 und 9 getestet; hier ein Parser-Test in `main.rs`:

```rust
#[test]
fn launch_account_and_offline_conflict() {
    assert!(Cli::try_parse_from(["hengst", "launch", "1.8.9", "--account", "a", "--offline", "b"]).is_err());
    assert!(Cli::try_parse_from(["hengst", "accounts", "use", "Steve"]).is_ok());
    assert!(Cli::try_parse_from(["hengst", "logout", "Steve"]).is_ok());
    Cli::command().debug_assert();
}
```

- [ ] **Step 2: Fehlschlag prüfen** — Run: `cargo test -p hengst-cli` — Expected: FAIL (Unterbefehle fehlen)

- [ ] **Step 3: Implementieren** wie in Interfaces.

- [ ] **Step 4: Alles grün und von Hand prüfen** (Global-Constraints-Befehle), dann mit leerem `--home <tmp>`:
  - `hengst --home <tmp> accounts` → `No accounts. Run "hengst login" to add one.`
  - `hengst --home <tmp> launch 1.8.9` → `no Microsoft account signed in; run "hengst login" first`, kein `<tmp>/versions`
  - `HENGST_MS_CLIENT_ID=` leer, `hengst login` → Meldung mit `HENGST_MS_CLIENT_ID`
  - `hengst --home <tmp> launch rd-132211 --offline Dev --dry-run` → Kommandozeile wie bisher

- [ ] **Step 5: Commit**

```bash
git add apps/cli/src/main.rs .github/workflows/ci.yml
git commit -m "feat(cli): login, accounts, logout and launch with a Microsoft account"
```

---

### Task 11: README, MSRV, PLAN-Status und echter Login

**Files:**
- Modify: `README.md` (Status, Usage, "Requires Rust 1.88 or newer" an beiden Stellen)
- Modify: `docs/PLAN.md` Abschnitt 11, Statuszeile (Deutsch)

**Interfaces:**
- Consumes: alles Vorherige

- [ ] **Step 1: README** (Englisch): Status-Satz auf "signs in with Microsoft accounts (`hengst login`), manages several accounts and starts any vanilla version"; solange `CLIENT_ID` `None` ist, der Zusatz "Until the launcher's Azure app is approved by Mojang, set `HENGST_MS_CLIENT_ID` to your own approved app id". Kurzer Usage-Block mit `hengst login`, `hengst accounts`, `hengst accounts use <name>`, `hengst logout <name>`, `hengst launch <version> [--account <name>]`. MSRV 1.85 → 1.88 (keyring-core/windows-native-keyring-store verlangen 1.88). Hinweis "Not an official Minecraft product" bleibt unverändert.

- [ ] **Step 2: PLAN.md Abschnitt 11**, Statuszeile ersetzen durch eine der beiden Varianten:
  - mit echtem Test: `Stand <Datum>: Phase 0 bis 2 erledigt (Phase 2: PR #<n>, Microsoft-Login per Device Code, Mehrfachkonten, Refresh, Offline-Sperre im Kern). Echt angemeldet und gestartet: <Versionen>. Offene Punkte je Phase stehen im Abschlussbericht des Final-Reviews.`
  - ohne Client-ID: `Stand <Datum>: Phase 2 (Login) umgesetzt und gegen Mock-Server getestet, PR #<n>. Echter Login offen, bis die Azure-App von Mojang freigegeben ist (Antrag bei Clemens); bis dahin Client-ID per HENGST_MS_CLIENT_ID.`

- [ ] **Step 3: Echter Login (nur mit freigegebener Client-ID)**, Sichtprüfung per Agentfenster:
  - `HENGST_MS_CLIENT_ID=<id> hengst login`, Code im Browser eingeben → `Signed in as <name>`
  - `hengst accounts` zeigt das Konto mit `*`; Credential Manager hat zwei `hengst-client`-Einträge; `accounts.json` enthält kein Token
  - Länge von Refresh- und Minecraft-Token notieren (nur die Zahl), muss unter 2560 Byte liegen
  - `hengst launch 1.21.11 --dry-run` zeigt `<redacted>` und `--userType msa`
  - `hengst launch 1.21.11` erreicht das Titelbild (stumme `options.txt`), Beitritt zu einem Online-Mode-Server klappt
  - `hengst logout <name>` entfernt beide Credential-Manager-Einträge

- [ ] **Step 4: Alles grün** (Global-Constraints-Befehle)

- [ ] **Step 5: Commit**

```bash
git add README.md docs/PLAN.md
git commit -m "docs: phase 2 login status"
```

---

## Self-Review

1. **Spec-Abdeckung:** Offline-Sperre im Kern (Task 3), Schwärzung (Task 2), `user_type` msa (Task 1), Notch-Vektor aus dem Final-Review (Task 1), Device Code `consumers` + Scope (Task 4), Xbox/XSTS mit XErr-Texten (Task 5), `login_with_xbox`/403/Besitz/Profil (Task 6), Kette (Task 7), Mehrfachkonten mit Keyring + JSON + Trait (Task 8), Refresh (Task 9), CLI (Task 10), Client-ID-Konstante + Env + Fehler (Task 4), README/PLAN (Task 11). PKCE ausdrücklich ausgeklammert (Header).
2. **Schritte:** jede Code-Stufe nennt Signatur, Datei und die festgelegten Werte; Fehlertexte sind wörtlich vorgegeben.
3. **Typen:** `Secret`, `AccountKind`, `Account::microsoft(String, String, Secret, String)`, `XboxToken`, `MinecraftToken`, `Profile`, `LoginSession`, `StoredAccount`, `AccountStore<S>`, `launch_account(store, Option<&MicrosoftAuth>, Option<&str>, u64)` werden in späteren Tasks mit denselben Namen genutzt.
4. **Review Focus:** alle fünf Punkte haben einen Test in der zuständigen Task (2, 8, 9, 9, 8).
5. **Umfang:** Code-Blöcke sind Testnamen, Assertions und Wire-Formate; keine Funktionskörper.
