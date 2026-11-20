# Hengst Client Phase 3 (Loader) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `hengst launch <version> --loader <fabric|quilt|legacy-fabric|neoforge|forge> [--loader-version <v>]` bereitet ein Modded-Spiel vollständig vor und startet es; ohne `--loader-version` mit der neuesten stabilen Loader-Version für diese Spielversion. `hengst loaders <loader> <version>` listet die verfügbaren Loader-Versionen.

**Architecture:** Neuer Modulbaum `crates/hengst-core/src/loaders/` im bestehenden Kern-Crate (kein eigenes Crate, Abweichung von PLAN.md 3 ist gewollt). Jeder Loader liefert eine Kind-Versions-JSON mit `inheritsFrom`; ein generischer Schritt `merge_version` legt sie auf die Vanilla-JSON. Fabric, Quilt und Legacy Fabric holen das Profil aus ihrer Meta-API. Forge und NeoForge laden den Installer-Jar, lesen `install_profile.json` und `version.json` (oder das Legacy-Format `install`/`versionInfo`) und führen die Client-Prozessoren generisch mit Java aus, wie Prism Launcher. Alle Basis-URLs stecken in `LoaderEndpoints` und sind in Tests auf wiremock umgebogen.

**Tech Stack:** wie Phase 1 (tokio, reqwest rustls, serde, sha1, zip 9, regex, clap 4, thiserror; Tests wiremock, tempfile). Keine neuen Abhängigkeiten.

**Spec:** `docs/PLAN.md` (Abschnitte 3, 4, 8, 10, 11, 12) und der Phase-3-Auftrag vom 20.11.2026 (Umfang Fabric, Quilt, Legacy Fabric, NeoForge, Forge; Ornithe und Cleanroom nicht in Phase 3).

**Ausgangslage:** Phase 2 (Microsoft-Login, Modul `auth/`, Kontobefehle) ist gemergt, bevor diese Phase beginnt. Der CLI-Befehl `launch` hat dann bereits eine Option `--account`; dieser Plan fasst sie und alles Kontobezogene nicht an. Wo Phase 2 `LaunchOptions`, `Endpoints` oder `Cmd::Launch` um Felder erweitert hat, bleiben diese unverändert, die neuen Felder kommen dazu. Arbeit auf Branch `phase-3-loader` von `main`.

**Live geprüft am 20.11.2026** (curl gegen die echten Dienste, Installer-Jars ausgepackt). Die Werte in den Tests stammen aus diesen Antworten:

| Quelle | Endpunkt | Befund |
|---|---|---|
| Fabric | `https://meta.fabricmc.net/v2/versions/loader/{game}` | Liste neueste zuerst, `loader.stable`; unbekannte Version: HTTP 400 bzw. `[]` |
| Fabric | `.../v2/versions/loader/{game}/{loader}/profile/json` | Profil mit `inheritsFrom`; asm/mixin mit `sha1`, `fabric-loader` und `intermediary` ohne Hash |
| Quilt | `https://meta.quiltmc.org/v3/versions/loader/{game}` (+ `/{loader}/profile/json`) | Liste **unsortiert**, kein `stable`-Feld, Vorabversionen mit `-beta.N`; Profil-Libraries ohne Hashes; unbekannte Version: 404 |
| Legacy Fabric | `https://meta.legacyfabric.net/v2/...` (wie Fabric) | Profil ersetzt LWJGL 2 (`org.lwjgl.lwjgl:*:2.9.4+legacyfabric.17`, Natives über `natives`-Map ohne `downloads`), kein `arguments`-Block; Maven leitet per 302 auf `repo.legacyfabric.net` um |
| Forge | `https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json`, `.../promotions_slim.json` | Spielversion → aufsteigende Liste voller Maven-Versionen (`1.7.10-10.13.4.1614-1.7.10`); Promos `<game>-recommended`/`<game>-latest` nur mit Kurzform |
| Forge | `https://maven.minecraftforge.net/net/minecraftforge/forge/{v}/forge-{v}-installer.jar(.sha1)` | Installer für alle Builds 1.7.10 bis 26.3 vorhanden, `.sha1` vorhanden |
| NeoForge | `https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge` | `{"isSnapshot":false,"versions":[...]}`; Schema `21.1.267` (= 1.21.1), `26.3.0.115` (= 26.3), Vorab `-beta`, Snapshots `+snapshot-N`, Aprilscherz `0.25w14...` |
| NeoForge 1.20.1 | `.../api/maven/versions/releases/net/neoforged/forge`, Installer `.../releases/net/neoforged/forge/{v}/forge-{v}-installer.jar` | Versionen `1.20.1-47.1.106`, dazu ein Ausreißer `47.1.82` ohne Präfix |
| Maven-Sidecars | `<jar-url>.sha1` | vorhanden für Fabric-, Quilt-, Legacy-Fabric-, Forge-, NeoForge- und Mojang-Libraries; fehlt bei zwei Scala-Jars von Forge 1.7.10/1.8.9 (die haben dafür `checksums`, Eintrag 0 = SHA1 der Jar, nachgerechnet) |

Installer-Formate (ausgepackt): Forge 1.7.10, 1.8.9 und 1.12.2 bis 14.23.5.2847 haben das Legacy-Format (`install` + `versionInfo`, Universal-Jar unter `install.filePath` im Installer). Ab 1.12.2 14.23.5.2851 gilt `spec` 0 (1.12.2 ohne Prozessoren, 1.16.5 mit vier), ab 1.20.1 `spec` 1 mit `sides`. Forge 26.3 und NeoForge 26.3 nutzen dasselbe Format mit anderen Prozessoren (`binarypatcher` bzw. `installertools PROCESS_MINECRAFT_JAR` mit `{ROOT}/libraries/`). `logging` in Forge-`version.json` ist immer `{}`.

## Entscheidungen

- **Forge-Abdeckung:** jeder Forge-Build für Minecraft 1.7.10 bis aktuell (26.x), der auf dem Forge-Maven einen Installer hat, in beiden Installer-Formaten (Legacy `install`/`versionInfo` und modern `spec` 0/1 mit Prozessoren). Damit sind 1.7.10, 1.8.9, 1.12.2, 1.16.5, 1.20.1 und 26.3 gedeckt. Spielversionen vor 1.7.10 lehnt der Kern vor jedem Download ab: `Forge for Minecraft 1.6.4 is not supported (Forge support starts at 1.7.10)`. Universal-Jars ohne Installer gibt es ab 1.7.10 nicht, daher kein Sonderweg.
- **NeoForge-Abdeckung:** 1.20.1 (Artefakt `net.neoforged:forge`) und alle Versionen ab 1.20.2 (`net.neoforged:neoforge`). Snapshot-Builds (`+snapshot`) und der Aprilscherz-Zweig `0.*` werden ignoriert.
- **Standardversion:** neueste stabile Loader-Version. Stabil heißt: Fabric/Legacy Fabric `loader.stable`; Quilt und NeoForge ohne `-` in der Version; Forge die Promo `<game>-recommended`. Gibt es keine stabile, nimmt der Kern die neueste überhaupt, die CLI schreibt dann `note: no stable <loader> release for Minecraft <game>, using <v>` auf stderr.
- **Hash-Prüfung:** Reihenfolge je Library: `downloads.artifact.sha1` oder `sha1` (Fabric-Stil) > `checksums[0]` (Forge-Legacy) > `.sha1`-Sidecar vom selben Maven (wird neben der Jar als `<jar>.sha1` gespeichert und danach lokal gelesen) > Abbruch mit `Error::NoChecksum`. Ungeprüft heruntergeladen wird nichts. Nicht kryptografisch abgesichert und nur durch HTTPS geschützt sind: die Meta-Antworten (Versionslisten, Fabric-/Quilt-/Legacy-Fabric-Profil-JSON, Forge-Metadaten/Promos, NeoForge-API) und die Mojmaps, die der Forge-Prozessor `DOWNLOAD_MOJMAPS` (1.17 bis 1.20.x) selbst lädt. Der Sidecar schützt vor kaputten Downloads, nicht vor einem kompromittierten Maven.
- **Prozessor-Ergebnisse:** nach dem Lauf werden alle deklarierten `outputs` gegen ihre SHA1 geprüft, `PATCHED` gegen `PATCHED_SHA` (falls vorhanden), und jede `version.json`-Library mit leerer `url` gegen ihre `sha1`. Prozessoren ohne Ausgabe-Hash (z. B. NeoForge 26.3) sind nur über die Existenz von `PATCHED` und den Exit-Code geprüft.
- **Java:** Spiel und Prozessoren laufen mit der Laufzeit, die die Vanilla-Version verlangt (`component_for(&vanilla)`), auch wenn die Kind-JSON eine `javaVersion` mitbringt. `--java` überschreibt beides.
- **Merge-Regeln** (offizieller Launcher + Prism): `id`, `mainClass`, `type` vom Kind; `arguments.jvm` und `arguments.game` = Eltern gefolgt von Kind; `minecraftArguments` vom Kind, sonst Eltern; Libraries = Kind zuerst, dann Eltern ohne die, deren Schlüssel `group:artifact[:classifier]` schon beim Kind steht; `logging` vom Kind nur, wenn es `client` enthält; `assetIndex`, `assets`, `downloads`, `javaVersion` immer von den Eltern. Hat das Kind JVM-Argumente, die Eltern aber keinen `arguments`-Block, werden die Legacy-Standardargumente (`LEGACY_JVM`) vorangestellt.
- **Client-Jar:** für Loader-Versionen liegt die Vanilla-Jar zusätzlich als `versions/<merged id>/<merged id>.jar` (Hardlink, sonst Kopie), damit `${version_name}.jar` in Forges `-DignoreList` passt (Layout des offiziellen Launchers). Prozessoren bekommen als `{MINECRAFT_JAR}` die Vanilla-Jar.
- **Instanzordner:** ohne `--instance` heißt er `<game>-<loader>` (z. B. `1.20.1-forge`), damit ein Loader-Update denselben Ordner (Mods, Welten) behält.
- **Ablage:** Fabric-Familien-Profile unter `meta/loaders/<loader>/<id>.json` (Offline-Rückfall), Installer unter `libraries/` an ihrem Maven-Pfad, Install-Marker `meta/loaders/<loader>/<id>.installed` (SHA1 des Installers), Prozessor-Log `logs/loader-install-<id>.log`, Arbeitsordner `tmp/loader-<id>/` (nach Erfolg gelöscht).
- **Prozessor-Sicherheit:** Prozessoren sind fremder Java-Code vom Loader-Maven; Vertrauensgrenze ist der per SHA1 geprüfte Installer. Pfade aus Metadaten gehen durch `join_relative`; Prozessor-Argumente, die nach der Ersetzung ein `..`-Segment enthalten, werden abgelehnt.

## Global Constraints

- Plattform im MVP: nur Windows 10/11 x64.
- "Minecraft" nie im Produktnamen; Fehlertexte dürfen das Spiel beim Namen nennen.
- Kein Spielstart mit Offline-Konto in Release-Builds: nur mit Feature `dev-offline` (bleibt wie Phase 2 es gebaut hat).
- Code, Kommentare, Fehlertexte, README: Englisch. `docs/`: Deutsch.
- Tokens nie loggen oder ausgeben; die Prozessor-Logs enthalten keine Kontodaten (Prozessoren bekommen kein Konto).
- Alle HTTP-Basis-URLs injizierbar (`LoaderEndpoints`); Standardtests machen keine Live-Aufrufe. Live-Tests sind `#[ignore]`.
- Jeder Pfad aus Remote-Metadaten (Maven-Koordinaten, Installer-Einträge, Versions-IDs) geht durch `paths::join_relative` bzw. `paths::validate_id` (Commit c11aea0).
- Wenig RAM: nie zwei Spielinstanzen, nie Build und Spiel gleichzeitig, Live-Tests nur mit `--test-threads=1`. Echte Testläufe schreiben vorher `soundCategory_master:0.0` in `options.txt`.
- Keine Emojis in Code, Doku, Commits.
- Jeder Task endet mit grünem `cargo test --workspace --all-features`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all` und einem Commit, der mit `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>` endet.

## Review Focus

1. **Loader passt nicht zur Spielversion** (Fabric 1.8.9, Forge 1.6.4, NeoForge 1.16.5, Quilt für einen Snapshot): klare Fehlermeldung, bevor ein einziges Vanilla-Library-Byte geladen wird. Test in Task 5 (`loader_errors_come_before_vanilla_downloads`) und Task 8.
2. **`--loader-version` in Kurz- oder Langform oder vertippt** (`47.4.28` vs. `1.20.1-47.4.28`, `10.13.4.1614` vs. `1.7.10-10.13.4.1614-1.7.10`, `0.0.1`): Kurz- und Langform finden dieselbe Version, Unbekanntes nennt `hengst loaders <loader> <game>` als Ausweg. Test in Task 3 und Task 8.
3. **Zweiter Start nach erfolgreicher Forge-Installation** darf die Prozessoren nicht erneut laufen lassen (Startzeit, RAM); ein gelöschtes oder beschädigtes `PATCHED`-Ergebnis oder ein abgebrochener Lauf muss dagegen zur Neuinstallation führen. Test in Task 10 (`install_state_*`).
4. **Bösartige oder kaputte Pfade in Installer und Metadaten** (`maven/../../x.jar` im Installer, Koordinate `..:evil:1`, Prozessor-Argument `{ROOT}/../evil`): `Error::UnsafePath`, nichts wird außerhalb des Datenordners geschrieben. Tests in Task 2, 9 und 10.
5. **Prozessor scheitert** (Java-Exit ≠ 0, Ausgabe-Hash falsch): Fehler nennt Prozessor, Exit-Code und Logpfad, der Install-Marker wird nicht geschrieben, der nächste Start versucht es erneut. Test in Task 11 (`failed_processor_leaves_no_marker`).

---

## Dateistruktur

```
crates/hengst-core/src/
  error.rs              + Loader-Fehlervarianten
  paths.rs              + loader_dir, logs, tmp
  version.rs            + inheritsFrom, Library.sha1/checksums/clientreq
  args.rs               LEGACY_JVM wird pub
  libraries.rs          Loader-Libraries (Maven-Natives, leere URL, clientreq)
  download.rs           + fill_missing_sha1 (Sidecar)
  launch.rs             Loader in prepare(), Endpoints.loaders, LaunchOptions.loader, Prepared.loader
  loaders/mod.rs        LoaderKind, LoaderVersion, LoaderRequest, LoaderEndpoints, Versionsvergleich, Auswahl, prepare_loader
  loaders/merge.rs      merge_version, library_key
  loaders/fabric.rs     Fabric, Quilt, Legacy Fabric (gemeinsame Meta-API)
  loaders/forge/mod.rs  Forge- und NeoForge-Versionslisten, Installer-Download, install()
  loaders/forge/installer.rs   install_profile.json lesen (beide Formate), eingebettete Dateien entpacken
  loaders/forge/processors.rs  Daten/Argumente ersetzen, Planen, Ausführen, Ergebnisse prüfen
crates/hengst-core/tests/
  fixtures/loaders/*.json      gekürzte echte Antworten
  loader_e2e.rs                Ende-zu-Ende gegen Mock (Fabric, Legacy Fabric, Forge Legacy, Forge spec 0)
  loader_live.rs               #[ignore] echte Starts
apps/cli/src/main.rs           --loader, --loader-version, Befehl loaders
.github/workflows/ci.yml       Loader-Dry-Run-Matrix
```

## Task-Reihenfolge

Streng nacheinander: 1 → 2 → 3 → 4 → 5 → 6 → 7 (Fabric-Familie echt gestartet) → 8 → 9 → 10 → 11 → 12 → 13. Kein Task läuft parallel zu einem anderen (RAM).

---

### Task 1: Versionsmodell, Fehler, Pfade, Merge

**Files:**
- Modify: `crates/hengst-core/src/version.rs` (Structs `VersionJson`, `Library`)
- Modify: `crates/hengst-core/src/error.rs`, `crates/hengst-core/src/paths.rs`, `crates/hengst-core/src/args.rs:49-53` (`LEGACY_JVM` → `pub const`), `crates/hengst-core/src/lib.rs`
- Create: `crates/hengst-core/src/loaders/mod.rs` (vorerst nur `pub mod merge;`), `crates/hengst-core/src/loaders/merge.rs`

**Interfaces:**
- Produces:
  - `VersionJson.inherits_from: Option<String>` (`#[serde(rename = "inheritsFrom")]`)
  - `Library.sha1: Option<String>`, `Library.checksums: Vec<String>` (`#[serde(default)]`), `Library.clientreq: Option<bool>`
  - `Error::{NoLoaderVersions { loader: String, game: String }, UnknownLoaderVersion { loader: String, game: String, version: String }, LoaderUnsupported { loader: String, game: String, reason: String }, NoChecksum { url: String }, ParentMismatch { expected: String, found: String }, Installer(String), Processor { jar: String, code: i32, log: PathBuf }}`
  - `Paths::loader_dir(&self, loader: &str) -> PathBuf` (`<root>/meta/loaders/<loader>`), `Paths::logs(&self) -> PathBuf` (`<root>/logs`), `Paths::tmp(&self) -> PathBuf` (`<root>/tmp`)
  - `pub const args::LEGACY_JVM: &[&str]`
  - `loaders::merge::library_key(name: &str) -> String`
  - `loaders::merge::merge_version(parent: &VersionJson, child: &VersionJson) -> Result<VersionJson>`

Fehlertexte exakt:
```
NoLoaderVersions:     "no {loader} versions available for Minecraft {game}"
UnknownLoaderVersion: "{loader} {version} is not available for Minecraft {game} (see `hengst loaders {loader} {game}`)"
LoaderUnsupported:    "{loader} for Minecraft {game} is not supported ({reason})"
NoChecksum:           "no checksum available for {url}; refusing to download it unverified"
ParentMismatch:       "loader profile inherits from {found}, expected {expected}"
Installer:            "invalid loader installer: {0}"
Processor:            "loader installer step {jar} failed with exit code {code}, see {log}"
```

- [ ] **Step 1: Failing Tests schreiben** (in `merge.rs`, `#[cfg(test)]`, Daten mit `serde_json::json!`)

```rust
#[test]
fn library_key_ignores_version_and_extension() {
    assert_eq!(library_key("org.lwjgl.lwjgl:lwjgl:2.9.4+legacyfabric.17"), "org.lwjgl.lwjgl:lwjgl");
    assert_eq!(library_key("org.lwjgl:lwjgl:3.3.3:natives-windows"), "org.lwjgl:lwjgl:natives-windows");
    assert_eq!(library_key("de.oceanlabs.mcp:mcp_config:1.12.2-20200226.224830@zip"), "de.oceanlabs.mcp:mcp_config");
}

#[test]
fn merges_fabric_profile_onto_modern_parent() {
    // parent: id 1.21.11, jvm ["-cp","${classpath}"], game ["--username","${auth_player_name}"],
    //   assetIndex 29, downloads.client, javaVersion java-runtime-delta/21, logging client,
    //   libraries [com.mojang:brigadier:1.3.10, org.lwjgl:lwjgl:3.3.3, org.lwjgl:lwjgl:3.3.3:natives-windows]
    // child: echtes Fabric-Profil-Muster: id fabric-loader-0.17.3-1.21.11, inheritsFrom 1.21.11,
    //   mainClass net.fabricmc.loader.impl.launch.knot.KnotClient,
    //   jvm ["-XX:+UnlockDiagnosticVMOptions","-XX:-UseBASE64Intrinsics","-DFabricMcEmu= net.minecraft.client.main.Main "], game [],
    //   javaVersion java-runtime-gamma/17, libraries [org.ow2.asm:asm:9.9 (sha1 c29635c8a7afa03d74b33c1884df8abb2b3f3dcc), net.fabricmc:fabric-loader:0.17.3]
    let m = merge_version(&parent, &child).unwrap();
    assert_eq!(m.id, "fabric-loader-0.17.3-1.21.11");
    assert_eq!(m.main_class, "net.fabricmc.loader.impl.launch.knot.KnotClient");
    let jvm = &m.arguments.as_ref().unwrap().jvm;
    assert_eq!(jvm.len(), 5);
    assert!(matches!(&jvm[0], Argument::Plain(s) if s == "-cp"));
    assert!(matches!(&jvm[4], Argument::Plain(s) if s.starts_with("-DFabricMcEmu=")));
    let names: Vec<_> = m.libraries.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names[..2], ["org.ow2.asm:asm:9.9", "net.fabricmc:fabric-loader:0.17.3"]);
    assert_eq!(names.len(), 5);
    assert_eq!(m.java_version.unwrap().major_version, 21);
    assert_eq!(m.asset_index.unwrap().id, "29");
    assert!(m.downloads.unwrap().client.is_some());
    assert!(m.logging.unwrap().client.is_some());
}

#[test]
fn child_libraries_replace_parent_by_key() {
    // parent 1.8.9: org.lwjgl.lwjgl:lwjgl:2.9.4-nightly-20150209 (rules not osx),
    //   org.lwjgl.lwjgl:lwjgl:2.9.2-nightly-20140822 (rules osx), org.lwjgl.lwjgl:lwjgl-platform:2.9.4-nightly-20150209 (natives),
    //   com.google.guava:guava:17.0
    // child (Legacy Fabric): org.lwjgl.lwjgl:lwjgl:2.9.4+legacyfabric.17, org.lwjgl.lwjgl:lwjgl-platform:2.9.4+legacyfabric.17
    let m = merge_version(&parent, &child).unwrap();
    let names: Vec<_> = m.libraries.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["org.lwjgl.lwjgl:lwjgl:2.9.4+legacyfabric.17",
        "org.lwjgl.lwjgl:lwjgl-platform:2.9.4+legacyfabric.17", "com.google.guava:guava:17.0"]);
}

#[test]
fn legacy_forge_replaces_minecraft_arguments_and_keeps_parent_logging() {
    // parent 1.12.2 legacy (minecraftArguments "--username ${auth_player_name}", logging client)
    // child: echtes Forge-1.12.2-Muster, minecraftArguments endet auf
    //   "--tweakClass net.minecraftforge.fml.common.launcher.FMLTweaker --versionType Forge", "logging": {}
    let m = merge_version(&parent, &child).unwrap();
    assert!(m.minecraft_arguments.unwrap().ends_with("--versionType Forge"));
    assert!(m.arguments.is_none());
    assert!(m.logging.unwrap().client.is_some());
}

#[test]
fn child_jvm_args_on_legacy_parent_get_legacy_defaults_first() {
    // parent: legacy, keine arguments; child: arguments.jvm ["-Dx=1"], keine game-Args
    let m = merge_version(&parent, &child).unwrap();
    let jvm: Vec<String> = m.arguments.unwrap().jvm.into_iter()
        .map(|a| match a { Argument::Plain(s) => s, _ => panic!() }).collect();
    let mut expected: Vec<String> = crate::args::LEGACY_JVM.iter().map(|s| s.to_string()).collect();
    expected.push("-Dx=1".into());
    assert_eq!(jvm, expected);
    assert!(m.minecraft_arguments.is_some());
}

#[test]
fn rejects_profile_for_other_parent() {
    // child.inheritsFrom = "1.21.10", parent.id = "1.21.11"
    assert!(matches!(merge_version(&parent, &child),
        Err(Error::ParentMismatch { expected, found }) if expected == "1.21.11" && found == "1.21.10"));
}
```

In `paths.rs` `layout_is_stable` ergänzen:
```rust
assert_eq!(p.loader_dir("fabric"), PathBuf::from("C:/h/meta/loaders/fabric"));
assert_eq!(p.logs(), PathBuf::from("C:/h/logs"));
assert_eq!(p.tmp(), PathBuf::from("C:/h/tmp"));
```

In `version.rs` neuer Test `parses_loader_fields`: `inheritsFrom` → `Some("1.21.11")`; Library mit `"sha1"` auf oberster Ebene, `"checksums": ["a","b"]`, `"clientreq": false` wird gelesen; Library ohne diese Felder hat `checksums == []`, `clientreq == None`.

- [ ] **Step 2: Tests laufen lassen, sie scheitern**

Run: `cargo test -p hengst-core merge`
Expected: FAIL (Modul/Funktionen fehlen)

- [ ] **Step 3: Implementieren**

Felder, Fehlervarianten, Pfade wie in Interfaces. `merge_version` nach den Merge-Regeln aus "Entscheidungen"; `inherits_from` des Ergebnisses ist `None`. `library_key`: `@ext` abschneiden, dann `group:artifact` plus `:classifier`, falls ein vierter Teil existiert.

- [ ] **Step 4: Tests grün**

Run: `cargo test -p hengst-core`
Expected: PASS, alte Tests unverändert grün.

- [ ] **Step 5: Lint, Format, Commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace --all-features`
```bash
git add crates/hengst-core
git commit -m "feat(core): merge loader version profiles onto vanilla versions

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Loader-Libraries auflösen und Hashes beschaffen

**Files:**
- Modify: `crates/hengst-core/src/libraries.rs` (`resolve`)
- Modify: `crates/hengst-core/src/download.rs` (neue Funktion)

**Interfaces:**
- Consumes: `Library.sha1`, `Library.checksums`, `Library.clientreq` (Task 1), `Error::NoChecksum`
- Produces:
  - `libraries::resolve` (Signatur unverändert) mit neuen Regeln:
    1. `clientreq == Some(false)` → übersprungen.
    2. `natives`-Map ohne `downloads` → nur die Native-Jar `maven_path("<name>:<classifier>")` an `url` (Standard `LIBRARIES_URL`), keine Haupt-Jar, kein Classpath-Eintrag.
    3. Maven-Fallback ohne `downloads` → `sha1 = lib.sha1.or(checksums[0])`.
    4. `downloads.artifact` mit leerer `url` → Classpath-Eintrag ja, kein `Download`.
  - `download::fill_missing_sha1(dl: &Downloader, items: &mut [Download]) -> Result<()>`: für jedes Item mit `sha1 == None`: liegt `<path>.sha1` schon da, daraus lesen; sonst `GET <url>.sha1`, erstes Token nehmen, mit `paths::validate_sha1` prüfen, als `<path>.sha1` speichern. HTTP 404 → `Error::NoChecksum { url }`. Abfragen mit derselben Parallelität wie `fetch_all`.

- [ ] **Step 1: Failing Tests schreiben**

`libraries.rs`:
```rust
#[test]
fn resolves_legacy_fabric_maven_natives_without_main_jar() {
    // echte Legacy-Fabric-Einträge
    let list = libs(json!([
        {"name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.4+legacyfabric.17", "url": "https://maven.legacyfabric.net/",
         "extract": {"exclude": ["META-INF/"]},
         "natives": {"linux": "natives-linux", "osx": "natives-osx", "windows": "natives-windows"}},
        {"name": "net.fabricmc:fabric-loader:0.20.1", "url": "https://maven.fabricmc.net/"},
        {"name": "org.ow2.asm:asm:9.10.1", "url": "https://maven.fabricmc.net/", "sha1": "ada2141c0cc52ee8f5c48cd5fa4ce0e794f22236"}
    ]));
    let r = resolve(&list, &Env::windows_x64(), &Paths::new("C:/h")).unwrap();
    let rel = "org/lwjgl/lwjgl/lwjgl-platform/2.9.4+legacyfabric.17/lwjgl-platform-2.9.4+legacyfabric.17-natives-windows.jar";
    assert_eq!(r.natives.len(), 1);
    assert_eq!(r.natives[0].path, Paths::new("C:/h").libraries().join(rel));
    assert!(r.downloads.iter().any(|d| d.url == format!("https://maven.legacyfabric.net/{rel}")));
    assert!(!r.classpath.iter().any(|p| p.to_string_lossy().contains("lwjgl-platform")));
    assert_eq!(r.classpath.len(), 2);
    let asm = r.downloads.iter().find(|d| d.url.ends_with("asm-9.10.1.jar")).unwrap();
    assert_eq!(asm.sha1.as_deref(), Some("ada2141c0cc52ee8f5c48cd5fa4ce0e794f22236"));
}

#[test]
fn legacy_forge_checksums_clientreq_and_installer_provided_jars() {
    let list = libs(json!([
        {"name": "jline:jline:2.13", "url": "https://maven.minecraftforge.net/",
         "checksums": ["2d9530d0a25daffaffda7c35037b046b627bb171"], "serverreq": true, "clientreq": false},
        {"name": "org.scala-lang:scala-xml_2.11:1.0.2", "url": "https://maven.minecraftforge.net/",
         "checksums": ["7a80ec00aec122fba7cd4e0d4cdd87ff7e4cb6d0", "62736b01689d56b6d09a0164b7ef9da2b0b9633d"]},
        {"name": "net.minecraftforge:forge:1.16.5-36.2.34", "downloads": {"artifact": {
         "path": "net/minecraftforge/forge/1.16.5-36.2.34/forge-1.16.5-36.2.34.jar", "url": "",
         "sha1": "b05f056824252928b59c746f8a28fe2cf36db6c0", "size": 212608}}}
    ]));
    let r = resolve(&list, &Env::windows_x64(), &Paths::new("C:/h")).unwrap();
    assert_eq!(r.downloads.len(), 1);
    assert_eq!(r.downloads[0].sha1.as_deref(), Some("7a80ec00aec122fba7cd4e0d4cdd87ff7e4cb6d0"));
    assert_eq!(r.classpath.len(), 2);
    assert!(r.classpath[1].ends_with("forge-1.16.5-36.2.34.jar"));
}

#[test]
fn rejects_maven_coordinates_that_escape() {
    let list = libs(json!([{"name": "..:evil:1", "url": "https://x/"}]));
    assert!(matches!(resolve(&list, &Env::windows_x64(), &Paths::new("C:/h")), Err(crate::Error::UnsafePath(_))));
}
```

`download.rs`:
```rust
#[tokio::test]
async fn fills_sha1_from_sidecar_and_caches_it() {
    // Mock: GET /lib.jar.sha1 -> "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d  lib.jar\n", .expect(1)
    let mut items = vec![item(&server, dir.path(), "lib.jar", None)];
    fill_missing_sha1(&dl, &mut items).await.unwrap();
    assert_eq!(items[0].sha1.as_deref(), Some("aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d"));
    // zweiter Lauf: aus <path>.sha1, kein weiterer Request (expect(1) prüft das beim Drop)
    let mut again = vec![item(&server, dir.path(), "lib.jar", None)];
    fill_missing_sha1(&dl, &mut again).await.unwrap();
    assert_eq!(again[0].sha1, items[0].sha1);
}

#[tokio::test]
async fn missing_sidecar_is_an_error() {
    // Mock: 404 für alles
    let err = fill_missing_sha1(&dl, &mut vec![item(&server, dir.path(), "x.jar", None)]).await.unwrap_err();
    assert!(matches!(err, Error::NoChecksum { url } if url.ends_with("/x.jar")));
}

#[tokio::test]
async fn rejects_garbage_sidecar() {
    // Mock: "<html>302 Found</html>"
    assert!(matches!(fill_missing_sha1(&dl, &mut items).await, Err(Error::UnsafePath(_))));
}

#[tokio::test]
async fn items_with_sha1_are_left_alone() {
    // Mock ohne Routen; Item mit Some(sha1) -> Ok, sha1 unverändert
}
```

- [ ] **Step 2: Tests scheitern**

Run: `cargo test -p hengst-core libraries download`
Expected: FAIL

- [ ] **Step 3: Implementieren**

Regeln 1 bis 4 in `resolve` (bestehende Tests bleiben unverändert grün, insbesondere `fabric.sha1 == None` im alten Test). `fill_missing_sha1` mit `futures::stream::iter(..).buffer_unordered(16)` über die Indizes der Items ohne Hash.

- [ ] **Step 4: Tests grün**

Run: `cargo test -p hengst-core`
Expected: PASS

- [ ] **Step 5: Lint, Format, Commit**

```bash
git commit -am "feat(core): resolve loader libraries and fetch maven sha1 sidecars

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Loader-Arten, Versionsvergleich, Auswahl

**Files:**
- Modify: `crates/hengst-core/src/loaders/mod.rs`

**Interfaces:**
- Produces:
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum LoaderKind { Fabric, Quilt, LegacyFabric, NeoForge, Forge }` mit `slug(self) -> &'static str` (`fabric`, `quilt`, `legacy-fabric`, `neoforge`, `forge`), `impl Display` (= slug), `impl FromStr` (slug, Groß/Klein egal; sonst `Err(String)` mit `unknown loader '<x>' (expected fabric, quilt, legacy-fabric, neoforge or forge)`)
  - `#[derive(Clone, Debug, PartialEq, Eq)] pub struct LoaderVersion { pub id: String, pub maven: String, pub stable: bool }` (`id` = Kurzform zum Anzeigen und Eintippen, `maven` = volle Maven-Version)
  - `#[derive(Clone, Debug)] pub struct LoaderRequest { pub kind: LoaderKind, pub version: Option<String> }`
  - `#[derive(Clone, Debug)] pub struct LoaderEndpoints { pub fabric: String, pub quilt: String, pub legacy_fabric: String, pub forge_files: String, pub forge_maven: String, pub neoforge_maven: String }` mit `Default` = `https://meta.fabricmc.net/v2`, `https://meta.quiltmc.org/v3`, `https://meta.legacyfabric.net/v2`, `https://files.minecraftforge.net/net/minecraftforge/forge`, `https://maven.minecraftforge.net`, `https://maven.neoforged.net`
  - `pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering`
  - `pub fn sort_newest_first(v: &mut [LoaderVersion])`
  - `pub fn pick_version<'a>(versions: &'a [LoaderVersion], requested: Option<&str>, kind: LoaderKind, game: &str) -> Result<&'a LoaderVersion>`

`compare_versions`: `+...` (Build-Metadaten) abschneiden; am ersten `-` in Basis und Vorabteil trennen; Basis komponentenweise numerisch (fehlende Komponente = 0, nicht numerisch = als Text); gleiche Basis: ohne Vorabteil ist größer; zwei Vorabteile punktweise, numerisch wo beide Zahlen sind.

`pick_version`: leere Liste → `NoLoaderVersions`; `requested` passt auf `id` oder `maven` → diese; sonst `UnknownLoaderVersion`; ohne `requested` die erste stabile, sonst die erste.

- [ ] **Step 1: Failing Tests schreiben**

```rust
#[test]
fn compares_real_loader_versions() {
    use std::cmp::Ordering::*;
    assert_eq!(compare_versions("0.31.0-beta.6", "0.30.2"), Greater);
    assert_eq!(compare_versions("0.30.2", "0.30.2-beta.1"), Greater);
    assert_eq!(compare_versions("0.20.0-beta.10", "0.20.0-beta.9"), Greater);
    assert_eq!(compare_versions("26.3.0.115", "26.3.0.71-beta"), Greater);
    assert_eq!(compare_versions("10.13.4.1614", "10.13.4.1566"), Greater);
    assert_eq!(compare_versions("0.4.8+build.155", "0.4.8"), Equal);
    assert_eq!(compare_versions("21.1.267", "21.1.267"), Equal);
}

#[test]
fn parses_and_prints_loader_kinds() {
    assert_eq!("legacy-fabric".parse::<LoaderKind>().unwrap(), LoaderKind::LegacyFabric);
    assert_eq!("NeoForge".parse::<LoaderKind>().unwrap(), LoaderKind::NeoForge);
    assert_eq!(LoaderKind::Forge.to_string(), "forge");
    assert!("ornithe".parse::<LoaderKind>().unwrap_err().contains("legacy-fabric"));
}

fn v(id: &str, maven: &str, stable: bool) -> LoaderVersion { /* ... */ }

#[test]
fn picks_latest_stable_then_falls_back() {
    let list = [v("0.31.0-beta.6", "0.31.0-beta.6", false), v("0.30.2", "0.30.2", true)];
    assert_eq!(pick_version(&list, None, LoaderKind::Quilt, "1.21.11").unwrap().id, "0.30.2");
    let betas = [v("26.1.1.1-beta", "26.1.1.1-beta", false)];
    assert_eq!(pick_version(&betas, None, LoaderKind::NeoForge, "26.1.1").unwrap().id, "26.1.1.1-beta");
}

#[test]
fn explicit_version_matches_short_or_full_form() {
    let list = [v("10.13.4.1614", "1.7.10-10.13.4.1614-1.7.10", true)];
    for req in ["10.13.4.1614", "1.7.10-10.13.4.1614-1.7.10"] {
        assert_eq!(pick_version(&list, Some(req), LoaderKind::Forge, "1.7.10").unwrap().maven, "1.7.10-10.13.4.1614-1.7.10");
    }
    let err = pick_version(&list, Some("0.0.1"), LoaderKind::Forge, "1.7.10").unwrap_err();
    assert_eq!(err.to_string(), "forge 0.0.1 is not available for Minecraft 1.7.10 (see `hengst loaders forge 1.7.10`)");
    assert!(matches!(pick_version(&[], None, LoaderKind::Fabric, "1.8.9"), Err(Error::NoLoaderVersions { .. })));
}
```

- [ ] **Step 2: Tests scheitern** — Run: `cargo test -p hengst-core loaders::` — Expected: FAIL
- [ ] **Step 3: Implementieren** wie in Interfaces beschrieben.
- [ ] **Step 4: Tests grün** — Run: `cargo test -p hengst-core` — Expected: PASS
- [ ] **Step 5: Lint, Format, Commit** — `feat(core): loader kinds, version ordering and default selection`

---

### Task 4: Fabric, Quilt und Legacy Fabric Meta

**Files:**
- Create: `crates/hengst-core/src/loaders/fabric.rs`
- Create: `crates/hengst-core/tests/fixtures/loaders/{fabric-loaders-1.21.11.json, fabric-profile-1.21.11-0.17.3.json, quilt-loaders-1.21.11.json, quilt-profile-1.21.11-0.29.2.json, legacy-fabric-loaders-1.8.9.json, legacy-fabric-profile-1.8.9-0.20.1.json}`
- Modify: `crates/hengst-core/src/loaders/mod.rs` (`pub mod fabric;`)

**Interfaces:**
- Consumes: `LoaderKind`, `LoaderVersion`, `LoaderEndpoints`, `sort_newest_first` (Task 3), `download::get_bytes`, `Paths::loader_dir` (Task 1)
- Produces:
  - `fabric::meta_base(kind: LoaderKind, ep: &LoaderEndpoints) -> &str` (nur für die drei Fabric-artigen; sonst `unreachable!`)
  - `async fabric::list_versions(client: &reqwest::Client, ep: &LoaderEndpoints, kind: LoaderKind, game: &str) -> Result<Vec<LoaderVersion>>`: `GET {base}/versions/loader/{game}` (Leerzeichen in `game` als `%20`), `loader.version` → `id` und `maven`, `stable` = `loader.stable` (Fabric, Legacy Fabric) bzw. "kein `-`" (Quilt), dann `sort_newest_first`. HTTP 400/404 oder leere Liste → `NoLoaderVersions`.
  - `async fabric::fetch_profile(client: &reqwest::Client, paths: &Paths, ep: &LoaderEndpoints, kind: LoaderKind, game: &str, version: &str) -> Result<VersionJson>`: `GET {base}/versions/loader/{game}/{version}/profile/json`, `game`, `version` und die `id` der Antwort mit `validate_id` prüfen, Bytes unter `paths.loader_dir(kind.slug()).join("<game>-<version>.json")` ablegen. Netzfehler (`Http` oder `Status` ≥ 500) und vorhandene Ablage → aus der Ablage lesen. HTTP 400/404 → `UnknownLoaderVersion`.

- [ ] **Step 1: Fixtures aus echten Antworten erzeugen**

In einem leeren Ordner außerhalb des Repos (Downloads sind untrusted), dann gekürzt ins Repo schreiben:
```bash
F=crates/hengst-core/tests/fixtures/loaders; mkdir -p $F
curl -s https://meta.fabricmc.net/v2/versions/loader/1.21.11 | python -I -c "import json,sys;d=json.load(sys.stdin)[:3];[x['launcherMeta']['libraries'].update(common=x['launcherMeta']['libraries']['common'][:1]) for x in d];print(json.dumps(d,indent=1))" > $F/fabric-loaders-1.21.11.json
curl -s https://meta.fabricmc.net/v2/versions/loader/1.21.11/0.17.3/profile/json > $F/fabric-profile-1.21.11-0.17.3.json
curl -s https://meta.quiltmc.org/v3/versions/loader/1.21.11 | python -I -c "import json,sys;keep={'0.20.0-beta.9','0.24.0','0.31.0-beta.6','0.30.2-beta.1','0.30.2','0.29.2'};d=[x for x in json.load(sys.stdin) if x['loader']['version'] in keep];[x.update(launcherMeta={}) for x in d];print(json.dumps(d,indent=1))" > $F/quilt-loaders-1.21.11.json
curl -s https://meta.quiltmc.org/v3/versions/loader/1.21.11/0.29.2/profile/json > $F/quilt-profile-1.21.11-0.29.2.json
curl -s https://meta.legacyfabric.net/v2/versions/loader/1.8.9 | python -I -c "import json,sys;d=json.load(sys.stdin)[:2];[x.update(launcherMeta={}) for x in d];print(json.dumps(d,indent=1))" > $F/legacy-fabric-loaders-1.8.9.json
curl -s https://meta.legacyfabric.net/v2/versions/loader/1.8.9/0.20.1/profile/json > $F/legacy-fabric-profile-1.8.9-0.20.1.json
```
Erwartet (Stand 20.11.2026): Fabric-Liste beginnt mit `0.20.1` (stable), `0.20.0`, `0.19.5`; Quilt-Datei in Originalreihenfolge `0.20.0-beta.9, 0.24.0, 0.31.0-beta.6, 0.30.2-beta.1, 0.30.2, 0.29.2`; Legacy-Fabric-Liste `0.20.1` (stable), `0.20.0`. Die Tests lesen die Fixtures per `include_str!("../../tests/fixtures/loaders/<datei>")`.

- [ ] **Step 2: Failing Tests schreiben** (`fabric.rs`, Mock-Server liefert die Fixtures)

```rust
#[tokio::test]
async fn fabric_list_is_newest_first_with_stable_flag() {
    let l = list_versions(&client, &ep(&server), LoaderKind::Fabric, "1.21.11").await.unwrap();
    assert_eq!(l.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(), ["0.20.1", "0.20.0", "0.19.5"]);
    assert_eq!(l.iter().map(|v| v.stable).collect::<Vec<_>>(), [true, false, false]);
}

#[tokio::test]
async fn quilt_list_is_sorted_and_prereleases_are_unstable() {
    let l = list_versions(&client, &ep(&server), LoaderKind::Quilt, "1.21.11").await.unwrap();
    assert_eq!(l.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(),
        ["0.31.0-beta.6", "0.30.2", "0.30.2-beta.1", "0.29.2", "0.24.0", "0.20.0-beta.9"]);
    assert_eq!(pick_version(&l, None, LoaderKind::Quilt, "1.21.11").unwrap().id, "0.30.2");
}

#[tokio::test]
async fn unknown_game_version_means_no_loader_versions() {
    // Mock: /v2/versions/loader/9.9.9 -> 400 "[]"; /v3/... -> 404; legacy 1.8.9-Pfad für fabric -> 200 "[]"
    for kind in [LoaderKind::Fabric, LoaderKind::Quilt] {
        assert!(matches!(list_versions(&client, &ep(&server), kind, "9.9.9").await,
            Err(Error::NoLoaderVersions { .. })));
    }
}

#[tokio::test]
async fn fetches_and_caches_profiles_for_all_three() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let f = fetch_profile(&client, &paths, &ep(&server), LoaderKind::Fabric, "1.21.11", "0.17.3").await.unwrap();
    assert_eq!(f.id, "fabric-loader-0.17.3-1.21.11");
    assert_eq!(f.inherits_from.as_deref(), Some("1.21.11"));
    assert_eq!(f.libraries.len(), 8);
    assert_eq!(f.libraries[0].sha1.as_deref(), Some("c29635c8a7afa03d74b33c1884df8abb2b3f3dcc"));
    assert!(f.libraries.iter().find(|l| l.name == "net.fabricmc:fabric-loader:0.17.3").unwrap().sha1.is_none());
    let q = fetch_profile(&client, &paths, &ep(&server), LoaderKind::Quilt, "1.21.11", "0.29.2").await.unwrap();
    assert_eq!(q.main_class, "org.quiltmc.loader.impl.launch.knot.KnotClient");
    assert!(q.libraries.iter().all(|l| l.sha1.is_none()));
    let lf = fetch_profile(&client, &paths, &ep(&server), LoaderKind::LegacyFabric, "1.8.9", "0.20.1").await.unwrap();
    assert!(lf.arguments.is_none());
    assert!(lf.libraries.iter().any(|l| l.natives.is_some()));
    assert!(paths.loader_dir("legacy-fabric").join("1.8.9-0.20.1.json").is_file());
}

#[tokio::test]
async fn falls_back_to_cached_profile_when_offline() {
    // erster Abruf 200 (up_to_n_times(1)), danach 503 -> zweiter Abruf liefert dieselbe id
}

#[tokio::test]
async fn unknown_loader_version_for_profile() {
    // Mock: 400 "no loader version found for 0.0.1"
    assert!(matches!(fetch_profile(/* ..., "0.0.1" */).await, Err(Error::UnknownLoaderVersion { .. })));
}
```

- [ ] **Step 3: Tests scheitern** — Run: `cargo test -p hengst-core fabric` — Expected: FAIL
- [ ] **Step 4: Implementieren** nach Interfaces.
- [ ] **Step 5: Tests grün** — Run: `cargo test -p hengst-core` — Expected: PASS
- [ ] **Step 6: Lint, Format, Commit** — `feat(core): Fabric, Quilt and Legacy Fabric meta clients`

---

### Task 5: Fabric-Familie in `prepare`

**Files:**
- Modify: `crates/hengst-core/src/launch.rs`, `crates/hengst-core/src/loaders/mod.rs`
- Modify: `crates/hengst-core/tests/launch_e2e.rs` (Struct-Literale um `loader: None` bzw. `..Endpoints::default()` ergänzen, sonst unverändert)
- Create: `crates/hengst-core/tests/loader_e2e.rs`

**Interfaces:**
- Consumes: Tasks 1 bis 4
- Produces:
  - `Endpoints.loaders: LoaderEndpoints` (`Default` setzt `LoaderEndpoints::default()`)
  - `LaunchOptions.loader: Option<LoaderRequest>`
  - `Prepared.loader: Option<ResolvedLoader>` mit `#[derive(Clone, Debug)] pub struct ResolvedLoader { pub kind: LoaderKind, pub version: LoaderVersion }` (in `loaders/mod.rs`)
  - `loaders::LoaderSetup { pub resolved: ResolvedLoader, pub child: VersionJson, pub extra_downloads: Vec<Download>, pub forge: Option<forge::ForgeInstall> }` (das Feld `forge` kommt in Task 11 dazu; bis dahin ohne es)
  - `async loaders::prepare_loader(dl: &Downloader, paths: &Paths, ep: &LoaderEndpoints, req: &LoaderRequest, game: &str, env: &Env) -> Result<LoaderSetup>`: Liste holen, `pick_version`, Fabric-artig → `fetch_profile`, `extra_downloads` leer. Liegt `req.version` vor und scheitert die Liste mit Netzfehler, wird ohne Prüfung direkt das Profil geholt (`stable: false`).
  - Ablauf in `prepare` (Reihenfolge ist Teil der Entscheidung): Manifest → Vanilla-JSON → **Loader auflösen** (Fehler hier, bevor eine Library geladen wird) → `merge_version` → Libraries + `extra_downloads` + Client-Jar + Log-Config → `fill_missing_sha1` → `fetch_all` → Client-Jar als `paths.client_jar(&merged.id)` verlinken/kopieren, wenn die IDs verschieden sind → Java für `component_for(&vanilla)` → (Task 11: Forge-Installation) → Assets, Natives (`.natives/<merged id>`), Argumente aus `merged`.
  - Standard-Instanz: `opts.instance` sonst `<game id>-<loader slug>` mit Loader, sonst wie bisher die Versions-ID.

- [ ] **Step 1: Failing E2E-Tests schreiben** (`tests/loader_e2e.rs`, `#![cfg(windows)]`, Mock-Aufbau wie `launch_e2e.rs`; das Fabric-Profil wird aus dem Fixture geladen, die Library-`url`s auf `{mock}/maven/` umgeschrieben, Jar-Inhalte sind kurze Bytes, Hashes passend gesetzt bzw. als `.sha1`-Sidecar gemountet)

```rust
#[tokio::test]
async fn prepares_fabric_on_a_modern_version() {
    // Vanilla "9.9" wie in launch_e2e.rs; Fabric-Liste für 9.9 mit 0.17.3 stable; Profil id "fabric-loader-0.17.3-9.9"
    let p = prepare(&paths, &opts_with(LoaderKind::Fabric, None), &ep).await.unwrap();
    assert_eq!(p.version_id, "fabric-loader-0.17.3-9.9");
    assert_eq!(p.game_dir, paths.instance("9.9-fabric"));
    assert_eq!(p.loader.as_ref().unwrap().version.id, "0.17.3");
    let main = p.args.iter().position(|a| a == "net.fabricmc.loader.impl.launch.knot.KnotClient").unwrap();
    let emu = p.args.iter().position(|a| a == "-DFabricMcEmu= net.minecraft.client.main.Main ").unwrap();
    assert!(emu < main);
    let cp = &p.args[p.args.iter().position(|a| a == "-cp").unwrap() + 1];
    assert!(cp.contains("fabric-loader-0.17.3.jar"));
    assert!(cp.ends_with(&paths.client_jar("fabric-loader-0.17.3-9.9").to_string_lossy().into_owned()));
    assert!(paths.client_jar("fabric-loader-0.17.3-9.9").is_file());
    assert_eq!(p.java, paths.java_runtime("java-runtime-delta").join("bin").join("java.exe"));
}

#[tokio::test]
async fn prepares_legacy_fabric_on_a_legacy_version() {
    // Vanilla "8.8": minecraftArguments, org.lwjgl.lwjgl:lwjgl:2.9.4-nightly-20150209 + lwjgl-platform mit natives-windows-Classifier;
    // Legacy-Fabric-Profil aus Fixture (inheritsFrom auf "8.8" gesetzt), lwjgl-platform-...-natives-windows.jar enthält "lwjgl64.dll"
    let p = prepare(&paths, &opts_with(LoaderKind::LegacyFabric, Some("0.20.1")), &ep).await.unwrap();
    let cp = &p.args[p.args.iter().position(|a| a == "-cp").unwrap() + 1];
    assert!(cp.contains("lwjgl-2.9.4+legacyfabric.17.jar"));
    assert!(!cp.contains("2.9.4-nightly-20150209"));
    assert!(p.game_dir.join(".natives").join(&p.version_id).join("lwjgl64.dll").is_file());
    assert!(p.args.iter().any(|a| a == "-Djava.library.path=".to_string() + &p.game_dir.join(".natives").join(&p.version_id).to_string_lossy()));
}

#[tokio::test]
async fn loader_errors_come_before_vanilla_downloads() {
    // Vanilla-JSON gemountet, aber KEINE Client-Jar/Library-Routen; Fabric-Liste für 9.9 -> "[]"
    let err = prepare(&paths, &opts_with(LoaderKind::Fabric, None), &ep).await.unwrap_err();
    assert!(matches!(err, Error::NoLoaderVersions { .. }), "{err}");
    assert!(!paths.libraries().exists());
}

#[tokio::test]
async fn unverifiable_loader_library_aborts() {
    // fabric-loader ohne sha1 und ohne Sidecar (404)
    assert!(matches!(prepare(/* ... */).await, Err(Error::NoChecksum { .. })));
}
```

- [ ] **Step 2: Tests scheitern** — Run: `cargo test -p hengst-core --test loader_e2e` — Expected: FAIL (Felder fehlen)
- [ ] **Step 3: Implementieren** (Ablauf wie in Interfaces; `launch_e2e.rs` nur um die neuen Felder ergänzen).
- [ ] **Step 4: Tests grün** — Run: `cargo test --workspace --all-features` — Expected: PASS
- [ ] **Step 5: Lint, Format, Commit** — `feat(core): prepare Fabric, Quilt and Legacy Fabric versions`

---

### Task 6: CLI und CI-Matrix für die Fabric-Familie

**Files:**
- Modify: `apps/cli/src/main.rs`, `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: `LoaderKind` (`FromStr`), `LoaderRequest`, `Prepared.loader`, `fabric::list_versions` (Task 8 ersetzt den Aufruf durch `loaders::list_versions`, das alle fünf abdeckt)
- Produces:
  - `hengst launch <VERSION> [--loader <LOADER>] [--loader-version <V>]` zusätzlich zu den bestehenden Optionen (inkl. `--account` aus Phase 2). `--loader-version` hat `requires = "loader"`. `LOADER` wird über `value_parser` mit `LoaderKind::from_str` gelesen.
  - `hengst loaders <LOADER> <VERSION> [--limit <N>]` (Standard 20): eine Zeile pro Version, neueste zuerst, Format `{id:<24} {stable|unstable}` und am Standardeintrag ` (default)`.
  - Bei `prepare` mit Loader ohne `--loader-version` und `!version.stable`: stderr `note: no stable <loader> release for Minecraft <game>, using <id>`.
  - CI: neuer Job `loader-matrix` (needs `test`, `windows-latest`, `fail-fast: false`), Einträge `fabric 1.21.11`, `fabric latest`, `quilt 1.21.11`, `legacy-fabric 1.8.9`, `legacy-fabric 1.12.2`, Befehl `cargo run --release -p hengst-cli -- --home "${{ runner.temp }}/hengst" launch <version> --loader <loader> --dry-run`.

- [ ] **Step 1: Failing Tests schreiben** (Unit-Tests in `main.rs`, `Cli::try_parse_from`)

```rust
#[test]
fn parses_loader_options() {
    let cli = Cli::try_parse_from(["hengst", "launch", "1.20.1", "--loader", "forge", "--loader-version", "47.4.28"]).unwrap();
    // Cmd::Launch { loader: Some(LoaderKind::Forge), loader_version: Some("47.4.28"), .. }
}

#[test]
fn loader_version_needs_loader() {
    assert!(Cli::try_parse_from(["hengst", "launch", "1.20.1", "--loader-version", "1"]).is_err());
}

#[test]
fn rejects_unknown_loader() {
    let err = Cli::try_parse_from(["hengst", "launch", "1.8.9", "--loader", "ornithe"]).unwrap_err().to_string();
    assert!(err.contains("legacy-fabric"));
}

#[test]
fn parses_loaders_command() {
    assert!(Cli::try_parse_from(["hengst", "loaders", "quilt", "1.21.11", "--limit", "5"]).is_ok());
}
```

- [ ] **Step 2: Tests scheitern** — Run: `cargo test -p hengst-cli` — Expected: FAIL
- [ ] **Step 3: Implementieren** nach Interfaces.
- [ ] **Step 4: Tests grün und Handprobe**

Run: `cargo test --workspace --all-features`
Run: `cargo run -p hengst-cli -- --home "$TEMP/hengst-dev" loaders quilt 1.21.11 --limit 3`
Expected: `0.31.0-beta.6 unstable`, `0.30.2 stable (default)` oder neuere Werte in derselben Form.
Run: `cargo run -p hengst-cli -- --home "$TEMP/hengst-dev" launch 1.21.11 --loader fabric --dry-run`
Expected: Java-Pfad, Argumente mit `net.fabricmc.loader.impl.launch.knot.KnotClient`, Instanz `1.21.11-fabric`.

- [ ] **Step 5: Lint, Format, Commit, CI**

```bash
git commit -am "feat(cli): --loader, --loader-version and loaders command

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
git push -u origin phase-3-loader
gh run watch
```
Expected: CI grün inkl. `loader-matrix`.

---

### Task 7: Echte Starts der Fabric-Familie (Abnahme Teil 1)

**Files:**
- Create: `crates/hengst-core/tests/loader_live.rs`

**Interfaces:**
- Consumes: `prepare`, `Prepared`, `LoaderRequest`, `Account::offline` (wenn Phase 2 `Account::offline` hinter ein Kern-Feature gelegt hat: Datei mit `#![cfg(all(windows, feature = "dev-offline"))]` und Aufruf mit `--features hengst-core/dev-offline`)
- Produces (nur in der Testdatei):
  - `fn mute_sound(game_dir: &Path) -> std::io::Result<()>`: `options.txt` anlegen oder Zeile `soundCategory_master:…` ersetzen bzw. anhängen, sodass genau `soundCategory_master:0.0` drinsteht; andere Zeilen bleiben.
  - `async fn live_launch(version: &str, kind: LoaderKind, memory_mb: u32, banner: Option<&str>)`: globaler `static LOCK: std::sync::Mutex<()>`, Datenordner `HENGST_LIVE_HOME` oder `%TEMP%\hengst-live` (bleibt zwischen Läufen, spart Downloads), `prepare` mit Live-Endpunkten, `mute_sound`, Prozess selbst starten (nicht über `run`) mit `stdout`/`stderr` gepipet in `<game_dir>/logs/hengst-live.log`; bestanden, wenn binnen 240 s eine Zeile auf `Backend library: LWJGL version|LWJGL Version: ` passt, `banner` (falls gesetzt) vorkam, der Prozess 15 s danach noch läuft und `<game_dir>/crash-reports` keine Datei enthält. Danach `kill` und `wait`, immer (auch bei Fehlschlag).

- [ ] **Step 1: Ignorierte Tests schreiben**

```rust
#[tokio::test] #[ignore] async fn fabric_1_21_11()       { live_launch("1.21.11", LoaderKind::Fabric, 2048, Some("with Fabric Loader")).await }
#[tokio::test] #[ignore] async fn fabric_latest()        { live_launch("latest", LoaderKind::Fabric, 2048, Some("with Fabric Loader")).await }
#[tokio::test] #[ignore] async fn quilt_1_21_11()        { live_launch("1.21.11", LoaderKind::Quilt, 2048, Some("with Quilt Loader")).await }
#[tokio::test] #[ignore] async fn legacy_fabric_1_8_9()  { live_launch("1.8.9", LoaderKind::LegacyFabric, 2048, Some("with Fabric Loader")).await }
#[tokio::test] #[ignore] async fn legacy_fabric_1_12_2() { live_launch("1.12.2", LoaderKind::LegacyFabric, 2048, Some("with Fabric Loader")).await }
#[test] fn mute_sound_rewrites_only_master_volume() { /* tempdir, options.txt mit "soundCategory_master:1.0\nfov:0.5\n" -> "soundCategory_master:0.0" und "fov:0.5" */ }
```

- [ ] **Step 2: Standardlauf bleibt grün** — Run: `cargo test --workspace --all-features` — Expected: PASS, Live-Tests als `ignored` gezählt.

- [ ] **Step 3: Live-Läufe nacheinander** (kein anderer Build, kein anderes Spiel offen)

Run: `cargo test -p hengst-core --test loader_live -- --ignored --test-threads=1`
Expected: 5 passed. Bei Fehlschlag: superpowers:systematic-debugging, Regressionstest im passenden Modul (gegen Mock), Fix, eigener Commit.

- [ ] **Step 4: Sichtprüfung mit Agentfenster**

`cargo build --release -p hengst-cli --features dev-offline`, dann je einmal `target/release/hengst launch 1.21.11 --loader fabric` und `... 1.8.9 --loader legacy-fabric` (nacheinander, vorher `options.txt` im Instanzordner stummschalten). Mit Agentfenster `list_windows` auf das Spielfenster warten und einen Screenshot des Titelbilds machen. Vanilla-Fabric zeigt ohne Mods keinen Loader-Hinweis im Titelbild, geprüft wird daher: Titelbild erreicht, kein Absturzdialog. Fenster mit `close_window` schließen.

- [ ] **Step 5: Lint, Format, Commit** — `test(core): live launch checks for the Fabric family`

---

### Task 8: Forge- und NeoForge-Versionslisten

**Files:**
- Create: `crates/hengst-core/src/loaders/forge/mod.rs`
- Create: `crates/hengst-core/tests/fixtures/loaders/{forge-maven-metadata.json, forge-promotions.json, neoforge-versions.json, neoforge-forge-versions.json}`
- Modify: `crates/hengst-core/src/loaders/mod.rs` (`pub mod forge;`, neue Weiche `list_versions`)

**Interfaces:**
- Consumes: `LoaderVersion`, `sort_newest_first`, `compare_versions`, `LoaderEndpoints`
- Produces:
  - `forge::forge_supports(game: &str) -> bool`: `false` genau für IDs der Form `1.X[.Y]` mit `(1, X, Y) < (1, 7, 10)`; alles andere `true`.
  - `forge::neoforge_game_version(version: &str) -> Option<String>`
  - `async forge::list_forge(client: &reqwest::Client, ep: &LoaderEndpoints, game: &str) -> Result<Vec<LoaderVersion>>`: `!forge_supports(game)` → `LoaderUnsupported { loader: "forge", game, reason: "Forge support starts at 1.7.10" }` ohne HTTP; sonst `GET {forge_files}/maven-metadata.json` und `{forge_files}/promotions_slim.json`; `maven` = Listeneintrag, `id` = `maven` ohne Präfix `<game>-` und ohne Suffix `-<game>`; `stable` = `id == promos["<game>-recommended"]`; sortiert.
  - `async forge::list_neoforge(client: &reqwest::Client, ep: &LoaderEndpoints, game: &str) -> Result<Vec<LoaderVersion>>`: `game == "1.20.1"` → `GET {neoforge_maven}/api/maven/versions/releases/net/neoforged/forge`, nur Einträge mit Präfix `1.20.1-`, `id` ohne Präfix; sonst `.../net/neoforged/neoforge`, Einträge mit `neoforge_game_version(v) == Some(game)`, `id == maven`; `stable` = kein `-`; sortiert.
  - `async loaders::list_versions(client: &reqwest::Client, ep: &LoaderEndpoints, kind: LoaderKind, game: &str) -> Result<Vec<LoaderVersion>>`: Weiche auf `fabric::list_versions`, `forge::list_forge`, `forge::list_neoforge`; leeres Ergebnis → `NoLoaderVersions`. `prepare_loader` und die CLI nutzen ab jetzt diese Weiche.

`neoforge_game_version`: `+` oder Präfix `0.` → `None`; Vorabteil abtrennen; erste Komponente ≥ 26: `A.B.C.D` → `A.B` wenn `C == 0`, sonst `A.B.C`; sonst `A.B.C` → `1.A` wenn `B == 0`, sonst `1.A.B`.

- [ ] **Step 1: Fixtures erzeugen**

```bash
curl -s https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json | python -I -c "import json,sys;d=json.load(sys.stdin);print(json.dumps({k:d[k][:2]+d[k][-2:] for k in ['1.6.4','1.7.10','1.20.1','26.3']},indent=1))" > $F/forge-maven-metadata.json
curl -s https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json | python -I -c "import json,sys;d=json.load(sys.stdin);p=d['promos'];print(json.dumps({'homepage':d['homepage'],'promos':{k:v for k,v in p.items() if k.split('-')[0] in ('1.6.4','1.7.10','1.20.1','26.3')}},indent=1))" > $F/forge-promotions.json
curl -s https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge | python -I -c "import json,sys;d=json.load(sys.stdin);keep=['0.25w14craftmine.3-beta','20.2.3-beta','21.1.266','21.1.267','26.1.0.0-alpha.1+snapshot-1','26.1.1.0-beta','26.1.1.1-beta','26.3.0.75-beta','26.3.0.114','26.3.0.115'];d['versions']=[v for v in d['versions'] if v in keep];print(json.dumps(d,indent=1))" > $F/neoforge-versions.json
curl -s https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/forge | python -I -c "import json,sys;d=json.load(sys.stdin);d['versions']=[v for v in d['versions'] if v in ('1.20.1-47.1.5','1.20.1-47.1.106','47.1.82')];print(json.dumps(d,indent=1))" > $F/neoforge-forge-versions.json
```
Erwartet: `1.7.10` endet auf `1.7.10-10.13.4.1614-1.7.10`, Promo `1.7.10-recommended` = `10.13.4.1614`; `1.20.1-recommended` = `47.4.28`, `1.20.1-latest` = `47.4.29`; `26.3-recommended` = `66.1.9`. Neuere Builds seit dem 20.11. ändern die letzten Einträge; Tests dann auf die Fixture-Werte anpassen, nicht auf Live-Werte.

- [ ] **Step 2: Failing Tests schreiben**

```rust
#[test]
fn forge_cutoff_is_1_7_10() {
    for (g, ok) in [("1.6.4", false), ("1.7.2", false), ("1.5.2", false), ("1.7.10", true), ("1.12.2", true), ("1.20.1", true), ("26.3", true)] {
        assert_eq!(forge_supports(g), ok, "{g}");
    }
}

#[test]
fn maps_neoforge_versions_to_game_versions() {
    for (v, g) in [("20.2.88", Some("1.20.2")), ("21.0.167", Some("1.21")), ("21.11.48", Some("1.21.11")),
                   ("26.3.0.115", Some("26.3")), ("26.1.1.0-beta", Some("26.1.1")),
                   ("26.1.0.0-alpha.1+snapshot-1", None), ("0.25w14craftmine.3-beta", None)] {
        assert_eq!(neoforge_game_version(v).as_deref(), g, "{v}");
    }
}

#[tokio::test]
async fn forge_list_uses_recommended_promo_and_short_ids() {
    let l = list_forge(&client, &ep(&server), "1.7.10").await.unwrap();
    assert_eq!(l[0].id, "10.13.4.1614");
    assert_eq!(l[0].maven, "1.7.10-10.13.4.1614-1.7.10");
    assert!(l[0].stable);
    let l = list_forge(&client, &ep(&server), "1.20.1").await.unwrap();
    assert_eq!(l[0].id, "47.4.29");
    assert!(!l[0].stable);
    assert_eq!(pick_version(&l, None, LoaderKind::Forge, "1.20.1").unwrap().id, "47.4.28");
}

#[tokio::test]
async fn forge_before_1_7_10_is_refused_without_network() {
    // Mock ohne Routen, .expect(0) auf alles
    let err = list_forge(&client, &ep(&server), "1.6.4").await.unwrap_err();
    assert_eq!(err.to_string(), "forge for Minecraft 1.6.4 is not supported (Forge support starts at 1.7.10)");
}

#[tokio::test]
async fn neoforge_lists_per_game_version() {
    let l = list_neoforge(&client, &ep(&server), "26.3").await.unwrap();
    assert_eq!(l.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(), ["26.3.0.115", "26.3.0.114", "26.3.0.75-beta"]);
    let l = list_neoforge(&client, &ep(&server), "1.21.1").await.unwrap();
    assert_eq!(l[0].id, "21.1.267");
    let l = list_neoforge(&client, &ep(&server), "1.20.1").await.unwrap();
    assert_eq!(l.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(), ["47.1.106", "47.1.5"]);
    assert_eq!(l[0].maven, "1.20.1-47.1.106");
    let l = list_neoforge(&client, &ep(&server), "26.1.1").await.unwrap();
    assert!(l.iter().all(|v| !v.stable));
    assert!(matches!(loaders::list_versions(&client, &ep(&server), LoaderKind::NeoForge, "1.16.5").await,
        Err(Error::NoLoaderVersions { .. })));
}
```

- [ ] **Step 3: Tests scheitern** — Run: `cargo test -p hengst-core forge` — Expected: FAIL
- [ ] **Step 4: Implementieren** nach Interfaces; CLI `loaders` und `prepare_loader` auf `loaders::list_versions` umstellen (für Forge/NeoForge liefert `prepare_loader` bis Task 11 `Error::Installer("not implemented yet")` nach der Versionsauswahl).
- [ ] **Step 5: Tests grün** — Run: `cargo test --workspace --all-features` — Expected: PASS
- [ ] **Step 6: Handprobe** — Run: `cargo run -p hengst-cli -- loaders forge 1.7.10 --limit 3` — Expected: `10.13.4.1614 stable (default)` oben.
- [ ] **Step 7: Lint, Format, Commit** — `feat(core): Forge and NeoForge version lists`

---

### Task 9: Installer laden und lesen (beide Formate)

**Files:**
- Create: `crates/hengst-core/src/loaders/forge/installer.rs`
- Create: `crates/hengst-core/tests/fixtures/loaders/{forge-1.8.9-install_profile.json, forge-1.12.2-2859-install_profile.json, forge-1.12.2-2859-version.json, forge-1.20.1-install_profile.json, forge-1.20.1-version.json, neoforge-26.3-install_profile.json}`
- Modify: `crates/hengst-core/src/loaders/forge/mod.rs`

**Interfaces:**
- Consumes: `Library`, `VersionJson`, `maven_path`, `join_relative`, `validate_id`, `sha1_file`, `Downloader`, `fill_missing_sha1`
- Produces (`installer.rs`):
  - `#[derive(Deserialize, Clone, Debug)] pub struct SidedData { pub client: String, pub server: String }`
  - `#[derive(Deserialize, Clone, Debug)] pub struct Processor { pub jar: String, #[serde(default)] pub classpath: Vec<String>, #[serde(default)] pub args: Vec<String>, #[serde(default)] pub outputs: BTreeMap<String, String>, pub sides: Option<Vec<String>> }`
  - `#[derive(Deserialize, Clone, Debug)] pub struct ModernProfile { pub spec: u32, pub minecraft: String, pub json: String, pub path: Option<String>, #[serde(default)] pub data: BTreeMap<String, SidedData>, #[serde(default)] pub processors: Vec<Processor>, #[serde(default)] pub libraries: Vec<Library> }`
  - `pub enum InstallerKind { Modern(ModernProfile), Legacy { path: String, file_path: String, minecraft: String } }`
  - `pub struct InstallerContents { pub kind: InstallerKind, pub version: VersionJson }`
  - `pub fn read_installer(installer: &Path) -> Result<InstallerContents>`: `install_profile.json` lesen; Schlüssel `install` → Legacy (`install.path`, `install.filePath`, `install.minecraft`, `versionInfo` als `VersionJson`); sonst modern, `version` aus dem Eintrag `json` ohne führendes `/`. Fehlende Einträge/kaputtes JSON → `Error::Installer`.
  - `pub fn extract_embedded(installer: &Path, contents: &mut InstallerContents, libraries: &Path) -> Result<()>`: modern: jeder Nicht-Ordner-Eintrag `maven/<rel>` → `join_relative(libraries, rel)`, nur schreiben, wenn die Datei fehlt. Legacy: Eintrag `file_path` → `libraries/maven_path(path)`; in `contents.version.libraries` den Eintrag mit `name == path` ersetzen durch `downloads.artifact { path: maven_path(path), url: "", sha1: sha1_file(...), size }` ohne `url`-Feld.
  - `pub fn check_game(contents: &InstallerContents, game: &str) -> Result<()>`: `minecraft` und `version.inherits_from` müssen `game` sein, sonst `ParentMismatch`.
  - `forge::installer_url(kind: LoaderKind, game: &str, maven: &str, ep: &LoaderEndpoints) -> String`: Forge `{forge_maven}/net/minecraftforge/forge/{m}/forge-{m}-installer.jar`; NeoForge 1.20.1 `{neoforge_maven}/releases/net/neoforged/forge/{m}/forge-{m}-installer.jar`; NeoForge sonst `{neoforge_maven}/releases/net/neoforged/neoforge/{m}/neoforge-{m}-installer.jar`
  - `async forge::fetch_installer(dl: &Downloader, paths: &Paths, url: &str) -> Result<PathBuf>`: Ziel = `libraries/` + URL-Pfad ab `net/` (per `join_relative`), Hash per `fill_missing_sha1`, dann `fetch_one`.

- [ ] **Step 1: Fixtures erzeugen** (Installer in einen eigenen leeren Ordner laden, nur `install_profile.json`/`version.json` herausziehen)

```bash
# Forge 1.8.9 Legacy: libraries auf forge, launchwrapper, jline (clientreq false), akka-actor (checksums) kürzen
# Forge 1.12.2-14.23.5.2859: install_profile unverändert, version.json libraries auf die ersten 3 kürzen
# Forge 1.20.1-47.4.28: install_profile unverändert (Prozessoren und data werden in Task 10 gebraucht), version.json libraries auf 4 kürzen
# NeoForge 26.3.0.115: install_profile unverändert
unzip -p forge-1.8.9-11.15.1.2318-1.8.9-installer.jar install_profile.json | python -I -c "import json,sys;d=json.load(sys.stdin);keep=('net.minecraftforge:forge','net.minecraft:launchwrapper','jline:jline','com.typesafe.akka:akka-actor_2.11');d['versionInfo']['libraries']=[l for l in d['versionInfo']['libraries'] if l['name'].rsplit(':',1)[0] in keep];print(json.dumps(d,indent=1))" > $F/forge-1.8.9-install_profile.json
# übrige analog mit unzip -p ... | python -I -c "...[:N]..."
```

- [ ] **Step 2: Failing Tests schreiben** (Tests bauen Installer-Jars im Speicher mit `zip::ZipWriter` aus den Fixtures plus kleinen Dummy-Einträgen)

```rust
#[test]
fn reads_legacy_installer_and_converts_universal_jar() {
    // Zip: install_profile.json (1.8.9-Fixture) + "forge-1.8.9-11.15.1.2318-1.8.9-universal.jar" (Bytes b"universal")
    let mut c = read_installer(&jar).unwrap();
    assert!(matches!(&c.kind, InstallerKind::Legacy { minecraft, .. } if minecraft == "1.8.9"));
    assert_eq!(c.version.main_class, "net.minecraft.launchwrapper.Launch");
    assert!(c.version.minecraft_arguments.as_ref().unwrap().contains("net.minecraftforge.fml.common.launcher.FMLTweaker"));
    extract_embedded(&jar, &mut c, &libs).unwrap();
    let rel = "net/minecraftforge/forge/1.8.9-11.15.1.2318-1.8.9/forge-1.8.9-11.15.1.2318-1.8.9.jar";
    assert_eq!(std::fs::read(libs.join(rel)).unwrap(), b"universal");
    let forge = c.version.libraries.iter().find(|l| l.name.starts_with("net.minecraftforge:forge:")).unwrap();
    let a = forge.downloads.as_ref().unwrap().artifact.as_ref().unwrap();
    assert_eq!((a.url.as_str(), a.sha1.clone()), ("", sha1_hex(b"universal")));
    check_game(&c, "1.8.9").unwrap();
    assert!(matches!(check_game(&c, "1.8.8"), Err(Error::ParentMismatch { .. })));
}

#[test]
fn reads_modern_spec0_without_processors() {
    // Zip: 1.12.2-2859 install_profile + version.json + maven/net/minecraftforge/forge/1.12.2-14.23.5.2859/forge-1.12.2-14.23.5.2859.jar
    let mut c = read_installer(&jar).unwrap();
    let InstallerKind::Modern(p) = &c.kind else { panic!() };
    assert_eq!((p.spec, p.processors.len()), (0, 0));
    assert_eq!(c.version.id, "1.12.2-forge-14.23.5.2859");
    extract_embedded(&jar, &mut c, &libs).unwrap();
    assert!(libs.join("net/minecraftforge/forge/1.12.2-14.23.5.2859/forge-1.12.2-14.23.5.2859.jar").is_file());
}

#[test]
fn reads_modern_spec1_with_processors_and_sides() {
    let c = read_installer(&jar_1_20_1).unwrap();
    let InstallerKind::Modern(p) = &c.kind else { panic!() };
    assert_eq!(p.spec, 1);
    assert_eq!(p.processors.len(), 10);
    assert_eq!(p.processors.iter().filter(|x| x.sides.as_ref().map_or(true, |s| s.iter().any(|s| s == "client"))).count(), 6);
    assert_eq!(p.data["PATCHED"].client, "[net.minecraftforge:forge:1.20.1-47.4.28:client]");
    assert_eq!(c.version.main_class, "cpw.mods.bootstraplauncher.BootstrapLauncher");
}

#[test]
fn refuses_embedded_paths_that_escape() {
    // Zip mit Eintrag "maven/../../evil.jar"
    assert!(matches!(extract_embedded(&jar, &mut c, &libs), Err(Error::UnsafePath(_))));
    assert!(!libs.parent().unwrap().join("evil.jar").exists());
}

#[test]
fn installer_urls() {
    let ep = LoaderEndpoints::default();
    assert_eq!(installer_url(LoaderKind::Forge, "1.7.10", "1.7.10-10.13.4.1614-1.7.10", &ep),
        "https://maven.minecraftforge.net/net/minecraftforge/forge/1.7.10-10.13.4.1614-1.7.10/forge-1.7.10-10.13.4.1614-1.7.10-installer.jar");
    assert_eq!(installer_url(LoaderKind::NeoForge, "1.20.1", "1.20.1-47.1.106", &ep),
        "https://maven.neoforged.net/releases/net/neoforged/forge/1.20.1-47.1.106/forge-1.20.1-47.1.106-installer.jar");
    assert_eq!(installer_url(LoaderKind::NeoForge, "26.3", "26.3.0.115", &ep),
        "https://maven.neoforged.net/releases/net/neoforged/neoforge/26.3.0.115/neoforge-26.3.0.115-installer.jar");
}

#[tokio::test]
async fn fetches_installer_verified_by_sidecar() {
    // Mock: installer.jar + installer.jar.sha1 (passend) -> Datei unter libraries/net/...; falscher Sidecar -> Error::Checksum
}
```

- [ ] **Step 3: Tests scheitern** — Run: `cargo test -p hengst-core installer` — Expected: FAIL
- [ ] **Step 4: Implementieren** nach Interfaces.
- [ ] **Step 5: Tests grün** — Run: `cargo test -p hengst-core` — Expected: PASS
- [ ] **Step 6: Lint, Format, Commit** — `feat(core): read Forge and NeoForge installers in both formats`

---

### Task 10: Prozessoren planen und Installationsstand prüfen

**Files:**
- Create: `crates/hengst-core/src/loaders/forge/processors.rs`

**Interfaces:**
- Consumes: `ModernProfile`, `Processor`, `SidedData` (Task 9), `maven_path`, `join_relative`, `sha1_file`, `Library`
- Produces:
  - `pub struct InstallEnv { pub root: PathBuf, pub libraries: PathBuf, pub minecraft_jar: PathBuf, pub game: String, pub installer: PathBuf, pub work: PathBuf }`
  - `#[derive(Clone, Debug, PartialEq, Eq)] pub struct ProcessorCommand { pub jar: PathBuf, pub classpath: Vec<PathBuf>, pub main_class: String, pub args: Vec<String>, pub outputs: Vec<(PathBuf, String)> }`
  - `pub fn resolve_data(profile: &ModernProfile, env: &InstallEnv) -> Result<HashMap<String, String>>`: Client-Werte; `[coords]` → Library-Pfad; `'x'` → `x`; sonst Eintrag im Installer (führendes `/` weg) nach `env.work` entpacken (per `join_relative`) und dessen Pfad. Dazu fest: `SIDE=client`, `MINECRAFT_JAR`, `MINECRAFT_VERSION=game`, `ROOT`, `INSTALLER`, `LIBRARY_DIR`.
  - `pub fn substitute(arg: &str, data: &HashMap<String, String>, libraries: &Path) -> Result<String>`: ganzes Argument `[coords]` → Library-Pfad; sonst jedes `{KEY}` ersetzen, unbekannter Schlüssel → `Error::Installer("unknown installer token {KEY}")`; Ergebnis mit einem `..`-Segment (getrennt an `/` und `\`) → `Error::UnsafePath`.
  - `pub fn jar_main_class(jar: &Path) -> Result<String>`: `Main-Class` aus `META-INF/MANIFEST.MF` (Fortsetzungszeilen mit führendem Leerzeichen zusammenfügen, CRLF und LF); fehlt er → `Error::Installer("<jar> has no Main-Class")`.
  - `pub fn plan(profile: &ModernProfile, env: &InstallEnv) -> Result<Vec<ProcessorCommand>>`: nur Prozessoren ohne `sides` oder mit `"client"`; `jar` und `classpath` über `maven_path` + `join_relative`; `main_class` aus der Jar (muss also schon geladen sein); `outputs` Schlüssel und Werte durch `substitute`.
  - `pub fn install_state_valid(cmds: &[ProcessorCommand], data: &HashMap<String, String>, version: &VersionJson, libraries: &Path, marker: &Path, installer_sha1: &str) -> Result<bool>`: `true` genau dann, wenn Marker-Datei = `installer_sha1`, alle `outputs` mit passender SHA1 existieren, `data["PATCHED"]` (falls vorhanden) existiert und zu `data["PATCHED_SHA"]` passt (falls vorhanden), und jede Library in `version` mit leerer `url` mit passender SHA1 existiert.

- [ ] **Step 1: Failing Tests schreiben** (Fixtures `forge-1.20.1-install_profile.json`, `neoforge-26.3-install_profile.json`; Dummy-Prozessor-Jars mit `MANIFEST.MF` im tempdir)

```rust
#[test]
fn resolves_data_like_the_forge_installer() {
    let d = resolve_data(&profile_1_20_1, &env).unwrap();
    assert_eq!(d["SIDE"], "client");
    assert_eq!(d["MC_SLIM_SHA"], "de86b035d2da0f78940796bb95c39a932ed84834");
    assert_eq!(PathBuf::from(&d["PATCHED"]), env.libraries.join("net/minecraftforge/forge/1.20.1-47.4.28/forge-1.20.1-47.4.28-client.jar"));
    assert_eq!(PathBuf::from(&d["MAPPINGS"]), env.libraries.join("de/oceanlabs/mcp/mcp_config/1.20.1-20230612.114412/mcp_config-1.20.1-20230612.114412-mappings.txt"));
    assert_eq!(PathBuf::from(&d["BINPATCH"]), env.work.join("data").join("client.lzma"));
    assert!(env.work.join("data").join("client.lzma").is_file());
}

#[test]
fn substitutes_tokens_and_coordinates() {
    let d = HashMap::from([("ROOT".to_string(), "C:/h".to_string()), ("SIDE".to_string(), "client".to_string())]);
    let libs = Path::new("C:/h/libraries");
    assert_eq!(substitute("{ROOT}/libraries/", &d, libs).unwrap(), "C:/h/libraries/");
    assert_eq!(substitute("--side", &d, libs).unwrap(), "--side");
    assert_eq!(PathBuf::from(substitute("[de.oceanlabs.mcp:mcp_config:1.20.1-20230612.114412@zip]", &d, libs).unwrap()),
        libs.join("de/oceanlabs/mcp/mcp_config/1.20.1-20230612.114412/mcp_config-1.20.1-20230612.114412.zip"));
    assert!(matches!(substitute("{NOPE}", &d, libs), Err(Error::Installer(_))));
    assert!(matches!(substitute("{ROOT}/../evil", &d, libs), Err(Error::UnsafePath(_))));
}

#[test]
fn reads_main_class_with_continuation_lines() {
    // MANIFEST.MF: "Manifest-Version: 1.0\r\nMain-Class: net.minecraftforge.binarypatcher.Consol\r\n eTool\r\n\r\n"
    assert_eq!(jar_main_class(&jar).unwrap(), "net.minecraftforge.binarypatcher.ConsoleTool");
}

#[test]
fn plans_only_client_processors() {
    let cmds = plan(&profile_1_20_1, &env).unwrap();
    assert_eq!(cmds.len(), 6);
    assert_eq!(&cmds[0].args[..2], ["--task", "MCP_DATA"]);
    let split = &cmds[3];
    assert_eq!(split.outputs.len(), 2);
    assert!(split.outputs.iter().any(|(p, h)| p.ends_with("client-1.20.1-20230612.114412-slim.jar") && h == "de86b035d2da0f78940796bb95c39a932ed84834"));
    assert!(split.args.contains(&env.minecraft_jar.to_string_lossy().into_owned()));
    let neo = plan(&profile_neo_26_3, &env).unwrap();
    assert_eq!(neo.len(), 1);
    assert!(neo[0].args.iter().any(|a| a == &format!("{}/libraries/", env.root.to_string_lossy())));
}

#[test]
fn install_state_detects_complete_and_broken_installs() {
    // alle Ausgaben + PATCHED + Marker passend -> true
    // install_state_patched_deleted: PATCHED löschen -> false
    // install_state_output_corrupt: eine Ausgabe mit falschem Inhalt -> false
    // install_state_marker_missing (abgebrochener Lauf): Marker fehlt -> false
    // install_state_other_installer: Marker mit anderer SHA1 -> false
}
```

- [ ] **Step 2: Tests scheitern** — Run: `cargo test -p hengst-core processors` — Expected: FAIL
- [ ] **Step 3: Implementieren** nach Interfaces.
- [ ] **Step 4: Tests grün** — Run: `cargo test -p hengst-core` — Expected: PASS
- [ ] **Step 5: Lint, Format, Commit** — `feat(core): plan Forge installer processors and check install state`

---

### Task 11: Prozessoren ausführen, Forge und NeoForge in `prepare`

**Files:**
- Modify: `crates/hengst-core/src/loaders/forge/processors.rs`, `crates/hengst-core/src/loaders/forge/mod.rs`, `crates/hengst-core/src/loaders/mod.rs`, `crates/hengst-core/src/launch.rs`
- Modify: `crates/hengst-core/tests/loader_e2e.rs`, `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: Tasks 8 bis 10, `LoaderSetup` (Task 5)
- Produces:
  - `pub async fn run_processors(java: &Path, cmds: &[ProcessorCommand], cwd: &Path, log: &Path) -> Result<()>`: nacheinander `java -cp <jar;classpath> <main_class> <args>`, `cwd` als Arbeitsordner, stdout und stderr an `log` angehängt (vor jedem Prozessor eine Kopfzeile mit Jar und Argumenten); Exit ≠ 0 → `Error::Processor { jar, code, log }`; nach jedem Prozessor `outputs` prüfen, falsche SHA1 → `Error::Checksum`.
  - `pub struct ForgeInstall { pub installer: PathBuf, pub installer_sha1: String, pub profile: Option<ModernProfile> }` (`None` bei Legacy)
  - `LoaderSetup.forge: Option<ForgeInstall>`; `prepare_loader` für Forge/NeoForge: `installer_url` → `fetch_installer` → `read_installer` → `check_game` → `extract_embedded`; `child = contents.version`; `extra_downloads = resolve(&profile.libraries, env, paths)?.downloads` (modern).
  - `pub async fn forge::finish_install(fi: &ForgeInstall, merged: &VersionJson, paths: &Paths, java: &Path, minecraft_jar: &Path, game: &str, kind: LoaderKind) -> Result<()>`: modern mit Prozessoren: `InstallEnv` mit `work = paths.tmp().join(format!("loader-{}", merged.id))`, `resolve_data`, `plan`; ist `install_state_valid` → fertig; sonst Marker löschen, `run_processors` mit Log `paths.logs().join(format!("loader-install-{}.log", merged.id))`, `PATCHED`/`PATCHED_SHA` und Libraries mit leerer `url` prüfen (fehlend → `Error::Installer("<path> missing after install")`, falsche SHA1 → `Error::Checksum`), Marker schreiben, `work` löschen. Ohne Prozessoren (Legacy, spec 0 von 1.12.2): nur die Libraries mit leerer `url` prüfen.
  - `prepare` ruft `finish_install` nach `ensure_runtime` und vor Assets/Natives auf.
  - CI `loader-matrix` um `forge 1.7.10`, `forge 1.8.9`, `forge 1.12.2`, `forge 1.16.5`, `forge 1.20.1`, `forge latest`, `neoforge 1.20.1`, `neoforge 1.21.1`, `neoforge latest` erweitern (Dry-Run führt die Prozessoren auf dem Runner aus).

- [ ] **Step 1: Failing Tests schreiben**

`loader_e2e.rs` (Mock-Forge-Maven, synthetische Installer aus den Task-9-Fixtures, `inheritsFrom`/`minecraft` auf die Mock-Versionen umgeschrieben):
```rust
#[tokio::test]
async fn prepares_legacy_format_forge() {
    // Vanilla "8.8" legacy; Forge-Liste: maven-metadata {"8.8": ["8.8-11.15.1.2318-8.8"]}, promos {"8.8-recommended": "11.15.1.2318"}
    // Installer: Legacy-Profil (minecraft/inheritsFrom "8.8"), universal-Jar; launchwrapper mit Sidecar, akka mit checksums
    let p = prepare(&paths, &opts_with(LoaderKind::Forge, None), &ep).await.unwrap();
    assert!(p.args.contains(&"net.minecraft.launchwrapper.Launch".to_string()));
    assert!(p.args.contains(&"net.minecraftforge.fml.common.launcher.FMLTweaker".to_string()));
    let cp = &p.args[p.args.iter().position(|a| a == "-cp").unwrap() + 1];
    assert!(cp.contains("forge-8.8-11.15.1.2318-8.8.jar"));
    assert!(!cp.contains("jline"));
    assert_eq!(p.game_dir, paths.instance("8.8-forge"));
}

#[tokio::test]
async fn prepares_spec0_forge_without_processors() {
    // 1.12.2-2859-Muster auf Vanilla "8.8": forge-Jar nur im Installer unter maven/
    let p = prepare(/* ... */).await.unwrap();
    assert!(p.args.iter().any(|a| a.contains("--versionType")));
}

#[tokio::test]
async fn forge_installer_for_wrong_game_version_is_refused() {
    // Installer mit minecraft "8.7" für Spielversion "8.8"
    assert!(matches!(prepare(/* ... */).await, Err(Error::ParentMismatch { .. })));
}
```

`processors.rs`:
```rust
#[tokio::test]
async fn failed_processor_leaves_no_marker() {
    // finish_install mit einem Profil mit einem Prozessor und java = Pfad, der nicht existiert
    // -> Err (Io oder Processor), Marker-Datei existiert nicht, work-Ordner darf bleiben
    let e = Error::Processor { jar: "net.minecraftforge:binarypatcher:1.1.1".into(), code: 1, log: PathBuf::from("C:/h/logs/x.log") };
    assert_eq!(e.to_string(), "loader installer step net.minecraftforge:binarypatcher:1.1.1 failed with exit code 1, see C:/h/logs/x.log");
}

#[tokio::test]
async fn valid_install_skips_processors() {
    // vollständiger Stand + Marker; java = nicht existierender Pfad -> Ok(()) (beweist: kein Start)
}
```

- [ ] **Step 2: Tests scheitern** — Run: `cargo test -p hengst-core --test loader_e2e` — Expected: FAIL
- [ ] **Step 3: Implementieren** nach Interfaces.
- [ ] **Step 4: Tests grün** — Run: `cargo test --workspace --all-features` — Expected: PASS
- [ ] **Step 5: Handprobe mit echtem Installer** (eine Version, kein Spielstart)

Run: `cargo run -p hengst-cli -- --home "$TEMP/hengst-dev" launch 1.20.1 --loader forge --dry-run`
Expected: Prozessoren laufen einmal (Log unter `logs/loader-install-1.20.1-forge-47.4.28.log`), Ausgabe mit `cpw.mods.bootstraplauncher.BootstrapLauncher`. Zweiter Aufruf startet keine Prozessoren (Log wächst nicht, Dauer unter 10 s).

- [ ] **Step 6: Lint, Format, Commit, CI**

```bash
git commit -am "feat(core): run Forge and NeoForge installers and prepare modded versions

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
git push
gh run watch
```
Expected: CI grün inkl. aller `loader-matrix`-Einträge.

---

### Task 12: Echte Starts Forge und NeoForge (Abnahme Teil 2)

**Files:**
- Modify: `crates/hengst-core/tests/loader_live.rs`

**Interfaces:**
- Consumes: `live_launch` (Task 7)

- [ ] **Step 1: Ignorierte Tests ergänzen**

```rust
#[tokio::test] #[ignore] async fn forge_1_7_10()     { live_launch("1.7.10", LoaderKind::Forge, 2048, None).await }
#[tokio::test] #[ignore] async fn forge_1_8_9()      { live_launch("1.8.9", LoaderKind::Forge, 2048, None).await }
#[tokio::test] #[ignore] async fn forge_1_12_2()     { live_launch("1.12.2", LoaderKind::Forge, 2048, None).await }
#[tokio::test] #[ignore] async fn forge_1_16_5()     { live_launch("1.16.5", LoaderKind::Forge, 2048, None).await }
#[tokio::test] #[ignore] async fn forge_1_20_1()     { live_launch("1.20.1", LoaderKind::Forge, 3072, None).await }
#[tokio::test] #[ignore] async fn forge_latest()     { live_launch("latest", LoaderKind::Forge, 3072, None).await }
#[tokio::test] #[ignore] async fn neoforge_1_20_1()  { live_launch("1.20.1", LoaderKind::NeoForge, 3072, None).await }
#[tokio::test] #[ignore] async fn neoforge_1_21_1()  { live_launch("1.21.1", LoaderKind::NeoForge, 3072, None).await }
#[tokio::test] #[ignore] async fn neoforge_latest()  { live_launch("latest", LoaderKind::NeoForge, 3072, None).await }
```
Für Forge/NeoForge gibt es kein Banner-Kriterium: dass das Spiel bis zur LWJGL-Initialisierung kommt, obwohl die Hauptklasse die des Loaders ist (`launchwrapper` mit `FMLTweaker`, `BootstrapLauncher`, `ForgeBootstrap`, `fml.startup.Client`), belegt den Loader-Start.

- [ ] **Step 2: Live-Läufe nacheinander**

Run: `cargo test -p hengst-core --test loader_live -- --ignored --test-threads=1`
Expected: 14 passed (5 aus Task 7, 9 neu). Fehler wie in Task 7 Step 3 behandeln, je Fix ein Regressionstest gegen Mock und ein eigener Commit.

- [ ] **Step 3: Sichtprüfung mit Agentfenster**

Release-Build mit `dev-offline`, dann nacheinander `hengst launch <v> --loader forge` für 1.7.10, 1.12.2, 1.20.1 und `--loader neoforge` für `latest`: Titelbild erreicht, Forge/NeoForge-Hinweis ("Forge ... mods loaded" bzw. NeoForge-Versionszeile) im Screenshot sichtbar, Fenster schließen. Ergebnistabelle (Loader, Version, Loader-Version, Fenster ja/nein, Startzeit, Auffälligkeiten) in den Abschlussbericht.

- [ ] **Step 4: Lint, Format, Commit** — `test(core): live launch checks for Forge and NeoForge`

---

### Task 13: README und Fahrplan

**Files:**
- Modify: `README.md` (Status-Absatz, Build-Beispiele)
- Modify: `docs/PLAN.md` Abschnitt 11 (Statuszeile unter der Überschrift)

- [ ] **Step 1: README**

Status-Absatz auf Englisch ergänzen: die CLI bereitet Vanilla und Modded (Fabric, Quilt, Legacy Fabric, NeoForge, Forge 1.7.10 and newer) vor und startet sie; Beispiele unter "Build":
```
target/release/hengst loaders fabric 1.21.11
target/release/hengst launch 1.20.1 --loader forge --dry-run
```
Der Hinweis "Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft." bleibt unverändert.

- [ ] **Step 2: PLAN.md Abschnitt 11** (Deutsch)

Neue Statuszeile über der Tabelle, Muster: `Stand <Datum>: Phase 3 erledigt, PR #<n>. Echt gestartet: Fabric 1.21.11 und <latest>, Quilt 1.21.11, Legacy Fabric 1.8.9 und 1.12.2, Forge 1.7.10, 1.8.9, 1.12.2, 1.16.5, 1.20.1 und <latest>, NeoForge 1.20.1, 1.21.1 und <latest>. Forge vor 1.7.10 bewusst nicht unterstützt.` Werte aus der Tabelle von Task 12.

- [ ] **Step 3: Prüfen und Commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace --all-features`
```bash
git commit -am "docs: mark phase 3 done in README and PLAN

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
git push
```
Danach PR `phase-3-loader` → `main` mit der Ergebnistabelle aus Task 12 und Review per code-review-Skill.

---

## Selbstprüfung

1. **Abdeckung:** Fabric/Quilt/Legacy Fabric (Tasks 4, 5, 7), Forge/NeoForge Listen (8), Installer beide Formate (9), Prozessoren generisch inkl. Seiten, Daten, Main-Class aus Manifest, Ausgabe-Hashes (10, 11), Merge (1), Hash-Politik (2, Entscheidungen), Java nach Vanilla (5), injizierbare URLs (3, alle Mock-Tests), Pfad-Eingrenzung (2, 9, 10), Live-Tests `#[ignore]` mit Ton aus und seriell (7, 12), CLI `--loader`/`--loader-version`/`loaders` (6), Doku (13). Ornithe/Cleanroom bewusst nicht.
2. **Schrittprüfung:** jede Code-Stufe nennt Signatur und Datei; Algorithmen ohne eindeutige Tests (Versionsvergleich, NeoForge-Zuordnung, Merge, Installationsstand) sind in Worten festgelegt.
3. **Typen:** `LoaderKind`, `LoaderVersion { id, maven, stable }`, `LoaderRequest`, `LoaderEndpoints`, `ResolvedLoader`, `LoaderSetup`, `ForgeInstall`, `ModernProfile`, `ProcessorCommand`, `InstallEnv` werden in genau einem Task definiert und später nur benutzt.
4. **Review Focus:** alle fünf Punkte haben benannte Tests in ihren Tasks.
5. **Umfang:** keine Funktionsrümpfe außer Tests; Fixture-Befehle statt Fixture-Inhalten.
