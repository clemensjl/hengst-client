# Begleitmod M1a (Struktur und Text-HUD) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Die Bausteine, die jede spätere Funktion braucht (Funktionsschalter für Mixins, Tastenbelegungen je Funktion, typisierte Optionen, vorwärtskompatible Konfiguration, Serverwechsel, Sprache und Uhr im Kontext, gemeinsame Adapter-Helfer), plus sieben Text-HUD-Module: 7 Richtung, 8 Ping, 9 Uhr, 10 Server-Adresse, 11 Speicher, 15 Geschwindigkeit, 21 Stoppuhr, auf 1.8.9, 1.21.11 und 26.3.

**Architecture:** Alles Neue entsteht in `common` mit JUnit-Tests; die Adapter bekommen nur zwei Lesemethoden (`pingMillis`, `worldIdentity`), eine generische Tastenregistrierung und die gemeinsamen Helfer `AdapterGuard` und `ScrollAccumulator`. Die Strukturaufgaben (Tasks 1 bis 9) ziehen bestehenden M0-Code auf die neuen Teile um, bei grünen Tests; die Module (Tasks 10 bis 13) sind reine `common`-Arbeit; die Spielprüfung (Tasks 14, 15), die Kostenmessung (16, bedingt 17), Doku (18) und der Lauf mit den ausgelieferten Jars über den Launcher (19) folgen.

**Tech Stack:** wie M0: Java 8 (common, legacy), 21 (1.21.11), 25 (26.3); Gradle 9.8.2 auf JDK 25; Fabric Loader 0.20.1; Fabric API 0.141.6+1.21.11 und 0.166.2+26.3; Legacy Fabric API 1.13.5+1.8.9; Legacy Yarn 1.8.9+build.604; Stonecutter 0.10; JUnit Jupiter 5.11.4.

**Spec:** `docs/begleitmod-spec.md` (Abschnitte 4 bis 9 und 12; Meilenstein M1a laut Fahrplan). Dazu die Entscheidungen D1 bis D7, F8 bis F10 und die Lehren aus `.superpowers/sdd/2026-11-20-begleitmod-m0-foundation/progress.md` (Abschnitt "Final review" und die Einträge zu Task 11 bis 14).

**Ausgangslage:** Worktree `C:\Users\jelec\Documents\Projekte\hengst-client-mod`, Branch `begleitmod-m0`, nach der Fix-Welle des M0-Abschlussreviews (F1 bis F7, F11 bis F15: Lizenzdatei in den Jars, Dimmung des Einstellungsbildschirms, Optionen der Keystrokes im Tick zwischengespeichert, explizite Versionsmengen, Beschriftungsschlüssel je Funktion, Spec 8.8 als absolutes Budget, neue Meilensteintabelle). Dieser Plan setzt diesen Stand voraus.

## Global Constraints

- Alles aus den Global Constraints des M0-Plans gilt weiter: Mod-ID `hengst`, Paket `io.github.clemensjl.hengst.mod`, `mod_version=0.1.0`, Jar-Namen, `fabric.mod.json`-Regeln, Code und Logtexte Englisch, `docs/` Deutsch, UI-Texte nur über `hengst/lang/en_us.json` und `de_de.json`, keine Emojis, Lizenz GPL-3.0-or-later.
- `common`- und `legacy`-Hauptcode: Java 8 (`options.release = 8`), keine Java-9+-API, keine Abhängigkeit. Test-Code von `common` kompiliert mit Release 17.
- Befehle laufen aus `mod/` mit `export JAVA_HOME="C:/Users/jelec/.jdks/openjdk-25.0.1"`. Einheitstests `./gradlew -p common test` (startet danach `interpretedTest` mit `-Xint`), Adapter `./gradlew -p legacy test`, `./gradlew -p modern :1.21.11:test`, `./gradlew -p modern :26.3:test`, Jars `./gradlew buildAll`, Spieltests `./gradlew -p modern :26.3:clientGameTests`, `./gradlew -p modern :1.21.11:clientGameTests`, `./gradlew -p legacy gameTests`, Quellen `./gradlew -p modern :26.3:genSources`, `./gradlew -p modern :1.21.11:genSources`, `./gradlew -p legacy genSources`. Am Ende jedes Tasks `./gradlew --stop`. Regeln aus `C:/Users/jelec/Documents/Projekte/hengst-client/.superpowers/sdd/mod-implementer-rules.md` gelten.
- Jeder Task, der Hauptcode von `common` oder einem Adapter ändert, endet mit grünem `./gradlew buildAll`; Tasks, die Adapter oder deren Spieltests ändern, zusätzlich mit allen drei Spieltests.
- Jeder Commit endet mit der Zeile `Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>`. Dateien einzeln stagen, nie `git add -A`.
- Bekanntes Windows-Ärgernis: `buildAll` oder ein Test-Task kann mit "Unable to delete directory ... test-results ... binary" scheitern. Dann die leeren `binary`-Verzeichnisse unter den betroffenen `build/test-results` löschen und den Lauf wiederholen; das ist kein Fehler des Codes.
- Bevor der Branch gepusht wird, bekommt jedes der drei ausgelieferten Jars (`mod/dist/`) einen Launcher-Rauchstart (Spiel startet, `[hengst] loaded on <version>` im Log). Das deckt Task 19; ein Push vor Task 19 braucht die Rauchstarts trotzdem.

Verbindliche Regeln aus den M0-Lehren (gelten für jeden Task):
1. **Hooks am echten Spiel prüfen.** Jeder Spiel-Name (Klasse, Methode, Feld, Deskriptor) wird vor der Verwendung in den erzeugten Quellen der jeweiligen Version (`genSources`) oder per `javap` auf dem Minecraft-Jar im Loom-Cache nachgeschlagen, nie nur aus der Spec-Tabelle oder diesem Plan übernommen. [B] in diesem Plan heißt: beim Planen am 2026-11-21 in den Quellen gesehen (Signatur), [K] heißt: noch zu prüfen. Das Ergebnis steht mit Datum im Task-Report.
2. **Spieltests prüfen echte Pixel.** Ein Modul gilt im Spiel als gezeichnet, wenn ein Framebuffer-Screenshot aus dem Zeichenpfad des Spiels in seinem Rechteck vom F1-Bild (HUD ausgeblendet) abweicht und das F1-Bild dort keine Textpixel hat (`checkShown`); ausgeblendet heißt gleich dem F1-Bild (`checkSame`/`checkHidden`). Gewartet wird nur auf Bedingungen mit Frist (`waitUntil`), feste Wartezeiten nur für echte Zeitfenster (CPS, Stoppuhr).
3. **Keine Test- oder Harness-Klassen in ausgelieferten Jars.** `unzip -l dist/*.jar` zeigt nichts aus `src/gametest` (kein `hengst-gametest`, keine Klasse mit `GameTest` oder `gametest` im Pfad).
4. **Ein Schreiber, ein Build.** Ein Agent schreibt im Worktree; nie zwei Gradle-Läufe gleichzeitig, nie Gradle neben einem laufenden Spiel; `org.gradle.workers.max=1` bleibt.
5. **Null Allokation im eingeschwungenen Zustand,** pro Frame und pro Tick, bewiesen durch die mit `@Tag("interpreted")` markierten Tests unter `-Xint`, für jedes neue Modul und jeden neuen Hook-Pfad. Ein String entsteht nur, wenn sich ein angezeigter Wert geändert hat.
6. **Optionen nie in `render` lesen.** HUD-Module lesen ihre Optionen in `tick` über die typisierten Zugriffe aus Task 1 und halten sie in primitiven Feldern; `width`, `height` und `render` lesen nur diese Felder.
7. **Nicht zeichnen bei F3, unter F1, außerhalb einer Welt, bei offenen eigenen Bildschirmen.** Das erledigt `HudRenderer.render`; kein neues Modul umgeht den Renderer.
8. **Sprachdateien vollständig.** `en_us.json` und `de_de.json` haben dieselbe Schlüsselmenge (Test `both_language_files_have_the_same_keys`), Deutsch mit echten Umlauten; dasselbe gilt für die Tastennamen in den Adapter-Sprachdateien (Test aus Task 8).
9. **Beschriftungsschlüssel** folgen dem Schema des M0-Fixes F7 (Commit be18b1b, `SettingsScreen` über `I18n.first`): Optionen `hengst.option.<feature>.<option>` (Rückfall `hengst.option.<option>`), Auswahlwerte `hengst.choice.<feature>.<value>` (Rückfall `hengst.choice.<value>`, dann der Rohwert). Wo dieser Plan "Werte `x` Text" unter einer Option nennt, ist der Schlüssel `hengst.choice.<feature>.x`.
10. **Neue Module sind beim ersten Start aus.** Der erste Start schaltet weiter nur die vier M0-Module ein; die bestehenden Spieltests behalten ihre Vier-Modul-Prüfungen.
11. Gemeinsame Test-Helfer liegen in `common/src/test/java/io/github/clemensjl/hengst/mod/common/testutil/` (`FakeTime`, `TestFeature`, `FakeCanvas` 320 x 240 mit 6 px je Zeichen und Schrifthöhe 9, `CountingCanvas`, `FakePlatform`, `FixedSizeModule`; `FakePlatform` und `FixedSizeModule` sind `final` und bleiben es); neu: `FakeWallClock` (Task 6), `GroupFeature` (Task 9), `TimedModule` (Task 16).
12. **Neue persistierte Daten nur dort, wo ein M0-Build sie erhält.** Ein bereits ausgelieferter M0-Build verwirft beim Zurückschreiben unbekannte Wurzelschlüssel eines Profils, unbekannte Schlüssel innerhalb der Einträge der vier M0-Module und unbekannte Schlüssel in `settings.json`; das lässt sich nachträglich nicht beheben. Behalten werden unbekannte Module (ganze Einträge) und unbekannte Schlüssel in `options`. Deshalb gilt ab M1a: neue persistierte Daten stehen nur als Moduleintrag oder in `options` eines Moduls; nichts Neues an der Wurzel eines Profils, nichts Neues direkt in einem Moduleintrag, nichts Neues in `settings.json`, dessen Verlust ein Spieler nach einem M0-Lauf bemerken würde. M1a hält das ein: die Stoppuhr speichert nichts (Zustand nur für die Sitzung), die Tastenbelegungen liegen in der `options.txt` des Spiels (nicht in `settings.json`), neue Module und Optionen sind Moduleinträge und `options`. Bekannte Einschränkung: nach einem M0-Lauf verwirft das Spiel selbst unbekannte Tastenzeilen (`key_key.hengst.stopwatch.*`) aus der `options.txt`; die Stoppuhr-Tasten sind dann neu zu belegen (steht in der README, Task 18).

## Festgelegte Bezeichner (persistiert in Dateien der Spieler)

Kollisionsfrei zu M0 (`fps` 1, `cps` 2, `keystrokes` 3, `coordinates` 6; Optionen `show_right`, `show_mouse`, `show_space`, `decimals`; Tasten `key.hengst.settings`, `key.hengst.hud_editor`).

| Nr | ID | Gruppe | Optionen (Schlüssel: Art, Standard) | Standardplatz (Anker, x, y) |
|---|---|---|---|---|
| 7 | `direction` | HUD | `style`: Auswahl `cardinal` / `cardinal_degrees` / `degrees`, Standard `cardinal_degrees` | TOP_LEFT, 2, 44 |
| 8 | `ping` | HUD | keine | TOP_RIGHT, -2, 52 |
| 9 | `clock` | HUD | `format`: Auswahl `24h` / `12h`, Standard `24h`; `seconds`: Schalter, aus | TOP_RIGHT, -2, 66 |
| 10 | `server_address` | HUD | `show_port`: Schalter, aus | TOP_RIGHT, -2, 80 |
| 11 | `memory` | HUD | `style`: Auswahl `percent` / `used_max` / `both`, Standard `both` | TOP_RIGHT, -2, 94 |
| 15 | `speed` | HUD | `include_vertical`: Schalter, aus; `decimals`: Ganzzahl 0 bis 2, Standard 1 | TOP_LEFT, 2, 58 |
| 21 | `stopwatch` | COMFORT | `tenths`: Schalter, an | TOP_LEFT, 2, 72 |

Die vier M0-Module behalten ihre Plätze (fps TOP_LEFT 2, 2; cps TOP_LEFT 2, 16; coordinates TOP_LEFT 2, 30; keystrokes BOTTOM_LEFT 2, -2). Rechte Spalte ab y 52, damit sie unter den Effekt-Symbolen der modernen Versionen (zwei Reihen, bis etwa y 50) und unter Toasts liegt.

Tastenbelegungen (Namen in `options.txt`, Kategorie "Hengst"): `key.hengst.settings` (Standard Rechte Umschalttaste, Modus PRESS), `key.hengst.hud_editor` (unbelegt, PRESS), `key.hengst.stopwatch.toggle` (unbelegt, PRESS), `key.hengst.stopwatch.reset` (unbelegt, PRESS). Indizes für `onFeatureKey` in dieser Reihenfolge: 0, 1, 2, 3.

## Review Focus

1. **Ein M1a-Profil wird von einer M0-Version gelesen und zurückgeschrieben, oder eine spätere Version schreibt Schlüssel, die M1a nicht kennt:** M1a und spätere Versionen erhalten unbekannte Schlüssel auf allen drei Ebenen. Ein schon ausgelieferter M0-Build verwirft unbekannte Wurzelschlüssel, unbekannte Schlüssel in Einträgen der vier M0-Module und unbekannte Schlüssel in `settings.json`; das ist nicht mehr zu ändern. Darum schreibt M1a nur, was M0 erhält (neue Module als eigene Einträge, neue Optionen in `options`), und der Test mit dem eingefrorenen M0-Leseverhalten belegt, was nach einem M0-Lauf übrig bleibt. Tests in Task 2 (`unknown_keys_survive_at_all_three_levels`, `m1a_profile_uses_only_keys_an_m0_build_preserves`, `unknown_settings_keys_survive_a_save`, `an_m0_round_trip_keeps_everything_m1a_wrote`, `an_m0_round_trip_loses_future_root_and_entry_keys_and_the_m1a_reader_sees_that`).
2. **Die Spielsprache wird umgestellt, während eine Welt offen ist:** spätestens nach einer Sekunde zeigen Module und neu geöffnete Bildschirme die neue Sprache, ohne Neustart; ohne Wechsel wird nichts neu geladen. Tests in Task 6 (`a_language_switch_is_picked_up_within_a_second`) und Task 10 (`direction_letters_follow_the_language`).
3. **Ping und Server-Adresse auf einem Server, der keine Latenz meldet, im Einzelspieler, beim Verbinden:** Einzelspieler und ohne Adresse unsichtbar (leerer Text, Größe 0), fehlende oder negative Latenz als `? ms`, 0 als `0 ms` (überholt durch die Entscheidung in Task 12: 0 gilt als unbekannt und zeigt `? ms`, siehe Spec 7.2), sehr lange Adressen gekürzt. Tests in Task 12 (`ping_is_hidden_without_a_server`, `ping_unknown_or_negative_shows_a_question_mark`, `ping_zero_is_shown` (inzwischen: Null ist unbekannt), `server_address_is_hidden_without_a_server`, `a_long_server_address_is_cut_in_the_middle`).
4. **Die Stoppuhr-Taste wird gehalten (Tastenwiederholung) oder bei offenem Bildschirm (Chat, Inventar) gedrückt:** ein Drücken schaltet genau einmal; bei offenem Bildschirm oder außerhalb einer Welt nichts; eine über das Schließen gehaltene Taste schaltet erst nach Loslassen und neuem Drücken. Tests in Task 7 (`a_repeated_press_while_held_counts_once`, `a_press_while_a_screen_is_open_is_ignored_and_needs_a_release`, `opening_a_screen_releases_a_held_hold_key`) und Task 13 (`one_press_through_the_core_toggles_once`, `the_stopwatch_key_with_a_screen_open_does_nothing`).
5. **Die Systemuhr wird verstellt, Mitternacht, Sommerzeit-Umstellung (Uhr), Stoppuhr bei Uhrumstellung:** die Uhr zeigt im nächsten Tick die neue Ortszeit, rollt über Mitternacht, folgt dem neuen UTC-Versatz; die Stoppuhr misst monoton und ist davon unberührt. Tests in Task 11 (`the_clock_rolls_over_midnight`, `the_clock_follows_a_backward_system_clock_change_at_the_next_tick`, `the_clock_follows_a_dst_offset_change`) und Task 13 (`the_stopwatch_uses_the_monotonic_time_source`).

---

## Dateistruktur (neu oder wesentlich geändert)

```
mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/
  AdapterGuard.java          Start-Schutz und Hook-Fehler der Adapter (Task 4)
  FeatureCatalog.java        alle Funktionen in Nummernfolge, Tastenliste je Version (Task 7)
  HengstCore.java            applyConfig, Tasten, Sprache, Serverwechsel
  feature/  FeatureGate, FeatureState, RestrictionPolicy (Task 5), KeySpec, DefaultKey (Task 7)
  platform/ ScrollAccumulator (Task 4), WallClock, SystemWallClock (Task 6), GameView (+ pingMillis, worldIdentity, Speicher)
  hud/      HudContext (I18n, Sprachgeneration, WallClock), HudRenderer (Gate, Messsonde)
  hud/modules/ DirectionModule, SpeedModule, ClockModule, MemoryModule, PingModule, ServerAddressModule, StopwatchModule
  ui/       TextFit (ersetzt TextWrap), ScreenClose (ersetzt SettingsClose und editor/HudEditorClose), ScrollList (Anzeige), SettingsScreen (Gruppen)
mod/modern/src/main/java/.../modern/  ModernKeys (generisch), ModernPlatform, HengstModernMod, ModernInput, ModernScreenHost
mod/legacy/src/main/java/.../legacy/  LegacyKeys (generisch), LegacyPlatform, HengstLegacyMod, LegacyInput, LegacyScreenHost, LegacyCanvas (nur Task 17)
```

---

### Task 1: Typisierte Optionszugriffe (D4), M0-Module umziehen

**Files:**
- Modify: `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/feature/OptionSpec.java`, `.../config/ModuleConfig.java`, `.../hud/modules/CpsModule.java`, `CoordinatesModule.java`, `KeystrokesModule.java`, `.../hud/HudModule.java` (Javadoc von `render`)
- Test: `mod/common/src/test/java/io/github/clemensjl/hengst/mod/common/config/TypedOptionTest.java` (neu), `.../feature/OptionReadAllocationTest.java`

**Interfaces:**
- Consumes: `OptionSpec`, `ModuleConfig.option(OptionSpec)` (M0).
- Produces:
  - `OptionSpec`: `public boolean coerceBool(Object raw)`, `public int coerceInt(Object raw)`, `public float coerceFloat(Object raw)`, `public int coerceChoice(Object raw)` (Index in den Werten), `public String value(int index)` (ohne Kopie des Arrays), `public int defaultIndex()`. Jede wirft `IllegalArgumentException` bei falscher Art. `coerce(Object)` liefert weiter dieselben Werte und ist auf diese Methoden umgebaut (eine Regel, zwei Ausgaben), behält aber die Identitätsabkürzung von heute (`OptionSpec.java`, Fall `FLOAT_RANGE`: ein `Float`, der schon auf dem Raster liegt, wird als dieselbe Referenz zurückgegeben): `coerce` boxt nichts, was nicht neu berechnet werden muss. Die typisierten Zugriffe geben Primitive zurück und boxen beim Lesen nie, auch nicht für einen `Double` aus der Datei.
  - `ModuleConfig`: `public boolean bool(OptionSpec spec)`, `public int integer(OptionSpec spec)`, `public float decimal(OptionSpec spec)`, `public int choice(OptionSpec spec)`; lesen den Rohwert aus der Map und rufen die `coerce*`-Methode; keine Allokation. `option(OptionSpec)` bleibt für die Einstellungs-UI.

- [ ] **Step 1: Write the failing tests**

```java
@Test void typed_reads_equal_the_boxed_reads_for_every_raw_value() {
    OptionSpec t = OptionSpec.toggle("t", true);
    OptionSpec i = OptionSpec.intRange("i", 1, 0, 2);
    OptionSpec f = OptionSpec.floatRange("f", 1f, 0.5f, 3f, 0.1f);
    OptionSpec c = OptionSpec.choice("c", "b", "a", "b", "c");
    Object[] raws = {null, Boolean.FALSE, "x", "c", 2L, 9L, -4L, 1.26, 2.6, Double.NaN, -1e300, 1.3f, Float.NaN, 500L};
    for (Object raw : raws) {
        ModuleConfig m = ModuleConfig.defaults();
        m.setOption("t", raw); m.setOption("i", raw); m.setOption("f", raw); m.setOption("c", raw);
        assertEquals(m.option(t), m.bool(t), "toggle " + raw);
        assertEquals(m.option(i), m.integer(i), "int " + raw);
        assertEquals((Float) m.option(f), m.decimal(f), 0f, "float " + raw);
        assertEquals(m.option(c), c.value(m.choice(c)), "choice " + raw);
    }
}
@Test void typed_read_of_the_wrong_kind_throws() {
    ModuleConfig m = ModuleConfig.defaults();
    assertThrows(IllegalArgumentException.class, () -> m.integer(OptionSpec.toggle("t", true)));
    assertThrows(IllegalArgumentException.class, () -> m.bool(OptionSpec.intRange("i", 0, 0, 2)));
    assertThrows(IllegalArgumentException.class, () -> m.decimal(OptionSpec.choice("c", "a", "a")));
    assertThrows(IllegalArgumentException.class, () -> m.choice(OptionSpec.floatRange("f", 1f, 0f, 2f, 0.5f)));
}
@Test void choice_index_value_and_default_index() {
    OptionSpec c = OptionSpec.choice("c", "b", "a", "b", "c");
    assertEquals(1, c.defaultIndex());
    ModuleConfig m = ModuleConfig.defaults();
    assertEquals(1, m.choice(c));
    m.setOption("c", "c");
    assertEquals(2, m.choice(c));
    assertSame(c.value(2), c.value(2));
}
@Test void coerce_keeps_the_identity_shortcut_for_a_float_on_the_grid() {
    OptionSpec f = OptionSpec.floatRange("f", 1f, 0.5f, 3f, 0.1f);
    Object onGrid = f.coerce(1.79);               // 1.8f, boxed once
    assertSame(onGrid, f.coerce(onGrid));
    assertEquals(1.8f, (Float) onGrid, 0f);
}
```

In `OptionReadAllocationTest` (bleibt `@Tag("interpreted")`):

```java
@Test void typed_reads_allocate_nothing() {
    OptionSpec t = OptionSpec.toggle("t", true), i = OptionSpec.intRange("i", 0, 0, 1000),
            f = OptionSpec.floatRange("f", 1f, 0.5f, 3f, 0.1f), c = OptionSpec.choice("c", "a", "a", "b");
    ModuleConfig m = ModuleConfig.defaults();
    m.setOption("t", Boolean.TRUE); m.setOption("i", 500L); m.setOption("f", 1.26); m.setOption("c", "b");
    long sink = 0;
    for (int k = 0; k < 1000; k++) { sink += (m.bool(t) ? 1 : 0) + m.integer(i) + (int) m.decimal(f) + m.choice(c); }
    long before = allocated();
    for (int k = 0; k < 10000; k++) { sink += (m.bool(t) ? 1 : 0) + m.integer(i) + (int) m.decimal(f) + m.choice(c); }
    assertEquals(0, allocated() - before);
    assertTrue(sink > 0);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `./gradlew -p common test --tests '*TypedOptionTest'`
Expected: FAIL, Methoden fehlen.

- [ ] **Step 3: Implement die Methoden aus dem Interfaces-Block, dann die M0-Module umziehen**

`CpsModule`: `cfg.bool(SHOW_RIGHT)`; `CoordinatesModule`: `cfg.integer(DECIMALS)`; `KeystrokesModule`: `cfg.bool(SHOW_MOUSE)`, `cfg.bool(SHOW_SPACE)`. Javadoc von `HudModule.render`: Optionen über die typisierten Zugriffe in `tick` lesen. Die Einstellungs-Widgets bleiben bei `option()`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `./gradlew -p common test` (darin ausdrücklich `OptionReadAllocationTest.a_float_option_stored_on_the_grid_is_read_without_allocation`, der weiter 0 Bytes verlangt), dann `./gradlew buildAll`
Expected: PASS, alle bisherigen Tests grün; `BUILD SUCCESSFUL`.

- [ ] **Step 5: Commit**

```bash
git add mod/common/src
git commit -m "feat(mod-common): typed option reads without boxing, m0 modules use them

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 2: Vorwärtskompatible Konfiguration (D5)

**Files:**
- Modify: `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/config/Profile.java`, `ModuleConfig.java`, `Settings.java`, `SettingsStore.java`; `docs/begleitmod-spec.md` (Abschnitt 5, Absatz "Konfiguration")
- Test: `mod/common/src/test/java/io/github/clemensjl/hengst/mod/common/config/ProfileStoreTest.java`, `SettingsStoreTest.java`, `M0CompatibilityTest.java` (neu)
- Test-Ressourcen (neu): `mod/common/src/test/resources/config/m1a-profile.json`, `m1a-settings.json` (Dateien, wie M1a sie schreibt)

**Interfaces:**
- Consumes: `Json`, `JsonObjects`, `AtomicFiles.SCHEMA` (M0).
- Produces: keine neue öffentliche API. Verhalten: `Profile` erhält alle Wurzelschlüssel außer `schema` und `modules` (Reihenfolge wie gelesen, nach `modules` geschrieben); `ModuleConfig` erhält Schlüssel eines Moduleintrags, die es nicht kennt (bekannt: `enabled`, `anchor`, `x`, `y`, `scale`, `background`, `shadow`, `textColor`, `options`), und schreibt sie nach den bekannten; `Settings` erhält Schlüssel außer `schema` und `activeProfile`. `copy()` von Profil und Modul kopiert sie tief mit. Ein bekannter Schlüssel mit falschem Typ fällt wie bisher auf den Standard zurück und wird damit überschrieben. Der Schutz vor Dateien mit höherem Schema bleibt unverändert. Ehrlich zur Rückwärtsrichtung: der schon ausgelieferte M0-Build tut das alles nicht; er verwirft unbekannte Wurzelschlüssel, unbekannte Schlüssel in Einträgen seiner vier Module und unbekannte Schlüssel in `settings.json`, und behält unbekannte Module und unbekannte Schlüssel in `options`. Das ist nicht rückwirkend zu ändern. Was M1a schreibt, liegt deshalb nur an Stellen, die M0 erhält (Global Constraints Regel 12); der Test `M0CompatibilityTest` belegt das mit einer eingefrorenen Kopie des M0-Lese- und Schreibverhaltens.

- [ ] **Step 1: Write the failing tests**

```java
@Test void unknown_keys_survive_at_all_three_levels() throws Exception {
    Files.write(dir.resolve("default.json"), ("{\"schema\":1,\"layoutVersion\":3,\"modules\":{\"fps\":{\"enabled\":true,"
            + "\"zIndex\":7,\"options\":{\"future_opt\":[1,2]}}},\"servers\":{\"a.net\":{\"deny\":[\"x\"]}}}").getBytes(UTF_8));
    Profile p = store().load("default");
    p.module("fps").setOffsetX(5);
    store().save("default", p);
    Map<String, Object> back = JsonObjects.object(Json.parse(new String(Files.readAllBytes(dir.resolve("default.json")), UTF_8)));
    assertEquals(Arrays.asList("schema", "modules", "layoutVersion", "servers"), new ArrayList<>(back.keySet()));
    assertEquals(3L, back.get("layoutVersion"));
    assertEquals(Collections.singletonMap("a.net", Collections.singletonMap("deny", Arrays.asList("x"))), back.get("servers"));
    Map<String, Object> fps = JsonObjects.object(JsonObjects.object(back.get("modules")).get("fps"));
    assertEquals(7L, fps.get("zIndex"));
    assertEquals(5L, fps.get("x"));
    assertEquals(Arrays.asList(1L, 2L), JsonObjects.object(fps.get("options")).get("future_opt"));
}
@Test void a_profile_copy_keeps_unknown_keys() throws Exception {
    Files.write(dir.resolve("default.json"), "{\"schema\":1,\"extra\":true,\"modules\":{\"fps\":{\"zIndex\":7}}}".getBytes(UTF_8));
    String name = store().createCopy("default");
    String json = new String(Files.readAllBytes(dir.resolve(name + ".json")), UTF_8);
    assertTrue(json.contains("\"extra\": true"));
    assertTrue(json.contains("\"zIndex\": 7"));
}
@Test void m1a_profile_uses_only_keys_an_m0_build_preserves() throws Exception {
    Profile p = new Profile();
    p.module("stopwatch").setEnabled(true);
    p.module("fps").setOption("new_option", true);
    store().save("default", p);
    Map<String, Object> root = JsonObjects.object(Json.parse(new String(Files.readAllBytes(dir.resolve("default.json")), UTF_8)));
    assertEquals(new HashSet<>(Arrays.asList("schema", "modules")), root.keySet());
    Set<String> m0Keys = new HashSet<>(Arrays.asList("enabled", "anchor", "x", "y", "scale", "background", "shadow", "textColor", "options"));
    for (Object entry : JsonObjects.object(root.get("modules")).values()) {
        assertTrue(m0Keys.containsAll(JsonObjects.object(entry).keySet()), "module keys " + entry);
    }
}
```

In `SettingsStoreTest`:

```java
@Test void unknown_settings_keys_survive_a_save() throws Exception {
    Files.write(file, "{\"schema\":1,\"activeProfile\":\"Profil 2\",\"servers\":{\"mc.x.net\":{\"off\":[\"minimap\"]}}}".getBytes(UTF_8));
    Settings s = store().load();
    s.setActiveProfile("default");
    store().save(s);
    Map<String, Object> back = JsonObjects.object(Json.parse(new String(Files.readAllBytes(file), UTF_8)));
    assertEquals("default", back.get("activeProfile"));
    assertEquals(Collections.singletonMap("mc.x.net", Collections.singletonMap("off", Arrays.asList("minimap"))), back.get("servers"));
}
```

(`store()` und `file` wie in den bestehenden Tests der Klasse; das Ausgabeformat `"key": value` mit Leerzeichen ist das von `Json.write`.)

Ressource `config/m1a-profile.json` (so schreibt M1a ein Profil mit je einem neuen Modul und je einer Option):

```json
{
  "schema": 1,
  "modules": {
    "fps": {"enabled": true, "anchor": "TOP_LEFT", "x": 2, "y": 2, "scale": 1.0, "background": true, "shadow": true, "textColor": 4294967295, "options": {}},
    "direction": {"enabled": true, "anchor": "TOP_LEFT", "x": 2, "y": 44, "scale": 1.0, "background": true, "shadow": true, "textColor": 4294967295, "options": {"style": "degrees"}},
    "clock": {"enabled": false, "anchor": "TOP_RIGHT", "x": -2, "y": 66, "scale": 1.5, "background": false, "shadow": true, "textColor": 4294967040, "options": {"format": "12h", "seconds": true}},
    "stopwatch": {"enabled": true, "anchor": "TOP_LEFT", "x": 2, "y": 72, "scale": 1.0, "background": true, "shadow": false, "textColor": 4294967295, "options": {"tenths": false}}
  }
}
```

Ressource `config/m1a-settings.json`: `{"schema": 1, "activeProfile": "Profil 2"}`.

```java
// M0CompatibilityTest. m0RewriteProfile und m0RewriteSettings sind die eingefrorene Kopie dessen, was der ausgelieferte
// M0-Build beim Laden und Speichern tut (nur Test-Code, nie in main): Profile.fromJson behält nur "modules"; der Eintrag
// eines der vier M0-Module wird von ModuleConfig neu aufgebaut (nur die bekannten Eintragsschlüssel, alle Schlüssel
// in "options"); Einträge anderer Module bleiben roh; SettingsStore.save schreibt nur "schema" und "activeProfile".
private static final Set<String> M0_MODULES = new HashSet<>(Arrays.asList("fps", "cps", "keystrokes", "coordinates"));
private static final Set<String> M0_ENTRY_KEYS = new HashSet<>(Arrays.asList(
        "enabled", "anchor", "x", "y", "scale", "background", "shadow", "textColor", "options"));

static Map<String, Object> m0RewriteProfile(Map<String, Object> root) {
    Map<String, Object> modules = new LinkedHashMap<>();
    for (Map.Entry<String, Object> e : JsonObjects.object(root.get("modules")).entrySet()) {
        if (!M0_MODULES.contains(e.getKey()) || !(e.getValue() instanceof Map)) { modules.put(e.getKey(), e.getValue()); continue; }
        Map<String, Object> kept = new LinkedHashMap<>();
        for (Map.Entry<String, Object> k : JsonObjects.object(e.getValue()).entrySet()) {
            if (M0_ENTRY_KEYS.contains(k.getKey())) { kept.put(k.getKey(), k.getValue()); }
        }
        modules.put(e.getKey(), kept);
    }
    Map<String, Object> out = new LinkedHashMap<>();
    out.put("schema", 1L); out.put("modules", modules);
    return out;
}
static Map<String, Object> m0RewriteSettings(Map<String, Object> root) {
    Map<String, Object> out = new LinkedHashMap<>();
    out.put("schema", 1L); out.put("activeProfile", root.get("activeProfile"));
    return out;
}
private String resource(String name) throws IOException {
    try (InputStream in = M0CompatibilityTest.class.getResourceAsStream(name)) { return new String(in.readAllBytes(), UTF_8); }
}

@Test void an_m0_round_trip_keeps_everything_m1a_wrote() throws Exception {
    Map<String, Object> original = JsonObjects.object(Json.parse(resource("/config/m1a-profile.json")));
    Map<String, Object> afterM0 = m0RewriteProfile(original);
    assertEquals(original, afterM0);                                   // nothing dropped, nothing changed
    Files.write(dir.resolve("default.json"), Json.write(afterM0).getBytes(UTF_8));
    Profile p = new ProfileStore(dir, clock, w -> { }).load("default");
    OptionSpec style = OptionSpec.choice("style", "cardinal_degrees", "cardinal", "cardinal_degrees", "degrees");
    OptionSpec format = OptionSpec.choice("format", "24h", "24h", "12h");
    assertTrue(p.module("direction").enabled());
    assertEquals("degrees", p.module("direction").option(style));
    assertFalse(p.module("clock").enabled());
    assertEquals(Anchor.TOP_RIGHT, p.module("clock").anchor());
    assertEquals("12h", p.module("clock").option(format));
    assertEquals(Boolean.TRUE, p.module("clock").option(OptionSpec.toggle("seconds", false)));
    assertEquals(1.5f, p.module("clock").scale(), 0f);
    assertEquals(Boolean.FALSE, p.module("stopwatch").option(OptionSpec.toggle("tenths", true)));
    assertFalse(p.module("stopwatch").shadow());
    Map<String, Object> settings = JsonObjects.object(Json.parse(resource("/config/m1a-settings.json")));
    assertEquals(settings, m0RewriteSettings(settings));
    Files.write(dir.resolve("settings.json"), Json.write(m0RewriteSettings(settings)).getBytes(UTF_8));
    assertEquals("Profil 2", new SettingsStore(dir.resolve("settings.json"), clock, w -> { }).load().activeProfile());
}
@Test void an_m0_round_trip_loses_future_root_and_entry_keys_and_the_m1a_reader_sees_that() throws Exception {
    // Not a promise but the recorded limit: what a later version adds outside the M0-safe places is gone after an M0 run.
    String future = "{\"schema\":1,\"layoutVersion\":3,\"modules\":{\"fps\":{\"enabled\":true,\"zIndex\":7,"
            + "\"options\":{\"future_opt\":[1,2]}},\"waypoints\":{\"enabled\":true,\"zIndex\":9,\"options\":{\"n\":1}}},"
            + "\"servers\":{\"a.net\":{\"deny\":[\"x\"]}}}";
    Map<String, Object> afterM0 = m0RewriteProfile(JsonObjects.object(Json.parse(future)));
    assertEquals(Arrays.asList("schema", "modules"), new ArrayList<>(afterM0.keySet()));           // layoutVersion, servers: gone
    Map<String, Object> modules = JsonObjects.object(afterM0.get("modules"));
    Map<String, Object> fps = JsonObjects.object(modules.get("fps"));
    assertFalse(fps.containsKey("zIndex"));                                                         // entry key of an M0 module: gone
    assertEquals(Arrays.asList(1L, 2L), JsonObjects.object(fps.get("options")).get("future_opt"));  // option key: kept
    assertEquals(9L, JsonObjects.object(modules.get("waypoints")).get("zIndex"));                   // unknown module: kept whole
    Files.write(dir.resolve("default.json"), Json.write(afterM0).getBytes(UTF_8));
    ProfileStore store = new ProfileStore(dir, clock, w -> { });
    store.save("default", store.load("default"));
    Map<String, Object> back = JsonObjects.object(Json.parse(new String(Files.readAllBytes(dir.resolve("default.json")), UTF_8)));
    assertEquals(new HashSet<>(Arrays.asList("schema", "modules")), back.keySet());
    assertEquals(9L, JsonObjects.object(JsonObjects.object(back.get("modules")).get("waypoints")).get("zIndex"));
    Map<String, Object> futureSettings = JsonObjects.object(Json.parse("{\"schema\":1,\"activeProfile\":\"default\",\"servers\":{\"a\":1}}"));
    assertEquals(new HashSet<>(Arrays.asList("schema", "activeProfile")), m0RewriteSettings(futureSettings).keySet());
}
```

(Importe: `java.io.InputStream`, `java.util.*`, `Anchor`, `OptionSpec`, `Json`, `JsonObjects`. `m0Rewrite*` rufen keinen Konfigurationscode aus `main`. Weicht das tatsächliche M0-Verhalten von dieser Kopie ab, korrigiert der Implementierer die Kopie nach `git show begleitmod-m0:mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/config/Profile.java`, `ModuleConfig.java` und `SettingsStore.java`, nicht den Test.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `./gradlew -p common test --tests '*ProfileStoreTest' --tests '*SettingsStoreTest'`
Expected: FAIL, unbekannte Schlüssel fehlen nach dem Speichern.

- [ ] **Step 3: Implement**

Je eine `LinkedHashMap<String, Object> extras` in `Profile`, `ModuleConfig` und `Settings` (package-private Zugriff für `SettingsStore`); `ModuleConfig.deepCopy` für Lesen, Kopieren und Schreiben. Spec 5, nach dem Satz "Unbekannte Module bleiben beim Speichern erhalten.": "Ab M1a bleiben unbekannte Schlüssel auf allen drei Ebenen erhalten (Wurzel eines Profils, Moduleintrag, `settings.json`), damit eine neuere Version nichts verliert, was eine noch neuere schrieb. Ein schon ausgelieferter M0-Build tut das nicht: er verwirft unbekannte Wurzelschlüssel, unbekannte Schlüssel in den Einträgen seiner vier Module und unbekannte Schlüssel in `settings.json` und behält nur unbekannte Module und unbekannte Schlüssel in `options`; das lässt sich nicht nachträglich beheben. Darum gilt für neue persistierte Daten, solange M0-Builds in Gebrauch sind: nur als eigener Moduleintrag oder innerhalb von `options` eines Moduls, nichts Neues an der Wurzel eines Profils, nichts Neues direkt in einem Moduleintrag, nichts Neues in `settings.json`, dessen Verlust ein Spieler bemerken würde. Tastenbelegungen liegen in der `options.txt` des Spiels; das Spiel selbst verwirft nach einem M0-Lauf unbekannte Tastenzeilen (bekannte Einschränkung). Die Schemanummer bleibt 1 bei additiven Änderungen (neue Module, Optionen, Felder) und steigt nur bei inkompatiblen (ein bestehendes Feld ändert Bedeutung oder Namen); Dateien mit höherem Schema werden weiter nie überschrieben."

- [ ] **Step 4: Run tests to verify they pass**

Run: `./gradlew -p common test`, dann `./gradlew buildAll`
Expected: PASS; `BUILD SUCCESSFUL`.

- [ ] **Step 5: Commit**

```bash
git add mod/common/src docs/begleitmod-spec.md
git commit -m "feat(mod-common): keep unknown config keys at every level, m0 round-trip test, schema rule in the spec

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 3: Ein Textanpassungs-Helfer `TextFit` (F9)

**Files:**
- Create: `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/ui/TextFit.java`
- Delete: `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/ui/TextWrap.java`
- Modify: `.../ui/Widget.java` (`fitted` nutzt `TextFit.prefix`, Zwischenspeicher bleibt), `.../ui/ChoiceWidget.java` (`middleCut` entfällt, `TextFit.middle`), `.../hud/modules/KeystrokesModule.java` (`fit` nutzt `TextFit.prefix`), `.../ui/SettingsScreen.java`, `.../editor/HudEditorScreen.java` (`TextFit.wrap`)
- Test: `mod/common/src/test/java/io/github/clemensjl/hengst/mod/common/ui/TextFitTest.java`; bestehende Aufrufe von `TextWrap` oder `ChoiceWidget.middleCut` in Tests auf `TextFit` umstellen

**Interfaces:**
- Produces: `public final class TextFit` mit
  - `public static String prefix(Canvas c, String text, int avail)`: der Text selbst (dieselbe Referenz), wenn er passt; sonst das längste passende Anfangsstück; `null` ergibt `""`.
  - `public static String middle(Canvas c, String text, int avail)`: der Text selbst, wenn er passt; sonst Anfang + `".."` + Ende mit möglichst vielen Zeichen (bei ungerader Zahl ein Zeichen mehr vorn); passt nichts, `".."`.
  - `public static String[] wrap(Canvas c, String text, int width)`: gierig nach Wörtern, ein Wort breiter als die Zeile bekommt eine eigene Zeile.
  Alle drei sind kalte Pfade (aufrufen, wenn Text oder Breite sich ändern) und verhalten sich genau wie die vier ersetzten Kopien.

- [ ] **Step 1: Write the failing tests** (FakeCanvas: 6 px je Zeichen)

```java
FakeCanvas c = new FakeCanvas();
@Test void prefix_keeps_a_fitting_text_and_cuts_the_end_otherwise() {
    String s = "Hello";
    assertSame(s, TextFit.prefix(c, s, 30));
    assertEquals("Hel", TextFit.prefix(c, s, 18));
    assertEquals("", TextFit.prefix(c, s, 5));
    assertEquals("", TextFit.prefix(c, null, 30));
}
@Test void middle_keeps_start_and_end() {
    assertEquals("abc..ij", TextFit.middle(c, "abcdefghij", 42));
    assertEquals("abcdefghij", TextFit.middle(c, "abcdefghij", 60));
    assertEquals("..", TextFit.middle(c, "abcdefghij", 13));
}
@Test void wrap_by_words_and_long_words_alone() {
    assertArrayEquals(new String[] {"aa bb", "cc"}, TextFit.wrap(c, "aa bb cc", 30));
    assertArrayEquals(new String[] {"a", "verylongword", "b"}, TextFit.wrap(c, "a verylongword b", 30));
    assertArrayEquals(new String[0], TextFit.wrap(c, "   ", 30));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `./gradlew -p common test --tests '*TextFitTest'`
Expected: FAIL, Klasse fehlt.

- [ ] **Step 3: Implement `TextFit`, die vier Kopien darauf umstellen, `TextWrap` löschen**

- [ ] **Step 4: Run tests to verify they pass**

Run: `./gradlew -p common test`, dann `./gradlew buildAll`
Expected: PASS (auch `SettingsScreenTest` mit den Profilnamen-Tests und `HudEditorScreenTest`); `BUILD SUCCESSFUL`.

- [ ] **Step 5: Commit**

```bash
git add mod/common/src
git commit -m "refactor(mod-common): one TextFit for prefix cut, middle cut and wrap

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 4: `AdapterGuard` und `ScrollAccumulator` in common, Adapter umziehen (F10)

**Files:**
- Create: `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/AdapterGuard.java`, `.../platform/ScrollAccumulator.java`
- Modify: `mod/modern/src/main/java/io/github/clemensjl/hengst/mod/modern/HengstModernMod.java`, `ModernInput.java` (Klasse `ScrollNotches` entfällt), `ModernScreenHost.java`; `mod/legacy/src/main/java/io/github/clemensjl/hengst/mod/legacy/HengstLegacyMod.java`, `LegacyInput.java` (`ScrollNotches` entfällt), `LegacyScreenHost.java`
- Test: `mod/common/src/test/java/io/github/clemensjl/hengst/mod/common/AdapterGuardTest.java`, `.../platform/ScrollAccumulatorTest.java`; `mod/modern/src/test/.../HengstModernModTest.java`, `ModernInputTest.java`, `mod/legacy/src/test/.../HengstLegacyModTest.java`, `LegacyInputTest.java` (Fälle zu Start, Hook-Fehlern und Mausrad wandern nach common; dort bleibt nur, was den Adapter selbst betrifft)

**Interfaces:**
- Produces:
  - `public final class AdapterGuard { public interface Log { void info(String message); void error(String message, Throwable error); } public static final int MAX_KINDS = 4; public AdapterGuard(String[] siteNames, Log log); public HengstCore start(Supplier<? extends Platform> platformFactory, TimeSource time); public boolean hookFailed(int site, Throwable error); }`
    - `start`: fängt `RuntimeException | LinkageError` beim Erzeugen der Plattform und beim Start des Kerns; Logzeilen wörtlich wie heute: `info("[hengst] loaded on " + version.id())`, bei Fehler `error("[hengst] core failed to start; the mod stays inactive and its key bindings do nothing", e)` und Rückgabe `null`.
    - `hookFailed`: bei `LinkageError` Rückgabe `true` (der Aufrufer setzt sein `ENABLED` auf false), geloggt nur beim ersten Mal: `"[hengst] the <site> hook does not match this game; the mod is disabled"`. Sonst `false`, geloggt einmal je Stelle und Ausnahmeklasse, höchstens `MAX_KINDS` Klassen je Stelle: `"[hengst] the <site> hook failed; further <SimpleName> there are not logged"`. Kalter Pfad.
  - `public final class ScrollAccumulator { public static final int MAX_NOTCHES = 100; public ScrollAccumulator(double unitsPerNotch); public int add(double amount); public void reset(); }`: summiert Rohwerte und gibt ganze Rasten mit Vorzeichen zurück (Richtung zum Nutzer hin negativ, wie geliefert); `1e-9` Toleranz, damit zehnmal 0,1 eine Raste ergibt; NaN und unendlich ergeben 0 und verändern den Rest nicht; jeder endliche Wert wird verarbeitet, und mehr als 100 Rasten je Ereignis werden auf ±100 gekappt, der Rest und der Überschuss verworfen (genau wie beide Adapter heute: `LegacyInput.ScrollNotches.add(int)` kappt `Integer.MAX_VALUE`/`Integer.MIN_VALUE` auf ±100, `ModernInput.ScrollNotches.add(double)` kappt 500,25 und -1e5 auf ±100). Es gibt keine eigene Obergrenze für das Roh-Ereignis mehr; der einzige Unterschied zu heute: ein endlicher Wert ab 1e6 liefert auf den modernen Versionen jetzt ±100 statt 0 (kein bestehender Test deckt das ab, beides ist eine verworfene Müll-Eingabe). Die bestehenden Adapter-Tests `LegacyInputTest.notchesPerEventAreCapped` und `ModernInputTest.oneEventEmitsAtMostOneHundredNotches` bleiben inhaltlich gültig und wandern mit denselben Werten nach `ScrollAccumulatorTest`. Modern `new ScrollAccumulator(1.0)`, 1.8.9 `new ScrollAccumulator(120.0)`.
  - Die Mod-Klassen behalten `public static boolean ENABLED`, `core()`, ihre Hook-Methoden und `SITE_*`-Konstanten; `static final AdapterGuard GUARD`; jede Fangstelle ruft `if (GUARD.hookFailed(SITE_X, e)) { ENABLED = false; }`.

- [ ] **Step 1: Write the failing tests**

```java
// AdapterGuardTest
List<String> lines = new ArrayList<>();
AdapterGuard guard = new AdapterGuard(new String[] {"HUD", "client tick"}, new AdapterGuard.Log() {
    public void info(String m) { lines.add("I " + m); }
    public void error(String m, Throwable t) { lines.add("E " + m); }
});
@Test void a_failing_platform_gives_null_and_one_error() {
    assertNull(guard.start(() -> { throw new IllegalStateException("x"); }, TimeSource.SYSTEM));
    assertEquals(Arrays.asList("E [hengst] core failed to start; the mod stays inactive and its key bindings do nothing"), lines);
}
@Test void a_linkage_error_disables_and_logs_once() {
    assertTrue(guard.hookFailed(0, new NoSuchMethodError("m")));
    assertTrue(guard.hookFailed(0, new NoSuchMethodError("m")));
    assertEquals(Arrays.asList("E [hengst] the HUD hook does not match this game; the mod is disabled"), lines);
}
@Test void other_failures_log_once_per_site_and_class_up_to_four_classes() {
    for (int k = 0; k < 3; k++) { assertFalse(guard.hookFailed(1, new IllegalStateException())); }
    guard.hookFailed(1, new IllegalArgumentException()); guard.hookFailed(1, new ArithmeticException());
    guard.hookFailed(1, new NullPointerException()); guard.hookFailed(1, new IndexOutOfBoundsException());
    assertEquals(4, lines.size());
    assertEquals("E [hengst] the client tick hook failed; further IllegalStateException there are not logged", lines.get(0));
}
// ScrollAccumulatorTest
@Test void whole_notches_with_sign_and_remainder() {
    ScrollAccumulator legacy = new ScrollAccumulator(120.0);
    assertEquals(1, legacy.add(120)); assertEquals(0, legacy.add(60)); assertEquals(1, legacy.add(60));
    assertEquals(-1, legacy.add(-130)); assertEquals(0, legacy.add(-100)); assertEquals(-1, legacy.add(-10));
    ScrollAccumulator modern = new ScrollAccumulator(1.0);
    int sum = 0; for (int k = 0; k < 10; k++) { sum += modern.add(0.1); }
    assertEquals(1, sum);
}
@Test void nan_and_infinity_are_dropped_and_large_values_capped_at_100_notches() {
    ScrollAccumulator a = new ScrollAccumulator(1.0);
    assertEquals(0, a.add(Double.NaN)); assertEquals(0, a.add(Double.POSITIVE_INFINITY)); assertEquals(0, a.add(Double.NEGATIVE_INFINITY));
    assertEquals(100, a.add(500.25)); assertEquals(0, a.add(0.0));          // the remainder .25 is dropped with the excess
    assertEquals(-100, a.add(-1e5)); assertEquals(0, a.add(0.0));
    assertEquals(100, a.add(2e6));                                          // finite and absurd: capped, not dropped
    assertEquals(1, a.add(1.0));
    a.reset(); assertEquals(0, a.add(0.6)); assertEquals(1, a.add(0.4));
}
@Test void legacy_int_deltas_at_the_extremes_are_capped_like_today() {
    ScrollAccumulator legacy = new ScrollAccumulator(120.0);
    assertEquals(100, legacy.add(Integer.MAX_VALUE));
    assertEquals(-100, legacy.add(Integer.MIN_VALUE));
    assertEquals(1, legacy.add(120));                                       // excess and remainder were dropped
    assertEquals(0, legacy.add(100)); legacy.reset(); assertEquals(0, legacy.add(100)); assertEquals(1, legacy.add(20));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `./gradlew -p common test --tests '*AdapterGuardTest' --tests '*ScrollAccumulatorTest'`
Expected: FAIL, Klassen fehlen.

- [ ] **Step 3: Implement, dann beide Adapter umziehen**

`startCore` der Mod-Klassen wird `GUARD.start(...)`; der Logger-Adapter für `AdapterGuard.Log` ist slf4j (modern) bzw. log4j (1.8.9) mit vorgebautem String. `ModernScreenHost` und `LegacyScreenHost` halten je einen `ScrollAccumulator`.

- [ ] **Step 4: Run tests and the games**

Run: `./gradlew -p common test`, `./gradlew -p legacy test`, `./gradlew -p modern :1.21.11:test`, `./gradlew -p modern :26.3:test`, `./gradlew buildAll`, dann nacheinander `./gradlew -p modern :26.3:clientGameTests`, `./gradlew -p modern :1.21.11:clientGameTests`, `./gradlew -p legacy gameTests`
Expected: alles grün; die Spieltest-Schritte zum Mausrad ("scroll notches", "wheel arrives as whole notches", "half notches accumulate") melden PASS; `unzip -l` der drei `dist/`-Jars zeigt keine Klasse aus `gametest`.

- [ ] **Step 5: Commit**

```bash
git add mod/common/src mod/modern/src mod/legacy/src
git commit -m "refactor(mod): adapter start guard, hook failure log and wheel accumulator in common

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 5: Funktionsschalter (D1), Standardplätze, ein Schließ-Runnable (F8)

**Files:**
- Create: `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/feature/FeatureGate.java`, `FeatureState.java`, `RestrictionPolicy.java`, `.../ui/ScreenClose.java`
- Delete: `.../ui/SettingsClose.java`, `.../editor/HudEditorClose.java`
- Modify: `.../feature/Feature.java`, `.../feature/FeatureRegistry.java`, `.../hud/HudModule.java`, die vier M0-Module (Standardplätze), `.../hud/HudRenderer.java`, `.../HengstCore.java`
- Test: `mod/common/src/test/java/io/github/clemensjl/hengst/mod/common/feature/FeatureStateTest.java` (neu), `.../ui/ScreenCloseTest.java` (neu), `.../HengstCoreTest.java`; Tests, die `SettingsClose` oder `HudEditorClose` nutzen (`SettingsScreenTest`, `HudEditorScreenTest`), auf `ScreenClose` umstellen

**Interfaces:**
- Consumes: `Profile`, `ModuleConfig`, `FeatureRegistry`, `HudRenderer` (M0).
- Produces:
  - `public interface RestrictionPolicy { boolean restricted(Feature feature, String server); RestrictionPolicy NONE = ...; }` (`server`: `GameView.serverAddress()` wie gemeldet, null im Einzelspieler; darf nicht allozieren; die echte Sperrliste kommt in M4).
  - `public final class FeatureGate { public FeatureGate(GameVersion version, RestrictionPolicy policy); public boolean active(Feature f, ModuleConfig cfg) /* cfg.enabled() && f.supports(version) && !policy.restricted(f, server) */; public void setServer(String server); public String server(); public GameVersion version(); }`
  - `public final class FeatureState { public static boolean on(int number) /* false für unbekannte und negative Nummern */; public static void rewrite(List<Feature> features, Profile profile, FeatureGate gate) /* nur der Kern ruft das */; }`: statisches `boolean[]`, Index = Funktionsnummer, wächst bei Bedarf (das 1.8.9-Harness registriert Nummern ab 900); `rewrite` setzt erst alles auf false. Mixins späterer Meilensteine lesen `FeatureState.on(23)` als erste Anweisung.
  - `FeatureRegistry`: `public List<Feature> all()` (Nummernfolge, unveränderlich; kalter Pfad).
  - `Feature`: `public void onConfigChanged(ModuleConfig cfg) {}`: der Kern ruft es für jede auf dieser Version unterstützte Funktion mit ihrer eigenen Konfiguration bei Start, nach dem Schließen der Einstellungen und des Editors, beim Profilwechsel und beim Serverwechsel; Funktionen ohne HUD kopieren dort ihre Optionen in primitive Felder (HUD-Module lesen weiter in `tick`, Regel 6). Ausnahmen daraus werden geloggt (eine neue Fehlerstelle `SITE_CONFIG` im Kern) und brechen nichts ab.
  - `HudModule`: `public Anchor defaultAnchor()` (TOP_LEFT), `public int defaultOffsetX()` (0), `public int defaultOffsetY()` (0); die M0-Module liefern ihre M0-Plätze (siehe Tabelle oben).
  - `HudRenderer`: neuer Konstruktor `HudRenderer(FeatureRegistry registry, FeatureGate gate)`; `rebuild` nimmt ein Modul genau dann, wenn `gate.active(...)`; der alte Konstruktor `(FeatureRegistry, GameVersion)` bleibt und nutzt `RestrictionPolicy.NONE`.
  - `public final class ScreenClose implements Runnable { public interface Save { void run() throws IOException; } public ScreenClose(Save save, Runnable rebuild, Platform platform, String what); }`: `run` speichert (`IOException` und `RuntimeException` werden als `"Could not save " + what + ": " + e` geloggt), baut neu (`RuntimeException` als `"Could not rebuild the HUD after closing " + what + ": " + e`), ruft `platform.closeScreen()` im `finally`. `what`: `"the settings"` bzw. `"the HUD editor (profile <name>)"`.
  - `HengstCore`: privates `applyConfig()` = `renderer.rebuild(profile)`, `FeatureState.rewrite(registry.all(), profile, gate)`, `onConfigChanged` je unterstützter Funktion; ersetzt jedes bisherige `renderer.rebuild(profile)` im Kern. Nach jedem Laden eines Profils (Start, Wechsel) erhält jedes HUD-Modul ohne Eintrag (`profile.moduleIds()`) einen Eintrag an seinem Standardplatz, ausgeschaltet; `applyFirstStartDefaults` schaltet nur noch die vier M0-Module ein. Neu: `static HengstCore start(Platform, TimeSource, Clock, RestrictionPolicy)` (package-private, für Tests); `public void applyConfigForTest()` (nur Tests, auch die Spieltests). `registerForTest` ruft `applyConfig()`.

- [ ] **Step 1: Write the failing tests**

```java
// HengstCoreTest
@Test void the_feature_state_follows_enabled_supported_and_restricted() {
    core = HengstCore.start(platform, time, clock, (f, server) -> f.id().equals("cps"));
    assertTrue(FeatureState.on(1)); assertFalse(FeatureState.on(2)); assertTrue(FeatureState.on(3)); assertTrue(FeatureState.on(6));
    assertFalse(FeatureState.on(7)); assertFalse(FeatureState.on(0)); assertFalse(FeatureState.on(-1)); assertFalse(FeatureState.on(999));
    assertEquals(3, core.renderer().activeCount());
}
@Test void the_feature_state_is_rewritten_when_the_settings_close() {
    core = start(); platform.inWorld = true;
    core.onOpenSettingsKey();
    platform.openedScreen.render(canvas, 0, 0, 0f);
    platform.openedScreen.mouseClicked(FPS_SWITCH_X, FPS_SWITCH_Y, 0);   // FPS aus
    assertTrue(FeatureState.on(1));
    platform.openedScreen.keyPressed(UiKey.ESCAPE, false, false);
    assertFalse(FeatureState.on(1));
    assertEquals(3, core.renderer().activeCount());
}
@Test void an_unsupported_feature_is_off_in_the_state() {
    core = start();
    core.activeProfile().module("future").setEnabled(true);
    core.registerForTest(new FixedSizeModule("future", 49, 10, 10, EnumSet.of(GameVersion.V26_3)));
    assertFalse(FeatureState.on(49));
}
@Test void on_config_changed_reaches_each_supported_feature_with_its_own_config() throws Exception {
    core = start(); platform.inWorld = true;
    RecordingFeature rec = new RecordingFeature("rec", 48);   // private static Klasse im Test: extends TestFeature, onConfigChanged zählt `calls` und merkt sich `last`
    core.registerForTest(rec);
    assertEquals(1, rec.calls);
    core.onOpenSettingsKey();
    platform.openedScreen.keyPressed(UiKey.ESCAPE, false, false);
    assertEquals(2, rec.calls);
    assertSame(core.activeProfile().module("rec"), rec.last);
}
@Test void a_module_without_an_entry_gets_its_default_place_and_stays_off() throws Exception {
    Files.createDirectories(profiles());
    Files.write(defaultFile(), "{\"schema\":1,\"modules\":{\"fps\":{\"enabled\":true,\"anchor\":\"TOP_RIGHT\",\"x\":-2,\"y\":2}}}".getBytes(UTF_8));
    core = start();
    ModuleConfig cps = core.activeProfile().module("cps");
    assertFalse(cps.enabled()); assertEquals(Anchor.TOP_LEFT, cps.anchor()); assertEquals(2, cps.offsetX()); assertEquals(16, cps.offsetY());
    assertEquals(Anchor.TOP_RIGHT, core.activeProfile().module("fps").anchor());
    assertEquals(1, core.renderer().activeCount());
}
// ScreenCloseTest
@Test void save_rebuild_close_in_this_order_and_close_even_after_failures() {
    // FakePlatform is final: the order is recorded by the save and rebuild lambdas, the close is seen through closeCount.
    List<String> order = new ArrayList<>();
    FakePlatform p = new FakePlatform(null);
    new ScreenClose(() -> { order.add("save closeCount=" + p.closeCount); throw new IOException("disk"); },
            () -> { order.add("rebuild closeCount=" + p.closeCount); throw new IllegalStateException(); },
            p, "the settings").run();
    assertEquals(Arrays.asList("save closeCount=0", "rebuild closeCount=0"), order);
    assertEquals(1, p.closeCount);                                    // closed last, in the finally block
    assertEquals(Arrays.asList("Could not save the settings: java.io.IOException: disk",
            "Could not rebuild the HUD after closing the settings: java.lang.IllegalStateException"), p.warnings);
}
@Test void a_clean_close_saves_rebuilds_and_closes_without_warnings() {
    List<String> order = new ArrayList<>();
    FakePlatform p = new FakePlatform(null);
    new ScreenClose(() -> order.add("save closeCount=" + p.closeCount), () -> order.add("rebuild closeCount=" + p.closeCount),
            p, "the HUD editor (profile default)").run();
    assertEquals(Arrays.asList("save closeCount=0", "rebuild closeCount=0"), order);
    assertEquals(1, p.closeCount); assertTrue(p.warnings.isEmpty());
}
```

(`FakePlatform.closeScreen()` zählt `closeCount`, solange kein Bildschirm als vom Spiel entfernt markiert ist; `FakePlatform` bleibt `final` und unverändert.)

`FeatureStateTest`: `rewrite` mit einer Liste aus `TestFeature`s der Nummern 1, 7 und 900 setzt genau die aktiven; ein zweites `rewrite` mit FPS aus setzt `on(1)` zurück auf false.

- [ ] **Step 2: Run tests to verify they fail**

Run: `./gradlew -p common test --tests '*HengstCoreTest' --tests '*FeatureStateTest' --tests '*ScreenCloseTest'`
Expected: FAIL, Typen fehlen.

- [ ] **Step 3: Implement die Typen aus dem Interfaces-Block, Kern umbauen**

Die Sitzungen von Einstellungen und Editor bekommen je ein `ScreenClose` mit `rebuild = this::applyConfig`; `closeEditor` behält seine Logik zu `dirty` und `unwritten`. Einstellungen: `save = model::saveIfDirty` (meldet Fehler selbst). Editor: speichert nur, wenn `model.dirty()`, dann `model.markSaved()`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `./gradlew -p common test`, dann `./gradlew buildAll`
Expected: PASS, alle bestehenden Tests grün (insbesondere `first_start_uses_the_default_places`, die Schließ- und Speichertests); `BUILD SUCCESSFUL`.

- [ ] **Step 5: Commit**

```bash
git add mod/common/src
git commit -m "feat(mod-common): feature state gate for mixins, default places, one screen close path

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 6: Sprache und Uhr im Kontext (D3), Serverwechsel (D6)

**Files:**
- Create: `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/platform/WallClock.java`, `.../platform/SystemWallClock.java` (package-private); `mod/common/src/test/java/io/github/clemensjl/hengst/mod/common/testutil/FakeWallClock.java`
- Modify: `.../hud/HudContext.java`, `.../HengstCore.java`
- Test: `.../HengstCoreTest.java`, `.../HengstCoreAllocationTest.java`

**Interfaces:**
- Consumes: `FeatureGate.setServer`, `applyConfig` (Task 5); `I18n.load` (M0).
- Produces:
  - `public interface WallClock { long currentTimeMillis(); int offsetMillis(long utcMillis); WallClock SYSTEM = SystemWallClock.INSTANCE; }`; `SystemWallClock` liest `System.currentTimeMillis()` und `TimeZone.getOffset(long)` einer beim Laden der Klasse einmal geholten `TimeZone.getDefault()` (eine Änderung der Zeitzone des Betriebssystems während des Spiels wird nicht gesehen; die JVM hält die Standardzone ohnehin fest).
  - `HudContext`: neuer Konstruktor `HudContext(Platform platform, TimeSource time, WallClock wallClock, FrameCounter frames, CpsCounter cps)`; der alte Vier-Parameter-Konstruktor bleibt (`WallClock.SYSTEM`); `public WallClock wallClock()`, `public I18n i18n()` (anfangs `I18n.load(platform.languageCode())`), `public int languageGeneration()` (beginnt bei 0), `public void setI18n(I18n i18n)` (nur der Kern; erhöht die Generation).
  - `HengstCore`: im Tick höchstens einmal je Sekunde `platform.languageCode()` lesen und mit dem zuletzt geladenen Code vergleichen (erst Referenz, dann `equals`); bei Änderung `ctx.setI18n(I18n.load(code))` vor `renderer.tick`. Das ist die eine erlaubte Ausnahme von Spec 8.6 (kein Dateizugriff im Tick): `I18n.load` liest die Sprachdatei aus dem Klassenpfad, einmal je Sprachwechsel, und nur im Tick, nie im Frame-Pfad (`onHudRender` und `HudRenderer.render` laden nichts nach, auch wenn `languageCode()` sich geändert hat). Ohne Wechsel kein Lesen. Einstellungen und Editor nehmen `ctx.i18n()` statt eines eigenen `I18n.load`. In jedem Tick `game.serverAddress()` gegen den letzten Wert (Referenz, dann `equals`); bei Änderung `gate.setServer(server)` und `applyConfig()`. Neu: `static HengstCore start(Platform, TimeSource, Clock, WallClock, RestrictionPolicy)` (package-private); die anderen Überladungen nutzen `WallClock.SYSTEM` und `RestrictionPolicy.NONE`.
  - `testutil/FakeWallClock implements WallClock { public long utcMillis; public int offset; public long switchAtUtc = Long.MAX_VALUE; public int offsetAfterSwitch; public int currentCalls; public int offsetCalls; }` (`currentTimeMillis()` liefert `utcMillis` und zählt `currentCalls`; `offsetMillis(utc)` liefert `offset` vor `switchAtUtc`, sonst `offsetAfterSwitch`, und zählt `offsetCalls`).

- [ ] **Step 1: Write the failing tests**

```java
@Test void a_language_switch_is_picked_up_within_a_second() {
    core = start(); platform.inWorld = true;
    assertEquals("Done", core.context().i18n().get("hengst.button.done"));
    int gen = core.context().languageGeneration();
    platform.languageCode = "de_de";
    time.advanceMillis(1000); core.onClientTick();
    assertEquals("Fertig", core.context().i18n().get("hengst.button.done"));
    assertEquals(gen + 1, core.context().languageGeneration());
    for (int i = 0; i < 100; i++) { time.advanceMillis(50); core.onClientTick(); }
    assertEquals(gen + 1, core.context().languageGeneration());      // loaded once per switch, not once per second
}
@Test void a_frame_never_reloads_the_language() {
    core = start(); platform.inWorld = true;
    int gen = core.context().languageGeneration();
    platform.languageCode = "de_de";
    time.advanceMillis(5000); core.onHudRender(canvas, 0f);          // frames only, no tick
    assertEquals(gen, core.context().languageGeneration());
    core.onClientTick();
    assertEquals(gen + 1, core.context().languageGeneration());
}
@Test void the_settings_opened_after_a_switch_are_german() {
    core = start(); platform.inWorld = true;
    platform.languageCode = "de_DE"; time.advanceMillis(1000); core.onClientTick();
    core.onOpenSettingsKey();
    FakeCanvas c = new FakeCanvas(); platform.openedScreen.render(c, 0, 0, 0f);
    assertTrue(c.calls.stream().anyMatch(s -> s.startsWith("text Fertig ")));
}
@Test void a_server_change_rewrites_the_state_with_the_restriction() {
    core = HengstCore.start(platform, time, clock, new FakeWallClock(), (f, s) -> "mc.example.net".equals(s) && f.id().equals("fps"));
    platform.inWorld = true; core.onClientTick();
    assertTrue(FeatureState.on(1)); assertEquals(4, core.renderer().activeCount());
    platform.serverAddress = "mc.example.net"; core.onClientTick();
    assertFalse(FeatureState.on(1)); assertEquals(3, core.renderer().activeCount());
    platform.serverAddress = null; core.onClientTick();
    assertTrue(FeatureState.on(1)); assertEquals(4, core.renderer().activeCount());
}
@Test void an_equal_server_string_is_not_a_change() {
    int[] asked = {0};
    core = HengstCore.start(platform, time, clock, new FakeWallClock(), (f, s) -> { asked[0]++; return false; });
    platform.inWorld = true; platform.serverAddress = "a.net"; core.onClientTick();
    int after = asked[0];
    for (int i = 0; i < 100; i++) { platform.serverAddress = new String("a.net"); core.onClientTick(); }
    assertEquals(after, asked[0]);
}
```

In `HengstCoreAllocationTest` (interpreted): `ticks_with_a_constant_server_and_language_allocate_nothing`: Start, `serverAddress = "mc.example.net"`, `languageCode = "de_de"`, 200 Ticks mit je 50 ms zum Einschwingen, dann 2000 Ticks mit je 50 ms und je einem Frame: 0 Bytes.

- [ ] **Step 2: Run tests to verify they fail**

Run: `./gradlew -p common test --tests '*HengstCoreTest'`
Expected: FAIL.

- [ ] **Step 3: Implement die Typen und Kernpfade aus dem Interfaces-Block**

- [ ] **Step 4: Run tests to verify they pass**

Run: `./gradlew -p common test`, dann `./gradlew buildAll`
Expected: PASS inklusive `interpretedTest`; `BUILD SUCCESSFUL`.

- [ ] **Step 5: Commit**

```bash
git add mod/common/src
git commit -m "feat(mod-common): language reload and wall clock in the hud context, server change detection

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 7: Tastenbelegungen je Funktion im Kern (D2)

**Files:**
- Create: `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/feature/KeySpec.java`, `.../feature/DefaultKey.java`, `.../FeatureCatalog.java`
- Modify: `.../feature/Feature.java`, `.../HengstCore.java` (Registrierung über `FeatureCatalog.create()`)
- Test: `mod/common/src/test/java/io/github/clemensjl/hengst/mod/common/HengstCoreKeysTest.java` (neu), `.../HengstCoreAllocationTest.java`

**Interfaces:**
- Consumes: `FeatureState.on` (Task 5).
- Produces:
  - `public enum DefaultKey { NONE, RIGHT_SHIFT }` (unabhängig von LWJGL 2, GLFW und SDL; spätere Meilensteine ergänzen Werte).
  - `public final class KeySpec { public enum Mode { HOLD, PRESS } public KeySpec(String id, DefaultKey defaultKey, Mode mode); public String id(); public DefaultKey defaultKey(); public Mode mode(); public String translationKey() /* "key.hengst." + id */; }`; `id` passt auf `[a-z0-9_]+(\.[a-z0-9_]+)*`, sonst `IllegalArgumentException`.
  - `Feature`: `public List<KeySpec> keys()` (leer), `public void onKey(int key, boolean down, long nowNanos) {}` (`key` = Index in `keys()`).
  - `public final class FeatureCatalog { public static List<Feature> create(); public static List<KeySpec> keys(GameVersion version); }`: `create` liefert neue Instanzen aller Funktionen in Nummernfolge (hier die vier M0-Module; Tasks 10 bis 13 tragen ihre ein); `keys` liefert die Kern-Tasten `settings` (RIGHT_SHIFT, PRESS) und `hud_editor` (NONE, PRESS) als Index 0 und 1, dann die Tasten der auf `version` unterstützten Funktionen in Nummernfolge.
  - `HengstCore`: `public List<KeySpec> keySpecs()` (gleich `FeatureCatalog.keys(version)`, beim Start gebaut; `registerForTest` hängt die Tasten der Testfunktion hinten an); `public void onFeatureKey(int index, boolean down)`. Vertrag für die Adapter (Task 8): je Tick und Taste erst `onFeatureKey(i, true)`, wenn seit dem letzten Tick mindestens ein Klick gezählt wurde, dann `onFeatureKey(i, false)`, wenn die Taste nicht mehr gedrückt ist. Der Kern führt `keyDown[i]`: ein `true` bei schon gedrückter Taste und ein `false` bei losgelassener tun nichts; ein Drücken wird nur ausgeliefert in einer Welt ohne offenen Bildschirm und, bei Funktions-Tasten, wenn `FeatureState.on(nummer)`; Index 0 ruft `onOpenSettingsKey()`, 1 `onOpenHudEditorKey()`; ein Loslassen geht nur an HOLD-Tasten, deren Drücken ausgeliefert wurde. Ist im Tick ein Bildschirm offen oder keine Welt da, bekommen ausgelieferte, gehaltene HOLD-Tasten ihr Loslassen sofort, und gehaltene Tasten brauchen ein echtes Loslassen, bevor ein neues Drücken zählt. Indizes außerhalb der Liste werden ignoriert. Ausnahmen aus `onKey` loggt eine neue Fehlerstelle `SITE_KEY`. Die Pflicht "nur in einer Welt" geht damit vom Adapter auf den Kern über (Javadoc anpassen).

- [ ] **Step 1: Write the failing tests**

```java
// KeyFeature: TestFeature "keys", Nummer 47, Tasten "test.hold" (NONE, HOLD) und "test.press" (NONE, PRESS),
// protokolliert onKey als "<key> down"/"<key> up" in eine Liste.
KeyFeature f;
int HOLD, PRESS;   // Indizes der Testtasten: hinter allen Tasten, die der Katalog schon besitzt (Kern-Tasten, ab Task 13 auch die Stoppuhr)
void startWithKeys() {
    core = start(); platform.inWorld = true; f = new KeyFeature();
    core.activeProfile().module("keys").setEnabled(true);
    int base = core.keySpecs().size();           // vor registerForTest: ändert sich nicht, wenn der Katalog Tasten dazubekommt
    core.registerForTest(f);
    assertEquals(base + 2, core.keySpecs().size());
    HOLD = base; PRESS = base + 1;
}

@Test void core_keys_come_first_with_their_persisted_names() {
    List<KeySpec> keys = FeatureCatalog.keys(GameVersion.V1_8_9);
    assertEquals("key.hengst.settings", keys.get(0).translationKey());
    assertEquals(DefaultKey.RIGHT_SHIFT, keys.get(0).defaultKey());
    assertEquals("key.hengst.hud_editor", keys.get(1).translationKey());
    assertEquals(DefaultKey.NONE, keys.get(1).defaultKey());
    assertEquals(KeySpec.Mode.PRESS, keys.get(1).mode());
}
@Test void the_settings_key_opens_the_settings_only_in_a_world_without_a_screen() {
    core = start();
    core.onFeatureKey(0, true); core.onFeatureKey(0, false);
    assertNull(platform.openedScreen);
    platform.inWorld = true;
    core.onFeatureKey(0, true);
    assertNotNull(platform.openedScreen);
}
@Test void hold_reports_press_and_release_press_only_presses() {
    startWithKeys();
    core.onFeatureKey(HOLD, true); core.onFeatureKey(HOLD, false);
    core.onFeatureKey(PRESS, true); core.onFeatureKey(PRESS, false);
    assertEquals(Arrays.asList("0 down", "0 up", "1 down"), f.events);
}
@Test void a_repeated_press_while_held_counts_once() {
    startWithKeys();
    for (int i = 0; i < 5; i++) { core.onFeatureKey(PRESS, true); }
    assertEquals(Arrays.asList("1 down"), f.events);
}
@Test void a_press_while_a_screen_is_open_is_ignored_and_needs_a_release() {
    startWithKeys();
    platform.screenOpen = true; core.onFeatureKey(PRESS, true); core.onClientTick();
    platform.screenOpen = false; core.onClientTick(); core.onFeatureKey(PRESS, true);
    assertTrue(f.events.isEmpty());
    core.onFeatureKey(PRESS, false); core.onFeatureKey(PRESS, true);
    assertEquals(Arrays.asList("1 down"), f.events);
}
@Test void opening_a_screen_releases_a_held_hold_key() {
    startWithKeys();
    core.onFeatureKey(HOLD, true);
    platform.screenOpen = true; core.onClientTick();
    core.onFeatureKey(HOLD, false);
    assertEquals(Arrays.asList("0 down", "0 up"), f.events);
}
@Test void leaving_the_world_releases_a_held_hold_key() {
    startWithKeys();
    core.onFeatureKey(HOLD, true);
    platform.inWorld = false; core.onClientTick();
    assertEquals(Arrays.asList("0 down", "0 up"), f.events);
}
@Test void keys_of_a_disabled_feature_do_nothing() {
    startWithKeys();
    core.activeProfile().module("keys").setEnabled(false); core.applyConfigForTest();
    core.onFeatureKey(PRESS, true);
    assertTrue(f.events.isEmpty());
}
@Test void out_of_range_indexes_and_bad_ids() {
    startWithKeys();
    core.onFeatureKey(-1, true); core.onFeatureKey(core.keySpecs().size(), true); core.onFeatureKey(99, true);
    assertThrows(IllegalArgumentException.class, () -> new KeySpec("Bad Id", DefaultKey.NONE, KeySpec.Mode.PRESS));
    assertThrows(IllegalArgumentException.class, () -> new KeySpec("", DefaultKey.NONE, KeySpec.Mode.PRESS));
}
@Test void the_catalog_has_unique_ids_and_numbers_and_matches_the_core() {
    Set<String> ids = new HashSet<>();
    for (KeySpec k : FeatureCatalog.keys(GameVersion.V26_3)) { assertTrue(ids.add(k.id()), k.id()); }
    core = start();
    assertEquals(ids(FeatureCatalog.keys(GameVersion.V1_8_9)), ids(core.keySpecs()));
}
```

In `HengstCoreAllocationTest` (interpreted): `feature_keys_allocate_nothing`: `KeyFeature` mit Zählern statt Liste (HOLD-Index = `core.keySpecs().size()` vor `registerForTest`, wie in `startWithKeys`), 1000 Paare `onFeatureKey(HOLD, true/false)` und 1000 Ticks: 0 Bytes.

- [ ] **Step 2: Run tests to verify they fail**

Run: `./gradlew -p common test --tests '*HengstCoreKeysTest'`
Expected: FAIL.

- [ ] **Step 3: Implement**

Der Kern baut beim Start aus `registry.all()` zwei Tabellen `Feature[] keyOwner` (null für die Kern-Tasten) und `int[] keyLocal`, dazu `boolean[] keyDown`, `boolean[] delivered`. `registerForTest` hängt die Tasten der Testfunktion an `keySpecs()` an und lässt alle vier Tabellen mitwachsen (sie werden sonst nur beim Start gebaut); sein Index liegt hinter allen bisherigen.

- [ ] **Step 4: Run tests to verify they pass**

Run: `./gradlew -p common test`, dann `./gradlew buildAll`
Expected: PASS; `BUILD SUCCESSFUL` (die Adapter rufen weiter `onOpenSettingsKey`, das bleibt öffentlich).

- [ ] **Step 5: Commit**

```bash
git add mod/common/src
git commit -m "feat(mod-common): key specs per feature, hold and press keys through the core

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 8: Adapter: generische Tasten, Ping- und Welt-Lesezugriff, Nachweis des Funktionsschalters

**Files:**
- Modify: `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/platform/GameView.java`, `mod/common/src/test/java/.../testutil/FakePlatform.java`
- Modify: `mod/modern/src/main/java/io/github/clemensjl/hengst/mod/modern/ModernKeys.java`, `HengstModernMod.java`, `ModernPlatform.java`, `mod/modern/src/main/resources/assets/hengst/lang/en_us.json`, `de_de.json` (unverändert, solange nur die zwei Kern-Tasten existieren)
- Modify: `mod/legacy/src/main/java/io/github/clemensjl/hengst/mod/legacy/LegacyKeys.java`, `HengstLegacyMod.java`, `LegacyPlatform.java`
- Test: `mod/modern/src/test/java/io/github/clemensjl/hengst/mod/modern/ModernKeysTest.java` (neu), `mod/legacy/src/test/java/io/github/clemensjl/hengst/mod/legacy/LegacyKeysTest.java` (neu), `mod/modern/src/test/java/.../HengstModernModTest.java` und `mod/legacy/src/test/java/.../HengstLegacyModTest.java` (die anonyme `new GameView()` in `workingPlatform` bekommt `@Override public int pingMillis() { return -1; }` und `@Override public Object worldIdentity() { return null; }`, sonst kompilieren beide Tests nicht; dazu je ein Allokationstest für `dispatch`), `mod/modern/src/gametest/.../HengstClientGameTest.java`, `mod/legacy/src/gametest/.../LegacyGameTest.java`

**Interfaces:**
- Consumes: `FeatureCatalog.keys`, `HengstCore.onFeatureKey` (Task 7), `FeatureState.on` (Task 5).
- Produces:
  - `GameView`: `int pingMillis()` (vom Server gemeldete Latenz des eigenen Spielers in ms; -1 im Einzelspieler, ohne Verbindung oder ohne eigenen Eintrag in der Spielerliste), `Object worldIdentity()` (die Weltinstanz des Clients, nur per Referenz verglichen; null ohne Welt; neu bei Beitritt und Dimensionswechsel). Beide allokationsfrei und beide abstrakt (ein Adapter ohne Umsetzung soll nicht kompilieren; deshalb stehen die zwei anonymen `GameView` der beiden Adapter-Tests in der Dateiliste). `FakePlatform`: Felder `public int pingMillis = -1`, `public Object worldIdentity` mit Overrides.
  - Modern: `pingMillis`: `mc.hasSingleplayerServer()` → -1; sonst `mc.getConnection()`, `mc.player`, `ClientPacketListener.getPlayerInfo(UUID)` mit `player.getUUID()`, `PlayerInfo.getLatency()` [B Signaturen per `javap` in 26.3 und 1.21.11; K: `getPlayerInfo(UUID)` ist ein Map-Zugriff ohne Allokation, in `genSources` prüfen]. `worldIdentity`: `mc.level` [B]; [K]: `ClientPacketListener.handleRespawn` legt bei Dimensionswechsel eine neue `ClientLevel` an.
  - 1.8.9: `pingMillis`: `mc.isInSingleplayer()` → -1; sonst `mc.getNetworkHandler().getPlayerListEntry(mc.player.getUuid()).getLatency()` [B in Legacy Yarn build.604 gesehen: `getPlayerListEntry(UUID)` ist `playerListEntries.get(uuid)`, `Entity.getUuid()`, `PlayerListEntry.getLatency()`]. `worldIdentity`: `mc.world` [B: `onPlayerRespawn` legt bei anderer Dimension eine neue `ClientWorld` an].
  - `ModernKeys`: `static void register(GameVersion version)` registriert für jede `KeySpec` aus `FeatureCatalog.keys(version)` eine `KeyMapping(spec.translationKey(), code(spec.defaultKey()), category)` in der Kategorie `hengst:main`, Index wie in der Liste; `static int code(DefaultKey key)` (NONE → `InputConstants.UNKNOWN.getValue()`, RIGHT_SHIFT → `InputConstants.KEY_RSHIFT`; jeder andere Wert wirft); `static void dispatch(Minecraft client)` ruft `dispatch(HengstModernMod.core(), mappings)` mit dem `KeyMapping[]`, das `register` aufgebaut hat; `static void dispatch(HengstCore core, KeyMapping[] mappings)` meldet je Index nach dem Vertrag aus Task 7 (`consumeClick()` leeren, ein `true` bei mindestens einem Klick, ein `false` wenn `!isDown()`) und läuft mit einer indexierten `for`-Schleife über das Array (kein Iterator, keine `List<KeySpec>`, keine Streams, keine Lambdas). Registriert wird vor dem Start des Kerns, wie in M0; die Version kommt aus einer statischen `ModernPlatform.gameVersion()` (die `version()` darauf zurückführt).
  - `LegacyKeys`: dasselbe mit `KeyBinding(String, int, String)` und Kategorie `key.categories.hengst`, `static int code(DefaultKey)` (NONE → `Keyboard.KEY_NONE`, RIGHT_SHIFT → `Keyboard.KEY_RSHIFT`), `wasPressed()` und `isPressed()`, `static void dispatch(HengstCore core, KeyBinding[] bindings)` wie oben mit indexierter Schleife.
  - [K] je modernem Knoten: ob eine Tastenwiederholung `KeyMapping.click` auslöst (`KeyboardHandler.keyPress`); der Kern verträgt beides (Task 7), das Ergebnis steht im Report.

- [ ] **Step 1: Namen prüfen**

Run: `./gradlew -p modern :26.3:genSources`, dann `./gradlew -p modern :1.21.11:genSources`, dann `./gradlew -p legacy genSources`
Expected: Quellen erzeugt; jede [K]-Stelle dieses Tasks nachgeschlagen, Ergebnis mit Fundstelle im Report.

- [ ] **Step 2: Write the failing tests**

```java
// ModernKeysTest (gleich LegacyKeysTest mit .lang-Dateien und Keyboard-Codes)
@Test void every_default_key_has_a_code() {
    for (DefaultKey k : DefaultKey.values()) { ModernKeys.code(k); }
    assertEquals(InputConstants.KEY_RSHIFT, ModernKeys.code(DefaultKey.RIGHT_SHIFT));
}
@Test void every_key_of_every_version_is_named_in_both_language_files() throws Exception {
    Set<String> en = langKeys("en_us"), de = langKeys("de_de");
    assertEquals(en, de);
    for (GameVersion v : new GameVersion[] {GameVersion.V1_21_11, GameVersion.V26_3}) {
        for (KeySpec k : FeatureCatalog.keys(v)) { assertTrue(en.contains(k.translationKey()), k.translationKey()); }
    }
    assertTrue(en.contains("key.category.hengst.main"));
}
```

In `HengstModernModTest` und `HengstLegacyModTest` je ein Allokationstest, der den Kern mit der dortigen `workingPlatform` startet:

```java
@Test void key_dispatch_allocates_nothing() {
    // [K] ob KeyMapping (modern) bzw. KeyBinding (1.8.9) ohne laufendes Spiel gebaut werden kann: in der Quelle prüfen;
    // geht es nicht, entfällt der Test im Adapter und derselbe Messlauf steht als Schritt im Spieltest.
    HengstCore core = HengstCore.start(workingPlatform(dir, new UiScreen[1]), TimeSource.SYSTEM);
    KeyMapping[] maps = new KeyMapping[core.keySpecs().size()];
    for (int i = 0; i < maps.length; i++) { maps[i] = new KeyMapping(core.keySpecs().get(i).translationKey(), -1, CATEGORY); }
    for (int i = 0; i < 2000; i++) { ModernKeys.dispatch(core, maps); }                 // einschwingen
    com.sun.management.ThreadMXBean mx = (com.sun.management.ThreadMXBean) ManagementFactory.getThreadMXBean();
    long before = mx.getThreadAllocatedBytes(Thread.currentThread().getId());
    for (int i = 0; i < 2000; i++) { ModernKeys.dispatch(core, maps); }
    assertTrue(mx.getThreadAllocatedBytes(Thread.currentThread().getId()) - before < 2048);   // ein Iterator je Aufruf wären über 60 KB
}
```

(1.8.9 mit `KeyBinding[]` und `LegacyKeys.dispatch`; `CATEGORY` ist die Kategorie, die `register` nutzt; die Konstruktor-Aufrufe sind je Version nach der Quelle anzupassen. Die Messung läuft ohne `-Xint`, deshalb die Schranke 2048 Byte statt 0 wie in den `interpreted`-Tests von common.)

Spieltests, je nach dem ersten HUD-Bild (`hengst-01-hud`): `check(FeatureState.on(1) && FeatureState.on(2) && FeatureState.on(3) && FeatureState.on(6), "M0 modules on in the feature state")`, `check(!FeatureState.on(7), "direction off in the feature state")`, FPS über `core.activeProfile().module("fps").setEnabled(false); core.applyConfigForTest();` aus, `check(!FeatureState.on(1), ...)`, wieder an. Der bestehende Schritt "Rechte Umschalttaste öffnet die Einstellungen" bleibt und belegt den Umzug der Kern-Tasten. Neu: `check(core.context().platform().game().pingMillis() == -1, "no ping in singleplayer")` und `check(core.context().platform().game().worldIdentity() != null, "world identity in a world")`.

- [ ] **Step 3: Run tests to verify they fail**

Run: `./gradlew -p modern :26.3:test`, `./gradlew -p legacy test`
Expected: FAIL, `code` und die neue Registrierung fehlen.

- [ ] **Step 4: Implement**

`dispatch` läuft am Tick-Ende vor `core.onClientTick()`, wie in M0, mit indexierter Schleife über ein einmal gebautes Array (der Allokationstest oben belegt es). Die zwei alten Felder `settings`/`hudEditor` entfallen; die Prüfung "nur in einer Welt" entfällt im Adapter. Die zwei neuen abstrakten `GameView`-Methoden bekommen ihre Umsetzung in `ModernPlatform.Game` und `LegacyPlatform.Game` und in den zwei Test-Doubles der Adapter-Tests.

- [ ] **Step 5: Run tests and the games**

Run: `./gradlew -p common test`, `./gradlew -p legacy test`, `./gradlew -p modern :1.21.11:test`, `./gradlew -p modern :26.3:test`, `./gradlew buildAll`, dann nacheinander die drei Spieltests.
Expected: alles grün, neue Schritte PASS; in den Steuerungsoptionen (Screenshot der Spieltests nicht nötig) bleiben die Namen `key.hengst.settings` und `key.hengst.hud_editor` gleich, also behalten Spieler ihre Belegung; `unzip -l` der Jars ohne Testklassen.

- [ ] **Step 6: Commit**

```bash
git add mod/common/src mod/modern/src mod/legacy/src
git commit -m "feat(mod): generic key registration from the core, ping and world identity reads, feature state proof

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 9: Gruppen-Überschriften und Bildlaufanzeige in den Einstellungen

**Files:**
- Modify: `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/ui/SettingsScreen.java`, `.../ui/ScrollList.java`
- Create: `mod/common/src/test/java/io/github/clemensjl/hengst/mod/common/testutil/GroupFeature.java`
- Test: `mod/common/src/test/java/io/github/clemensjl/hengst/mod/common/ui/SettingsScreenTest.java` (neue Tests, neue Helfer, Literale der Listenzeilen, siehe Geometrie), `.../HengstCoreTest.java` (`FPS_SWITCH_Y` und die Schleife in `an_exception_in_a_screen_input_does_not_escape_and_is_logged_once`)

**Interfaces:**
- Consumes: `Feature.group()`, Sprachschlüssel `hengst.group.hud|visual|comfort` (M0).
- Produces:
  - Die Funktionsliste zeigt je Gruppe (Reihenfolge `HUD`, `VISUAL`, `COMFORT`) eine nicht anklickbare Überschriftzeile (20 px hoch wie jede Zeile, Text `hengst.group.<gruppe>` in `0xFFAAAAAA` bei x + 4, keine Füllung), darunter die Funktionen der Gruppe nach Nummer. Gruppen ohne unterstützte Funktion bekommen keine Überschrift. `SettingsModel.features()` bleibt nach Nummer sortiert, die anfängliche Auswahl bleibt FPS.
  - `ScrollList`: `public boolean overflows()` (Zeilen × Zeilenhöhe > Höhe); läuft die Liste über, werden die Zeilen 4 px schmaler und rechts in der Liste ein Schacht `fill(x+w-2, y, x+w, y+h, 0x40FFFFFF)` mit Daumen `fill(x+w-2, ty, x+w, ty+th, 0xC0FFFFFF)` gezeichnet; `th = max(8, h * sichtbareZeilen / Zeilen)`, `ty = y + (h - th) * offset / maxOffset` (bei `maxOffset` 0 oben). Nur Anzeige, kein Ziehen (Mausrad wie bisher). Entscheidung: bei 320 x 240 hat die Liste 7 sichtbare Zeilen, mit M1a 13 Zeilen (11 Funktionen, 2 Überschriften), deshalb braucht es die Anzeige.
  - Geometrie: eine Zeile der Funktionsliste liegt bei `y = 36 + Zeile * 20` (Höhe 20), ihr Klickpunkt bei `36 + Zeile * 20 + 5`; die Zeilen zählen jetzt die Überschriften mit (Zeile 0 ist die erste Überschrift). Die Zeilen der rechten Seite (Optionsliste, x ab 144) und die Kopfzeile (y 15) bleiben unverändert. Jedes Literal, das eine Listenzeile adressiert, ändert sich nach dieser Regel; vollständig:
    - `HengstCoreTest`: `FPS_SWITCH_Y` 46 → 66 (Überschrift HUD ist Zeile 0, FPS Zeile 1). In `an_exception_in_a_screen_input_does_not_escape_and_is_logged_once` die Schleife `for (int y = 46; y < 46 + 20 * 5; ...)` → `for (int y = 66; y < 66 + 20 * 5; ...)` (fps, cps, keystrokes, coordinates und `bad` sind jetzt die Zeilen 1 bis 5).
    - `SettingsScreenTest`, Registry `reg()` oder `allModules()` (alle Gruppe HUD, eine Überschrift): `toggling_feature_marks_changed`, `close_saves_rebuilds_then_closes_in_that_order`, `edit_hud_saves_pending_changes_first`: `mouseClicked(12, 46, 0)` → `mouseClicked(12, 66, 0)`. `slider_option_is_coerced_and_marks_changed`: Zeile von `coordinates` `36 + 3 * 20 + 5` → `36 + 4 * 20 + 5` (121; der Kommentar "4th row" wird "row 4, the header is row 0"); der Klick auf den Schieber `mouseClicked(144 + 84, 36 + 3 * 20 + 5, 0)` bleibt (rechte Seite). `toggle_option_and_text_colour_choice`: `keystrokes` `36 + 2 * 20 + 5` → `36 + 3 * 20 + 5`; alle `mouseClicked(200, ...)` darin bleiben. `german_texts_fit_the_screen_and_their_widgets_at_320_by_240`: `mouseClicked(60, 36 + row * 20 + 5, 0)` → `mouseClicked(60, 36 + (row + 1) * 20 + 5, 0)`. `steady_state_rendering_allocates_nothing_noticeable`: `coordinates` `36 + 3 * 20 + 5` → `36 + 4 * 20 + 5`.
    - `feature_list_scrolls_with_the_wheel_and_clamps` (12 Funktionen der Gruppe HUD = 13 Zeilen): die obere Schranke `(12 - list.height() / 20) * 20` → `(13 - list.height() / 20) * 20`; nach der letzten Rolle von einer Raste steht Zeile 1 (nicht mehr Zeile 0) am Listenanfang, weil Zeile 0 die Überschrift ist: der Klick `mouseClicked(12, 36 + 5, 0)` trifft `f0`, die Prüfung `config("f1").enabled()` wird `config("f0").enabled()`. Die Erwartungen `list.offset() == 20` nach einer Raste und `0` am Anfang bleiben.
    - `a_feature_that_is_not_on_the_hud_gets_no_hud_style_rows` (fps HUD, zoom VISUAL: Überschrift HUD 0, fps 1, Überschrift Visual 2, zoom 3): `mouseClicked(60, 36 + 5, 0)` → `mouseClicked(60, 36 + 1 * 20 + 5, 0)` (fps) und `mouseClicked(60, 36 + 20 + 5, 0)` → `mouseClicked(60, 36 + 3 * 20 + 5, 0)` (zoom).
    - `openZoom` (nur `zoom` in VISUAL: Überschrift Zeile 0, zoom Zeile 1): `mouseClicked(60, 36 + 5, 0)` → `mouseClicked(60, 36 + 20 + 5, 0)`. In `choice_values_are_shown_...` ist `int row = 36 + 20 + 5` die Optionsliste rechts (x 200) und bleibt.
    - Danach bleibt keine Erwartung der Klasse übrig, die eine Listenzeile ohne diese Regel adressiert (zur Kontrolle `grep -n "mouseClicked(\(12\|60\)," SettingsScreenTest.java HengstCoreTest.java`).
  - Die Spieltest-Prüfung `LegacyGameTest.settingsWheel` (Offset 20 nach einer Raste) setzt voraus, dass die Liste im Spielfenster überläuft: mit den vier M0-Modulen sind es jetzt 5 Zeilen statt 4; Task 15 prüft das und passt bei Bedarf die Fenstergröße des Harness oder die Erwartung an.

- [ ] **Step 1: Write the failing tests**

```java
// Helfer in SettingsScreenTest (neben `texts(...)`), exakte Signaturen:
static final EnumSet<GameVersion> ALL = EnumSet.allOf(GameVersion.class);
/** n Funktionen "f0" .. "f<n-1>" mit den Nummern 1 .. n, alle Gruppe HUD. */
private static FeatureRegistry registryWith(int n) {
    FeatureRegistry r = new FeatureRegistry();
    for (int i = 0; i < n; i++) { r.register(new TestFeature("f" + i, i + 1, ALL)); }
    return r;
}
private FeatureRegistry defaultRegistry() { return allModules(); }                       // fps, cps, keystrokes, coordinates
/** Zeichnet den Bildschirm neu und gibt die Aufrufe dieses Frames zurück. */
private List<String> drawn() { canvas.calls.clear(); screen.render(canvas, 0, 0, 0f); return canvas.calls; }
/** Öffnet mit festen englischen Texten, damit Überschriften und Namen der Testfunktionen lesbar sind (die echte Sprachdatei kennt "comfy" und "vis" nicht). */
private void openNamed(FeatureRegistry reg) {
    open(reg, "en_us");
    screen = new SettingsScreen(model, I18n.of(texts("hengst.group.hud", "HUD", "hengst.group.visual", "Visual", "hengst.group.comfort", "Comfort",
            "hengst.feature.fps", "fps", "hengst.feature.cps", "cps", "hengst.feature.comfy", "comfy", "hengst.feature.vis", "vis"), texts()), () -> closeCount++);
}
/** Alle Texte, die der letzte Frame in der Funktionsliste zeichnet (x 8 bis 136, y 36 bis 186), von oben nach unten: Überschriften und Funktionsnamen. */
private List<String> listTexts() {
    drawn();
    List<String> out = new ArrayList<>();
    for (Drawn d : textCalls(canvas, null)) { if (d.x >= 8 && d.x < 136 && d.y >= 36 && d.y < 186) { out.add(d.text); } }
    return out;
}
// testutil/GroupFeature: public final class GroupFeature extends Feature
//   public GroupFeature(String id, int number, FeatureGroup group)                                  // alle Versionen
//   public GroupFeature(String id, int number, FeatureGroup group, EnumSet<GameVersion> versions)   // super(id, number, group, versions)

@Test void features_are_grouped_under_headers_in_group_order() {
    FeatureRegistry reg = new FeatureRegistry();
    reg.register(new TestFeature("fps", 1, ALL)); reg.register(new TestFeature("cps", 2, ALL));
    reg.register(new GroupFeature("comfy", 21, FeatureGroup.COMFORT)); reg.register(new GroupFeature("vis", 25, FeatureGroup.VISUAL));
    openNamed(reg);
    assertEquals(Arrays.asList("HUD", "fps", "cps", "Visual", "vis", "Comfort", "comfy"), listTexts());   // 7 Zeilen, alle sichtbar
}
@Test void a_group_without_supported_features_has_no_header() {
    FeatureRegistry reg = new FeatureRegistry();
    reg.register(new TestFeature("fps", 1, ALL));
    reg.register(new GroupFeature("vis", 25, FeatureGroup.VISUAL, EnumSet.of(GameVersion.V26_3)));
    openNamed(reg);   // Version 1.8.9
    assertEquals(Arrays.asList("HUD", "fps"), listTexts());
}
@Test void clicking_a_header_selects_and_changes_nothing() {
    open(defaultRegistry(), "en_us");
    screen.render(canvas, 0, 0, 0f);
    Feature before = model.selected();
    assertFalse(screen.mouseClicked(60, 46, 0));         // Zeile 0 (y 36 bis 56) ist die Überschrift HUD
    assertSame(before, model.selected());
    assertFalse(model.dirty());
}
@Test void the_scroll_indicator_appears_only_when_the_list_overflows() {
    open(registryWith(4), "en_us");
    assertFalse(drawn().contains("fill 134 36 136 186 40ffffff"));
    open(registryWith(11), "en_us");   // 11 HUD-Funktionen + 1 Überschrift = 12 Zeilen, 7 sichtbar
    assertTrue(drawn().contains("fill 134 36 136 186 40ffffff"));
    assertTrue(drawn().contains("fill 134 36 136 123 c0ffffff"));   // th = max(8, 150 * 7 / 12) = 87
    screen.mouseScrolled(40, 100, -10);
    assertTrue(drawn().contains("fill 134 99 136 186 c0ffffff"));   // maxOffset 100, ty = 36 + 63
}
```

(Geometrie aus den bestehenden Konstanten bei 320 x 240: Liste x 8 bis 136, y 36, Höhe 150 = Hinweis in zwei Zeilen über dem Fertig-Knopf; `GroupFeature`, `registryWith`, `defaultRegistry`, `drawn`, `openNamed` und `listTexts` sind oben mit exakter Signatur und Textzuordnung definiert). Weicht die heutige Listenhöhe ab, rechnet der Implementierer die drei Zahlen nach der Formel oben neu und schreibt den Rechenweg in den Test-Kommentar.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `./gradlew -p common test --tests '*SettingsScreenTest'`
Expected: FAIL.

- [ ] **Step 3: Implement**

Überschriftzeile als private Klasse `GroupRow extends Widget` in `SettingsScreen`; `mouseClicked` liefert false.

- [ ] **Step 4: Run tests to verify they pass**

Run: `./gradlew -p common test`, dann `./gradlew buildAll`
Expected: PASS (auch `german_texts_fit_the_screen_and_their_widgets_at_320_by_240` und `feature_list_scrolls_with_the_wheel_and_clamps` mit angepassten Werten); `BUILD SUCCESSFUL`.

- [ ] **Step 5: Commit**

```bash
git add mod/common/src
git commit -m "feat(mod-common): group headers and a scroll indicator in the settings list

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 10: Module 7 Richtung und 15 Geschwindigkeit

**Files:**
- Create: `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/hud/modules/DirectionModule.java`, `SpeedModule.java`
- Modify: `.../FeatureCatalog.java`, `mod/common/src/main/resources/hengst/lang/en_us.json`, `de_de.json`
- Test: `mod/common/src/test/java/.../hud/modules/DirectionModuleTest.java`, `SpeedModuleTest.java`, `InfoModulesAllocationTest.java` (neu, `@Tag("interpreted")`), `ModulesTest.java` (`ids_numbers_and_versions_are_the_persisted_ones`, `option_specs_match_the_persisted_keys_and_defaults` um die neuen Module erweitert)

**Interfaces:**
- Consumes: `TextHudModule`, typisierte Optionen (Task 1), `HudContext.i18n()`, `languageGeneration()` (Task 6), `GameView.playerYaw()`, Positionen, `worldIdentity()` (Task 8), Standardplätze (Task 5).
- Produces:
  - `DirectionModule extends TextHudModule`: ID `direction`, Nummer 7, alle drei Versionen, Option `style` (`cardinal`, `cardinal_degrees`, `degrees`; Standard `cardinal_degrees`), Platz TOP_LEFT 2, 44. Kurs `h = ((yaw + 180) mod 360 + 360) mod 360` (Minecraft: Gierwinkel 0 blickt nach Süden, 180 nach Norden), angezeigt `d = round(h) mod 360`, Sektor `((2d + 45) / 90) mod 8` (Ganzzahldivision) für N, NO/NE, O/E, SO/SE, S, SW, W, NW aus `hengst.direction.n|ne|e|se|s|sw|w|nw`. Texte: `cardinal` "NE", `cardinal_degrees` "NE 45°", `degrees` "45°"; nicht endlicher Gierwinkel "?". Gierwinkel je Tick; die 360 (bzw. 8) Texte des aktuellen Stils und der aktuellen Sprache werden beim Wechsel von Stil oder Sprachgeneration einmal gebaut, danach wählt der Tick nur noch aus der Tabelle (null Allokation auch beim Drehen).
  - `SpeedModule extends TextHudModule`: ID `speed`, Nummer 15, Optionen `include_vertical` (aus) und `decimals` (0 bis 2, Standard 1), Platz TOP_LEFT 2, 58. Je Tick Abstand zur Position des letzten Ticks (waagerecht, mit `include_vertical` räumlich), Mittel über die letzten bis zu 10 Abstände (0,5 s, Ringpuffer `double[10]`), mal 20 = Blöcke je Sekunde, Text `"<wert> m/s"` mit `decimals` Nachkommastellen (halb aufwärts gerundet). Zurückgesetzt (Puffer leer, Text "0.0 m/s" bei 1 Stelle, der Tick merkt nur die Position) bei: räumlichem Sprung über 10 Blöcke in einem Tick (Teleport), neuer `worldIdentity`-Referenz (Dimensionswechsel, neue Welt), nicht endlicher Position. Außerhalb einer Welt leerer Text.
  - Sprachschlüssel (en / de): `hengst.feature.direction` Direction / Richtung; `hengst.feature.speed` Speed / Geschwindigkeit; `hengst.direction.n|ne|e|se|s|sw|w|nw` N, NE, E, SE, S, SW, W, NW / N, NO, O, SO, S, SW, W, NW; `hengst.option.direction.style` Display / Anzeige; Werte `cardinal` Letters / Buchstaben, `cardinal_degrees` Letters and degrees / Buchstaben und Grad, `degrees` Degrees / Grad; `hengst.option.speed.include_vertical` Count vertical movement / Senkrechte Bewegung mitzählen; `hengst.option.speed.decimals` Decimals / Nachkommastellen.

- [ ] **Step 1: Write the failing tests** (`ctx` wie in `ModulesTest`: `FakePlatform`, `FakeTime`, `inWorld = true`)

```java
@Test void direction_formats() {
    DirectionModule m = new DirectionModule(); ModuleConfig cfg = ModuleConfig.defaults();
    Object[][] cases = {{180f, "N 0°"}, {0f, "S 180°"}, {-90f, "E 90°"}, {90f, "W 270°"}, {-135f, "NE 45°"},
            {540f, "N 0°"}, {-720.4f, "S 180°"}, {202.4f, "N 22°"}, {202.6f, "NE 23°"}, {179.6f, "N 0°"}};
    for (Object[] c : cases) { platform.playerYaw = (Float) c[0]; m.tick(ctx, cfg); assertEquals(c[1], m.text(), "yaw " + c[0]); }
    cfg.setOption("style", "cardinal"); platform.playerYaw = -135f; m.tick(ctx, cfg); assertEquals("NE", m.text());
    cfg.setOption("style", "degrees"); m.tick(ctx, cfg); assertEquals("45°", m.text());
    platform.playerYaw = Float.NaN; m.tick(ctx, cfg); assertEquals("?", m.text());
}
@Test void direction_letters_follow_the_language() {
    DirectionModule m = new DirectionModule(); ModuleConfig cfg = ModuleConfig.defaults();
    platform.playerYaw = -90f; m.tick(ctx, cfg); assertEquals("E 90°", m.text());
    ctx.setI18n(I18n.load("de_de")); m.tick(ctx, cfg); assertEquals("O 90°", m.text());
}
@Test void speed_while_walking_with_each_number_of_decimals() {
    SpeedModule m = new SpeedModule(); ModuleConfig cfg = ModuleConfig.defaults(); platform.worldIdentity = new Object();
    m.tick(ctx, cfg);
    for (int i = 0; i < 10; i++) { platform.playerX += 0.2155; m.tick(ctx, cfg); }
    assertEquals("4.3 m/s", m.text());
    cfg.setOption("decimals", 2L); m.tick(ctx, cfg);   // ein Tick ohne Bewegung: Mittel über 10 Werte, einer davon 0
    assertEquals("3.88 m/s", m.text());
    cfg.setOption("decimals", 0L); platform.playerX += 0.2155; m.tick(ctx, cfg);
    assertEquals("4 m/s", m.text());
}
@Test void the_average_covers_only_the_ticks_seen_so_far() {
    SpeedModule m = new SpeedModule(); ModuleConfig cfg = ModuleConfig.defaults(); platform.worldIdentity = new Object();
    m.tick(ctx, cfg); platform.playerZ -= 0.25; m.tick(ctx, cfg);
    assertEquals("5.0 m/s", m.text());
}
@Test void vertical_movement_counts_only_when_asked() {
    SpeedModule m = new SpeedModule(); ModuleConfig cfg = ModuleConfig.defaults(); platform.worldIdentity = new Object();
    m.tick(ctx, cfg); platform.playerY -= 0.5; m.tick(ctx, cfg); assertEquals("0.0 m/s", m.text());
    cfg.setOption("include_vertical", true); platform.playerY -= 0.5; m.tick(ctx, cfg); assertEquals("5.0 m/s", m.text());
}
@Test void a_teleport_and_a_world_change_reset_the_average() {
    SpeedModule m = new SpeedModule(); ModuleConfig cfg = ModuleConfig.defaults(); platform.worldIdentity = new Object();
    m.tick(ctx, cfg);
    for (int i = 0; i < 10; i++) { platform.playerX += 0.25; m.tick(ctx, cfg); }
    assertEquals("5.0 m/s", m.text());
    platform.playerX += 500; m.tick(ctx, cfg); assertEquals("0.0 m/s", m.text());
    platform.playerX += 0.25; m.tick(ctx, cfg); assertEquals("5.0 m/s", m.text());
    platform.worldIdentity = new Object(); platform.playerX += 3; m.tick(ctx, cfg); assertEquals("0.0 m/s", m.text());
}
@Test void speed_is_empty_outside_a_world_and_resets_on_return() {
    SpeedModule m = new SpeedModule(); ModuleConfig cfg = ModuleConfig.defaults(); platform.worldIdentity = new Object();
    m.tick(ctx, cfg); platform.playerX += 0.25; m.tick(ctx, cfg);
    platform.inWorld = false; m.tick(ctx, cfg); assertEquals("", m.text());
    platform.inWorld = true; platform.playerX += 0.25; m.tick(ctx, cfg); assertEquals("0.0 m/s", m.text());
}
```

`InfoModulesAllocationTest` (interpreted): Richtung und Geschwindigkeit aktiv im echten `HudRenderer`, 3000 Frames und Ticks zum Einschwingen, dann 20000 Frames und 20000 Ticks bei gleichbleibenden Werten (Spieler steht): 0 Bytes; zusätzlich `direction_while_turning_allocates_nothing`: Richtung mit einem Gierwinkel, der sich jeden Tick um 7° dreht: 0 Bytes in 20000 Ticks; und `speed_while_moving_allocates_nothing`: `platform.worldIdentity` gesetzt, jeden Tick `platform.playerX += 0.25` (konstant 5,0 m/s, der Ringpuffer wird belegt und überschrieben, der Text bleibt "5.0 m/s"), 3000 Ticks zum Einschwingen, dann 20000 Ticks: 0 Bytes.

- [ ] **Step 2: Run tests to verify they fail**

Run: `./gradlew -p common test --tests '*DirectionModuleTest' --tests '*SpeedModuleTest'`
Expected: FAIL, Klassen fehlen.

- [ ] **Step 3: Implement die Module und tragen sie in `FeatureCatalog.create()` ein**

- [ ] **Step 4: Run tests to verify they pass**

Run: `./gradlew -p common test`, dann `./gradlew buildAll`
Expected: PASS inklusive `both_language_files_have_the_same_keys`, `first_start_enables_m0_modules_and_writes_nothing_yet` (weiter 4 aktiv); `BUILD SUCCESSFUL`.

- [ ] **Step 5: Commit**

```bash
git add mod/common/src
git commit -m "feat(mod-common): direction and speed hud modules

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 11: Module 9 Uhr und 11 Speicher

**Files:**
- Create: `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/hud/modules/ClockModule.java`, `MemoryModule.java`
- Modify: `.../platform/GameView.java` (Speicher als `default`-Methoden), `.../FeatureCatalog.java`, Sprachdateien, `mod/common/src/test/.../testutil/FakePlatform.java` (Felder für den Speicher)
- Test: `.../hud/modules/ClockModuleTest.java`, `MemoryModuleTest.java`, `InfoModulesAllocationTest.java`, `ModulesTest.java`

**Interfaces:**
- Consumes: `HudContext.wallClock()`, `i18n()` (Task 6), typisierte Optionen.
- Produces:
  - `GameView`: `default long memoryUsedBytes()` (`Runtime.getRuntime().totalMemory() - freeMemory()`), `default long memoryMaxBytes()` (`maxMemory()`); keine Adapterarbeit. `FakePlatform`: `public long memoryUsed`, `public long memoryMax` mit Overrides.
  - `ClockModule`: ID `clock`, Nummer 9, Optionen `format` (`24h`, `12h`; Standard `24h`) und `seconds` (aus), Platz TOP_RIGHT -2, 66. Ortszeit = `utc + wallClock.offsetMillis(utc)`. Texte: 24h `"09:05"`, mit Sekunden `"09:05:07"`; 12h `"9:05 AM"`, `"9:05:07 AM"`, Mitternacht `"12:00 AM"`, Mittag `"12:00 PM"`, 13 Uhr `"1:00 PM"`; AM/PM aus `hengst.clock.am|pm`. Je Tick nur `currentTimeMillis()` und ein Vergleich: neu gerechnet (mit genau einem `offsetMillis`-Aufruf) erst, wenn `utc` die beim letzten Rechnen bestimmte nächste Minuten- bzw. Sekundengrenze in UTC erreicht, wenn `utc` kleiner ist als beim letzten Rechnen (Uhr zurückgestellt), bei Wechsel einer Option oder der Sprachgeneration.
  - `MemoryModule`: ID `memory`, Nummer 11, Option `style` (`percent`, `used_max`, `both`; Standard `both`), Platz TOP_RIGHT -2, 94. MB = Bytes >> 20 (abgerundet), Prozent = belegt × 100 / max (abgerundet). Texte: `percent` `"RAM 25%"`, `used_max` `"RAM 1024/4096 MB"`, `both` `"RAM 25% 1024/4096 MB"`; ist `max` `Long.MAX_VALUE` oder ≤ 0, in jedem Stil `"RAM 1024 MB"`. Abfrage beim ersten Tick nach dem Einschalten, dann alle 20 Ticks.
  - Sprachschlüssel: `hengst.feature.clock` Clock / Uhr; `hengst.feature.memory` Memory / Arbeitsspeicher; `hengst.clock.am` AM / AM; `hengst.clock.pm` PM / PM; `hengst.option.clock.format` Format / Format, Werte `24h` 24-hour / 24 Stunden, `12h` 12-hour / 12 Stunden; `hengst.option.clock.seconds` Show seconds / Sekunden anzeigen; `hengst.option.memory.style` Display / Anzeige, Werte `percent` Percent / Prozent, `used_max` Used / max / Belegt / max., `both` Both / Beides.

- [ ] **Step 1: Write the failing tests** (`ctx` mit `FakeWallClock wall`)

```java
static long utc(String iso) { return Instant.parse(iso).toEpochMilli(); }
@Test void clock_formats() {
    ClockModule m = new ClockModule(); ModuleConfig cfg = ModuleConfig.defaults();
    wall.utcMillis = utc("2026-11-21T08:05:07Z"); wall.offset = 3_600_000;
    m.tick(ctx, cfg); assertEquals("09:05", m.text());
    cfg.setOption("seconds", true); m.tick(ctx, cfg); assertEquals("09:05:07", m.text());
    cfg.setOption("format", "12h"); m.tick(ctx, cfg); assertEquals("9:05:07 AM", m.text());
    cfg.setOption("seconds", false);
    wall.offset = 0;
    wall.utcMillis = utc("2026-11-21T12:30:00Z"); m.tick(ctx, cfg); assertEquals("12:30 PM", m.text());
    wall.utcMillis = utc("2026-11-21T00:30:00Z"); m.tick(ctx, cfg); assertEquals("12:30 AM", m.text());
    wall.utcMillis = utc("2026-11-21T13:00:00Z"); m.tick(ctx, cfg); assertEquals("1:00 PM", m.text());
}
@Test void the_clock_rolls_over_midnight() {
    ClockModule m = new ClockModule(); ModuleConfig cfg = ModuleConfig.defaults();
    wall.utcMillis = utc("2026-11-21T23:59:59.900Z"); m.tick(ctx, cfg); assertEquals("23:59", m.text());
    wall.utcMillis += 100; m.tick(ctx, cfg); assertEquals("00:00", m.text());
}
@Test void the_clock_follows_a_backward_system_clock_change_at_the_next_tick() {
    ClockModule m = new ClockModule(); ModuleConfig cfg = ModuleConfig.defaults();
    wall.utcMillis = utc("2026-11-21T10:00:30Z"); m.tick(ctx, cfg); assertEquals("10:00", m.text());
    wall.utcMillis -= 3_600_000; m.tick(ctx, cfg); assertEquals("09:00", m.text());
    wall.utcMillis += 3 * 3_600_000; m.tick(ctx, cfg); assertEquals("12:00", m.text());
}
@Test void the_clock_follows_a_dst_offset_change() {
    ClockModule m = new ClockModule(); ModuleConfig cfg = ModuleConfig.defaults();
    wall.offset = 7_200_000; wall.switchAtUtc = utc("2026-10-25T01:00:00Z"); wall.offsetAfterSwitch = 3_600_000;
    wall.utcMillis = utc("2026-10-25T00:59:30Z"); m.tick(ctx, cfg); assertEquals("02:59", m.text());
    wall.utcMillis += 30_000; m.tick(ctx, cfg); assertEquals("02:00", m.text());
}
@Test void ticks_between_boundaries_do_not_ask_for_the_offset() {
    ClockModule m = new ClockModule(); ModuleConfig cfg = ModuleConfig.defaults();
    wall.utcMillis = utc("2026-11-21T10:00:00Z"); m.tick(ctx, cfg);
    int calls = wall.offsetCalls;
    for (int i = 0; i < 20; i++) { wall.utcMillis += 50; m.tick(ctx, cfg); }
    assertEquals(calls, wall.offsetCalls);
}
@Test void memory_formats_and_unknown_max() {
    MemoryModule m = new MemoryModule(); ModuleConfig cfg = ModuleConfig.defaults();
    platform.memoryUsed = 1024L << 20; platform.memoryMax = 4096L << 20;
    m.tick(ctx, cfg); assertEquals("RAM 25% 1024/4096 MB", m.text());
    cfg.setOption("style", "percent"); for (int i = 0; i < 20; i++) { m.tick(ctx, cfg); } assertEquals("RAM 25%", m.text());
    cfg.setOption("style", "used_max"); for (int i = 0; i < 20; i++) { m.tick(ctx, cfg); } assertEquals("RAM 1024/4096 MB", m.text());
    platform.memoryMax = Long.MAX_VALUE; for (int i = 0; i < 20; i++) { m.tick(ctx, cfg); } assertEquals("RAM 1024 MB", m.text());
}
@Test void memory_is_sampled_once_per_second() {
    MemoryModule m = new MemoryModule(); ModuleConfig cfg = ModuleConfig.defaults();
    platform.memoryUsed = 1024L << 20; platform.memoryMax = 4096L << 20; m.tick(ctx, cfg);
    platform.memoryUsed = 2048L << 20;
    for (int i = 0; i < 19; i++) { m.tick(ctx, cfg); }
    assertEquals("RAM 25% 1024/4096 MB", m.text());
    m.tick(ctx, cfg); assertEquals("RAM 50% 2048/4096 MB", m.text());
}
```

(Ein Optionswechsel gilt sofort beim nächsten Tick nur für die Uhr; der Speicher zeigt einen neuen Stil erst bei der nächsten Abfrage, deshalb die 20 Ticks im Test.)

`InfoModulesAllocationTest`: Uhr (Zeit läuft innerhalb einer Minute in 50-ms-Schritten) und Speicher (gleiche Werte) im Renderer: 0 Bytes in 20000 Frames und Ticks.

- [ ] **Step 2: Run tests to verify they fail**

Run: `./gradlew -p common test --tests '*ClockModuleTest' --tests '*MemoryModuleTest'`
Expected: FAIL.

- [ ] **Step 3: Implement und eintragen**

- [ ] **Step 4: Run tests to verify they pass**

Run: `./gradlew -p common test`, dann `./gradlew buildAll`
Expected: PASS; `BUILD SUCCESSFUL`.

- [ ] **Step 5: Commit**

```bash
git add mod/common/src
git commit -m "feat(mod-common): clock and memory hud modules

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 12: Module 8 Ping und 10 Server-Adresse

**Files:**
- Create: `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/hud/modules/PingModule.java`, `ServerAddressModule.java`
- Modify: `.../FeatureCatalog.java`, Sprachdateien
- Test: `.../hud/modules/PingModuleTest.java`, `ServerAddressModuleTest.java`, `InfoModulesAllocationTest.java`, `ModulesTest.java`

**Interfaces:**
- Consumes: `GameView.serverAddress()` (M0, null im Einzelspieler und beim Verbinden nach F11), `GameView.pingMillis()` (Task 8).
- Produces:
  - `PingModule`: ID `ping`, Nummer 8, keine Optionen, Platz TOP_RIGHT -2, 52. Sichtbarkeit je Tick: ohne Welt oder mit `serverAddress() == null` leerer Text (Größe 0, nicht gezeichnet). Wert beim Sichtbarwerden sofort, danach alle 20 Ticks: `pingMillis() >= 0` → `"<n> ms"`, sonst `"? ms"`.
  - `ServerAddressModule`: ID `server_address`, Nummer 10, Option `show_port` (aus), Platz TOP_RIGHT -2, 80. Ohne Welt oder Adresse leerer Text (je Tick geprüft); die Adresse wird beim Sichtbarwerden und dann alle 20 Ticks gelesen und nur bei Änderung (Referenz, dann `equals`) neu aufbereitet. Ohne `show_port`: `[v6]:port` wird `[v6]`; eine Adresse mit genau einem `:` verliert alles ab dem `:`; sonst unverändert (reine IPv6 ohne Klammern). Groß- und Kleinschreibung bleibt. Länger als 40 Zeichen: erste 19 + `".."` + letzte 19.
  - Sprachschlüssel: `hengst.feature.ping` Ping / Ping; `hengst.feature.server_address` Server address / Serveradresse; `hengst.option.server_address.show_port` Show port / Port anzeigen.

- [ ] **Step 1: Write the failing tests**

```java
@Test void ping_is_hidden_without_a_server() {
    PingModule m = new PingModule(); ModuleConfig cfg = ModuleConfig.defaults();
    platform.pingMillis = 42; m.tick(ctx, cfg);
    assertEquals("", m.text()); assertEquals(0, m.width(new FakeCanvas(), cfg));
    platform.serverAddress = "mc.example.net"; m.tick(ctx, cfg); assertEquals("42 ms", m.text());
    platform.serverAddress = null; m.tick(ctx, cfg); assertEquals("", m.text());
}
@Test void ping_is_sampled_once_per_second() {
    PingModule m = new PingModule(); ModuleConfig cfg = ModuleConfig.defaults();
    platform.serverAddress = "mc.example.net"; platform.pingMillis = 42; m.tick(ctx, cfg);
    platform.pingMillis = 57;
    for (int i = 0; i < 19; i++) { m.tick(ctx, cfg); }
    assertEquals("42 ms", m.text());
    m.tick(ctx, cfg); assertEquals("57 ms", m.text());
}
@Test void ping_unknown_or_negative_shows_a_question_mark() {
    PingModule m = new PingModule(); ModuleConfig cfg = ModuleConfig.defaults();
    platform.serverAddress = "mc.example.net"; platform.pingMillis = -1; m.tick(ctx, cfg);
    assertEquals("? ms", m.text());
}
@Test void ping_zero_is_shown() {
    PingModule m = new PingModule(); ModuleConfig cfg = ModuleConfig.defaults();
    platform.serverAddress = "192.168.0.5"; platform.pingMillis = 0; m.tick(ctx, cfg);
    assertEquals("0 ms", m.text());
}
@Test void server_address_port_handling() {
    String[][] cases = {{"mc.example.net:25570", "mc.example.net"}, {"[2001:db8::1]:25565", "[2001:db8::1]"},
            {"2001:db8::1", "2001:db8::1"}, {"Example.NET", "Example.NET"}};
    for (String[] c : cases) {
        ServerAddressModule m = new ServerAddressModule(); ModuleConfig cfg = ModuleConfig.defaults();
        platform.serverAddress = c[0]; m.tick(ctx, cfg); assertEquals(c[1], m.text(), c[0]);
        cfg.setOption("show_port", true);
        ServerAddressModule n = new ServerAddressModule(); n.tick(ctx, cfg); assertEquals(c[0], n.text(), c[0]);
    }
}
@Test void a_long_server_address_is_cut_in_the_middle() {
    ServerAddressModule m = new ServerAddressModule(); ModuleConfig cfg = ModuleConfig.defaults();
    platform.serverAddress = "aaaaaaaaaa.bbbbbbbbbb.cccccccccc.dddddddddd.eeeee";
    m.tick(ctx, cfg);
    assertEquals("aaaaaaaaaa.bbbbbbbb..cc.dddddddddd.eeeee", m.text());   // 49 Zeichen, Zeichen 0 bis 18 + ".." + Zeichen 30 bis 48
    assertEquals(40, m.text().length());
}
@Test void server_address_is_hidden_without_a_server() {
    ServerAddressModule m = new ServerAddressModule(); ModuleConfig cfg = ModuleConfig.defaults();
    m.tick(ctx, cfg); assertEquals("", m.text());
}
@Test void server_address_keeps_its_text_while_the_address_is_unchanged() {
    ServerAddressModule m = new ServerAddressModule(); ModuleConfig cfg = ModuleConfig.defaults();
    platform.serverAddress = "mc.example.net:25570"; m.tick(ctx, cfg);
    String first = m.text();
    for (int i = 0; i < 40; i++) { platform.serverAddress = new String("mc.example.net:25570"); m.tick(ctx, cfg); }
    assertSame(first, m.text());
}
```

(Die Adresse hat 49 Zeichen, nicht 50: a an 0 bis 9, Punkt an 10, b an 11 bis 20, Punkt an 21, c an 22 bis 31, Punkt an 32, d an 33 bis 42, Punkt an 43, e an 44 bis 48. Kürzung: erste 19 Zeichen (Index 0 bis 18, `aaaaaaaaaa.bbbbbbbb`) + `".."` + letzte 19 Zeichen (Index 30 bis 48, `cc.dddddddddd.eeeee`), zusammen 40. Der Implementierer prüft die erwartete Zeichenkette mit `s.substring(0, 19) + ".." + s.substring(30)`, bevor er den Test festschreibt.)

`InfoModulesAllocationTest`: Ping und Server-Adresse aktiv mit fester Adresse und festem Ping, 20000 Frames und Ticks: 0 Bytes.

- [ ] **Step 2: Run tests to verify they fail**

Run: `./gradlew -p common test --tests '*PingModuleTest' --tests '*ServerAddressModuleTest'`
Expected: FAIL.

- [ ] **Step 3: Implement und eintragen**

- [ ] **Step 4: Run tests to verify they pass**

Run: `./gradlew -p common test`, dann `./gradlew buildAll`
Expected: PASS; `BUILD SUCCESSFUL`.

- [ ] **Step 5: Commit**

```bash
git add mod/common/src
git commit -m "feat(mod-common): ping and server address hud modules

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 13: Modul 21 Stoppuhr mit zwei Tasten

**Files:**
- Create: `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/hud/modules/StopwatchModule.java`
- Modify: `.../hud/HudModule.java`, `.../hud/TextHudModule.java` (Konstruktor mit Gruppe), `.../ui/SettingsScreen.java` (HUD-Stilzeilen für jede `HudModule`, siehe unten), `.../FeatureCatalog.java`, Sprachdateien in common; `mod/modern/src/main/resources/assets/hengst/lang/en_us.json`, `de_de.json`; `mod/legacy/src/main/resources/assets/hengst/lang/en_US.lang`, `de_DE.lang`
- Test: `.../hud/modules/StopwatchModuleTest.java`, `InfoModulesAllocationTest.java`, `ModulesTest.java`, `.../HengstCoreKeysTest.java`, `.../ui/SettingsScreenTest.java`

**Interfaces:**
- Consumes: `KeySpec`, `Feature.onKey`, `HengstCore.onFeatureKey` (Task 7), Adapter-Registrierung (Task 8).
- Produces:
  - `HudModule(String id, int number, FeatureGroup group, EnumSet<GameVersion> versions)` und dasselbe für `TextHudModule`; die alten Konstruktoren bleiben (Gruppe HUD).
  - `StopwatchModule`: ID `stopwatch`, Nummer 21, Gruppe COMFORT (Entscheidung: ein Werkzeug mit eigenen Tasten, bringt die zweite Gruppe in die Liste; bleibt mit dem Eigentümer offen: Gruppe COMFORT), Option `tenths` (an), Platz TOP_LEFT 2, 72. Tasten `stopwatch.toggle` (NONE, PRESS) und `stopwatch.reset` (NONE, PRESS), Indizes 2 und 3 in `FeatureCatalog.keys`. Zeit nur aus `TimeSource` (monoton, `nowNanos` von `onKey`, `ctx.time()` im Tick); Start/Stopp addiert die gelaufene Zeit; Zurücksetzen stoppt und setzt auf 0; Loslassen tut nichts; der Zustand gilt für die Sitzung und wird nicht gespeichert. Text je Tick, neu nur bei geänderter angezeigter Zehntelsekunde: unter einer Stunde `"m:ss.t"` (`"0:00.0"`, `"1:05.3"`), ab einer Stunde `"h:mm:ss.t"` (`"1:00:00.0"`); ohne `tenths` ohne `".t"`; Zehntel abgerundet.
  - Einstellungsbildschirm: Hintergrund-, Schatten- und Farbzeile erscheinen für jede Funktion, die ein `HudModule` ist (`f instanceof HudModule`, unabhängig von der Gruppe; bisher `f.group() == FeatureGroup.HUD` in `SettingsScreen.buildOptions`), weil die Stoppuhr in COMFORT liegt und trotzdem mit diesem Stil zeichnet; Funktionen ohne HUD (die anonyme Funktion `zoom` in VISUAL) behalten keine Stilzeilen.
  - Sprachschlüssel common: `hengst.feature.stopwatch` Stopwatch / Stoppuhr; `hengst.option.stopwatch.tenths` Show tenths / Zehntel anzeigen. Adapter (modern JSON, 1.8.9 `.lang` in UTF-8, [B] `TranslationStorage` liest UTF-8): `key.hengst.stopwatch.toggle` Stopwatch start/stop / Stoppuhr Start/Stopp; `key.hengst.stopwatch.reset` Stopwatch reset / Stoppuhr zurücksetzen.

- [ ] **Step 1: Write the failing tests**

```java
StopwatchModule m = new StopwatchModule(); ModuleConfig cfg = ModuleConfig.defaults();
@Test void the_stopwatch_shows_zero_until_started() {
    m.tick(ctx, cfg); assertEquals("0:00.0", m.text());
    cfg.setOption("tenths", false); m.tick(ctx, cfg); assertEquals("0:00", m.text());
}
@Test void start_stop_and_continue() {
    m.onKey(0, true, time.now);
    time.advanceMillis(65_340); m.tick(ctx, cfg); assertEquals("1:05.3", m.text());
    m.onKey(0, true, time.now);
    time.advanceMillis(10_000); m.tick(ctx, cfg); assertEquals("1:05.3", m.text());
    m.onKey(0, true, time.now);
    time.advanceMillis(1_000); m.tick(ctx, cfg); assertEquals("1:06.3", m.text());
}
@Test void hours_and_reset() {
    m.onKey(0, true, time.now); time.advanceMillis(3_600_050); m.tick(ctx, cfg); assertEquals("1:00:00.0", m.text());
    cfg.setOption("tenths", false); m.tick(ctx, cfg); assertEquals("1:00:00", m.text());
    m.onKey(1, true, time.now); time.advanceMillis(2_000); m.tick(ctx, cfg); assertEquals("0:00", m.text());
}
@Test void releases_do_nothing() {
    m.onKey(0, false, time.now); time.advanceMillis(1_000); m.tick(ctx, cfg); assertEquals("0:00.0", m.text());
}
@Test void the_stopwatch_uses_the_monotonic_time_source() {
    // Die Wanduhr ist im Kontext verdrahtet; eine verstellte Wanduhr hat keinen Einfluss, und die Stoppuhr fragt sie nie.
    FakeWallClock wall = new FakeWallClock();
    HudContext wctx = new HudContext(platform, time, wall, new FrameCounter(time), new CpsCounter(time));
    m.onKey(0, true, time.now); time.advanceMillis(2_000); wall.utcMillis -= 3_600_000;
    m.tick(wctx, cfg); assertEquals("0:02.0", m.text());
    time.advanceMillis(1_000); wall.utcMillis += 7_200_000; m.tick(wctx, cfg); assertEquals("0:03.0", m.text());
    assertEquals(0, wall.currentCalls); assertEquals(0, wall.offsetCalls);
}
// HengstCoreKeysTest
@Test void the_stopwatch_keys_have_the_persisted_names_at_indices_2_and_3() {
    core = start();
    assertEquals("key.hengst.stopwatch.toggle", core.keySpecs().get(2).translationKey());
    assertEquals("key.hengst.stopwatch.reset", core.keySpecs().get(3).translationKey());
    assertEquals(DefaultKey.NONE, core.keySpecs().get(2).defaultKey());
    assertEquals(4, core.keySpecs().size());           // keine weitere Funktion hat Tasten: die Testtasten aus Task 7 liegen ab Index 4
}
@Test void one_press_through_the_core_toggles_once() {
    core = start(); platform.inWorld = true;
    core.activeProfile().module("stopwatch").setEnabled(true); core.applyConfigForTest();
    core.onFeatureKey(2, true); core.onFeatureKey(2, true); core.onFeatureKey(2, false);
    time.advanceMillis(1_500); core.onClientTick();
    assertEquals("0:01.5", stopwatch(core).text());
}
@Test void the_stopwatch_key_with_a_screen_open_does_nothing() {
    core = start(); platform.inWorld = true;
    core.activeProfile().module("stopwatch").setEnabled(true); core.applyConfigForTest();
    platform.screenOpen = true; core.onFeatureKey(2, true); core.onFeatureKey(2, false);
    platform.screenOpen = false; time.advanceMillis(1_500); core.onClientTick();
    assertEquals("0:00.0", stopwatch(core).text());
}
```

(`stopwatch(core)`: das Modul aus `core.registry().byId("stopwatch")`.) Die Adapter-Tests aus Task 8 (`every_key_of_every_version_is_named_in_both_language_files`) schlagen ohne die neuen Adapter-Schlüssel fehl und belegen so deren Vollständigkeit.

In `SettingsScreenTest`:

```java
@Test void a_hud_module_in_another_group_gets_the_hud_style_rows() {
    FeatureRegistry r = new FeatureRegistry();
    r.register(new StopwatchModule());                       // Gruppe COMFORT: Überschrift Zeile 0, Stoppuhr Zeile 1
    open(r, "en_us");
    screen.render(canvas, 0, 0, 0f);
    screen.mouseClicked(60, 36 + 20 + 5, 0);
    assertEquals("stopwatch", model.selected().id());
    assertEquals(4, screen.optionList().widgets().size());   // background, shadow, colour, tenths
}
```

`a_feature_that_is_not_on_the_hud_gets_no_hud_style_rows` (Task 9) bleibt unverändert grün: die anonyme Funktion `zoom` ist kein `HudModule`.

`InfoModulesAllocationTest`: Stoppuhr angehalten 20000 Frames und Ticks: 0 Bytes.

- [ ] **Step 2: Run tests to verify they fail**

Run: `./gradlew -p common test --tests '*StopwatchModuleTest' --tests '*HengstCoreKeysTest'`
Expected: FAIL.

- [ ] **Step 3: Implement, eintragen, Adapter-Sprachdateien ergänzen**

- [ ] **Step 4: Run tests to verify they pass**

Run: `./gradlew -p common test`, `./gradlew -p legacy test`, `./gradlew -p modern :1.21.11:test`, `./gradlew -p modern :26.3:test`, `./gradlew buildAll`
Expected: PASS; `BUILD SUCCESSFUL`.

- [ ] **Step 5: Commit**

```bash
git add mod/common/src mod/modern/src/main/resources mod/legacy/src/main/resources
git commit -m "feat(mod): stopwatch hud module with start/stop and reset keys

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 14: Spieltest modern: die sieben Module einzeln im echten Spiel

**Files:**
- Modify: `mod/modern/src/gametest/java/io/github/clemensjl/hengst/mod/modern/HengstClientGameTest.java`

**Interfaces:**
- Consumes: alle Module (Tasks 10 bis 13), `applyConfigForTest`, `FeatureState.on`, `keySpecs()`; Helfer des Tests (`layout`, `shot`, `f1Baseline`, `checkShown`, `checkSame`, `waitUntil`, `waitRealTime`, `mapping`).
- Produces: eine Methode `infoModules(ClientGameTestContext ctx, HengstCore core)`, aufgerufen in `fullPhase` nach den M0-Prüfungen und vor `dragFpsToBottomRight`. Je Modul nacheinander: auf dem Client einschalten (`module(id).setEnabled(true)`, `applyConfigForTest()`), `check(FeatureState.on(nr))`, auf den erwarteten Text warten, Screenshot `hengst-4x-<id>`, F1-Basis, Pixelprüfung im Rechteck des Moduls aus `layout(ctx)`, Ausschalten, `check(!FeatureState.on(nr))`. Erwartungen:
  - `direction`: Gierwinkel auf dem Client `mc.player.setYRot(180f)` [K Setter-Name]: Text `"N 0°"`, gezeichnet (Pixel).
  - `speed`: still `"0.0 m/s"`; W eine Sekunde halten: Wert ≥ 3,0 (Zahl aus dem Text), gezeichnet.
  - `clock`: Text passt auf `\d\d:\d\d` und liegt höchstens eine Minute neben `LocalTime.now()`; gezeichnet.
  - `memory`: Text passt auf `RAM \d+% \d+/\d+ MB`; gezeichnet.
  - `ping` und `server_address` im Einzelspieler: Text leer, kein Rechteck im Layout, Pixel gleich der F1-Basis an ihrem Standardplatz (`checkSame`). [K] Bietet die Fabric-Client-Gametest-API des Knotens einen Dedicated-Server-Kontext (in den Quellen von Fabric API 0.166.2 bzw. 0.141.6 nach `TestDedicatedServerContext` suchen), verbindet der Test zusätzlich dorthin und prüft `"<n> ms"` bzw. die Adresse als Pixel; sonst steht "Mehrspieler nicht geprüft" im Report.
  - `stopwatch`: die Tasten `key.hengst.stopwatch.toggle` auf K und `key.hengst.stopwatch.reset` auf J binden (`KeyMapping.setKey`, `KeyMapping.resetMapping()` [K]). K und J sind auf beiden Versionen in den Vanilla-Belegungen unbelegt [K: verify in den Quellen der Version: `KeyMapping`-Konstruktoren in `Options`, 1.21.11 und 26.3 belegen unter anderem L mit "Advancements", daher nicht L]; der Test prüft trotzdem vor dem Binden über `options.keyMappings`, dass keine andere Belegung K oder J trägt, und bricht sonst mit der Belegung im Text ab (eine Taste gehört dem Spiel nie zweimal). Die Belegungen werden in einem `finally` zurückgesetzt (`mapping.setKey(mapping.getDefaultKey())` und `KeyMapping.resetMapping()`), auch wenn ein Schritt fehlschlägt. Ablauf: K drücken, 1,5 s echte Zeit, K drücken: angezeigter Wert zwischen 1,0 und 2,5 s; Chat öffnen, K drücken, schließen: Text unverändert; J: `"0:00.0"`; gezeichnet.
  - Danach `check(core.renderer().activeCount() == 4)`: die M0-Prüfungen und die Neustart-Phase laufen unverändert.

- [ ] **Step 1: Namen prüfen**

Run: `./gradlew -p modern :26.3:genSources`, `./gradlew -p modern :1.21.11:genSources`
Expected: [K]-Stellen dieses Tasks nachgeschlagen, Ergebnis im Report.

- [ ] **Step 2: Schritte schreiben (der Test ist hier der Code), dann laufen lassen**

Run: `./gradlew -p modern :26.3:clientGameTests`, dann `./gradlew -p modern :1.21.11:clientGameTests`
Expected: beide Phasen grün auf beiden Knoten; im Log je Modul `[hengst-gametest] PASS ...`. Gegenprobe einmal pro Knoten (nicht committen): Hintergrund des Richtungsmoduls auf Alpha 0 und Textfarbe gleich dem Himmel gesetzt lässt die Pixelprüfung fehlschlagen.

- [ ] **Step 3: Commit**

```bash
git add mod/modern/src/gametest
git commit -m "test(mod-modern): in-game checks of the seven text modules on 1.21.11 and 26.3

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 15: Spieltest 1.8.9: die sieben Module einzeln im echten Spiel

**Files:**
- Modify: `mod/legacy/src/gametest/java/io/github/clemensjl/hengst/mod/legacy/LegacyGameTest.java`

**Interfaces:**
- Consumes: wie Task 14; Helfer des Harness (`onClient`, `layout()`, `shot`, `f1Baseline`, `checkShown`, `checkSame`, `waitUntil`, `waitRealTime`, `key`, `keyDown`).
- Produces: `infoModules(HengstCore core)` in `fullPhase` nach den M0-Prüfungen und vor `dragFpsToBottomRight`, mit denselben Erwartungen wie Task 14. Abweichungen: Gierwinkel über das Feld `player.yaw = 180f` [B]; Laufen über `keyDown(Keyboard.KEY_W, ...)`; Stoppuhr-Tasten über `KeyBinding.setCode(int)` und `KeyBinding.updateKeysByCode()` [K Yarn-Namen] auf `Keyboard.KEY_K` (Start/Stopp) und `KEY_J` (Zurücksetzen); beide sind in den Vanilla-Belegungen von 1.8.9 unbelegt [K: verify in `GameOptions`], der Test prüft das trotzdem über `GameOptions.keysAll` vor dem Binden und setzt beide Belegungen in einem `finally` auf ihren Standard zurück (`setCode(getDefaultKeyCode())`, `updateKeysByCode()`); Chat über `Keyboard.KEY_T`. Die Prüfung `settingsWheel` (Offset 20 nach einer Raste) braucht eine überlaufende Liste: mit den Überschriften sind es 5 Zeilen statt 4 (Task 9); läuft die Liste im Fenster des Harness nicht über, wird die Fenstergröße oder die Erwartung dort angepasst und im Report genannt. Mehrspieler ist im 1.8.9-Harness nicht möglich (zwei Spiele gleichzeitig sind verboten, ein Dedicated Server fehlt): Ping und Server-Adresse werden nur als unsichtbar im Einzelspieler geprüft.

- [ ] **Step 1: Namen prüfen**

Run: `./gradlew -p legacy genSources`
Expected: [K]-Stellen nachgeschlagen.

- [ ] **Step 2: Schritte schreiben, dann laufen lassen**

Run: `./gradlew -p legacy gameTests`
Expected: beide Phasen grün; Screenshots `hengst-4x-*` in `legacy/build/run/gameTest/screenshots/`; Gegenprobe wie Task 14 (nicht committen).

- [ ] **Step 3: Commit**

```bash
git add mod/legacy/src/gametest
git commit -m "test(mod-legacy): in-game checks of the seven text modules on 1.8.9

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 16: Kosten je Modul messen (D7)

**Files:**
- Create: `mod/common/src/test/java/io/github/clemensjl/hengst/mod/common/testutil/TimedModule.java`
- Modify: `mod/common/src/main/java/io/github/clemensjl/hengst/mod/common/hud/HudRenderer.java`, `mod/modern/src/gametest/.../HengstClientGameTest.java`, `mod/legacy/src/gametest/.../LegacyGameTest.java`, `docs/begleitmod-spec.md` (Abschnitt 8.8)
- Test: `mod/common/src/test/java/.../hud/HudRendererTest.java`, `.../HengstCoreAllocationTest.java`

**Interfaces:**
- Produces:
  - `HudRenderer`: `public void setRenderTimes(long[] lastFrameNanos)` (nur Tests, auch die Spieltests; null ist Standard): ist ein Array gesetzt, schreibt `render` je aktivem Modul (Index wie `activeModules()`) die Dauer von `width`/`height`, `resolve`, Transform und `render` dieses Frames in ns über `ctx.time().nanoTime()`; übersprungene Module bekommen 0. Ohne Array kostet es eine Abfrage je Frame.
  - Messung in beiden Spieltests: 120 Frames einschwingen, dann 600 Frames sammeln, Median. Modern liest die Werte im `ProbeElement` nach `inner`, 1.8.9 im Harness-Rückruf `afterHud()`. Messfälle: die vier M0-Module zusammen (Summe je Frame); jedes der sieben neuen Module allein; alle elf zusammen. Budgets (Entscheidung): vier M0-Module zusammen höchstens 50 µs (Spec 8.8), jedes neue Textmodul allein höchstens 10 µs, alle elf zusammen höchstens 120 µs. Logzeile je Fall `[hengst-gametest] COST <fall> median <ns> ns`. Gemessen wird auf allen drei Versionen dieselbe Größe: die Zeit, die der Kern je Modul und Frame im Modul verbringt. Auf den modernen Versionen ist das die CPU-Zeit bis zur Abgabe des Zeichenauftrags (das Spiel zeichnet erst später gesammelt), auf 1.8.9 enthält sie die Aufrufe der festen Pipeline. Die Kosten des späteren Zeichnens erfasst diese Messung nicht; sie deckt der Bildratenvergleich in Task 19 ab. Die 50 µs für die vier M0-Module sind das Budget dieser gemessenen Größe auf allen drei Versionen (sie sind keine Fortschreibung der M0-Zahlen aus dem Bildratenvergleich, 26.3 etwa 26 µs, 1.8.9 etwa 34 µs, 1.21.11 nicht auflösbar).
  - `testutil/TimedModule` (einmal hier definiert): `public final class TimedModule extends HudModule` mit `public TimedModule(String id, int number, int width, int height, FakeTime time, long renderNanos)`; `width(Canvas, ModuleConfig)` und `height(Canvas, ModuleConfig)` liefern die Maße, `render(Canvas, ModuleConfig, HudContext)` rechnet `time.now += renderNanos` und zeichnet nichts; Gruppe HUD, alle Versionen (`super(id, number, EnumSet.allOf(GameVersion.class))`). `FixedSizeModule` bleibt `final` und unverändert.
  - Spec 8.8 bekommt den Satz: "Die Messung je Modul im Spieltest erfasst die Zeit, die der Kern je Frame im Modul verbringt: auf 1.21.11 und 26.3 bis zur Abgabe der Zeichenaufträge, weil das Spiel später gesammelt zeichnet, auf 1.8.9 einschließlich der Aufrufe der festen Pipeline. Sie ist nicht der Bildratenvergleich der M0-Abschlussprüfung (26.3 etwa 26, 1.8.9 etwa 34 Mikrosekunden, der die Kosten des späteren Zeichnens mitzählt und je Meilenstein einmal wiederholt wird). Budget der gemessenen Größe (Median im Spieltest, je Version): die vier M0-Module zusammen 50 µs, jedes Textmodul ab M1a allein 10 µs, alle elf Module von M0 und M1a zusammen 120 µs."

- [ ] **Step 1: Write the failing tests**

```java
@Test void render_times_are_written_per_module_when_asked() {
    // TimedModule (testutil), dessen render die FakeTime um 7 µs vorstellt; zweites Modul mit Größe 0
    FeatureRegistry reg = new FeatureRegistry();
    reg.register(new TimedModule("t1", 40, 20, 10, time, 7_000));
    reg.register(new TimedModule("t2", 41, 0, 0, time, 9_000));
    Profile profile = new Profile();
    profile.module("t1").setEnabled(true); profile.module("t2").setEnabled(true);
    HudRenderer renderer = new HudRenderer(reg, GameVersion.V1_8_9);
    renderer.rebuild(profile);
    FakeCanvas canvas = new FakeCanvas();
    long[] times = new long[2];
    renderer.setRenderTimes(times);
    renderer.render(canvas, ctx);
    assertEquals(7_000, times[0]);
    assertEquals(0, times[1]);
    renderer.setRenderTimes(null);
    renderer.render(canvas, ctx);
    assertEquals(7_000, times[0]);
}
```

In `HengstCoreAllocationTest`: `render_with_render_times_set_allocates_nothing`: Array gesetzt, 20000 Frames: 0 Bytes.

- [ ] **Step 2: Run tests to verify they fail**

Run: `./gradlew -p common test --tests '*HudRendererTest'`
Expected: FAIL.

- [ ] **Step 3: Implement, Messschritte in beide Spieltests einbauen**

- [ ] **Step 4: Run tests and the games**

Run: `./gradlew -p common test`, `./gradlew buildAll`, dann nacheinander `./gradlew -p modern :26.3:clientGameTests`, `./gradlew -p modern :1.21.11:clientGameTests`, `./gradlew -p legacy gameTests`
Expected: grün auf allen drei; die Medianwerte (alle Fälle, alle Versionen) als Tabelle im Report. Reißt ein Budget auf 1.8.9: Task 17 ausführen (die Prüfung bleibt rot, bis Task 17 sie grün macht; dann diesen Task erst mit Task 17 committen). Reißt ein Budget auf einer modernen Version: anhalten und mit Messwerten melden (NEEDS_CONTEXT), nicht das Budget ändern.

- [ ] **Step 5: Commit**

```bash
git add mod/common/src mod/modern/src/gametest mod/legacy/src/gametest docs/begleitmod-spec.md
git commit -m "test(mod): per-module frame cost measured in game against the spec budget

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 17 (bedingt): GL-Zustand auf 1.8.9 einmal je Frame setzen

**Auslöser:** In Task 16 liegt auf 1.8.9 der Median der vier M0-Module über 50 µs, eines neuen Moduls über 10 µs oder aller elf über 120 µs. Ohne Auslöser wird dieser Task nicht ausgeführt; der Ledger vermerkt "Task 17 nicht nötig" mit den Messwerten.

**Files:**
- Modify: `mod/legacy/src/main/java/io/github/clemensjl/hengst/mod/legacy/LegacyCanvas.java`, `HengstLegacyMod.java` (`onHud`)

**Interfaces:**
- Produces: `LegacyCanvas.beginBatch()` und `endBatch()`, nur im HUD-Hook um `core.onHudRender` (Bildschirme bleiben beim heutigen Verhalten). In einem Batch merkt sich die Leinwand den Modus FILL oder TEXT und schaltet den Zustand nur beim Moduswechsel: FILL = Blending an mit `blendFuncSeparate(770, 771, 1, 0)`, Textur aus, Alpha-Test aus, gezeichnet über `Tessellator`/`BufferBuilder` mit Positionsformat [K Yarn-Namen in build.604] statt `DrawableHelper.fill`; TEXT = Blending an, Textur an, Alpha-Test an. Die Zeichenreihenfolge bleibt (kein Sammeln über Module hinweg). `endBatch` stellt den heutigen Endzustand her: Blending aus, Textur an, Alpha-Test an, Farbe weiß. Die Grenzen für Alpha (Text unter Alpha 4 übersprungen, Fills mit Alpha 0 übersprungen) bleiben.

- [ ] **Step 1: Namen prüfen** (`./gradlew -p legacy genSources`, `Tessellator`, `BufferBuilder`, `VertexFormats`)

- [ ] **Step 2: Implement**

- [ ] **Step 3: Run the game**

Run: `./gradlew -p legacy test`, `./gradlew buildAll`, `./gradlew -p legacy gameTests`
Expected: grün, darunter die bestehenden Schritte zu blassen Fills ("fill with alpha 25 drawn"), zum Modul über einem Hotbar-Gegenstand und "vanilla hotbar, hearts and chat line identical"; die Kostenprüfung aus Task 16 jetzt im Budget, neue Medianwerte im Report.

- [ ] **Step 4: Commit** (zusammen mit Task 16, falls dessen Prüfung rot war)

```bash
git add mod/legacy/src/main
git commit -m "perf(mod-legacy): set gl state once per mode change in the hud pass

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 18: Doku nachführen

**Files:**
- Modify: `docs/begleitmod-spec.md`, `mod/README.md`

**Interfaces:**
- Consumes: Reports der Tasks 1 bis 17 (geprüfte Hook-Namen, Messwerte).

- [ ] **Step 1: Spec**

Abschnitt 4.3: neue Schnittstellen und Einstiegspunkte mit Signatur (`GameView.pingMillis()`, `worldIdentity()`, `memoryUsedBytes()`, `memoryMaxBytes()`; `WallClock`; `HengstCore.onFeatureKey(int, boolean)`, `keySpecs()`; `KeySpec`, `DefaultKey`; `FeatureState.on(int)`; `RestrictionPolicy`), Satz: "Mixins späterer Funktionen prüfen als Erstes `FeatureState.on(<nummer>)`." Abschnitt 6: Gruppen-Überschriften, Bildlaufanzeige, Tasten je Funktion in der Kategorie "Hengst" (Stoppuhr unbelegt). Abschnitt 7: für 7, 8, 10, 15 die in M1a genutzten Hooks mit geprüftem Namen und Datum auf [B] setzen oder korrigieren; Gruppe der Stoppuhr. Abschnitt 9.2: Satz, dass der Kern den Serverwechsel im Tick erkennt und die Sperrliste ab M4 über `RestrictionPolicy` einspeist. Abschnitt 8, Regel 6 ("Kein Dateizugriff im Frame oder Tick"): Satz "Einzige Ausnahme: Beim Wechsel der Spielsprache liest der Tick einmal die Sprachdatei aus dem Klassenpfad (`I18n.load`), höchstens einmal je Wechsel und nie im Frame-Pfad." Abschnitt 5 (Konfiguration) und Abschnitt 8, Regel 8 stehen mit dem Wortlaut aus Task 2 beziehungsweise Task 16 schon im Text; hier nur gegenlesen, dass sie dort ohne Widerspruch zu 4.3 stehen.

- [ ] **Step 2: README**

Abschnitt "Install into an instance": Tastennamen der Stoppuhr; ein Absatz "Known limitations" (englisch, wie die README): (1) nach einem Lauf mit einem M0-Build verwirft das Spiel selbst die unbekannten Tastenzeilen `key_key.hengst.stopwatch.*` aus `options.txt`, die Stoppuhr-Tasten sind dann neu zu belegen; (2) ein M0-Build verwirft beim Zurückschreiben unbekannte Wurzelschlüssel eines Profils, unbekannte Schlüssel in den Einträgen seiner vier Module und unbekannte Schlüssel in `settings.json`; M1a legt deshalb neue Daten nur als Moduleintrag oder in `options` ab, und die Stoppuhr speichert nichts. Neuer Abschnitt "What was verified (M1a)" bleibt leer bis Task 19.

- [ ] **Step 3: Commit**

```bash
git add docs/begleitmod-spec.md mod/README.md
git commit -m "docs(mod): m1a interfaces, keys, groups and verified hooks

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

### Task 19: Gesamtprüfung mit den ausgelieferten Jars über den Launcher

**Files:**
- Modify: `mod/README.md` (Abschnitt "What was verified (M1a)"), `docs/begleitmod-spec.md` (Abschnitt 12: M1a erledigt mit Datum)

**Interfaces:**
- Consumes: `mod/dist/` aus `./gradlew clean buildAll`; Launcher-Checkout `C:\Users\jelec\Documents\Projekte\hengst-client`.

- [ ] **Step 1: Bauen**

Run: `./gradlew clean buildAll`; `unzip -l` der drei Jars
Expected: `BUILD SUCCESSFUL`; drei Jars; keine Testklassen; `LICENSE_hengst` enthalten. Scheitert der Lauf mit "Unable to delete directory ... test-results ... binary": die leeren `binary`-Verzeichnisse löschen und wiederholen (Global Constraints).

- [ ] **Step 2: Spielläufe wie in M0**

Vorgehen, Instanzen, Laufzeiten und Steuerung genau wie in `.superpowers/sdd/2026-11-20-begleitmod-m0-foundation/task-14-report.md`, Abschnitte "Setup", "Per version" und "E: shipped jars". Je Version, nacheinander, nie zwei Spiele oder Spiel und Build gleichzeitig:
1. Log `[hengst] loaded on <version>`, mit `-Dmixin.debug.verbose=true` alle Mod-Mixins angewandt, keine Mixin-Warnung zur Mod.
2. Beim ersten Start nur die vier M0-Module; in den Einstellungen die Gruppen "HUD" und "Komfort" (Deutsch) bzw. "Comfort", Bildlaufanzeige bei 320 x 240 GUI-Pixeln.
3. Jedes der sieben Module einschalten: es erscheint an seinem Standardplatz mit dem Format aus der Tabelle; Ping und Server-Adresse im Einzelspieler unsichtbar.
4. Stoppuhr: in den Steuerungsoptionen unter "Hengst" mit den zwei Namen; eine Taste binden, starten, stoppen, zurücksetzen.
5. Zuerst nach Osten blicken (1.21.11 und 26.3 `/tp @s ~ ~ ~ -90 0`, 1.8.9 `/tp @p ~ ~ ~ -90 0` [K: Selektor und Winkelargumente der Version prüfen]): die Richtung zeigt `E 90°`. Dann die Sprache im Spiel auf Deutsch stellen, Welt bleibt offen: die Richtung zeigt nach höchstens einer Sekunde `O 90°` statt `E 90°` (am Spawn mit Blickwinkel 0 zeigt sie in beiden Sprachen `S`, der Wechsel wäre dort nicht zu sehen).
6. 26.3: wie in M0 über den eigenen Spieltest im vom Launcher gestarteten Spiel (Schritte 2 bis 5 dort, soweit der Test sie enthält).
7. FPS-Vergleich nach der Methode aus Abschnitt "D: cost measurement" des Task-14-Reports: ohne Mod, mit den vier M0-Modulen, mit allen elf; Tabelle in den README-Abschnitt.

- [ ] **Step 3: README und Spec**

"What was verified (M1a)" mit Datum, Laufzeiten, Ergebnissen aus Step 2, Messwerten aus Task 16 und Step 2.7, und "Not verified" (mindestens: Mehrspieler auf 1.8.9; auf modernen Versionen, falls Task 14 keinen Server-Kontext fand). Spec 12: M1a "Erledigt <Datum>".

- [ ] **Step 4: Commit**

```bash
git add mod/README.md docs/begleitmod-spec.md
git commit -m "docs(mod): m1a verified with the shipped jars on 1.8.9, 1.21.11 and 26.3

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>"
```

---

## Self-Review

1. **Spec-Abdeckung:** D1 Task 5 (+ Nachweis Task 8), D2 Tasks 7, 8, 13, D3 Task 6 (I18n, WallClock) und Tasks 8, 11 (GameView), D4 Task 1, D5 Task 2, D6 Task 6, D7 Tasks 16, 17, F8 Task 5, F9 Task 3, F10 Task 4, Gruppen und Bildlaufanzeige Task 9, Module 7 bis 21 Tasks 10 bis 13, Spielprüfung Tasks 14, 15, 19, Doku Tasks 2, 16, 18, 19. Nicht in M1a: Item- und Sprite-Zeichnen (M1b), echte Sperrliste (M4).
2. **Schritt-Scan:** jede Test-Stufe nennt Name und Zusicherungen; die Spieltest-Tasks beschreiben Schritte und Erwartungen, weil der Test dort der Code ist; die drei Rechenstellen mit Ableitung (Bildlaufanzeige, Kürzung der Adresse, Geschwindigkeit mit 2 Stellen) stehen mit Rechenweg da.
3. **Typen:** `applyConfigForTest`, `keySpecs`, `onFeatureKey(int, boolean)`, `FeatureState.on(int)`, `FeatureGate.active(Feature, ModuleConfig)`, `HudContext.setI18n/i18n/languageGeneration/wallClock`, `GameView.pingMillis/worldIdentity/memoryUsedBytes/memoryMaxBytes`, `ScreenClose(Save, Runnable, Platform, String)`, `TextFit.prefix/middle/wrap`, `ScrollAccumulator(double)`, `AdapterGuard(String[], Log)` werden in den späteren Tasks mit genau diesen Namen genutzt.
4. **Review Focus:** fünf Zeilen, jede mit Tests in Task 2, 6, 10, 11, 12, 13; die Tests von Zeile 1 prüfen auch, was M0-Builds nach einem Rückschreiben übrig lassen (`M0CompatibilityTest`).
5. **Umfang:** Code-Blöcke sind Tests und Commits; Körper nur, wo eine Regel sonst offen wäre (Kurs und Sektor, Zeitgrenzen, Port-Regel).
