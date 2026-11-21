# Begleitmod M0 (Fundament) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eine Mod-Jar je Spielversion (1.8.9, 1.21.11, 26.3), die im Spiel FPS, CPS, Keystrokes und Koordinaten anzeigt, einen Einstellungsbildschirm und einen HUD-Editor mit Profilen hat, mit allem Spiel-unabhängigen in einem getesteten `common`-Modul.

**Architecture:** Gradle-Composite unter `mod/` aus drei Builds: `common` (reines Java, Ziel 8, JUnit 5), `modern` (Stonecutter 0.10, Knoten 1.21.11 und 26.3, Fabric Loom 1.19.4 in der jeweils passenden Variante) und `legacy` (Legacy Fabric, Loom-remap 1.16.x + Legacy Looming 1.16.1). Adapter registrieren genau einen HUD-Hook, einen Tick-Hook, zwei Tasten und einen Klick-Mixin und reichen alles an `HengstCore` weiter; Bildschirme zeichnet `common` über die `Canvas`-Schnittstelle.

**Tech Stack:** Java 8 (common, legacy), 21 (1.21.11), 25 (26.3); Gradle 9.8.2 auf JDK 25; Fabric Loader 0.20.1; Fabric API 0.141.6+1.21.11 und 0.166.2+26.3; Legacy Fabric API 1.13.5+1.8.9; Legacy Yarn 1.8.9+build.604; Mojang-Mappings für 1.21.11; JUnit Jupiter 5.11.4.

**Spec:** `docs/begleitmod-spec.md` (Abschnitte 3 bis 8 und 12, Meilenstein M0).

**Ausgangslage:** Arbeit auf Branch `begleitmod-m0` von `main`. Die Rust-Seite wird nicht angefasst. Die Spielprüfungen in Task 11 bis 14 brauchen `hengst launch ... --loader fabric|legacy-fabric` aus Phase 3 (`docs/superpowers/plans/2026-11-20-phase-3-loader.md`). Ist Phase 3 noch nicht gemergt, läuft die Spielprüfung stattdessen über Looms `runClient` (Befehl steht im jeweiligen Task); der Rest des Plans hängt nicht an Phase 3.

## Global Constraints

- Mod-ID `hengst`, Anzeigename "Hengst", Paket `io.github.clemensjl.hengst.mod`, Maven-Gruppe `io.github.clemensjl.hengst`, `mod_version=0.1.0`, Jar-Name `hengst-mod-0.1.0+<mc>.jar`.
- `common` kompiliert mit `options.release = 8` und hat keine Laufzeitabhängigkeit (kein Gson, keine Minecraft-Klasse, keine Java-9+-API im Hauptcode). Nur `compileTestJava` nutzt `options.release = 17` (Test-Code darf `com.sun.management` und `String.repeat` verwenden); der Hauptcode bleibt bei 8.
- Gradle 9.8.2 (Wrapper unter `mod/`), Gradle-JVM ist JDK 25; `org.gradle.jvmargs=-Xmx2G`, `org.gradle.workers.max=2` (wenig RAM).
- Versionen: Loom 1.19.4 (modern, Plugin-IDs `net.fabricmc.fabric-loom` für 26.3 und `net.fabricmc.fabric-loom-remap` für 1.21.11); Loom-remap und `legacy-looming` beide 1.16.1 (legacy, dieselbe Version nach Spec 4.1); Stonecutter 0.10; Fabric Loader 0.20.1 überall.
- `fabric.mod.json`: `"license": "GPL-3.0-or-later"`, Beschreibung endet mit "Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft."; Abhängigkeit `fabric-api` (modern) bzw. `legacy-fabric-api` (legacy).
- Code, Kommentare, Log- und Fehlertexte Englisch; `docs/` Deutsch; UI-Texte über `hengst/lang/en_us.json` und `de_de.json`.
- Laufzeitregeln aus Spec 8: keine Allokation im Frame-Pfad, aktive Module als Array, Mixins mit `require = 1`, als Erstes ein `static boolean`-Check, keine Reflection, kein Dateizugriff in Frame/Tick außer gebündeltem Speichern.
- Keine Emojis in Code, Doku, Commits.
- Jeder in einem Task aufgeführte Test ist als Code mit konkreten Werten ausgeschrieben; ein Test, der nur aus einem Kommentar besteht, ist ein Planfehler und wird gemeldet, nicht still ergänzt. Gemeinsame Test-Helfer liegen in `common/src/test/java/io/github/clemensjl/hengst/mod/common/testutil/` und sind im jeweils frühesten Task definiert (`FakeTime`, `TestFeature` in Task 3; `FakeCanvas`, `CountingCanvas`, `FakePlatform`, `FixedSizeModule` in Task 6).
- Bildschirme von `common` (Editor, Einstellungen) schließen nie selbst: `onClose` des Bildschirms ruft `HengstCore`, und der speichert, baut den Renderer neu und ruft `Platform.closeScreen()` (Task 10). Solange einer dieser Bildschirme offen ist, zeichnet `HengstCore` keine HUD-Module.
- Wenig RAM: nie Gradle-Build und Spiel gleichzeitig, nie zwei Spiele gleichzeitig, kein Build parallel zu einem `cargo`-Lauf eines anderen Agenten. Echte Spielstarts setzen vorher `soundCategory_master:0.0` in `options.txt` der Instanz.
- Spielstarts für Prüfungen: Dev-Build des Launchers mit Feature `dev-offline` und Einzelspielerwelt ("Dev"); Sichtprüfung mit Agentfenster (Spiel auf dem Agent-Desktop starten, Tasten per `press_key`, Klicks per `click_point`-Raster, Bild per Screenshot).
- Jeder Task endet mit grünen Tests (`./gradlew :common:test` aus `mod/`, sobald es Tests gibt) und einem Build aller berührten Jars, dann ein Commit mit Trailer `Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>`.

## Review Focus

1. **GUI-Skalierung oder Fenstergröße ändert sich nach dem Speichern des Layouts, oder ein Modul ist bei Skalierung 3,0 breiter als der Bildschirm:** Module bleiben an ihrer Ecke und vollständig sichtbar (breiter als der Bildschirm: linksbündig bei x = 0). Tests in Task 5 (`resolve_clamps_after_screen_shrinks`, `resolve_wider_than_screen_starts_at_zero`).
2. **Kaputte, leere oder halb geschriebene Konfigurationsdatei** (Absturz beim Speichern, Hand-Edit): Sicherung `*.broken-<Zeit>`, Standardwerte (beim ersten Start die M0-Standardmodule), kein Absturz, beim nächsten Speichern wieder gültig. Tests in Task 4 (`load_broken_profile_backs_up_and_returns_defaults`, `load_empty_file_backs_up_and_returns_defaults`, `save_overwrites_and_leaves_no_tmp`, `save_after_broken_load_writes_valid_file`), Task 10 (`broken_config_logs_one_warning_and_starts`) und Task 2 (Parser-Fehler mit Position).
3. **HUD versteckt (F1), eigener Bildschirm (Editor, Einstellungen) offen, nicht in einer Welt:** keine Module gezeichnet (unter dem Editor würden sie sonst doppelt gezeichnet); keine Klicks gezählt, solange irgendein Bildschirm (Chat, Inventar, Menü) offen ist. Tests in Task 10 (`hud_not_rendered_when_hidden`, `hud_not_rendered_outside_world`, `hud_not_rendered_while_own_screen_open`, `clicks_ignored_while_screen_open`).
4. **Sehr schnelles Klicken (mehr als 64 Klicks in einer Sekunde, Doppelklick-Mäuse, Taste gehalten):** CPS zählt nur Drück-Kanten, liefert bei Überlauf 64 statt falscher kleiner Werte, fällt nach 1 s ohne Klick auf 0. Tests in Task 7 (`cps_caps_at_buffer_size`, `cps_drops_to_zero_after_window`).
5. **Ziehen im Editor endet außerhalb des Fensters oder mit Esc mitten im Ziehen:** Position eingeklemmt, Ziehen beendet, genau ein Speichern. Tests in Task 8 (`drag_outside_screen_is_clamped`, `escape_during_drag_ends_drag_and_closes`).

---

## Dateistruktur

```
mod/
  settings.gradle, build.gradle (buildAll), gradle.properties, gradlew(.bat), gradle/wrapper/
  common/
    settings.gradle, build.gradle
    src/main/java/io/github/clemensjl/hengst/mod/common/
      HengstCore.java                 Fassade, Einstiegspunkte der Adapter
      I18n.java                       Sprachdateien laden, Rückfall en_us
      platform/  GameVersion, Canvas, GameView, GameKey, InputView, UiKey, UiScreen, Platform, TimeSource
      json/      Json, JsonException, JsonObjects
      feature/   Feature, FeatureGroup, FeatureRegistry, OptionSpec
      config/    ModuleConfig, Profile, ProfileStore, Settings, SettingsStore, AtomicFiles
      layout/    Anchor, MutableRect, LayoutMath
      hud/       HudModule, TextHudModule, HudContext, HudRenderer, FrameCounter, CpsCounter
      hud/modules/  FpsModule, CpsModule, KeystrokesModule, CoordinatesModule
      editor/    HudEditorModel, HudEditorScreen
      ui/        Widget, ToggleWidget, SliderWidget, ChoiceWidget, ButtonWidget, ScrollList, SettingsModel, SettingsScreen
    src/main/resources/hengst/lang/en_us.json, de_de.json
    src/test/java/io/github/clemensjl/hengst/mod/common/...   (je Paket ein Test, dazu testutil/FakeTime, TestFeature, FakeCanvas, CountingCanvas, FakePlatform, FixedSizeModule)
  modern/
    settings.gradle, stonecutter.gradle, build.gradle, gradle.properties
    versions/1.21.11/gradle.properties, versions/26.3/gradle.properties
    src/main/java/io/github/clemensjl/hengst/mod/modern/
      HengstModernMod.java  ModernPlatform.java  ModernCanvas.java  ModernScreenHost.java  ModernKeys.java
      mixin/MouseHandlerMixin.java
    src/main/resources/fabric.mod.json, hengst.mixins.json, assets/hengst/lang/en_us.json, de_de.json, assets/hengst/icon.png
  legacy/
    settings.gradle, build.gradle, gradle.properties
    src/main/java/io/github/clemensjl/hengst/mod/legacy/
      HengstLegacyMod.java  LegacyPlatform.java  LegacyCanvas.java  LegacyScreenHost.java  LegacyKeys.java
      mixin/KeyBindingMixin.java
    src/main/resources/fabric.mod.json, hengst.mixins.json, assets/hengst/lang/en_US.lang, de_DE.lang, assets/hengst/icon.png
docs/PLAN.md                       Abschnitte 6, 11 (Zeile Phase 7), 13.3
CLAUDE.md                          Stack-Zeile für mod/
.gitignore                         mod/**/build, mod/**/.gradle, mod/dist
```

---

### Task 1: Gradle-Gerüst, leere Mod-Jars für alle drei Versionen, PLAN.md auf die neue Entscheidung

**Files:**
- Create: `mod/settings.gradle`, `mod/build.gradle`, `mod/gradle.properties`, `mod/gradlew`, `mod/gradlew.bat`, `mod/gradle/wrapper/gradle-wrapper.properties`, `mod/gradle/wrapper/gradle-wrapper.jar`
- Create: `mod/common/settings.gradle`, `mod/common/build.gradle`, `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/package-info.java`
- Create: `mod/modern/settings.gradle`, `mod/modern/stonecutter.gradle`, `mod/modern/build.gradle`, `mod/modern/gradle.properties`, `mod/modern/versions/1.21.11/gradle.properties`, `mod/modern/versions/26.3/gradle.properties`, `mod/modern/src/main/java/io/github/clemensjl/hengst/mod/modern/HengstModernMod.java`, `mod/modern/src/main/resources/fabric.mod.json`, `mod/modern/src/main/resources/assets/hengst/icon.png`
- Create: `mod/legacy/settings.gradle`, `mod/legacy/build.gradle`, `mod/legacy/gradle.properties`, `mod/legacy/src/main/java/io/github/clemensjl/hengst/mod/legacy/HengstLegacyMod.java`, `mod/legacy/src/main/resources/fabric.mod.json`, `mod/legacy/src/main/resources/assets/hengst/icon.png`
- Modify: `docs/PLAN.md` (Abschnitt 6 ab "Zwei Wege:", Abschnitt 11 Zeile "7 In-Game", Abschnitt 13 Punkt 3), `CLAUDE.md` (Abschnitt Stack), `.gitignore`

**Interfaces:**
- Consumes: nichts.
- Produces: Gradle-Pfade `:common:test`, `:modern:1.21.11:build`, `:modern:26.3:build`, `:legacy:build`, `buildAll` (Ergebnis `mod/dist/hengst-mod-0.1.0+{1.8.9,1.21.11,26.3}.jar`). Einstiegspunkte `HengstModernMod implements net.fabricmc.api.ClientModInitializer`, `HengstLegacyMod implements net.fabricmc.api.ClientModInitializer`, beide loggen `[hengst] loaded on <mc_version>` (die Logzeile bleibt in Task 11 und 13 erhalten; `static boolean ENABLED` und `static HengstCore core()` kommen erst dort, wenn `HengstCore` existiert).

- [ ] **Step 1: Voraussetzung prüfen**

Run: `java -version` (muss 25 melden; Gradle nutzt `JAVA_HOME`).
Expected: `openjdk version "25...`. Fehlt JDK 25: anhalten und den Menschen bitten, ein JDK 25 (z. B. Temurin) zu installieren; Mojangs Laufzeiten im Launcher-Ordner sind JREs ohne `javac`.

- [ ] **Step 2: Wrapper und Composite-Wurzel anlegen**

Wrapper auf `distributionUrl=https\://services.gradle.org/distributions/gradle-9.8.2-bin.zip` mit `validateDistributionUrl=true` (Wrapper-Jar und Skripte aus der Gradle-9.8.2-Distribution, z. B. mit einer lokal vorhandenen Gradle-Installation `gradle wrapper --gradle-version 9.8.2`, oder aus `FabricMC/fabric-example-mod` Zweig `26.3` kopieren, dort ist es genau 9.8.2).
`mod/settings.gradle`: `rootProject.name = 'hengst-mod'`, `includeBuild 'common'`, `includeBuild 'modern'`, `includeBuild 'legacy'`.
`mod/gradle.properties`: `org.gradle.jvmargs=-Xmx2G`, `org.gradle.workers.max=2`, `org.gradle.parallel=false`, `mod_version=0.1.0`, `maven_group=io.github.clemensjl.hengst`.
`mod/build.gradle`: Task `buildAll` vom Typ `Sync`, `dependsOn` auf `gradle.includedBuild('common').task(':test')`, `gradle.includedBuild('modern').task(':1.21.11:build')`, `...task(':26.3:build')`, `gradle.includedBuild('legacy').task(':build')`; `from 'modern/versions/1.21.11/build/libs'`, `'modern/versions/26.3/build/libs'`, `'legacy/build/libs'`; `include 'hengst-mod-*.jar'`, `exclude '*-sources.jar'`; `into 'dist'`.

- [ ] **Step 3: `common` als Java-Bibliothek**

`common/settings.gradle`: `rootProject.name = 'common'` (Voraussetzung dafür, dass `io.github.clemensjl.hengst:common` im Composite durch den Build ersetzt wird).
`common/build.gradle`: Plugin `java-library`, `group = 'io.github.clemensjl.hengst'`, `version = '0.1.0'`, `tasks.withType(JavaCompile).configureEach { options.release = 8; options.encoding = 'UTF-8' }`, `testImplementation platform('org.junit:junit-bom:5.11.4')`, `testImplementation 'org.junit.jupiter:junit-jupiter'`, `testRuntimeOnly 'org.junit.platform:junit-platform-launcher'`, `tasks.named('compileTestJava') { options.release = 17 }` (nur Test-Code, damit `com.sun.management.ThreadMXBean` und `String.repeat` aufgelöst werden; der Hauptcode bleibt bei 8), `test { useJUnitPlatform() }`, Repository `mavenCentral()`.

- [ ] **Step 4: `modern` mit Stonecutter**

`modern/settings.gradle`: `pluginManagement` mit Repositories `https://maven.fabricmc.net/`, `https://maven.kikugie.dev/releases`, `gradlePluginPortal()` und `plugins { id 'net.fabricmc.fabric-loom' version '1.19.4'; id 'net.fabricmc.fabric-loom-remap' version '1.19.4' }`; `plugins { id 'dev.kikugie.stonecutter' version '0.10' }`; `stonecutter { create(rootProject) { versions '1.21.11', '26.3'; vcsVersion = '26.3' } }`; `includeBuild '../common'`.
`modern/stonecutter.gradle`: `plugins { id 'dev.kikugie.stonecutter'; id 'net.fabricmc.fabric-loom' apply false }` (legt das gemeinsame Loom-Artefakt auf den Klassenpfad), `stonecutter active '26.3'`.
`versions/1.21.11/gradle.properties`: `minecraft_version=1.21.11`, `fabric_api_version=0.141.6+1.21.11`, `java_release=21`, `loom_remap=true`. `versions/26.3/gradle.properties`: `minecraft_version=26.3`, `fabric_api_version=0.166.2+26.3`, `java_release=25`, `loom_remap=false`.
`modern/build.gradle`: `def remap = project.property('loom_remap').toBoolean()`; `apply plugin: remap ? 'net.fabricmc.fabric-loom-remap' : 'net.fabricmc.fabric-loom'`; `dependencies { minecraft "com.mojang:minecraft:${minecraft_version}"; if (remap) mappings loom.officialMojangMappings(); add(remap ? 'modImplementation' : 'implementation', "net.fabricmc:fabric-loader:0.20.1"); add(remap ? 'modImplementation' : 'implementation', "net.fabricmc.fabric-api:fabric-api:${fabric_api_version}"); implementation 'io.github.clemensjl.hengst:common:0.1.0'; include 'io.github.clemensjl.hengst:common:0.1.0' }`; `options.release = java_release as int`; `base.archivesName = 'hengst-mod'`, `version = "${mod_version}+${minecraft_version}"`; `processResources` ersetzt `${version}` und `${minecraft_version}` in `fabric.mod.json`. `mod_version` kommt aus `mod/modern/gradle.properties` mit dem Inhalt `mod_version=0.1.0` und `maven_group=io.github.clemensjl.hengst` (dieselben Werte wie `mod/gradle.properties`; ein eingebundener Build erbt die Eigenschaften der Wurzel nicht).
`fabric.mod.json` (modern): `schemaVersion 1`, `id hengst`, `version ${version}`, `name Hengst`, `environment client`, `entrypoints.client ["io.github.clemensjl.hengst.mod.modern.HengstModernMod"]`, `depends { fabricloader ">=0.20.1", minecraft "${minecraft_version}", fabric-api "*", java ">=21" }`, `license`, `description` und `icon` laut Global Constraints.

- [ ] **Step 5: `legacy` nach dem Legacy-Fabric-Beispiel**

`legacy/settings.gradle`: `pluginManagement` mit `https://maven.fabricmc.net/`, `https://maven.legacyfabric.net/`, `gradlePluginPortal()`; `includeBuild '../common'`.
`legacy/gradle.properties`: `mod_version=0.1.0`, `maven_group=io.github.clemensjl.hengst`.
`legacy/build.gradle`: `plugins { id 'net.fabricmc.fabric-loom-remap' version '1.16.1'; id 'legacy-looming' version '1.16.1' }` (beide Plugins müssen dieselbe Version haben, Spec 4.1); `dependencies { minecraft 'com.mojang:minecraft:1.8.9'; mappings legacy.yarn('1.8.9', '604'); modImplementation 'net.fabricmc:fabric-loader:0.20.1'; modImplementation 'net.legacyfabric.legacy-fabric-api:legacy-fabric-api:1.13.5+1.8.9'; implementation 'io.github.clemensjl.hengst:common:0.1.0'; include 'io.github.clemensjl.hengst:common:0.1.0' }`; `options.release = 8`; Archivname und Version wie modern mit `+1.8.9`. Alle Koordinaten und Versionen dieses Tasks (Plugins, Mappings, Fabric API, Legacy Fabric API) vor dem Eintragen gegen die echten Repositories (`maven.fabricmc.net`, `maven.legacyfabric.net`, Gradle-Plugin-Portal) prüfen; Abweichungen im Commit begründen.
`fabric.mod.json` (legacy): wie modern, aber `depends { fabricloader ">=0.20.1", minecraft "1.8.9", legacy-fabric-api "*" }`, Einstiegspunkt `io.github.clemensjl.hengst.mod.legacy.HengstLegacyMod`. Logger: `org.apache.logging.log4j.LogManager` (1.8.9) bzw. `org.slf4j.LoggerFactory` (modern).

- [ ] **Step 6: Bauen und Ergebnis prüfen**

Run (aus `mod/`): `./gradlew buildAll`
Expected: `BUILD SUCCESSFUL`, `ls dist` zeigt genau `hengst-mod-0.1.0+1.21.11.jar`, `hengst-mod-0.1.0+1.8.9.jar`, `hengst-mod-0.1.0+26.3.jar`.
Run: `unzip -l dist/hengst-mod-0.1.0+26.3.jar` und dasselbe für die beiden anderen.
Expected: je `fabric.mod.json`, `META-INF/jars/common-0.1.0.jar`, die Einstiegsklasse. `unzip -p dist/hengst-mod-0.1.0+1.21.11.jar fabric.mod.json` enthält `"minecraft": "1.21.11"`.
Wenn der Composite-Build an zwei Loom-Versionen scheitert (Spec 4.1, ungeprüft): `buildAll` als Shell-Skript `mod/build-all.sh` mit drei getrennten Aufrufen `./gradlew -p common test`, `./gradlew -p modern build`, `./gradlew -p legacy build` plus Kopieren nach `dist/` ersetzen, `mod/settings.gradle` entfernen, im Commit begründen.
Wenn Loom-remap 1.16.1 und `legacy-looming` 1.16.1 nicht auflösen oder nicht starten: beide auf `1.16-SNAPSHOT` wie im Legacy-Fabric-Beispiel (immer dieselbe Version für beide), im Commit begründen.

- [ ] **Step 7: PLAN.md, CLAUDE.md, .gitignore**

PLAN.md 6: Absatz "Zwei Wege:" bis "...Vanilla-Info hinaus geben." ersetzen durch: Entscheidung 20.11.2026 eigene Begleitmod mit 50 Funktionen für 1.8.9, 1.21.11 und 26.3, von Grund auf geschrieben, kein Code aus geschlossenen Clients, Details und Fahrplan M0 bis M8 in `docs/begleitmod-spec.md`; der Satz zu Server-Anti-Cheats und Vanilla-Info bleibt. Die HUD- und Komfortliste davor bleibt.
PLAN.md 11, Zeile Phase 7: Inhalt "Begleitmod M0 bis M8 (docs/begleitmod-spec.md), Auslieferung über den Launcher".
PLAN.md 13.3: "Entschieden 20.11.2026 (ersetzt 31.10.): eigene Begleitmod statt AxolotlClient, siehe docs/begleitmod-spec.md".
CLAUDE.md Stack: neue Zeile "Begleitmod unter mod/ (Gradle 9.8.2, JDK 25): `./gradlew :common:test`, `./gradlew buildAll` aus `mod/`."
`.gitignore`: `mod/**/build/`, `mod/**/.gradle/`, `mod/dist/`.

- [ ] **Step 8: Commit**

```bash
git add mod docs/PLAN.md CLAUDE.md .gitignore
git commit -m "build(mod): gradle composite for 1.8.9, 1.21.11 and 26.3 with empty mod jars

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 2: JSON-Leser und -Schreiber in common

**Files:**
- Create: `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/json/Json.java`, `JsonException.java`, `JsonObjects.java`
- Test: `mod/common/src/test/java/io/github/clemensjl/hengst/mod/common/json/JsonTest.java`

**Interfaces:**
- Consumes: nichts.
- Produces:
  - `public static Object Json.parse(String text) throws JsonException` liefert `LinkedHashMap<String,Object>`, `ArrayList<Object>`, `String`, `Long` (Ganzzahl im long-Bereich), `Double` (sonst), `Boolean`, `null`.
  - `public static String Json.write(Object value)` schreibt mit zwei Leerzeichen Einrückung, Schlüssel in Einfügereihenfolge, `\n` als Zeilenende, Nicht-ASCII unverändert, Steuerzeichen als `\uXXXX`. Andere Typen: `IllegalArgumentException`.
  - `public JsonException(String message, int offset)`, `public int offset()`.
  - `JsonObjects`: `static Map<String,Object> object(Object value)` (leere Map, wenn kein Objekt), `static String string(Map<String,Object> m, String key, String fallback)`, `static int integer(Map<String,Object> m, String key, int fallback)`, `static float decimal(Map<String,Object> m, String key, float fallback)`, `static boolean bool(Map<String,Object> m, String key, boolean fallback)`, `static long longValue(Map<String,Object> m, String key, long fallback)`; falscher Typ ergibt den Rückfallwert. `integer` und `longValue` akzeptieren nur `Long`; `integer` liefert für einen Wert außerhalb des `int`-Bereichs den Rückfallwert, `decimal` akzeptiert `Long` und `Double`.

- [ ] **Step 1: Write the failing tests**

```java
@Test void roundtrip_keeps_order_and_types() throws Exception {
    String src = "{\"b\":1,\"a\":[true,null,2.5,\"x\"],\"c\":{}}";
    Map<String,Object> m = (Map<String,Object>) Json.parse(src);
    assertEquals(Arrays.asList("b","a","c"), new ArrayList<>(m.keySet()));
    assertEquals(1L, m.get("b"));
    assertEquals(Arrays.asList(true, null, 2.5, "x"), m.get("a"));
    assertEquals(m, Json.parse(Json.write(m)));
}
@Test void escapes_roundtrip() throws Exception {
    String s = "q\"b\\s/\n\t\u0001\u00e4\ud83d\ude00";
    assertEquals(s, Json.parse(Json.write(s)));
    assertEquals("\u00e4", Json.parse("\"\\u00e4\""));
}
@Test void errors_report_offset() {
    JsonException e = assertThrows(JsonException.class, () -> Json.parse("{\"a\":1,}"));
    assertEquals(7, e.offset());
}
@Test void rejects_trailing_garbage_and_empty() {
    assertThrows(JsonException.class, () -> Json.parse("{} x"));
    assertThrows(JsonException.class, () -> Json.parse(""));
    assertThrows(JsonException.class, () -> Json.parse("{\"a\":"));
}
@Test void tolerates_bom_and_whitespace() throws Exception {
    assertEquals(Collections.emptyMap(), Json.parse("\ufeff  {}\r\n"));
}
@Test void nesting_deeper_than_64_is_rejected() {
    StringBuilder b = new StringBuilder();
    for (int i = 0; i < 65; i++) b.append('[');
    for (int i = 0; i < 65; i++) b.append(']');
    assertThrows(JsonException.class, () -> Json.parse(b.toString()));
}
@Test void big_integers_become_double() throws Exception {
    assertEquals(1.0E20, Json.parse("100000000000000000000"));
    assertEquals(-3L, Json.parse("-3"));
}
@Test void accessors_fall_back_on_wrong_type() throws Exception {
    Map<String,Object> m = JsonObjects.object(Json.parse("{\"i\":\"x\",\"f\":2,\"b\":1}"));
    assertEquals(7, JsonObjects.integer(m, "i", 7));
    assertEquals(2.0f, JsonObjects.decimal(m, "f", 0f));
    assertFalse(JsonObjects.bool(m, "b", false));
    assertTrue(JsonObjects.object("nope").isEmpty());
}
@Test void long_accessor_and_int_range() throws Exception {
    Map<String,Object> m = JsonObjects.object(Json.parse("{\"c\":4294967295,\"d\":2.5,\"s\":\"x\"}"));
    assertEquals(4294967295L, JsonObjects.longValue(m, "c", 0L));
    assertEquals(9L, JsonObjects.longValue(m, "s", 9L));
    assertEquals(9L, JsonObjects.longValue(m, "d", 9L));
    assertEquals(7, JsonObjects.integer(m, "c", 7));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `./gradlew :common:test --tests '*JsonTest'`
Expected: FAIL, Klassen fehlen.

- [ ] **Step 3: Implement `Json`, `JsonException`, `JsonObjects`**

Rekursiver Abstieg über ein `char`-Array mit Positionszähler; Tiefe als Parameter, ab 65 `JsonException("too deep", pos)`. Zahlen nach JSON-Grammatik (kein `+`, keine führenden Nullen), Ganzzahl über `Long.parseLong` mit Rückfall auf `Double.parseDouble`. Ganzzahlige `Double` beim Schreiben ohne `.0`? Nein: `Double` immer mit Dezimalteil (`2.5`, `1.0`), `Long` und `Integer` ohne.

- [ ] **Step 4: Run tests to verify they pass**

Run: `./gradlew :common:test --tests '*JsonTest'`
Expected: PASS (9 Tests).

- [ ] **Step 5: Commit**

```bash
git add mod/common
git commit -m "feat(mod-common): minimal json reader and writer

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 3: Plattform-Schnittstellen, Features, Optionen, Übersetzungen

**Files:**
- Create: `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/platform/{GameVersion,Canvas,GameView,GameKey,InputView,UiKey,UiScreen,Platform,TimeSource}.java`
- Create: `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/feature/{Feature,FeatureGroup,FeatureRegistry,OptionSpec}.java`
- Create: `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/I18n.java`, `mod/common/src/main/resources/hengst/lang/en_us.json`, `de_de.json`
- Test: `mod/common/src/test/java/io/github/clemensjl/hengst/mod/common/feature/FeatureRegistryTest.java`, `.../common/I18nTest.java`, `.../common/testutil/FakeTime.java`, `.../common/testutil/TestFeature.java`

**Interfaces:**
- Consumes: `Json.parse`, `JsonObjects` (Task 2).
- Produces:
  - Die Plattform-Schnittstellen exakt wie Spec 4.3 (mit `GameView.screenOpen()`, `Platform.languageCode()`, `Platform.logWarning(String)`).
  - `public abstract class Feature { protected Feature(String id, int number, FeatureGroup group, EnumSet<GameVersion> versions); public final String id(); public final int number(); public final FeatureGroup group(); public final boolean supports(GameVersion v); public List<OptionSpec> options() /* leer */; }`
  - `public enum FeatureGroup { HUD, VISUAL, COMFORT }`
  - `public final class FeatureRegistry { public void register(Feature f); public Feature byId(String id); public List<Feature> supportedOn(GameVersion v); }`; doppelte ID oder Nummer: `IllegalArgumentException`; `supportedOn` nach Nummer sortiert, unveränderlich.
  - `public final class OptionSpec { public enum Kind { TOGGLE, INT_RANGE, FLOAT_RANGE, CHOICE } public static OptionSpec toggle(String key, boolean def); public static OptionSpec intRange(String key, int def, int min, int max); public static OptionSpec floatRange(String key, float def, float min, float max, float step); public static OptionSpec choice(String key, String def, String... values); public String key(); public Kind kind(); public Object defaultValue(); public Object coerce(Object raw); public float min(); public float max(); public float step(); public String[] values(); }` `coerce` klemmt Zahlen auf `[min,max]` (Float auf Schrittweite gerundet), unbekannte Choice-Werte und falsche Typen ergeben `defaultValue()`. Rückgabetypen von `coerce` und `defaultValue()`: TOGGLE `Boolean`, INT_RANGE `Integer`, FLOAT_RANGE `Float`, CHOICE `String`. `min()`/`max()`/`step()` liefern für INT_RANGE die Grenzen als `float` und Schrittweite 1, für FLOAT_RANGE die übergebenen Werte, für TOGGLE und CHOICE 0; `values()` liefert nur bei CHOICE die Werte (eine Kopie), sonst ein leeres Array. Die Slider- und Choice-Widgets in Task 9 bauen ausschließlich auf diesen Zugriffen auf.
  - `public final class I18n { public static I18n load(String languageCode); public String get(String key); public String get(String key, Object arg); }` lädt `/hengst/lang/<code>.json` vom Klassenpfad; der Code wird vor dem Nachschlagen mit `toLowerCase(Locale.ROOT)` in Kleinbuchstaben gewandelt (1.8.9 meldet `de_DE`, die Jar-Datei heißt `de_de.json`); fehlende Datei oder fehlender Schlüssel fällt auf `en_us`, dann auf den Schlüssel selbst zurück; `get(key, arg)` ersetzt `%s`.
  - `testutil/FakeTime implements TimeSource { public long now; public long nanoTime() /* liefert now */; public void advanceMillis(long ms) /* now += ms * 1_000_000 */; }` (öffentlicher Standardkonstruktor, `now` beginnt bei 0).
  - `testutil/TestFeature extends Feature { public TestFeature(String id, int number, EnumSet<GameVersion> versions) /* Gruppe HUD, keine Optionen */; public static List<String> ids(List<? extends Feature> features) /* Ids in Listenreihenfolge */ }`
  - Sprachschlüssel M0 (beide Dateien): `hengst.title`, `hengst.notice` ("Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft." in beiden Sprachen unverändert), `hengst.group.hud|visual|comfort`, `hengst.feature.fps|cps|keystrokes|coordinates`, `hengst.color.white|yellow|lime|aqua|red|orange|gray|black`, `hengst.option.background|shadow|text_color|show_right|show_mouse|show_space|decimals`, `hengst.button.edit_hud|new_profile|delete_profile|done`, `hengst.profile`, `hengst.editor.hint`.

- [ ] **Step 1: Write the failing tests**

```java
@Test void duplicate_id_or_number_is_rejected() {
    FeatureRegistry r = new FeatureRegistry();
    r.register(new TestFeature("a", 1, EnumSet.allOf(GameVersion.class)));
    assertThrows(IllegalArgumentException.class, () -> r.register(new TestFeature("a", 2, EnumSet.allOf(GameVersion.class))));
    assertThrows(IllegalArgumentException.class, () -> r.register(new TestFeature("b", 1, EnumSet.allOf(GameVersion.class))));
}
@Test void supported_on_filters_and_sorts() {
    FeatureRegistry r = new FeatureRegistry();
    r.register(new TestFeature("totems", 14, EnumSet.of(GameVersion.V1_21_11, GameVersion.V26_3)));
    r.register(new TestFeature("fps", 1, EnumSet.allOf(GameVersion.class)));
    assertEquals(Arrays.asList("fps"), TestFeature.ids(r.supportedOn(GameVersion.V1_8_9)));
    assertEquals(Arrays.asList("fps", "totems"), TestFeature.ids(r.supportedOn(GameVersion.V26_3)));
}
@Test void option_coerce_clamps_and_falls_back() {
    assertEquals(2, OptionSpec.intRange("d", 0, 0, 2).coerce(9L));
    assertEquals(0.5f, OptionSpec.floatRange("s", 1f, 0.5f, 3f, 0.1f).coerce(0.1));
    assertEquals(1.3f, (Float) OptionSpec.floatRange("s", 1f, 0.5f, 3f, 0.1f).coerce(1.26), 1e-6f);
    assertEquals("a", OptionSpec.choice("c", "a", "a", "b").coerce("zzz"));
    assertEquals(true, OptionSpec.toggle("t", true).coerce("yes"));
}
@Test void i18n_falls_back_to_english_then_key() {
    I18n de = I18n.load("de_de");
    assertNotEquals("hengst.title", de.get("hengst.title"));
    assertEquals(I18n.load("en_us").get("hengst.feature.fps"), I18n.load("xx_yy").get("hengst.feature.fps"));
    assertEquals("no.such.key", de.get("no.such.key"));
}
@Test void option_accessors_expose_range_and_values() {
    OptionSpec f = OptionSpec.floatRange("s", 1f, 0.5f, 3f, 0.1f);
    assertEquals(0.5f, f.min()); assertEquals(3f, f.max()); assertEquals(0.1f, f.step());
    OptionSpec i = OptionSpec.intRange("d", 1, 1, 5);
    assertEquals(1f, i.min()); assertEquals(5f, i.max()); assertEquals(1f, i.step());
    assertArrayEquals(new String[] {"a", "b"}, OptionSpec.choice("c", "a", "a", "b").values());
    assertEquals(0, OptionSpec.toggle("t", true).values().length);
}
@Test void language_code_is_lowercased() {
    assertEquals(I18n.load("de_de").get("hengst.button.done"), I18n.load("de_DE").get("hengst.button.done"));
    assertNotEquals(I18n.load("en_us").get("hengst.button.done"), I18n.load("de_DE").get("hengst.button.done"));
}
@Test void both_language_files_have_the_same_keys() throws Exception {
    Set<String> en = langKeys("en_us"), de = langKeys("de_de");
    assertEquals(en, de);
    assertTrue(en.containsAll(Arrays.asList("hengst.title", "hengst.notice", "hengst.button.done", "hengst.color.white", "hengst.color.black", "hengst.editor.hint")));
}
static Set<String> langKeys(String code) throws Exception {
    try (InputStream in = I18nTest.class.getResourceAsStream("/hengst/lang/" + code + ".json")) {
        assertNotNull(in);
        return JsonObjects.object(Json.parse(new String(in.readAllBytes(), StandardCharsets.UTF_8))).keySet();
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `./gradlew :common:test --tests '*FeatureRegistryTest' --tests '*I18nTest'`
Expected: FAIL, Klassen fehlen.

- [ ] **Step 3: Implement die Typen aus dem Interfaces-Block**

`GameVersion.id()` liefert "1.8.9", "1.21.11", "26.3". `TimeSource.SYSTEM` als `System::nanoTime` (Java 8 erlaubt Methodenreferenz im Interface-Feld).

- [ ] **Step 4: Run tests to verify they pass**

Run: `./gradlew :common:test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add mod/common
git commit -m "feat(mod-common): platform interfaces, feature registry, options and i18n

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 4: Konfiguration und Profile

**Files:**
- Create: `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/config/{ModuleConfig,Profile,ProfileStore,Settings,SettingsStore,AtomicFiles}.java`, `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/layout/Anchor.java`
- Test: `mod/common/src/test/java/io/github/clemensjl/hengst/mod/common/config/ProfileStoreTest.java`, `SettingsStoreTest.java`

**Interfaces:**
- Consumes: `Json`, `JsonObjects` (Task 2); `OptionSpec` (Task 3).
- Produces:
  - `public enum Anchor { TOP_LEFT(0f,0f), TOP_CENTER(.5f,0f), TOP_RIGHT(1f,0f), MIDDLE_LEFT(0f,.5f), CENTER(.5f,.5f), MIDDLE_RIGHT(1f,.5f), BOTTOM_LEFT(0f,1f), BOTTOM_CENTER(.5f,1f), BOTTOM_RIGHT(1f,1f); public float fx(); public float fy(); public static Anchor parse(String name, Anchor fallback); }`
  - `public final class ModuleConfig` mit Gettern/Settern für `enabled` (Standard false), `anchor` (TOP_LEFT), `offsetX`, `offsetY` (0), `scale` (1.0, Setter klemmt auf 0,5 bis 3,0), `background` (true), `shadow` (true), `textColor` (0xFFFFFFFF, `int`), `Object option(OptionSpec spec)` (über `spec.coerce`), `void setOption(String key, Object value)`; `public static ModuleConfig defaults()`; `public ModuleConfig copy()`.
  - `public final class Profile { public ModuleConfig module(String id) /* legt Standard an */; public Set<String> moduleIds(); public Profile copy(); public static Profile fromJson(Map<String,Object> json); public Map<String,Object> toJson(); }` JSON: `{"schema":1,"modules":{"fps":{"enabled":true,"anchor":"TOP_LEFT","x":2,"y":2,"scale":1.0,"background":true,"shadow":true,"textColor":4294967295,"options":{...}}}}`; `Json.write` kennt kein `Float`, daher schreibt `toJson` `float`-Werte (`scale` und `Float`-Optionen) als `Double.valueOf(Float.toString(f))` (1.3f wird zu `1.3`, nicht `1.2999999523162842`) und `textColor` als vorzeichenloses `Long` (`color & 0xFFFFFFFFL`); `fromJson` liest `textColor` mit `(int) JsonObjects.longValue(...)`; Einträge, die nicht gelesen werden konnten, bleiben roh erhalten und werden unverändert zurückgeschrieben.
  - `public final class ProfileStore { public static final String DEFAULT = "default"; public ProfileStore(Path dir, java.time.Clock clock, Consumer<String> warn); public List<String> names(); public Profile load(String name); public boolean loadedFromDefaults() /* true, wenn der letzte load() die Datei fehlend, leer oder kaputt (und zur Seite gelegt) vorfand */; public void save(String name, Profile p) throws IOException; public String createCopy(String sourceName) throws IOException; public void delete(String name) throws IOException; public static boolean isValidName(String name); }` Dateien `<dir>/<name>.json`. `names()` sortiert, enthält immer `default`. `createCopy` liest die Quelle von der Platte und vergibt `Profil 2`, `Profil 3`, ... (erste freie Zahl, Schreibweise nach Spec 5). `delete("default")` und ungültige Namen: `IllegalArgumentException`.
  - `public final class Settings { public Settings() /* activeProfile = "default" */; public String activeProfile(); public void setActiveProfile(String name); }`, `public final class SettingsStore { public SettingsStore(Path file, java.time.Clock clock, Consumer<String> warn); public Settings load(); public void save(Settings s) throws IOException; }` JSON `{"schema":1,"activeProfile":"default"}`; aktives Profil mit ungültigem Namen ergibt `default`.
  - `AtomicFiles`: `static void writeUtf8(Path target, String content) throws IOException` (in `<target>.tmp` schreiben, dann `Files.move(..., ATOMIC_MOVE, REPLACE_EXISTING)`, Rückfall ohne `ATOMIC_MOVE`), `static Path backupBroken(Path file, java.time.Clock clock) throws IOException` (Umbenennen nach `<name>.broken-yyyyMMdd-HHmmss`).

- [ ] **Step 1: Write the failing tests**

```java
@TempDir Path dir;
Clock clock = Clock.fixed(Instant.parse("2026-11-20T10:15:30Z"), ZoneOffset.UTC);
List<String> warnings = new ArrayList<>();
ProfileStore store() { return new ProfileStore(dir, clock, warnings::add); }

@Test void missing_profile_loads_defaults() { assertFalse(store().load("default").module("fps").enabled()); }
@Test void save_then_load_roundtrip() throws Exception {
    Profile p = new Profile(); ModuleConfig fps = p.module("fps");
    fps.setEnabled(true); fps.setAnchor(Anchor.TOP_RIGHT); fps.setOffsetX(-3); fps.setScale(1.5f);
    store().save("default", p);
    ModuleConfig back = store().load("default").module("fps");
    assertTrue(back.enabled()); assertEquals(Anchor.TOP_RIGHT, back.anchor()); assertEquals(-3, back.offsetX()); assertEquals(1.5f, back.scale());
}
@Test void load_broken_profile_backs_up_and_returns_defaults() throws Exception {
    Files.write(dir.resolve("default.json"), "{\"schema\":1,\"modules\":{".getBytes(UTF_8));
    assertFalse(store().load("default").module("fps").enabled());
    assertTrue(Files.exists(dir.resolve("default.json.broken-20261120-101530")));
    assertEquals(1, warnings.size());
}
@Test void load_empty_file_backs_up_and_returns_defaults() throws Exception {
    Files.write(dir.resolve("default.json"), new byte[0]);
    assertFalse(store().load("default").module("fps").enabled());
    assertTrue(Files.exists(dir.resolve("default.json.broken-20261120-101530")));
    assertEquals(1, warnings.size());
}
@Test void save_after_broken_load_writes_valid_file() throws Exception {
    Files.write(dir.resolve("default.json"), "{".getBytes(UTF_8));
    Profile p = store().load("default"); p.module("fps").setEnabled(true);
    store().save("default", p);
    assertTrue(store().load("default").module("fps").enabled());
    assertEquals(1, warnings.size());
}
@Test void loaded_from_defaults_flag() throws Exception {
    ProfileStore s = store();
    s.load("default"); assertTrue(s.loadedFromDefaults());
    s.save("default", new Profile());
    s.load("default"); assertFalse(s.loadedFromDefaults());
}
@Test void text_color_and_float_option_roundtrip() throws Exception {
    Profile p = new Profile(); ModuleConfig m = p.module("fps");
    m.setTextColor(0xFFFFAA00); m.setOption("opacity", 1.3f);
    store().save("default", p);
    String json = new String(Files.readAllBytes(dir.resolve("default.json")), UTF_8);
    assertTrue(json.contains("4294945280")); assertTrue(json.contains("1.3"));
    ModuleConfig back = store().load("default").module("fps");
    assertEquals(0xFFFFAA00, back.textColor());
    assertEquals(1.3f, (Float) back.option(OptionSpec.floatRange("opacity", 1f, 0.5f, 3f, 0.1f)), 1e-6f);
}
@Test void unknown_modules_survive_a_save() throws Exception {
    Files.write(dir.resolve("default.json"), "{\"schema\":1,\"modules\":{\"future\":{\"x\":[1,2]}}}".getBytes(UTF_8));
    Profile p = store().load("default"); p.module("fps").setEnabled(true); store().save("default", p);
    assertTrue(new String(Files.readAllBytes(dir.resolve("default.json")), UTF_8).contains("\"future\""));
}
@Test void wrong_types_fall_back_per_field() throws Exception {
    Files.write(dir.resolve("default.json"), "{\"schema\":1,\"modules\":{\"fps\":{\"enabled\":\"yes\",\"scale\":99,\"anchor\":\"NOWHERE\"}}}".getBytes(UTF_8));
    ModuleConfig c = store().load("default").module("fps");
    assertFalse(c.enabled()); assertEquals(3.0f, c.scale()); assertEquals(Anchor.TOP_LEFT, c.anchor());
}
@Test void save_overwrites_and_leaves_no_tmp() throws Exception {
    Files.write(dir.resolve("default.json.tmp"), "stale".getBytes(UTF_8));
    Profile p = new Profile(); p.module("fps").setEnabled(true);
    store().save("default", p);
    assertTrue(store().load("default").module("fps").enabled());
    store().save("default", new Profile());
    assertFalse(Files.exists(dir.resolve("default.json.tmp")));
    assertFalse(store().load("default").module("fps").enabled());
}
@Test void create_copy_names_and_delete_rules() throws Exception {
    assertEquals("Profil 2", store().createCopy("default"));
    assertEquals("Profil 3", store().createCopy("default"));
    assertEquals(Arrays.asList("Profil 2", "Profil 3", "default"), store().names());
    assertThrows(IllegalArgumentException.class, () -> store().delete("default"));
    store().delete("Profil 2");
    assertEquals(Arrays.asList("Profil 3", "default"), store().names());
}
@Test void name_validation() {
    assertTrue(ProfileStore.isValidName("PvP 1_a-b"));
    assertFalse(ProfileStore.isValidName("../x")); assertFalse(ProfileStore.isValidName(""));
    assertFalse(ProfileStore.isValidName("con.json"));
    assertTrue(ProfileStore.isValidName("a".repeat(32))); assertFalse(ProfileStore.isValidName("a".repeat(33)));
    for (String reserved : new String[] {"CON", "con", "NUL", "aux", "PRN", "com1", "COM9", "LPT1"}) assertFalse(ProfileStore.isValidName(reserved), reserved);
    assertTrue(ProfileStore.isValidName("console")); assertTrue(ProfileStore.isValidName("com10"));
    assertFalse(ProfileStore.isValidName(" a")); assertFalse(ProfileStore.isValidName("a ")); assertTrue(ProfileStore.isValidName("a b"));
}
```

`SettingsStoreTest` (gleiche Uhr und `warnings` wie oben, `store()` = `new SettingsStore(dir.resolve("settings.json"), clock, warnings::add)`):

```java
@Test void settings_roundtrip() throws Exception {
    Settings s = new Settings(); s.setActiveProfile("Profil 2"); store().save(s);
    assertEquals("Profil 2", store().load().activeProfile());
}
@Test void missing_settings_file_gives_default_without_warning() {
    assertEquals("default", store().load().activeProfile()); assertTrue(warnings.isEmpty());
}
@Test void broken_settings_file_backs_up_and_gives_default() throws Exception {
    Files.write(dir.resolve("settings.json"), "{".getBytes(UTF_8));
    assertEquals("default", store().load().activeProfile());
    assertTrue(Files.exists(dir.resolve("settings.json.broken-20261120-101530")));
    assertEquals(1, warnings.size());
}
@Test void invalid_active_profile_name_gives_default() throws Exception {
    Files.write(dir.resolve("settings.json"), "{\"schema\":1,\"activeProfile\":\"../x\"}".getBytes(UTF_8));
    assertEquals("default", store().load().activeProfile());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `./gradlew :common:test --tests '*StoreTest'`
Expected: FAIL.

- [ ] **Step 3: Implement die Typen aus dem Interfaces-Block**

Namensregel `[A-Za-z0-9 _-]{1,32}`, zusätzlich Windows-Gerätenamen (`con`, `prn`, `aux`, `nul`, `com1`-`com9`, `lpt1`-`lpt9`, ohne Rücksicht auf Groß/Klein, nur der ganze Name) verboten und kein führendes oder abschließendes Leerzeichen. `load` setzt das Flag `loadedFromDefaults`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `./gradlew :common:test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add mod/common
git commit -m "feat(mod-common): profiles and settings stored as json with atomic writes

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 5: Layout-Mathematik

**Files:**
- Create: `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/layout/{MutableRect,LayoutMath}.java`
- Test: `mod/common/src/test/java/io/github/clemensjl/hengst/mod/common/layout/LayoutMathTest.java`

**Interfaces:**
- Consumes: `Anchor`, `ModuleConfig` (Task 4).
- Produces:
  - `public final class MutableRect { public int x, y, w, h; public boolean contains(int px, int py); }`
  - `public static void LayoutMath.resolve(ModuleConfig cfg, int w, int h, int screenW, int screenH, MutableRect out)`: `w`, `h` sind bereits skaliert; `out.x = round(fx * (screenW - w)) + offsetX`, analog y; dann geklemmt auf `[0, max(0, screenW - w)]`. Allokiert nichts.
  - `public static Anchor LayoutMath.anchorFor(int centerX, int centerY, int screenW, int screenH)`: Bildschirmdrittel (links/mitte/rechts × oben/mitte/unten).
  - `public static void LayoutMath.reanchor(ModuleConfig cfg, int x, int y, int w, int h, int screenW, int screenH)`: setzt Anker aus der Modulmitte und den Versatz so, dass `resolve` danach genau `x`, `y` liefert.

- [ ] **Step 1: Write the failing tests**

```java
ModuleConfig cfg(Anchor a, int ox, int oy) { ModuleConfig c = ModuleConfig.defaults(); c.setAnchor(a); c.setOffsetX(ox); c.setOffsetY(oy); return c; }
@Test void top_right_is_flush_right() { MutableRect r = new MutableRect(); LayoutMath.resolve(cfg(Anchor.TOP_RIGHT, -2, 2), 40, 10, 320, 240, r); assertEquals(278, r.x); assertEquals(2, r.y); }
@Test void center_is_centered() { MutableRect r = new MutableRect(); LayoutMath.resolve(cfg(Anchor.CENTER, 0, 0), 40, 10, 320, 240, r); assertEquals(140, r.x); assertEquals(115, r.y); }
@Test void resolve_clamps_after_screen_shrinks() { MutableRect r = new MutableRect(); LayoutMath.resolve(cfg(Anchor.TOP_LEFT, 600, 400), 40, 10, 320, 240, r); assertEquals(280, r.x); assertEquals(230, r.y); }
@Test void resolve_wider_than_screen_starts_at_zero() { MutableRect r = new MutableRect(); LayoutMath.resolve(cfg(Anchor.TOP_RIGHT, 0, 0), 400, 10, 320, 240, r); assertEquals(0, r.x); }
@Test void anchor_follows_thirds() { assertEquals(Anchor.BOTTOM_RIGHT, LayoutMath.anchorFor(300, 230, 320, 240)); assertEquals(Anchor.CENTER, LayoutMath.anchorFor(160, 120, 320, 240)); }
@Test void reanchor_roundtrips_for_every_position_and_screen() {
    MutableRect r = new MutableRect();
    for (int x = 0; x <= 280; x += 7) for (int y = 0; y <= 230; y += 5) {
        ModuleConfig c = ModuleConfig.defaults();
        LayoutMath.reanchor(c, x, y, 40, 10, 320, 240);
        LayoutMath.resolve(c, 40, 10, 320, 240, r);
        assertEquals(x, r.x); assertEquals(y, r.y);
    }
}
@Test void corner_stays_at_corner_on_bigger_screen() {
    ModuleConfig c = ModuleConfig.defaults(); MutableRect r = new MutableRect();
    LayoutMath.reanchor(c, 278, 228, 40, 10, 320, 240);
    LayoutMath.resolve(c, 40, 10, 640, 480, r);
    assertEquals(598, r.x); assertEquals(468, r.y);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `./gradlew :common:test --tests '*LayoutMathTest'`
Expected: FAIL.

- [ ] **Step 3: Implement `MutableRect` und `LayoutMath`**

Runden mit `Math.round(float)`; `reanchor` rechnet den Versatz als `x - round(fx * (screenW - w))`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `./gradlew :common:test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add mod/common
git commit -m "feat(mod-common): anchored hud layout math

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 6: HUD-Modulbasis und allokationsfreier Renderer

**Files:**
- Create: `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/hud/{HudModule,TextHudModule,HudContext,HudRenderer,FrameCounter,CpsCounter}.java` (die beiden Zähler hier nur als Klassen mit Konstruktor `(TimeSource time)` und leeren Methodenkörpern, Logik in Task 7)
- Test: `mod/common/src/test/java/io/github/clemensjl/hengst/mod/common/hud/HudRendererTest.java`, `.../testutil/FakeCanvas.java`, `.../testutil/CountingCanvas.java`, `.../testutil/FakePlatform.java`, `.../testutil/FixedSizeModule.java`

**Interfaces:**
- Consumes: Plattform-Typen, `Feature`, `FeatureRegistry`, `FakeTime`, `TestFeature` (Task 3); `Profile`, `ModuleConfig` (Task 4); `LayoutMath`, `MutableRect` (Task 5).
- Produces:
  - `public abstract class HudModule extends Feature { protected HudModule(String id, int number, EnumSet<GameVersion> versions); public void tick(HudContext ctx, ModuleConfig cfg) {} public abstract int width(Canvas c, ModuleConfig cfg); public abstract int height(Canvas c, ModuleConfig cfg); public abstract void render(Canvas c, ModuleConfig cfg, HudContext ctx); }` (`render` zeichnet ab (0,0), Gruppe immer `HUD`; `tick` bekommt die Konfiguration des Moduls, damit Module Optionen wie `show_right` oder `decimals` zur Tick-Zeit lesen können).
  - `public abstract class TextHudModule extends HudModule` mit `protected TextHudModule(String id, int number, EnumSet<GameVersion> versions)`, `protected static final int PAD = 2`, `protected final void setText(String text)` (behält die Referenz, wenn `equals`, setzt sonst die zwischengespeicherte Breite zurück), `public final String text()` (öffentlich, weil Tests in anderen Paketen es lesen); `width = textWidth(text) + 2*PAD` (einmal je neuem Text berechnet), `height = fontHeight + 2*PAD`; `render` zeichnet Hintergrund `0x6F000000` nur bei `cfg.background()` und Text in `cfg.textColor()` mit `cfg.shadow()`.
  - `public final class FrameCounter { public FrameCounter(TimeSource time); public void onFrame(); public int fps(); }` und `public final class CpsCounter { public CpsCounter(TimeSource time); public void onPress(int button); public int cps(int button); }` (hier mit leeren Körpern, `fps()` und `cps()` liefern 0; Logik in Task 7).
  - `public final class HudContext { public HudContext(Platform platform, TimeSource time, FrameCounter frames, CpsCounter cps); public Platform platform(); public TimeSource time(); public FrameCounter frames(); public CpsCounter cps(); }`
  - `public final class HudRenderer { public HudRenderer(FeatureRegistry registry, GameVersion version); public void rebuild(Profile profile); public int activeCount(); public HudModule[] activeModules(); public ModuleConfig[] activeConfigs(); public void tick(HudContext ctx); public void render(Canvas c, HudContext ctx); }`: `tick` ruft `activeModules()[i].tick(ctx, activeConfigs()[i])`. `rebuild` füllt zwei parallele Arrays mit den eingeschalteten `HudModule`, die `supports(version)` erfüllen, in Nummernreihenfolge. `render` je Modul: Breite/Höhe mal `scale` (gerundet), `LayoutMath.resolve` in ein Feld-`MutableRect`, `c.pushTransform(rect.x, rect.y, scale)`, `module.render`, `c.popTransform()`.
  - Test-Hilfen (alle in `testutil`, öffentlich, hier definiert und von Task 7 bis 10 wiederverwendet): `FakeCanvas implements Canvas` (320 × 240, `fontHeight 9`, `textWidth = 6 * length`, protokolliert Aufrufe in `public final List<String> calls` im Format `"fill x1 y1 x2 y2 aarrggbb"` (ARGB als 8 Hex-Ziffern klein), `"text <s> x y aarrggbb <shadow>"`, `"push tx ty s"` (Floats mit `String.valueOf`), `"pop"`), `CountingCanvas implements Canvas` (gleiche Maße, zählt nur `int`-Felder, allokiert nichts), `FakePlatform implements Platform, GameView, InputView` (`game()` und `input()` liefern `this`) mit Konstruktor `public FakePlatform(Path configDir)` und diesen öffentlichen Feldern: `GameVersion version = GameVersion.V1_8_9`, `boolean inWorld, hudHidden, debugOverlayOpen, screenOpen`, `double playerX, playerY, playerZ`, `float playerYaw`, `String serverAddress` (null), `String languageCode = "en_us"`, `Set<GameKey> down = EnumSet.noneOf(GameKey.class)` (`isDown(k)` = `down.contains(k)`), `Map<GameKey,String> labels = new EnumMap<>(GameKey.class)` (`keyLabel(k)` = `labels.getOrDefault(k, k.name())`), `List<String> warnings` (`logWarning` hängt an), `UiScreen openedScreen` (`openScreen(s)` setzt `openedScreen = s` und `screenOpen = true`), `int closeCount` (`closeScreen()` setzt `openedScreen = null`, `screenOpen = false` und zählt hoch), `Path configDir` (aus dem Konstruktor).
  - `FixedSizeModule extends HudModule` mit `public FixedSizeModule(String id, int number, int width, int height)` und `public FixedSizeModule(String id, int number, int width, int height, EnumSet<GameVersion> versions)` (der kürzere Konstruktor unterstützt alle Versionen); `width(c, cfg)` und `height(c, cfg)` liefern die festen Werte, `render` füllt `c.fill(0, 0, width, height, 0xFFFFFFFF)`, `tick` tut nichts.

- [ ] **Step 1: Write the failing tests**

```java
@TempDir Path dir;
FakePlatform platform; FakeTime time; HudContext ctx;
@BeforeEach void setUp() {
    platform = new FakePlatform(dir); time = new FakeTime();
    ctx = new HudContext(platform, time, new FrameCounter(time), new CpsCounter(time));
}
static final class Fixed extends TextHudModule {
    Fixed(String id, int nr) { super(id, nr, EnumSet.allOf(GameVersion.class)); }
    @Override public void tick(HudContext ctx, ModuleConfig cfg) { setText("Hello"); }
}
@Test void rebuild_takes_only_enabled_and_supported_in_number_order() {
    FeatureRegistry reg = new FeatureRegistry(); reg.register(new Fixed("b", 6)); reg.register(new Fixed("a", 1));
    Profile p = new Profile(); p.module("a").setEnabled(true); p.module("b").setEnabled(true);
    HudRenderer r = new HudRenderer(reg, GameVersion.V1_8_9); r.rebuild(p);
    assertEquals(2, r.activeCount()); assertEquals("a", r.activeModules()[0].id());
    p.module("a").setEnabled(false); r.rebuild(p); assertEquals(1, r.activeCount());
}
@Test void render_places_scaled_module_at_resolved_rect() {
    // Modul "a" TOP_RIGHT, Versatz -2/2, Skalierung 2: Breite (5*6+4)*2 = 68, x = 320-68-2 = 250
    FeatureRegistry reg = new FeatureRegistry(); reg.register(new Fixed("a", 1));
    Profile p = new Profile(); ModuleConfig a = p.module("a");
    a.setEnabled(true); a.setAnchor(Anchor.TOP_RIGHT); a.setOffsetX(-2); a.setOffsetY(2); a.setScale(2f);
    HudRenderer r = new HudRenderer(reg, GameVersion.V1_8_9); r.rebuild(p);
    FakeCanvas c = new FakeCanvas();
    r.tick(ctx); r.render(c, ctx);
    assertEquals("push 250.0 2.0 2.0", c.calls.get(0));
    assertEquals("pop", c.calls.get(c.calls.size() - 1));
}
@Test void text_reference_kept_when_equal() {
    Fixed m = new Fixed("a", 1); ModuleConfig cfg = ModuleConfig.defaults();
    m.tick(ctx, cfg); String first = m.text(); m.tick(ctx, cfg); assertSame(first, m.text());
    assertEquals("Hello", m.text()); assertEquals(34, m.width(new FakeCanvas(), cfg)); assertEquals(13, m.height(new FakeCanvas(), cfg));
}
@Test void hud_render_allocates_nothing_in_steady_state() {
    // 4 Module aktiv, CountingCanvas, 200 Frames aufwärmen, dann 1000 Frames messen (compileTestJava läuft mit release 17)
    FeatureRegistry reg = new FeatureRegistry(); Profile p = new Profile();
    for (int i = 1; i <= 4; i++) { reg.register(new Fixed("m" + i, i)); p.module("m" + i).setEnabled(true); }
    HudRenderer renderer = new HudRenderer(reg, GameVersion.V1_8_9); renderer.rebuild(p);
    CountingCanvas counting = new CountingCanvas();
    for (int i = 0; i < 200; i++) { renderer.tick(ctx); renderer.render(counting, ctx); }
    com.sun.management.ThreadMXBean mx = (com.sun.management.ThreadMXBean) ManagementFactory.getThreadMXBean();
    long id = Thread.currentThread().getId();
    long before = mx.getThreadAllocatedBytes(id);
    for (int i = 0; i < 1000; i++) renderer.render(counting, ctx);
    long allocated = mx.getThreadAllocatedBytes(id) - before;
    assertTrue(allocated < 1024, "allocated " + allocated + " bytes in 1000 frames");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `./gradlew :common:test --tests '*HudRendererTest'`
Expected: FAIL.

- [ ] **Step 3: Implement die Typen aus dem Interfaces-Block**

Keine Schleife mit Iterator, kein Autoboxing im `render`-Pfad (`scale` als `float` aus `ModuleConfig`, nicht über `option(...)`).

- [ ] **Step 4: Run tests to verify they pass**

Run: `./gradlew :common:test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add mod/common
git commit -m "feat(mod-common): hud module base and allocation-free renderer

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 7: Zähler und die vier M0-Module (FPS, CPS, Keystrokes, Koordinaten)

**Files:**
- Modify: `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/hud/FrameCounter.java`, `CpsCounter.java`
- Create: `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/hud/modules/{FpsModule,CpsModule,KeystrokesModule,CoordinatesModule}.java`
- Test: `mod/common/src/test/java/io/github/clemensjl/hengst/mod/common/hud/CountersTest.java`, `.../hud/modules/ModulesTest.java`

**Interfaces:**
- Consumes: `HudModule`, `TextHudModule` (`tick(HudContext, ModuleConfig)`, öffentliches `text()`), `HudContext` (Task 6); `OptionSpec`, `ModuleConfig.option` (Tasks 3, 4); `FakeTime` (Task 3); `FakeCanvas`, `FakePlatform` (Task 6).
- Produces:
  - `public FrameCounter(TimeSource time); public void onFrame(); public int fps();` Das erste Fenster beginnt beim Konstruktor. `onFrame` prüft zuerst: sind seit Fensterbeginn ≥ 1 s vergangen, dann `fps = round(frames * 1e9 / elapsedNanos)`, `frames = 0`, Fensterbeginn = jetzt; danach `frames++`. Vor dem ersten vollen Fenster ist `fps()` 0.
  - `public CpsCounter(TimeSource time); public static final int CAPACITY = 64; public void onPress(int button); public int cps(int button);` nur Knopf 0 und 1, andere ignoriert (`cps` liefert für andere Knöpfe 0); Ringpuffer `long[CAPACITY]` je Knopf; `cps` zählt Zeitstempel jünger als 1 s, höchstens `CAPACITY`.
  - Alle vier Module haben einen öffentlichen Konstruktor ohne Argumente, unterstützen alle Versionen und lesen ihre Optionen im Tick über `cfg.option(OptionSpec)` (`tick(HudContext ctx, ModuleConfig cfg)`, Task 6).
  - `FpsModule` (`"fps"`, 1): Text `"<n> FPS"`, Neuberechnung im Tick.
  - `CpsModule` (`"cps"`, 2): Option `toggle("show_right", true)`; Text `"<l> | <r> CPS"` bzw. `"<l> CPS"`.
  - `KeystrokesModule` (`"keystrokes"`, 3): Optionen `toggle("show_mouse", true)`, `toggle("show_space", true)`; Raster aus Tasten 18 × 18 px, Abstand 2 px; Zeilen W / A S D / LMB RMB (je 28 × 18) / Leertaste (58 × 10); Breite 58, Höhe 38 + (show_mouse ? 20 : 0) + (show_space ? 12 : 0); gedrückt: Fläche `0xB0FFFFFF`, Text `0xFF000000`, sonst `0x6F000000` und `cfg.textColor()`; liest `InputView.isDown` je Frame, Beschriftungen aus `InputView.keyLabel` alle 20 Ticks; Maus-Zellen zeigen `"LMB"`/`"RMB"`, darunter nichts weiter.
  - `CoordinatesModule` (`"coordinates"`, 6): Option `intRange("decimals", 0, 0, 2)`; Text `"X 12  Y 64  Z -30"` (zwei Leerzeichen zwischen den Achsen), bei 0 Nachkommastellen `Math.floor` wie die Blockposition, sonst auf die Stellen abgeschnitten (nicht gerundet); neuer String nur, wenn sich einer der drei skalierten `long`-Werte geändert hat (wiederverwendeter `StringBuilder`); außerhalb einer Welt leerer Text und Breite 0 (`width` überschreibt die Basis und liefert 0, wenn der Text leer ist); beim Abschneiden steht das Vorzeichen getrennt von der Ganzzahl (-0,5 ergibt bei 2 Stellen `-0.50`).

- [ ] **Step 1: Write the failing tests**

```java
@TempDir Path dir;
FakePlatform platform; FakeTime time; HudContext ctx; ModuleConfig cfg;
@BeforeEach void setUp() {
    platform = new FakePlatform(dir); time = new FakeTime(); cfg = ModuleConfig.defaults();
    ctx = new HudContext(platform, time, new FrameCounter(time), new CpsCounter(time));
}
@Test void fps_counts_frames_per_second() {
    FakeTime t = new FakeTime(); FrameCounter f = new FrameCounter(t);
    for (int i = 0; i < 120; i++) { f.onFrame(); t.advanceMillis(5); }   // 600 ms
    assertEquals(0, f.fps());
    for (int i = 0; i < 80; i++) { f.onFrame(); t.advanceMillis(5); }    // 1000 ms gesamt
    f.onFrame(); assertEquals(200, f.fps());
}
@Test void cps_counts_presses_in_window() {
    FakeTime t = new FakeTime(); CpsCounter c = new CpsCounter(t);
    for (int i = 0; i < 10; i++) { c.onPress(0); t.advanceMillis(50); }
    assertEquals(10, c.cps(0)); assertEquals(0, c.cps(1));
}
@Test void cps_drops_to_zero_after_window() {
    FakeTime t = new FakeTime(); CpsCounter c = new CpsCounter(t); c.onPress(1); t.advanceMillis(1001); assertEquals(0, c.cps(1));
}
@Test void cps_caps_at_buffer_size() {
    FakeTime t = new FakeTime(); CpsCounter c = new CpsCounter(t);
    for (int i = 0; i < 100; i++) { c.onPress(0); t.advanceMillis(5); }
    assertEquals(CpsCounter.CAPACITY, c.cps(0));
}
@Test void cps_ignores_other_buttons() {
    CpsCounter c = new CpsCounter(new FakeTime());
    c.onPress(2); c.onPress(-1);
    assertEquals(0, c.cps(0)); assertEquals(0, c.cps(1)); assertEquals(0, c.cps(2));
}
@Test void coordinates_floor_and_reuse_text() {
    platform.playerX = -0.5; platform.playerY = 64.9; platform.playerZ = 12.0; platform.inWorld = true;
    CoordinatesModule coords = new CoordinatesModule();
    coords.tick(ctx, cfg); assertEquals("X -1  Y 64  Z 12", coords.text());
    String first = coords.text(); platform.playerX = -0.7; coords.tick(ctx, cfg); assertSame(first, coords.text());
}
@Test void coordinates_with_two_decimals_truncate() {
    platform.inWorld = true; platform.playerX = 1.239; platform.playerY = 64.0; platform.playerZ = -0.5;
    cfg.setOption("decimals", 2);
    CoordinatesModule coords = new CoordinatesModule();
    coords.tick(ctx, cfg);
    assertEquals("X 1.23  Y 64.00  Z -0.50", coords.text());
}
@Test void cps_text_with_and_without_right() {
    for (int i = 0; i < 3; i++) ctx.cps().onPress(0);
    ctx.cps().onPress(1);
    CpsModule cps = new CpsModule();
    cps.tick(ctx, cfg); assertEquals("3 | 1 CPS", cps.text());
    cfg.setOption("show_right", Boolean.FALSE);
    cps.tick(ctx, cfg); assertEquals("3 CPS", cps.text());
}
@Test void keystrokes_size_follows_options() {
    KeystrokesModule k = new KeystrokesModule(); FakeCanvas c = new FakeCanvas();
    assertEquals(58, k.width(c, cfg)); assertEquals(70, k.height(c, cfg));
    cfg.setOption("show_mouse", Boolean.FALSE); cfg.setOption("show_space", Boolean.FALSE);
    assertEquals(58, k.width(c, cfg)); assertEquals(38, k.height(c, cfg));
}
@Test void keystrokes_draws_pressed_key_highlighted() {
    platform.down.add(GameKey.FORWARD);
    KeystrokesModule k = new KeystrokesModule(); FakeCanvas c = new FakeCanvas();
    k.tick(ctx, cfg); k.render(c, cfg, ctx);
    assertTrue(c.calls.contains("fill 20 0 38 18 b0ffffff"));   // W gedrückt
    assertTrue(c.calls.contains("fill 0 20 18 38 6f000000"));    // A nicht gedrückt
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `./gradlew :common:test --tests '*CountersTest' --tests '*ModulesTest'`
Expected: FAIL.

- [ ] **Step 3: Implement Zähler und Module wie im Interfaces-Block**

Abschneiden mit Nachkommastellen: skaliert als `long v = (long) Math.floor(value * 10^d)`, Ausgabe mit Vorzeichen und `d` Stellen ohne `String.format`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `./gradlew :common:test`
Expected: PASS, inklusive `hud_render_allocates_nothing_in_steady_state` aus Task 6, jetzt mit den vier echten Modulen statt der Test-Module (Test entsprechend auf echte Module umstellen).

- [ ] **Step 5: Commit**

```bash
git add mod/common
git commit -m "feat(mod-common): fps, cps, keystrokes and coordinates modules

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 8: HUD-Editor

**Files:**
- Create: `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/editor/{HudEditorModel,HudEditorScreen}.java`
- Test: `mod/common/src/test/java/io/github/clemensjl/hengst/mod/common/editor/HudEditorModelTest.java`, `HudEditorScreenTest.java`

**Interfaces:**
- Consumes: `HudRenderer.activeModules()/activeConfigs()`, `HudContext`, `FakeCanvas`, `FakePlatform`, `FixedSizeModule` (Task 6); `LayoutMath`, `MutableRect` (Task 5); `UiScreen`, `UiKey`, `Canvas`, `FakeTime` (Task 3); `I18n` (Task 3).
- Produces:
  - `public final class HudEditorModel { public static final int SNAP = 4; public HudEditorModel(HudModule[] modules, ModuleConfig[] configs); public void layout(Canvas c); public MutableRect rect(int index); public int hit(int mx, int my); public void press(int mx, int my); public void drag(int mx, int my); public void release(); public boolean dragging(); public void scroll(double amount); public void nudge(int dx, int dy); public void hideSelected(); public int selected(); public boolean dirty(); }`
    - `hit`: höchster Index, dessen Rechteck den Punkt enthält, sonst -1. `press` wählt aus (oder hebt Auswahl auf) und beginnt Ziehen. `drag` verschiebt um die Mausdifferenz, rastet ein, wenn eine Kante oder die Mitte des Moduls höchstens `SNAP` px von Bildschirmrand (0, Breite), Bildschirmmitte oder einer Kante eines anderen Moduls entfernt ist, klemmt in den Bildschirm und schreibt über `LayoutMath.reanchor` in die Konfiguration. `scroll(+1)` erhöht `scale` des ausgewählten Moduls um 0,1 (Grenzen 0,5 bis 3,0, Ankerpunkt bleibt); ohne Auswahl tut `scroll` nichts und setzt `dirty` nicht. Das Vorzeichen von `amount` bestimmt die Richtung, die Größe nicht (Adapter reichen `Integer.signum` des Rad-Deltas durch). `nudge` verschiebt das ausgewählte Modul. `hideSelected` setzt `enabled=false`. Jede Änderung setzt `dirty`.
  - `public final class HudEditorScreen implements UiScreen { public HudEditorScreen(HudEditorModel model, HudContext ctx, I18n i18n, Runnable onClose); }`: zeichnet abgedunkelten Hintergrund `0x50000000`, alle Module, Rahmen `0x80FFFFFF`, Auswahl `0xFFFFAA00`, Hinweiszeile `hengst.editor.hint` unten mittig. Tasten: Pfeile → `nudge(±1)` bzw. `±10` mit Umschalt; `DELETE` → `hideSelected`; `ESCAPE` → Ziehen beenden, `onClose.run()` genau einmal. Mausrad → `scroll`. `closed()` ruft `onClose` nur, wenn es noch nicht lief. Der Bildschirm schließt sich nicht selbst: `onClose` ist der Rückruf von `HengstCore` (speichern, Renderer neu bauen, `Platform.closeScreen()`, Task 10).

- [ ] **Step 1: Write the failing tests**

```java
// zwei Module a und b (je 40x10, FixedSizeModule), Bildschirm 320x240 (FakeCanvas)
@TempDir Path dir;
FakeCanvas canvas = new FakeCanvas();
HudModule[] modules; ModuleConfig[] configs; HudEditorModel model; int closeCount;
@BeforeEach void setUp() {
    modules = new HudModule[] { new FixedSizeModule("a", 1, 40, 10), new FixedSizeModule("b", 2, 40, 10) };
    configs = new ModuleConfig[] { ModuleConfig.defaults(), ModuleConfig.defaults() };
    for (ModuleConfig c : configs) c.setEnabled(true);
    configs[1].setOffsetX(250); configs[1].setOffsetY(100);   // a: x 0..40, y 0..10; b: x 250..290, y 100..110
    model = new HudEditorModel(modules, configs); model.layout(canvas);
}
@Test void hit_prefers_topmost() {
    configs[1].setOffsetX(20); configs[1].setOffsetY(0); model.layout(canvas);   // a: 0..40, b: 20..60, überlappen
    assertEquals(1, model.hit(30, 5)); assertEquals(0, model.hit(5, 5)); assertEquals(-1, model.hit(300, 200));
}
@Test void drag_moves_and_reanchors() {
    model.press(5, 5); model.drag(305, 235); model.release();
    assertEquals(Anchor.BOTTOM_RIGHT, configs[0].anchor()); model.layout(canvas);
    assertEquals(280, model.rect(0).x); assertEquals(230, model.rect(0).y); assertTrue(model.dirty());
}
@Test void drag_snaps_to_screen_center() {
    model.press(5, 5); model.drag(148, 5); model.release();   // roh x = 143, Mitte 163, 3 px neben 160
    model.layout(canvas);
    assertEquals(140, model.rect(0).x); assertEquals(0, model.rect(0).y);
}
@Test void drag_snaps_to_other_module_edge() {
    configs[1].setOffsetX(60); configs[1].setOffsetY(100); model.layout(canvas);   // b: x 60..100, y 100..110
    model.press(5, 5); model.drag(107, 105); model.release();   // roh x = 102 (2 px rechts von b), y = 100
    model.layout(canvas);
    assertEquals(100, model.rect(0).x); assertEquals(100, model.rect(0).y);
}
@Test void drag_outside_screen_is_clamped() { model.press(5, 5); model.drag(-500, 9000); model.release(); model.layout(canvas); assertEquals(0, model.rect(0).x); assertEquals(230, model.rect(0).y); }
@Test void scroll_scales_within_limits() {
    model.press(5, 5); model.release();
    model.scroll(1); assertEquals(1.1f, configs[0].scale(), 1e-6f);
    for (int i = 0; i < 30; i++) model.scroll(1);
    assertEquals(3.0f, configs[0].scale());
    for (int i = 0; i < 40; i++) model.scroll(-1);
    assertEquals(0.5f, configs[0].scale());
}
@Test void scroll_without_selection_does_nothing() {
    model.scroll(1);
    assertEquals(1.0f, configs[0].scale()); assertEquals(1.0f, configs[1].scale()); assertFalse(model.dirty());
}
@Test void nudge_and_hide() {
    model.press(5, 5); model.release();
    model.nudge(10, 0); model.layout(canvas);
    assertEquals(10, model.rect(0).x); assertEquals(0, model.rect(0).y); assertTrue(model.dirty());
    model.hideSelected();
    assertFalse(configs[0].enabled()); assertTrue(configs[1].enabled());
}
@Test void press_on_empty_area_clears_selection() {
    model.press(5, 5); model.release(); assertEquals(0, model.selected());
    model.press(200, 200); assertFalse(model.dragging()); assertEquals(-1, model.selected());
}
@Test void escape_during_drag_ends_drag_and_closes() {
    FakeTime t = new FakeTime();
    HudContext ctx = new HudContext(new FakePlatform(dir), t, new FrameCounter(t), new CpsCounter(t));
    HudEditorScreen screen = new HudEditorScreen(model, ctx, I18n.load("en_us"), () -> closeCount++);
    screen.render(canvas, 0, 0, 0f);
    screen.mouseClicked(5, 5, 0); screen.mouseDragged(50, 50, 0);
    assertTrue(model.dragging());
    assertTrue(screen.keyPressed(UiKey.ESCAPE, false, false));
    assertFalse(model.dragging()); assertEquals(1, closeCount); screen.closed(); assertEquals(1, closeCount);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `./gradlew :common:test --tests '*HudEditor*'`
Expected: FAIL.

- [ ] **Step 3: Implement `HudEditorModel` und `HudEditorScreen`**

Einrasten: getrennt für x und y, jeweils der nächste Kandidat innerhalb `SNAP`; Kandidaten für x: 0, Bildschirmbreite, Bildschirmmitte, linke/rechte Kanten anderer sichtbarer Module; verglichen werden linke Kante, Mitte und rechte Kante des gezogenen Moduls.

- [ ] **Step 4: Run tests to verify they pass**

Run: `./gradlew :common:test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add mod/common
git commit -m "feat(mod-common): hud editor with drag, snap, scale and nudge

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 9: Einstellungsbildschirm

**Files:**
- Create: `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/ui/{Widget,ToggleWidget,SliderWidget,ChoiceWidget,ButtonWidget,ScrollList,SettingsModel,SettingsScreen}.java`
- Test: `mod/common/src/test/java/io/github/clemensjl/hengst/mod/common/ui/WidgetsTest.java`, `SettingsScreenTest.java`

**Interfaces:**
- Consumes: `FeatureRegistry`, `OptionSpec` (mit `min()/max()/step()/values()`), `I18n`, `UiScreen`, `UiKey`, `Canvas`, `TestFeature` (Task 3); `Profile`, `ProfileStore`, `Settings`, `ModuleConfig` (Task 4); `FakeCanvas` (Task 6); `FpsModule` (Task 7).
- Produces:
  - `public abstract class Widget { public int x, y, w, h; public abstract void render(Canvas c, int mx, int my); public boolean mouseClicked(int mx, int my, int button) { return false; } /* true: Klick innerhalb des Widgets verarbeitet */ public boolean mouseDragged(int mx, int my) { return false; } public void mouseReleased() {} }`
  - `ToggleWidget(String label, BooleanSupplier get, Consumer<Boolean> set)` (Klick schaltet um); `SliderWidget(String label, float min, float max, float step, DoubleSupplier get, DoubleConsumer set)` (Klick und Ziehen setzen den Wert aus der x-Position, auf `step` gerundet, geklemmt); `ChoiceWidget(String label, String[] values, Supplier<String> get, Consumer<String> set)` (Links weiter, Rechts zurück, zyklisch); `ButtonWidget(String label, Runnable action)`.
  - `public final class ScrollList { public ScrollList(int x, int y, int w, int h); public void setWidgets(List<Widget> widgets, int rowHeight); public void scroll(double amount); public int offset(); /* render, mouseClicked, mouseDragged, mouseReleased delegieren mit Versatz */ }` 12 px je Rastung, Versatz geklemmt auf `[0, max(0, inhalt - h)]`.
  - `public final class SettingsModel { public SettingsModel(FeatureRegistry registry, GameVersion version, ProfileStore profiles, Settings settings, Supplier<Profile> activeProfile, Consumer<String> switchProfile, Runnable changed, Runnable openHudEditor, Runnable saveNow, Consumer<String> warn); public List<Feature> features(); public Feature selected(); public void select(String id); public ModuleConfig config(String id); public List<String> profiles(); public String activeProfileName(); public void switchTo(String name); public String copyActive(); public void deleteActive(); public void openHudEditor(); }` `saveNow` speichert das aktive Profil sofort (HengstCore: Profil schreiben, `dirty` zurücksetzen), `warn` nimmt Fehlermeldungen entgegen (HengstCore: `platform.logWarning`). `copyActive` ruft zuerst `saveNow.run()` (der Store kopiert von der Platte, ungespeicherte Änderungen würden sonst fehlen), dann `profiles.createCopy(activeName)`, dann `switchProfile.accept(neuerName)`, und gibt den neuen Namen zurück; `deleteActive` und `copyActive` fangen `IOException`, melden sie über `warn.accept(e.getMessage())` und lassen das aktive Profil unverändert (`copyActive` gibt dann den bisherigen Namen zurück). `deleteActive` auf `default` tut nichts; nach dem Löschen ist `default` aktiv (`switchProfile.accept("default")`). `config(id)` liefert `activeProfile.get().module(id)`; `activeProfileName()` ist `settings.activeProfile()`.
  - `public final class SettingsScreen implements UiScreen { public SettingsScreen(SettingsModel model, I18n i18n, Runnable onClose); }`: links `ScrollList` mit einer `ToggleWidget` je Feature (Name aus `hengst.feature.<id>`), Klick auf den Namen wählt es aus (M0-Abweichung von Spec 6, bewusst: die vier M0-Features liegen alle in der Gruppe HUD, die Liste ist deshalb flach; Gruppenüberschriften aus `hengst.group.*` kommen mit der ersten zweiten Gruppe, spätestens M1); rechts Optionen des gewählten Features: `background`, `shadow`, Textfarbe als `ChoiceWidget` über 8 feste Farben (Namen aus `hengst.color.white|yellow|lime|aqua|red|orange|gray|black`, ARGB in dieser Reihenfolge `0xFFFFFFFF, 0xFFFFFF55, 0xFF55FF55, 0xFF55FFFF, 0xFFFF5555, 0xFFFFAA00, 0xFFAAAAAA, 0xFF000000`; eine Profilfarbe außerhalb der Tabelle zeigt den ersten Eintrag, ohne den Wert zu ändern), dann je `OptionSpec` das passende Widget; oben Profil-`ChoiceWidget` und Knöpfe Neu (Kopie), Löschen, HUD bearbeiten; unten `hengst.notice` und Knopf Fertig. Esc oder Fertig: `onClose.run()` genau einmal (das Schließen übernimmt `HengstCore`, Task 10). Jede Änderung ruft `changed`.

- [ ] **Step 1: Write the failing tests**

```java
@Test void toggle_flips_on_click_inside_only() {
    boolean[] v = {false};
    ToggleWidget t = new ToggleWidget("T", () -> v[0], b -> v[0] = b);
    t.x = 10; t.y = 10; t.w = 100; t.h = 20;
    assertFalse(t.mouseClicked(5, 15, 0)); assertFalse(v[0]);
    assertTrue(t.mouseClicked(50, 15, 0)); assertTrue(v[0]);
    assertTrue(t.mouseClicked(50, 15, 0)); assertFalse(v[0]);
}
@Test void slider_maps_position_and_snaps() {
    // Wert = min + (mx - x) / w * (max - min), dann n = Math.round((Wert - min) / step) auf double, Ergebnis min + n * step
    double[] v = {1.0};
    SliderWidget s = new SliderWidget("S", 0.5f, 3.0f, 0.1f, () -> v[0], d -> v[0] = d);
    s.x = 10; s.y = 10; s.w = 100; s.h = 20;
    assertTrue(s.mouseClicked(62, 15, 0)); assertEquals(1.8, v[0], 1e-5);   // 0.5 + 0.52 * 2.5 = 1.8, kein Rundungsgrenzfall
    assertTrue(s.mouseDragged(500, 15)); assertEquals(3.0, v[0], 1e-9);     // über den rechten Rand geklemmt
    s.mouseReleased();
    assertFalse(s.mouseDragged(20, 15)); assertEquals(3.0, v[0], 1e-9);     // nach dem Loslassen kein Ziehen mehr
}
@Test void choice_cycles_both_directions() {
    String[] v = {"c"};
    ChoiceWidget w = new ChoiceWidget("C", new String[] {"a", "b", "c"}, () -> v[0], s -> v[0] = s);
    w.x = 0; w.y = 0; w.w = 100; w.h = 20;
    assertTrue(w.mouseClicked(5, 5, 0)); assertEquals("a", v[0]);   // Linksklick: weiter, von c zyklisch nach a
    assertTrue(w.mouseClicked(5, 5, 1)); assertEquals("c", v[0]);   // Rechtsklick: zurück, von a zyklisch nach c
}
@Test void scroll_list_clamps_offset() {
    ScrollList l = new ScrollList(0, 0, 200, 100);
    List<Widget> ws = new ArrayList<>();
    for (int i = 0; i < 20; i++) ws.add(new ButtonWidget("b" + i, () -> {}));
    l.setWidgets(ws, 20);
    l.scroll(-100); assertEquals(300, l.offset());   // Inhalt 400, Höhe 100
    l.scroll(100); assertEquals(0, l.offset());
    l.scroll(-1); assertEquals(12, l.offset());       // 12 px je Rastung
}
// gemeinsamer Aufbau der Klasse SettingsScreenTest
@TempDir Path dir;
Clock clock = Clock.fixed(Instant.parse("2026-11-20T10:15:30Z"), ZoneOffset.UTC);
List<String> warnings = new ArrayList<>();
ProfileStore store; Settings settings; Profile[] active = { new Profile() };
int changed, editorOpened, closeCount;
FakeCanvas canvas = new FakeCanvas();
SettingsModel model; SettingsScreen screen;
FeatureRegistry reg() {
    FeatureRegistry reg = new FeatureRegistry();
    reg.register(new FpsModule()); reg.register(new TestFeature("modernonly", 20, EnumSet.of(GameVersion.V26_3)));
    return reg;
}
@BeforeEach void setUp() {
    store = new ProfileStore(dir, clock, warnings::add); settings = new Settings();
    model = new SettingsModel(reg(), GameVersion.V1_8_9, store, settings, () -> active[0],
        name -> { settings.setActiveProfile(name); active[0] = store.load(name); },
        () -> changed++, () -> editorOpened++,
        () -> { try { store.save(settings.activeProfile(), active[0]); } catch (IOException e) { throw new UncheckedIOException(e); } },
        warnings::add);
    screen = new SettingsScreen(model, I18n.load("en_us"), () -> closeCount++);
}
@Test void features_list_shows_only_supported() {
    assertEquals(Arrays.asList("fps"), TestFeature.ids(model.features()));
}
@Test void toggling_feature_marks_changed() {
    screen.render(canvas, 0, 0, 0f);
    assertFalse(model.config("fps").enabled());
    assertTrue(screen.mouseClicked(12, 46, 0));   // erste Listenzeile (y 36..56), Schaltkästchen x 8..28
    assertTrue(model.config("fps").enabled()); assertEquals(1, changed);
}
@Test void copy_and_delete_profile() {
    assertEquals("Profil 2", model.copyActive());
    assertEquals("Profil 2", model.activeProfileName()); assertTrue(model.profiles().contains("Profil 2"));
    model.deleteActive();
    assertEquals("default", model.activeProfileName()); assertFalse(model.profiles().contains("Profil 2"));
    model.deleteActive();
    assertEquals("default", model.activeProfileName()); assertTrue(model.profiles().contains("default"));
}
@Test void copy_includes_unsaved_changes() throws Exception {
    model.config("fps").setEnabled(true);   // nur im Speicher, noch nicht auf der Platte
    String copy = model.copyActive();
    assertTrue(store.load(copy).module("fps").enabled());
}
@Test void copy_io_failure_goes_to_warn_hook() throws Exception {
    Path blocked = dir.resolve("blocked"); Files.write(blocked, new byte[0]);   // eine Datei statt eines Ordners
    ProfileStore broken = new ProfileStore(blocked, clock, s -> {});
    SettingsModel m = new SettingsModel(reg(), GameVersion.V1_8_9, broken, settings, () -> active[0],
        n -> {}, () -> {}, () -> {}, () -> {}, warnings::add);
    assertEquals("default", m.copyActive());
    assertEquals(1, warnings.size());
}
@Test void escape_closes_once() {
    assertTrue(screen.keyPressed(UiKey.ESCAPE, false, false));
    screen.closed();
    assertEquals(1, closeCount);
}
@Test void notice_is_drawn() {
    screen.render(canvas, 0, 0, 0f);
    String notice = I18n.load("en_us").get("hengst.notice");
    assertTrue(canvas.calls.stream().anyMatch(c -> c.startsWith("text " + notice + " ")));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `./gradlew :common:test --tests '*WidgetsTest' --tests '*SettingsScreenTest'`
Expected: FAIL.

- [ ] **Step 3: Implement Widgets, `ScrollList`, `SettingsModel`, `SettingsScreen`**

Layout ab Bildschirmgröße aus `Canvas` beim ersten `render` und bei Größenänderung neu; linke Spalte 40 % der Breite, Zeilenhöhe 20, Rand 8. Festgelegt (die Tests klicken darauf): Kopfzeile mit Profilwahl und Knöpfen bei y = 8 (Höhe 20), die Feature-Liste beginnt bei x = 8, y = 36 und endet 36 px über dem unteren Rand; in jeder Zeile deckt das `ToggleWidget` die linken 20 px (x 8 bis 28), der Rest der Zeile ist die Auswahlfläche des Namens. `hengst.notice` wird als ein einziger `text`-Aufruf gezeichnet (kein Umbruch in M0).

- [ ] **Step 4: Run tests to verify they pass**

Run: `./gradlew :common:test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add mod/common
git commit -m "feat(mod-common): settings screen with feature list, options and profiles

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 10: HengstCore-Fassade

**Files:**
- Create: `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/HengstCore.java`
- Test: `mod/common/src/test/java/io/github/clemensjl/hengst/mod/common/HengstCoreTest.java`

**Interfaces:**
- Consumes: alles aus Tasks 3 bis 9, die Test-Helfer `FakePlatform`, `FakeCanvas`, `FixedSizeModule` (Task 6), `FakeTime`, `TestFeature` (Task 3).
- Produces (Spec 4.3):
  - `public static HengstCore start(Platform platform, TimeSource time)` und für Tests `static HengstCore start(Platform platform, TimeSource time, java.time.Clock clock)`: registriert `FpsModule`, `CpsModule`, `KeystrokesModule`, `CoordinatesModule`; Konfiguration unter `platform.configDir().resolve("hengst")` (`settings.json`, `profiles/`); Warnungen gehen an `platform.logWarning`. Die Entscheidung "erster Start" fällt nach dem Laden: ist das aktive Profil `default` und meldet `ProfileStore.loadedFromDefaults()` true (Datei fehlt, ist leer oder war kaputt und wurde zur Seite gelegt), gelten die M0-Standardwerte: FPS (TOP_LEFT 2/2), CPS (TOP_LEFT 2/16), Koordinaten (TOP_LEFT 2/30) und Keystrokes (BOTTOM_LEFT 2/-2) eingeschaltet. Ein kaputtes `default.json` liefert so dieselben Standardmodule wie eine fehlende Datei. Geschrieben wird erst bei der ersten Änderung.
  - `public void onClientTick()`: Module ticken (nur in einer Welt), dann speichern, wenn `dirty` und seit dem letzten Speichern ≥ 2 s.
  - `public void onHudRender(Canvas canvas, float partialTick)`: `frames.onFrame()`; zeichnet nur, wenn `inWorld && !hudHidden` und kein eigener Bildschirm (Editor, Einstellungen) offen ist (Flag `ownScreenOpen`, gesetzt in `onOpen*Key`, gelöscht in `onClose`); unter dem Editor würden die Module sonst doppelt gezeichnet.
  - `public void onMouseButton(int button, boolean pressed)`: zählt nur Drücken, nur in einer Welt, nur ohne offenen Bildschirm.
  - `public void onOpenSettingsKey()`, `public void onOpenHudEditorKey()`: öffnen `SettingsScreen` bzw. `HudEditorScreen` über `platform.openScreen` und setzen `ownScreenOpen` (ist schon einer offen, passiert nichts). Der gemeinsame `onClose`-Rückruf beider Bildschirme ist idempotent über dieses Flag und macht in dieser Reihenfolge: Flag löschen, speichern (Profil und `dirty` zurücksetzen), Renderer neu bauen, `platform.closeScreen()`; die Adapter schließen nur, wenn der aktuelle Bildschirm ihr eigener Host ist. Verdrahtung des `SettingsModel`: `switchProfile` setzt `Settings.activeProfile`, schreibt `settings.json` sofort, lädt das Profil und baut den Renderer neu; `saveNow` speichert das aktive Profil sofort; `warn` ist `platform::logWarning`.
  - `public void shutdown()`: speichert, wenn `dirty`.
  - Für Tests: `public Profile activeProfile()`, `public HudRenderer renderer()`, `public HudContext context()`, `public FeatureRegistry registry()` (damit ein Test ein zusätzliches Feature registrieren und `renderer().rebuild(activeProfile())` aufrufen kann).

- [ ] **Step 1: Write the failing tests**

```java
@TempDir Path dir;
FakePlatform platform; FakeTime time; FakeCanvas canvas; Clock clock; HengstCore core;
Path profiles() { return dir.resolve("hengst/profiles"); }
HengstCore start() { return HengstCore.start(platform, time, clock); }
@BeforeEach void setUp() {
    platform = new FakePlatform(dir); time = new FakeTime(); canvas = new FakeCanvas();
    clock = Clock.fixed(Instant.parse("2026-11-20T10:15:30Z"), ZoneOffset.UTC);
}
@Test void first_start_enables_m0_modules_and_writes_nothing_yet() {
    core = start();
    assertEquals(4, core.renderer().activeCount());
    assertFalse(Files.exists(profiles().resolve("default.json")));
}
@Test void hud_not_rendered_when_hidden() { core = start(); platform.inWorld = true; platform.hudHidden = true; core.onHudRender(canvas, 0f); assertTrue(canvas.calls.isEmpty()); }
@Test void hud_not_rendered_outside_world() {
    core = start(); platform.inWorld = false; platform.hudHidden = false;
    core.onHudRender(canvas, 0f); assertTrue(canvas.calls.isEmpty());
}
@Test void hud_not_rendered_while_own_screen_open() {
    core = start(); platform.inWorld = true;
    core.onClientTick(); core.onHudRender(canvas, 0f);
    assertFalse(canvas.calls.isEmpty());
    canvas.calls.clear();
    core.onOpenSettingsKey();
    assertNotNull(platform.openedScreen);
    core.onHudRender(canvas, 0f); assertTrue(canvas.calls.isEmpty());
    assertTrue(platform.openedScreen.keyPressed(UiKey.ESCAPE, false, false));   // Esc schließt über onClose
    assertEquals(1, platform.closeCount);
    core.onHudRender(canvas, 0f); assertFalse(canvas.calls.isEmpty());
}
@Test void clicks_ignored_while_screen_open() {
    core = start();
    platform.inWorld = true; platform.screenOpen = true; core.onMouseButton(0, true);
    platform.screenOpen = false; core.onMouseButton(0, true); core.onMouseButton(0, false);
    assertEquals(1, core.context().cps().cps(0));
}
@Test void editor_change_is_saved_on_close() {
    core = start(); platform.inWorld = true; core.onClientTick();   // FPS-Text "0 FPS": 34 x 13 bei (2, 2)
    core.onOpenHudEditorKey();
    UiScreen editor = platform.openedScreen;
    editor.render(canvas, 0, 0, 0f);
    editor.mouseClicked(5, 5, 0); editor.mouseDragged(105, 205, 0); editor.mouseReleased(105, 205, 0);   // Mitte (119, 208): unteres Drittel, mittlere Spalte
    assertTrue(editor.keyPressed(UiKey.ESCAPE, false, false));
    assertEquals(1, platform.closeCount);
    ProfileStore reread = new ProfileStore(profiles(), clock, platform.warnings::add);
    assertEquals(Anchor.BOTTOM_CENTER, reread.load("default").module("fps").anchor());
}
@Test void saves_are_debounced_to_two_seconds() {
    core = start(); platform.inWorld = true;
    core.onOpenSettingsKey();
    UiScreen settings = platform.openedScreen;
    settings.render(canvas, 0, 0, 0f);
    for (int i = 0; i < 5; i++) { settings.mouseClicked(12, 46, 0); time.advanceMillis(200); core.onClientTick(); }   // 5 Änderungen in 1 s
    assertFalse(Files.exists(profiles().resolve("default.json")));
    time.advanceMillis(1100); core.onClientTick();   // 2,1 s seit dem Start
    assertTrue(Files.exists(profiles().resolve("default.json")));
    assertFalse(new ProfileStore(profiles(), clock, platform.warnings::add).load("default").module("fps").enabled());   // FPS war an, 5 Klicks schalten es aus
}
@Test void shutdown_saves_dirty_state() {
    core = start(); platform.inWorld = true;
    core.onOpenSettingsKey();
    platform.openedScreen.render(canvas, 0, 0, 0f);
    platform.openedScreen.mouseClicked(12, 46, 0);   // FPS aus
    assertFalse(Files.exists(profiles().resolve("default.json")));
    core.shutdown();
    assertFalse(new ProfileStore(profiles(), clock, platform.warnings::add).load("default").module("fps").enabled());
}
@Test void broken_config_logs_one_warning_and_starts() throws Exception {
    Files.createDirectories(profiles());
    Files.write(profiles().resolve("default.json"), "{\"schema\":1,\"modules\":{".getBytes(UTF_8));
    core = start();
    assertEquals(1, platform.warnings.size());
    assertEquals(4, core.renderer().activeCount());   // kaputte Datei wurde zur Seite gelegt, es gelten die M0-Standardmodule
    assertTrue(Files.exists(profiles().resolve("default.json.broken-20261120-101530")));
}
@Test void unsupported_module_never_renders() {
    core = start();   // FakePlatform.version ist V1_8_9
    core.registry().register(new FixedSizeModule("future", 99, 10, 10, EnumSet.of(GameVersion.V26_3)));
    core.activeProfile().module("future").setEnabled(true);
    core.renderer().rebuild(core.activeProfile());
    assertEquals(4, core.renderer().activeCount());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `./gradlew :common:test --tests '*HengstCoreTest'`
Expected: FAIL.

- [ ] **Step 3: Implement `HengstCore`**

Speicherfehler (`IOException`) gehen als eine Warnung an `logWarning`, der Zustand bleibt `dirty`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `./gradlew :common:test`
Expected: PASS, alle Tests aus Task 2 bis 10.

- [ ] **Step 5: Commit**

```bash
git add mod/common
git commit -m "feat(mod-common): core facade wiring config, hud, editor and settings

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 11: Adapter 26.3

**Files:**
- Create: `mod/modern/src/main/java/io/github/clemensjl/hengst/mod/modern/{ModernPlatform,ModernCanvas,ModernScreenHost,ModernKeys}.java`, `mod/modern/src/main/java/io/github/clemensjl/hengst/mod/modern/mixin/MouseHandlerMixin.java`, `mod/modern/src/main/resources/hengst.mixins.json`, `mod/modern/src/main/resources/assets/hengst/lang/en_us.json`, `de_de.json`
- Modify: `mod/modern/src/main/java/io/github/clemensjl/hengst/mod/modern/HengstModernMod.java`, `mod/modern/src/main/resources/fabric.mod.json` (Eintrag `"mixins": ["hengst.mixins.json"]`)

**Interfaces:**
- Consumes: `HengstCore` und die Plattform-Schnittstellen (Tasks 3, 10).
- Produces:
  - `HengstModernMod` behält `implements ClientModInitializer` und die Logzeile `[hengst] loaded on <mc>` aus Task 1 (`LOGGER.info("[hengst] loaded on {}", platform.version().id())`, Step 4 erwartet sie) und bekommt `static boolean ENABLED` (anfangs false, als letzter Schritt von `onInitializeClient` nach `HengstCore.start` auf true) sowie `static HengstCore core()`; `onInitializeClient`: `ModernKeys.register()`, `HengstCore.start(new ModernPlatform(), TimeSource.SYSTEM)`, `HudElementRegistry.addLast(Identifier.fromNamespaceAndPath("hengst", "hud"), (graphics, delta) -> { CANVAS.begin(graphics); core.onHudRender(CANVAS, delta.getGameTimeDeltaPartialTick(false)); })` mit einem einmal angelegten `static final ModernCanvas CANVAS` (keine Allokation je Frame) und dem Teiltick aus dem `DeltaTracker` [K, Methodenname in `genSources` prüfen]; `HudElementRegistry.addLast(...)` mit `extractRenderState(GuiGraphicsExtractor, DeltaTracker)`, `ClientTickEvents.END_CLIENT_TICK` → Tasten abfragen (`consumeClick()` je Taste → `onOpenSettingsKey`/`onOpenHudEditorKey`) und `core.onClientTick()`, `ClientLifecycleEvents.CLIENT_STOPPING` → `core.shutdown()`.
  - `ModernCanvas implements Canvas` mit `void begin(GuiGraphicsExtractor g)`; `fill` → `g.fill`, `text` → `g.text(font, s, x, y, argb)` (Schatten-Variante [K]), `width/height` → `g.guiWidth()/guiHeight()`, `pushTransform` → `g.pose().pushMatrix(); g.pose().translate(x, y); g.pose().scale(s)`, `popTransform` → `popMatrix()`.
  - `ModernPlatform implements Platform`: `version()` = `V26_3`; `GameView` aus `Minecraft.getInstance()` (`player`, `options.hideGui`, Debug-Overlay, `screen != null`); `InputView` über die Vanilla-`KeyMapping`s in `options` (`keyUp`, `keyDown`, `keyLeft`, `keyRight`, `keyJump`, `keyShift`, `keySprint`, `keyAttack`, `keyUse`) mit `isDown()` und `getTranslatedKeyMessage().getString()`; `configDir` = `FabricLoader.getInstance().getConfigDir()`; `playerYaw()` aus `player.getYRot()` [K]; `serverAddress()` aus `Minecraft.getCurrentServer()` (`ip`, null im Einzelspieler und im Hauptmenü) [K]; `languageCode` aus dem Sprachmanager (Kleinschreibung, z. B. `de_de`); `openScreen(UiScreen)` → `Gui.setScreen(new ModernScreenHost(ui))` [B: `setScreen` liegt in 26.3 auf `Gui`]; `closeScreen()` → `setScreen(null)`, aber nur wenn der aktuelle Bildschirm ein `ModernScreenHost` ist; Logger slf4j.
  - `ModernScreenHost extends Screen`: Render-Methode delegiert an `UiScreen.render` über einen eigenen `ModernCanvas`; Maus- und Tastenereignisse auf `UiScreen` mit Übersetzung von GLFW-/SDL-Tastencodes auf `UiKey` (`InputConstants.KEY_ESCAPE` usw.) [K]; `isPauseScreen() = false`; `removed()` → `ui.closed()`.
  - `ModernKeys`: zwei `KeyMapping`s `key.hengst.settings` (Rechte Umschalttaste) und `key.hengst.hud_editor` (unbelegt), Kategorie `hengst:main` (KeyMapping-Kategorie-API von 26.3 [K]), registriert über `KeyMappingHelper.registerKeyMapping`. Die Adapter-Sprachdateien `assets/hengst/lang/en_us.json` und `de_de.json` enthalten genau drei Schlüssel: `key.hengst.settings` ("Hengst settings" / "Hengst-Einstellungen"), `key.hengst.hud_editor` ("Hengst HUD editor" / "Hengst-HUD-Editor") und `key.category.hengst.main` ("Hengst" / "Hengst") [K: Schlüsselform der Kategorie in `genSources` prüfen].
  - `MouseHandlerMixin`: `@Inject(method = "onButton", at = @At("HEAD"), require = 1)` auf `MouseHandler.onButton(long, MouseButtonInfo, int)` [B]; `if (!HengstModernMod.ENABLED) return;` dann `HengstModernMod.core().onMouseButton(info.button(), action == 1)` (Feldname `button()` an `MouseButtonInfo` [K]).

- [ ] **Step 1: Namen gegen das echte Spiel prüfen**

Run (aus `mod/`): `./gradlew :modern:26.3:genSources` (lädt 26.3 über Loom, nur wenn kein anderer Build läuft).
Expected: Quellen erzeugt. Die mit [K] markierten Namen (Screen-Render-Methode, Schatten-Variante von `text`, `MouseButtonInfo.button()`, `KeyMapping`-Konstruktor und Kategorie, Debug-Overlay-Abfrage, `pose().translate`) in den erzeugten Quellen nachschlagen und im Code genau so verwenden. Abweichungen gegenüber diesem Plan als Kommentar in die Commit-Nachricht.

- [ ] **Step 2: Implement die Klassen aus dem Interfaces-Block**

- [ ] **Step 3: Bauen**

Run: `./gradlew :modern:26.3:build`
Expected: `BUILD SUCCESSFUL`; `unzip -l versions/26.3/build/libs/hengst-mod-0.1.0+26.3.jar` zeigt `hengst.mixins.json` und `META-INF/jars/common-0.1.0.jar`.

- [ ] **Step 4: Prüfung im Spiel (26.3)**

Vorbereitung (aus dem Repo-Wurzelordner, kein Gradle läuft): `cargo run -p hengst-cli --features dev-offline -- launch 26.3 --loader fabric --instance hengst-mod-26.3 --dry-run`. Dann nach `%APPDATA%\HengstClient\instances\hengst-mod-26.3\mods\` kopieren: `mod/modern/versions/26.3/build/libs/hengst-mod-0.1.0+26.3.jar` und `fabric-api-0.166.2+26.3.jar` von `https://maven.fabricmc.net/net/fabricmc/fabric-api/fabric-api/0.166.2+26.3/`. In `options.txt` der Instanz `soundCategory_master:0.0`.
Start über Agentfenster auf dem Agent-Desktop: `cargo run -p hengst-cli --features dev-offline -- launch 26.3 --loader fabric --instance hengst-mod-26.3`. (Ohne Phase 3: `./gradlew :modern:26.3:runClient` aus `mod/`.)
Expected:
1. Log der Instanz enthält `[hengst] loaded on 26.3` und keinen `Mixin apply failed`.
2. Neue Einzelspielerwelt: oben links FPS, CPS, Koordinaten, unten links Keystrokes (Screenshot). Laufen mit W: die W-Zelle wird hell; Koordinaten ändern sich.
3. Fünfmal schnell links klicken: CPS zeigt kurz 4 bis 6, eine Sekunde später 0. Mit offenem Chat klicken: CPS bleibt 0.
4. Rechte Umschalttaste öffnet die Einstellungen; Hinweis "Not an official Minecraft product..." ist sichtbar; CPS ausschalten, Fertig: CPS verschwindet.
5. HUD bearbeiten: FPS nach unten rechts ziehen, Esc. Spiel beenden. `config/hengst/profiles/default.json` enthält bei `fps` `"anchor": "BOTTOM_RIGHT"`. Erneut starten: FPS steht unten rechts.
6. F1: alle Module weg.

- [ ] **Step 5: Commit**

```bash
git add mod/modern
git commit -m "feat(mod-modern): 26.3 adapter with hud, keys, click mixin and screens

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 12: Adapter 1.21.11 über Stonecutter

**Files:**
- Modify: `mod/modern/src/main/java/io/github/clemensjl/hengst/mod/modern/{HengstModernMod,ModernPlatform,ModernCanvas,ModernScreenHost,ModernKeys}.java`, `mod/modern/src/main/java/io/github/clemensjl/hengst/mod/modern/mixin/MouseHandlerMixin.java`

**Interfaces:**
- Consumes: Task 11.
- Produces: dieselben Klassen, jetzt mit Stonecutter-Bedingungen für `<26.1`. Belegte Unterschiede (Spec 3): `GuiGraphics` statt `GuiGraphicsExtractor` (`//~ if <26.1 'GuiGraphicsExtractor' -> 'GuiGraphics'`), `HudElement.render(GuiGraphics, DeltaTracker)` statt `extractRenderState`, `drawString(font, s, x, y, argb)` (5 Argumente, Spec 3, belegt) statt `text`; die Variante `drawString(font, s, x, y, argb, shadow)` mit Schattenparameter ist [K] und wird vor der Verwendung für `Canvas.text(..., shadow=false)` per `genSources` bestätigt, `KeyBindingHelper.registerKeyBinding` (Paket `client.keybinding.v1`) statt `KeyMappingHelper.registerKeyMapping`, `Minecraft.setScreen` statt `Gui.setScreen`, Screen-Render-Methode der 1.21.11 [K]. `MouseHandler.onButton(long, MouseButtonInfo, int)` ist in beiden gleich [B].

- [ ] **Step 1: Namen prüfen**

Run: `./gradlew :modern:1.21.11:genSources`
Expected: Quellen erzeugt; [K]-Namen für 1.21.11 nachschlagen, insbesondere ob `GuiGraphics.drawString(Font, String, int, int, int, boolean)` existiert (sonst Schatten über die 5-Argument-Form und eine Ersatzlösung für `shadow=false`, im Commit begründen).

- [ ] **Step 2: Bedingungen einfügen**

Nur Stellen, die sich unterscheiden, bekommen `//? if` bzw. `//~`. Die aktive Version bleibt `26.3` im Quellbaum (`stonecutter active '26.3'`); vor dem Commit muss der Quellbaum wieder auf 26.3 stehen.

- [ ] **Step 3: Beide Knoten bauen**

Run: `./gradlew :modern:1.21.11:build :modern:26.3:build`
Expected: `BUILD SUCCESSFUL`. `unzip -l versions/1.21.11/build/libs/hengst-mod-0.1.0+1.21.11.jar` zeigt eine Refmap (`*-refmap.json`), und `unzip -p ... hengst.mixins.json` enthält einen `"refmap"`-Eintrag (Loom schreibt ihn beim Remappen; fehlt er, `loom { mixin { defaultRefmapName = 'hengst-refmap.json' } }` setzen und den Eintrag nur für den remappenden Knoten per `processResources` ergänzen). Die 26.3-Jar hat keine Refmap.

- [ ] **Step 4: Prüfung im Spiel (1.21.11)**

Wie Task 11 Step 4 mit `1.21.11`, Instanz `hengst-mod-1.21.11`, Jar `mod/modern/versions/1.21.11/build/libs/hengst-mod-0.1.0+1.21.11.jar`, Fabric API `0.141.6+1.21.11`. Ohne Phase 3: `./gradlew :modern:1.21.11:runClient`.
Expected: dieselben sechs Punkte, Log `[hengst] loaded on 1.21.11`.

- [ ] **Step 5: Commit**

```bash
git add mod/modern
git commit -m "feat(mod-modern): 1.21.11 node through stonecutter conditions

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 13: Adapter 1.8.9 (Legacy Fabric)

**Files:**
- Create: `mod/legacy/src/main/java/io/github/clemensjl/hengst/mod/legacy/{LegacyPlatform,LegacyCanvas,LegacyScreenHost,LegacyKeys}.java`, `mod/legacy/src/main/java/io/github/clemensjl/hengst/mod/legacy/mixin/KeyBindingMixin.java`, `mod/legacy/src/main/resources/hengst.mixins.json`, `mod/legacy/src/main/resources/assets/hengst/lang/en_US.lang`, `de_DE.lang`
- Modify: `mod/legacy/src/main/java/io/github/clemensjl/hengst/mod/legacy/HengstLegacyMod.java`, `mod/legacy/src/main/resources/fabric.mod.json` (Eintrag `"mixins": ["hengst.mixins.json"]`)

**Interfaces:**
- Consumes: `HengstCore` und Plattform-Schnittstellen (Tasks 3, 10).
- Produces (Namen Legacy Yarn build.604, [B] aus den Mappings, Spec 7):
  - `HengstLegacyMod` behält `implements ClientModInitializer` und die Logzeile `[hengst] loaded on 1.8.9` aus Task 1 (Step 4 erwartet sie) und bekommt `static boolean ENABLED` (anfangs false, als letzter Schritt von `onInitializeClient` nach `HengstCore.start` auf true) sowie `static HengstCore core()` (der Mixin kommt nur so an den Kern); `onInitializeClient`: `LegacyKeys.register()` über `KeyBindingHelper.registerKeyBinding(KeyBinding)` [B], `HengstCore.start(new LegacyPlatform(), TimeSource.SYSTEM)`, `HudRenderCallback.EVENT.register((client, tickDelta) -> core.onHudRender(canvas, tickDelta))` [B, Parametertyp der Version 1.13.5 prüfen], `ClientTickEvents.END_CLIENT_TICK` [B] → Tasten (`wasPressed()` [B]) und `core.onClientTick()`; Shutdown über `ClientLifecycleEvents.CLIENT_STOPPING` [K, Name in 1.13.5 prüfen].
  - `LegacyCanvas implements Canvas`: Größe aus `new Window(client)` einmal je Frame ist eine Allokation, daher Größe im HUD-Callback aus einem einmal je Auflösungsänderung neu erzeugten `Window` [K, Konstruktor prüfen] oder aus `client.width/height` geteilt durch `getScaleFactor()`; `fill` → `DrawableHelper.fill(IIIII)V` [B]; `text` → `TextRenderer.draw(String, float, float, int, boolean)` [B, Deskriptor `(Ljava/lang/String;FFIZ)I`]; `textWidth` → `getStringWidth(String)` [B]; `pushTransform` → `GlStateManager.pushMatrix(); translatef; scalef` [K]; nach dem Text `GlStateManager.color(1,1,1,1)` zurücksetzen [K].
  - `LegacyPlatform implements Platform`: `version()` = `V1_8_9`; Spieler `client.player` mit Feldern `x`, `y`, `z`, `yaw` [B]; `options.hudHidden`, `options.debugEnabled` [B]; `currentScreen != null` [B]; Tasten `options.forwardKey`, `backKey`, `leftKey`, `rightKey`, `jumpKey`, `sneakKey`, `sprintKey`, `attackKey`, `useKey` [B] mit `isPressed()` [B]; Beschriftung über `GameOptions.getFormattedNameForKeyCode(getCode())` [K]; `playerYaw()` aus `player.yaw` [B]; `serverAddress()` aus `client.getCurrentServerEntry()` (`address`, null im Einzelspieler und im Hauptmenü) [K]; `languageCode()` aus `client.options.language` [K] (1.8.9 meldet `de_DE`, `I18n.load` wandelt in Kleinbuchstaben); `configDir` über `FabricLoader.getInstance().getConfigDir()`; `openScreen` → `client.setScreen(new LegacyScreenHost(ui))` [B]; `closeScreen()` → `client.setScreen(null)`, aber nur wenn der aktuelle Bildschirm ein `LegacyScreenHost` ist; Logger log4j.
  - `LegacyScreenHost extends Screen`: `render(IIF)V`, `mouseClicked(III)V`, `mouseReleased(III)V`, `mouseDragged(IIIJ)V`, `keyPressed(CI)V` mit LWJGL-2-Tastencodes (`Keyboard.KEY_ESCAPE` usw.) auf `UiKey`, Mausrad in `handleMouse()V` über `Mouse.getEventDWheel()` [B Namen]; `shouldPauseGame() = false`; `removed()` → `ui.closed()`. Der Override ruft `super.keyPressed` nicht auf und reicht Esc nur an `UiScreen` weiter; das Schließen (Esc wie Fertig) übernimmt `HengstCore` über `Platform.closeScreen()` (Task 10), es gibt keinen Sonderfall im Adapter. Das Mausrad reicht der Adapter als `Integer.signum(Mouse.getEventDWheel())` (also +1, -1 oder 0, nicht +-120) an `UiScreen.mouseScrolled` weiter; `ScrollList` rechnet 12 px je Rastung, `HudEditorModel.scroll(+1)` ist ein Schritt von 0,1.
  - `KeyBindingMixin`: `@Inject(method = "onKeyPressed(I)V", at = @At("HEAD"), require = 1)` auf `KeyBinding` [B]; `if (!HengstLegacyMod.ENABLED) return;` Maustasten kommen als `code < 0` an (`button = code + 100`) [K: im Quelltext von `MinecraftClient.tick` bestätigen], nur dann `HengstLegacyMod.core().onMouseButton(button, true)`.
  - `LegacyKeys`: zwei `KeyBinding`s `key.hengst.settings` (Standardtaste `Keyboard.KEY_RSHIFT`) und `key.hengst.hud_editor` (unbelegt, `Keyboard.KEY_NONE`), Kategorie `key.categories.hengst`; `en_US.lang` und `de_DE.lang` enthalten genau drei Schlüssel: `key.hengst.settings=Hengst settings` bzw. `Hengst-Einstellungen`, `key.hengst.hud_editor=Hengst HUD editor` bzw. `Hengst-HUD-Editor`, `key.categories.hengst=Hengst`.

- [ ] **Step 1: Namen prüfen**

Run: `./gradlew :legacy:genSources`
Expected: Quellen erzeugt; alle [K]-Stellen nachschlagen, insbesondere wie `MinecraftClient.tick` Maustasten an `KeyBinding.onKeyPressed` weitergibt. Ist es nicht `button - 100`, Mixin-Ziel auf die Stelle in `MinecraftClient.tick` umstellen, an der `Mouse.getEventButton()` gelesen wird (`@Inject` mit `@At(value = "INVOKE", target = "Lorg/lwjgl/input/Mouse;getEventButtonState()Z", shift = AFTER)`), und im Commit begründen.

- [ ] **Step 2: Implement die Klassen aus dem Interfaces-Block**

- [ ] **Step 3: Bauen**

Run: `./gradlew :legacy:build`
Expected: `BUILD SUCCESSFUL`; Jar enthält `hengst.mixins.json`, eine Refmap, `META-INF/jars/common-0.1.0.jar`; `javap -v` auf eine Mod-Klasse zeigt `major version: 52` (Java 8).

- [ ] **Step 4: Prüfung im Spiel (1.8.9)**

Vorbereitung: `cargo run -p hengst-cli --features dev-offline -- launch 1.8.9 --loader legacy-fabric --instance hengst-mod-1.8.9 --dry-run`; nach `instances\hengst-mod-1.8.9\mods\` kopieren: `mod/legacy/build/libs/hengst-mod-0.1.0+1.8.9.jar` und `legacy-fabric-api-1.13.5+1.8.9.jar` (Maven `https://repo.legacyfabric.net/legacyfabric/net/legacyfabric/legacy-fabric-api/legacy-fabric-api/1.13.5+1.8.9/` oder Modrinth-Projekt `legacy-fabric-api`, Version 1.13.5 mit Loader `legacy-fabric`, nicht 1.22 bis 1.24, die sind nur für Ornithe); `soundCategory_master:0.0` in `options.txt`. Start wie Task 11 mit `1.8.9 --loader legacy-fabric`. Ohne Phase 3: `./gradlew :legacy:runClient`.
Expected: dieselben sechs Punkte wie Task 11 Step 4, Log `[hengst] loaded on 1.8.9`; zusätzlich: nach dem Öffnen und Schließen der Einstellungen sind Chat-Text und Hotbar nicht verfärbt (GL-Farbzustand zurückgesetzt).

- [ ] **Step 5: Commit**

```bash
git add mod/legacy
git commit -m "feat(mod-legacy): 1.8.9 adapter on legacy fabric

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 14: Gesamtprüfung M0 und Kostenmessung

**Files:**
- Modify: `docs/begleitmod-spec.md` (Abschnitt 7: [K]-Markierungen der in M0 genutzten Hooks auf [B] setzen oder korrigieren, mit Datum; Abschnitt 12: M0 als erledigt markieren)

**Interfaces:**
- Consumes: alle Jars aus Tasks 11 bis 13.
- Produces: `mod/dist/` mit drei Jars; Messwerte im Commit-Text.

- [ ] **Step 1: Alles bauen und testen**

Run (aus `mod/`): `./gradlew clean buildAll`
Expected: `BUILD SUCCESSFUL`, `dist/` enthält genau drei Jars, alle `common`-Tests grün.

- [ ] **Step 2: FPS-Kosten je Version messen (Spec 8.8)**

Nacheinander, nie zwei Spiele gleichzeitig, kein Build parallel: je Version dieselbe Einzelspielerwelt (Superflach, Seed `hengst`), Spawn, Blick nach Norden, Render-Distanz 8, FPS-Limit aus, VSync aus. 30 s warten, dann 30 s FPS-Mittel aus der eigenen FPS-Anzeige ablesen (Screenshots im Abstand von 5 s, Mittel der sechs Werte). Danach die Mod-Jar aus `mods/` nehmen und mit F3 dasselbe messen.
Expected: Verlust mit Mod höchstens 2 % je Version. Werte als Tabelle in die Commit-Nachricht.

- [ ] **Step 3: Konfigurationsrobustheit im Spiel**

Für eine Version (26.3): Spiel beenden, `config/hengst/profiles/default.json` durch `{` ersetzen, starten.
Expected: Spiel startet, Standard-HUD erscheint, Log enthält genau eine Warnung, daneben liegt `default.json.broken-<Zeit>`.

- [ ] **Step 4: Spec nachführen**

In Abschnitt 7 jede in M0 tatsächlich genutzte [K]-Stelle (Klick-Semantik 1.8.9, Screen-Render-Methoden, Text mit Schatten 26.3, Tastenkategorie 26.3) auf den geprüften Namen und [B] setzen; in Abschnitt 12 bei M0 "erledigt 2026-xx-xx".

- [ ] **Step 5: Commit**

```bash
git add docs/begleitmod-spec.md
git commit -m "docs(mod): m0 verified on 1.8.9, 1.21.11 and 26.3

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```
