# Begleitmod: Spezifikation

Stand 22.11.2026 (M1a). Ersetzt PLAN.md Entscheidung 13.3 ("AxolotlClient bündeln"): Hengst Client bekommt eine eigene Client-Mod mit 50 Funktionen, von Grund auf geschrieben, gleichzeitig für die Spielversionen **1.8.9, 1.21.11 und 26.3**. Ort im Repo: `mod/`. Lizenz wie das Repo: GPL-3.0-or-later.

Alle Fakten zur Toolchain unten sind am 20.11.2026 gegen Primärquellen geprüft (Quelle steht daneben). Was nicht prüfbar war, steht als **ungeprüft** da.

## 1. Ziele

1. 50 Funktionen (Liste in Abschnitt 7) auf drei Spielversionen, mit einer gemeinsamen Codebasis für alles, was nicht vom Spiel abhängt.
2. Korrekt: Verhalten jeder Funktion ist auf allen Versionen gleich, wo die Version es zulässt, und in der Matrix (Abschnitt 7) ausdrücklich anders, wo nicht.
3. Billig zur Laufzeit: keine Allokation pro Frame im HUD-Pfad, keine Reflection in heißen Pfaden, Mixins so schmal wie möglich, abgeschaltete Funktionen kosten nichts (Regeln in Abschnitt 8).
4. Regelkonform: keine Funktion gibt einen Vorteil über Vanilla-Information hinaus. Funktionen, die Server teils verbieten, lassen sich je Server abschalten und werden über eine mitgelieferte Sperrliste automatisch gesperrt (Abschnitt 9).
5. Testbar: Kernlogik läuft in JUnit ohne Spiel; jede Version hat eine konkrete Prüfung im echten Spiel.

## 2. Nicht-Ziele

- Kein Code aus geschlossenen Clients (Lunar, Feather, Badlion), auch keine dekompilierten Teile oder Assets daraus. Funktionsideen sind frei, Umsetzung ist eigen.
- Keine weiteren Spielversionen in dieser Spezifikation (neue Version = eigener Plan, siehe 4.4).
- Keine Forge-/NeoForge-/Quilt-Varianten der Mod. Quilt Loader lädt Fabric-Mods ohnehin, wird aber nicht getestet.
- Keine Cosmetics (eigene Phase laut PLAN.md 13.6), keine Server-Anbindung, kein Netzwerkverkehr der Mod (Ausnahme später: optionales Server-Signal zum Abschalten, Abschnitt 9.4).
- Keine Entity-Radar-, X-Ray-, Höhlen-Minimap-, Auto-Klick- oder Hilfsfunktionen, die über Vanilla-Information hinausgehen.
- Keine eigene Benchmark-Szene in diesem Dokument (bleibt PLAN.md 5, eigener Plan).

## 3. Toolchain je Version (geprüft 20.11.2026)

| | 1.8.9 | 1.21.11 | 26.3 |
|---|---|---|---|
| Loader | Legacy Fabric (Fabric Loader 0.20.1, Intermediary `net.legacyfabric:intermediary:1.8.9`) [1] | Fabric Loader 0.20.1 (stabil) [2] | Fabric Loader 0.20.1 [2] |
| Gradle-Plugin | `net.fabricmc.fabric-loom-remap` 1.16.x + `legacy-looming` 1.16.1 ("Version must be the same as fabric-loom's") [3][4] | `net.fabricmc.fabric-loom-remap` 1.19.4 [5][6] | `net.fabricmc.fabric-loom` 1.19.4 (nicht remappend) [5][6] |
| Mappings | Legacy Yarn `1.8.9+build.604` [7] | Mojang (`loom.officialMojangMappings()`), Laufzeit Intermediary [8] | keine: das Spiel ist unverschleiert, Mojang-Namen sind die echten Namen; `intermediary` ist `0.0.0`, Yarn für 26.3 ist leer, die Versions-JSON hat keine `client_mappings` [2][9][10] |
| API | Legacy Fabric API `1.13.5+1.8.9` (letzte für den Loader `legacy-fabric`, 31.05.2026) [11][12] | Fabric API `0.141.6+1.21.11` [13] | Fabric API `0.166.2+26.3` [13] |
| Java bauen | Gradle-JVM JDK 25, `options.release = 8` (wie im Legacy-Fabric-Beispiel) [3] | JDK 25, `release = 21` [8] | JDK 25, `release = 25` [6] |
| Java Ziel (Laufzeit des Spiels) | 8 (`jre-legacy`) [9] | 21 (`java-runtime-delta`) [9] | 25 (`java-runtime-epsilon`) [9] |
| Gradle | 9.8.2 (Wrapper der offiziellen Beispiel-Mods 1.21.11 und 26.3) [8]; Legacy-Beispiel nutzt 9.4.0, Loom 1.16 verlangt mindestens 9.4 [3][14] | 9.8.2 | 9.8.2 |
| Mod-Abhängigkeit in `fabric.mod.json` | `legacy-fabric-api` | `fabric-api` | `fabric-api` (die Mod-ID `fabric` gibt es ab 26.1 nicht mehr) [15] |

Aktuelle Gradle-Version ist 9.9.2 [16]; wir pinnen 9.8.2, weil die Fabric-Beispiele genau damit gebaut werden. Loom 1.19 ist das aktuelle Release (Tag 23.10.2026, 1.19.4 auf dem Maven), 1.20 ist Alpha [5][6].

**Was "unverschleiert" für uns heißt** [10][15]:
- 26.3 baut mit dem Plugin `net.fabricmc.fabric-loom`; `mappings` entfällt, `modImplementation` wird `implementation`, `remapJar` wird `jar`.
- Mixin-Ziele sind die echten Mojang-Namen. Es gibt keine Refmap für 26.3 und keine Übersetzung zur Laufzeit.
- 1.21.11 baut mit Mojang-Mappings und wird beim Packen nach Intermediary remappt. Loom 1.19 schreibt dabei die Mixin-Ziele direkt in die Annotationen um, eine Refmap gibt es nicht und braucht es nicht (geprüft 2026-11-21: die Jar läuft über den Launcher im Intermediary-Spiel). Weil beide Versionen im Quelltext Mojang-Namen nutzen, ist geteilter Quelltext möglich; die Unterschiede sind echte Umbenennungen von Mojang (Beispiele unten).
- Keine Mod für 1.21.11 oder älter läuft ohne Neukompilierung auf 26.x [15].

**Belegte Umbenennungen 1.21.11 → 26.3** (aus dem Quelltext der Fabric API, Zweige `1.21.11` und `26.3`) [17]:
- HUD: `HudElement.render(GuiGraphics, DeltaTracker)` → `HudElement.extractRenderState(GuiGraphicsExtractor, DeltaTracker)`.
- Zeichnen: `GuiGraphics.drawString(Font, String, int, int, int)` → `GuiGraphicsExtractor.text(Font, String, int, int, int)`; `fill`, `guiWidth()`, `guiHeight()`, `pose().pushMatrix()/scale()/popMatrix()` gibt es in beiden.
- Tasten: `KeyBindingHelper.registerKeyBinding(KeyMapping)` (Paket `client.keybinding.v1`) → `KeyMappingHelper.registerKeyMapping(KeyMapping)` (Paket `client.keymapping.v1`).
- Welt-Rendern: `WorldRenderEvents` (Paket `rendering.v1.world`) → `LevelRenderEvents` (Paket `rendering.v1.level`); `ClientWorldEvents` → `ClientLevelEvents`.
- In-Game-HUD-Klasse: `Gui` (1.21.11) → `Hud` (26.3). In 26.3 ist `Gui` die Bildschirmverwaltung mit `setScreen`; in 1.21.11 liegt `setScreen` auf `Minecraft`.
- `VanillaHudElements.STATUS_EFFECTS` → `MOB_EFFECTS`; `INFO_BAR_FOREGROUND` neu in 26.3.
- Eingabe: 26.3 nutzt SDL3 (`org.lwjgl:lwjgl-sdl:3.4.3` in der Versions-JSON [9], Klasse `com.mojang.blaze3d.platform.SDLEventHandler` [17]); `MouseHandler.onButton(long, MouseButtonInfo, int)` und `KeyboardHandler.keyPress(long, int, KeyEvent)` existieren in beiden Versionen [17].
- In den 26.3-Mixins der Fabric API taucht `VulkanGpuSurfaceMixin` auf [17]: 26.3 hat ein neues Render-Backend. Funktionen mit eigenem Shader oder Framebuffer-Zugriff (Motion Blur, Menü-Blur auf 1.8.9-Art, Glint) sind auf 26.3 die riskantesten.

### 3.1 1.8.9: Legacy Fabric statt Ornithe

**Entscheidung: Legacy Fabric.** Gründe:
1. Der Launcher unterstützt in Phase 3 Fabric, Quilt, Legacy Fabric, Forge und NeoForge, nicht Ornithe (Phase-3-Plan `docs/superpowers/plans/2026-11-20-phase-3-loader.md`). Mit Ornithe könnte der Launcher die Mod auf 1.8.9 nicht starten.
2. Legacy Fabric ist auf der Fabric-Meta-Ebene aktuell: Loader 0.20.1 für 1.8.9 [1], Legacy Looming 1.16.1 [4], Legacy Yarn build.604 [7], Legacy Fabric API 1.13.5+1.8.9 mit HUD-Callback, Tick-Events und Tastenregistrierung [11][18].

**Risiko, das der Mensch kennen muss:** Die Legacy-Fabric-API wird seit 01.11.2026 nur noch für den Loader `ornithe` veröffentlicht (Modrinth: 1.22.0 vom 01.11. bis 1.24.0 vom 20.11.2026 haben `loaders: ["ornithe"]`; die letzte Version für `legacy-fabric` ist 1.13.5 vom 31.05.2026) [12]. Der Standardzweig des API-Repos heißt `port/ornithe-gen-2` und baut auf Ornithe Feather, Calamus-Intermediary gen2 und OSL [18][19]. Legacy Fabric läuft also weiter, wird aber offenbar zu Ornithe hin verschoben. Folgen:
- Die 1.8.9-Variante bleibt auf Legacy Fabric API 1.13.5 eingefroren, bis der Launcher Ornithe kann.
- Der 1.8.9-Adapter ist so dünn, dass ein Wechsel auf Ornithe nur `mod/legacy/` betrifft (andere Klassennamen, z. B. Yarn `MinecraftClient` vs. Feather `Minecraft` [18]), nicht `common`.
- Empfehlung an den Menschen: Ornithe als Loader im Launcher nach Phase 3 einplanen (PLAN.md 3 und 4 führen ihn schon) und vor M3 neu entscheiden.

## 4. Architektur

```
mod/common   reines Java, Ziel Java 8, keine Spiel-Abhängigkeit
             Modul-Registry, Konfiguration und Profile (JSON), HUD-Layout-Modell,
             HUD-Editor-Logik, Einstellungs-UI-Modell, CPS/Combo/Timer/Speed-Logik,
             Server-Sperrlogik. JUnit 5.
mod/modern   Fabric, Stonecutter-Knoten 1.21.11 und 26.3, ein Quellbaum
             dünne Adapter: Rendern, Eingabe, Events, Mixins
mod/legacy   Legacy Fabric 1.8.9, eigener Quellbaum
             dünne Adapter wie oben
```

Die Adapter rufen `common` über eine kleine Fassade auf (`HengstCore`) und stellen ihm Plattform-Schnittstellen bereit (4.3). `common` kennt keine Minecraft-Klasse.

### 4.1 Mehrversions-Werkzeug

**1.21.11 und 26.3 teilen sich einen Quellbaum über Stonecutter 0.10** (Maven `dev.kikugie:stonecutter`, Release 0.10 vom 10.11.2026) [20]. Belegt aus dem Stonecutter-Quelltext (Zweig `0.10`): `stonecutter { create(rootProject) { versions("1.21.11", "26.3"); vcsVersion = "26.3" } }` [21]; Bedingungen per Kommentar (`//? if >=26.1 {`), Ersetzungen (`//~ if >=26.1 'GuiGraphics' -> 'GuiGraphicsExtractor'`, Beispiel steht so im README) [21]. Das Beispiel im Stonecutter-README ist genau unsere Umbenennung, das spricht für den Weg. Ein öffentliches Projekt nutzt Stonecutter bereits mit 26.2, 26.1.2 (unverschleiert) und 1.21.11 (verschleiert) in einem Baum [22].

Die zwei Loom-Varianten kommen aus demselben Artefakt `net.fabricmc:fabric-loom` (beide Plugin-Marker zeigen auf `fabric-loom:1.19.4`) [23]. Das Build-Skript lädt Loom einmal und wendet je Knoten `net.fabricmc.fabric-loom` (26.3) oder `net.fabricmc.fabric-loom-remap` (1.21.11) an. Abhängigkeiten werden mit `add(remap ? "modImplementation" : "implementation", ...)` gesetzt, der Ergebnis-Task ist `remapJar` bzw. `jar`. Ein Zusatz-Plugin (loom-back-compat [22]) ist nicht nötig.

**1.8.9 kann nicht in denselben Baum:** andere Loom-Version (1.16 statt 1.19, Legacy Looming hängt fest an 1.16) [3][4], anderes Java-Ziel (8), andere Namen (Legacy Yarn statt Mojang), andere API. 1.8.9 ist deshalb ein eigener Gradle-Build `mod/legacy`. Gemeinsam ist nur `common`.

**Ungeprüft:** ob zwei Loom-Versionen (1.19.4 und 1.16.x) in einem Composite-Build nebeneinander laufen. Gradle lädt die Skripte jedes eingebundenen Builds in einem eigenen Classloader; der Plan prüft das in Task 1 und fällt sonst auf getrennte Aufrufe (`gradlew -p legacy build`) zurück.

### 4.2 Gradle-Layout unter `mod/`

```
mod/
  settings.gradle            Composite-Wurzel: includeBuild("common"), ("modern"), ("legacy")
  build.gradle               Sammel-Task buildAll (alle drei Jars nach mod/dist/)
  gradle.properties          mod_version, maven_group
  gradlew, gradlew.bat, gradle/wrapper/  Gradle 9.8.2
  common/
    settings.gradle, build.gradle         java-library, release 8, JUnit 5
    src/main/java/io/github/clemensjl/hengst/mod/common/...
    src/main/resources/hengst/server-restrictions.json   (ab M4)
    src/test/java/...
  modern/
    settings.gradle          pluginManagement (Fabric-Maven), Stonecutter 0.10, versions 1.21.11 + 26.3
    stonecutter.gradle       Stonecutter-Controller, aktive Version 26.3
    build.gradle             je Knoten Loom-Variante, Java-Release, Abhängigkeiten
    versions/1.21.11/gradle.properties   minecraft_version, fabric_api_version, java_release=21, loom_remap=true
    versions/26.3/gradle.properties      minecraft_version, fabric_api_version, java_release=25, loom_remap=false
    src/main/java/io/github/clemensjl/hengst/mod/modern/...
    src/main/resources/fabric.mod.json, hengst.mixins.json, assets/hengst/lang/*.json
  legacy/
    settings.gradle, build.gradle, gradle.properties     Loom-remap 1.16.x + legacy-looming 1.16.1
    src/main/java/io/github/clemensjl/hengst/mod/legacy/...
    src/main/resources/fabric.mod.json, hengst.mixins.json, assets/hengst/lang/*.lang
  dist/                      Build-Ergebnis, in .gitignore
```

- Mod-ID `hengst`, Name "Hengst", Paket `io.github.clemensjl.hengst.mod`. Jar-Namen `hengst-mod-<mod_version>+<mc>.jar`.
- `common` wird in jede Mod-Jar per Loom `include` (Jar-in-Jar) eingebettet. Es hat keine Laufzeitabhängigkeiten.
- JSON: `common` bringt einen eigenen kleinen JSON-Leser/-Schreiber mit. Grund: 1.8.9 liefert Gson 2.2.4, 1.21.11 Gson 2.13.2, 26.3 Gson 2.14.0 [9]; eine Gson-API, die in allen dreien gleich ist, wäre eine Wette, ein eigenes Gson in der Jar gibt Klassenkonflikte.
- Übersetzungen: `en_us` und `de_de`. Alle Texte der Mod-Oberfläche liegen als `hengst/lang/<code>.json` in `common` und werden von `common` selbst geladen (Sprache über `Platform.languageCode()`). Auflösung eines Sprachcodes: der Code wird klein geschrieben und muss 2 bis 16 Zeichen aus `a-z`, `0-9` und `_` haben, sonst gilt er als nicht unterstützt (Englisch) und erreicht nie den Klassenlader. Dann die Datei mit genau diesem Code, sonst die erste mitgelieferte Datei derselben Sprache (`de_at` und `de_ch` lesen `de_de`), sonst `en_us`. Fehlt in der gewählten Sprache ein Schlüssel, gilt der englische Text, fehlt er auch dort, wird der Schlüssel selbst gezeigt. Nur die Namen der Tastenbelegungen, die das Spiel im Steuerungsmenü selbst übersetzt, stehen zusätzlich in den Sprachdateien der Adapter (modern `.json`, 1.8.9 `.lang`).

### 4.3 Adapter-Schnittstellen (Java 8, Paket `io.github.clemensjl.hengst.mod.common.platform`)

```java
public enum GameVersion {
    V1_8_9("1.8.9"), V1_21_11("1.21.11"), V26_3("26.3");
    public String id();
}

/** Zeichenfläche für einen Frame. Koordinaten in skalierten GUI-Pixeln. */
public interface Canvas {
    int width();
    int height();
    int fontHeight();
    int textWidth(String text);
    void fill(int x1, int y1, int x2, int y2, int argb);
    void text(String text, int x, int y, int argb, boolean shadow);
    void pushTransform(float translateX, float translateY, float scale);
    void popTransform();
}

/** Lesender Blick auf den Spielzustand. Aufruf nur im Client-Thread. */
public interface GameView {
    boolean inWorld();
    double playerX();
    double playerY();
    double playerZ();
    float playerYaw();
    boolean hudHidden();          // F1
    boolean debugOverlayOpen();   // F3
    boolean screenOpen();         // irgendein Bildschirm offen (Chat, Inventar, Menü)
    String serverAddress();       // null: Einzelspieler, Hauptmenü oder noch keine Welt; sonst die Adresse, wie sie eingetragen wurde
    int pingMillis();             // vom Server gemeldete Latenz in ms; -1 im Einzelspieler, ohne Verbindung oder ohne eigenen Listeneintrag
    Object worldIdentity();       // Weltinstanz, nur per Referenz vergleichen und nicht behalten; null ohne Welt; neu bei Beitritt und Dimensionswechsel
    default long memoryUsedBytes();   // belegter Heap der JVM (Runtime), ohne Allokation
    default long memoryMaxBytes();    // größtmöglicher Heap; Long.MAX_VALUE ohne Grenze
}

public enum GameKey { FORWARD, BACK, LEFT, RIGHT, JUMP, SNEAK, SPRINT, ATTACK, USE }

public interface InputView {
    boolean isDown(GameKey key);
    String keyLabel(GameKey key);   // Anzeigename der aktuellen Belegung, z. B. "W"
}

public enum UiKey { ESCAPE, ENTER, BACKSPACE, DELETE, TAB, LEFT, RIGHT, UP, DOWN, OTHER }

/** Ein Bildschirm, den common zeichnet; der Adapter stellt nur die Hülle. */
public interface UiScreen {
    void render(Canvas canvas, int mouseX, int mouseY, float partialTick);
    boolean mouseClicked(int mouseX, int mouseY, int button);
    boolean mouseReleased(int mouseX, int mouseY, int button);
    boolean mouseDragged(int mouseX, int mouseY, int button);
    boolean mouseScrolled(int mouseX, int mouseY, double amount);
    boolean keyPressed(UiKey key, boolean shift, boolean control);
    void closed();
}

public interface Platform {
    GameVersion version();
    GameView game();
    InputView input();
    java.nio.file.Path configDir();       // <Spielordner>/config
    String languageCode();                // aktuelle Spielsprache, z. B. "de_de" (1.8.9 meldet "de_DE"); null, solange das Spiel sie noch nicht geladen hat
    void logWarning(String message);
    void openScreen(UiScreen screen);     // ersetzt den aktuellen Bildschirm
    void closeScreen();
}

public interface TimeSource {
    long nanoTime();
    TimeSource SYSTEM = System::nanoTime;
}

/** Die Wanduhr hinter Uhr-artigen HUD-Modulen; für Tests ersetzbar. Beide Methoden sind für den Tick, nicht für den Frame. */
public interface WallClock {
    WallClock SYSTEM = ...;
    long currentTimeMillis();            // Millisekunden seit der Epoche, UTC
    int offsetMillis(long utcMillis);    // Versatz der lokalen Zeitzone zu UTC zu diesem Zeitpunkt, Sommerzeit eingeschlossen
}
```

Einstiegspunkte von den Adaptern in `common` (Klasse `io.github.clemensjl.hengst.mod.common.HengstCore`):

```java
public static HengstCore start(Platform platform, TimeSource time);
public void onClientTick();                               // Ende jedes Client-Ticks, 20 Hz
public void onFrame();                                     // genau einmal pro gerendertem Frame, auch unter F1 und bei offenen Bildschirmen (FPS-Zähler)
public void onHudRender(Canvas canvas, float partialTick); // aus dem HUD-Hook der Version; zählt keine Frames
public void onMouseButton(int button, boolean pressed);    // 0 links, 1 rechts, 2 Mitte
public void onOpenSettingsKey();
public void onOpenHudEditorKey();
public void onFeatureKey(int index, boolean down);         // jede Zustandsänderung einer Tastenbelegung aus keySpecs(), nach Index
public List<KeySpec> keySpecs();                           // die Belegungen, die die Adapter registrieren, in Indexfolge
public void shutdown();                                    // speichert offene Änderungen
```

**Tasten.** Jede Belegung ist ein `KeySpec(String id, DefaultKey defaultKey, KeySpec.Mode mode)` mit `Mode` `HOLD` (Funktion bekommt Drücken und Loslassen) oder `PRESS` (nur Drücken); `translationKey()` ist `"key.hengst." + id`, der Name, unter dem das Spiel die Belegung speichert. `DefaultKey` kennt `NONE` (unbelegt) und `RIGHT_SHIFT`; jeder Adapter bildet ihn auf seine Tastencodes ab. `FeatureCatalog.keys(GameVersion)` liefert die Liste: Index 0 `settings` (Standard Rechte Umschalttaste), Index 1 `hud_editor` (unbelegt), danach die Tasten der Funktionen, die diese Version unterstützt, in Nummernfolge (M1a: Index 2 `stopwatch.toggle`, 3 `stopwatch.reset`, beide unbelegt). Die Adapter registrieren genau diese Liste und melden je Tick und Belegung zuerst `onFeatureKey(i, true)`, wenn seit dem letzten Tick mindestens ein Klick gezählt wurde, dann `onFeatureKey(i, false)`, wenn die Taste nicht mehr gedrückt ist. Der Kern führt den Haltezustand: Ein Drücken zählt nur in einer Welt ohne offenen Bildschirm und nur, solange `FeatureState.on(<Nummer der Funktion>)` wahr ist; danach braucht es ein echtes Loslassen vor dem nächsten Drücken (Tastenwiederholung zählt einmal). Ein Loslassen geht nur an `HOLD`-Belegungen, deren Drücken zugestellt wurde, auch nach dem Abschalten der Funktion; öffnet sich ein Bildschirm oder verlässt der Spieler die Welt, gibt der Kern gehaltene `HOLD`-Tasten selbst frei, bei Fokusverlust des Fensters der Adapter. `onOpenSettingsKey()` und `onOpenHudEditorKey()` bleiben bestehen; die Adapter melden die beiden Kerntasten über `onFeatureKey(0, ...)` und `onFeatureKey(1, ...)`, damit dieselben Regeln gelten (also öffnen auch sie nur in einer Welt ohne offenen Bildschirm).

**Funktionszustand für Mixins.** `FeatureState.on(int nummer)` (Paket `...common.feature`) sagt für jede Funktionsnummer aus Abschnitt 7, ob sie jetzt läuft: eingeschaltet, auf dieser Version unterstützt und nicht gesperrt (`FeatureGate`, `RestrictionPolicy.restricted(Feature, String server)`; bis M4 ist die Richtlinie `RestrictionPolicy.NONE`). Unbekannte und negative Nummern liefern `false`. Der Zustand ist ein `volatile`-Feld mit einem Array, das der Kern bei jeder Konfigurationsänderung neu baut und mit einer Zuweisung veröffentlicht; Leser (Mixins auf dem Server-, Netzwerk- oder Mesh-Thread) sehen den alten oder den neuen Zustand, nie einen halben, und das Lesen allokiert nicht. Mixins späterer Funktionen prüfen als Erstes `FeatureState.on(<nummer>)`. Eine Funktion, deren `onConfigChanged` eine Ausnahme wirft, gilt in diesem Durchgang als aus. `Feature` hat dafür `onConfigChanged(ModuleConfig)`, `keys()`, `onKey(int, boolean, long)` und `hintKey()` (Schlüssel eines kurzen Hinweistexts, den die Einstellungen über den Optionen der Funktion zeigen).

**Gemeinsame Helfer der Adapter** (in `common`, Paket `...common` und `...common.platform`): `AdapterGuard` (Fehlerabschottung: ein Fehler beim Start lässt die Mod inaktiv, statt das Spiel zu beenden; ein Fehler in einem Hook wird je Hook-Stelle und Ausnahmeklasse einmal protokolliert, ein `LinkageError` schaltet die Mod ab), `ScrollAccumulator` (sammelt Mausradeinheiten zu ganzen Zeilen) und `FeatureCatalog` (alle Funktionen in Nummernfolge, an einer Stelle).

Spätere Meilensteine ergänzen Einstiegspunkte (`onServerJoin(String)`, `onDisconnect()`, `onChatMessage(...)`, `onAttack(...)`) und Schnittstellen (`WorldView`, `ItemView`), jeweils im Plan des Meilensteins mit genauer Signatur.

### 4.4 Neue Spielversion

Eine neue 26.x-Version wird ein weiterer Stonecutter-Knoten in `modern`. Fällt sie zu weit ab, wird der älteste modern-Knoten fallen gelassen. 1.8.9 bleibt eigener Build. Jede neue Version ist ein eigener Plan mit dem Prüfschritt aus M0 Task 13.

## 5. Kernlogik in `common`

- **Modul-Registry:** jedes Feature ist ein `Feature` mit ID (`fps`, `cps`, ...), Nummer aus Abschnitt 7 und `supports(GameVersion)` aus der Matrix. HUD-Funktionen sind `HudModule` (Unterklasse). Nicht unterstützte Features erscheinen in der UI nicht. `FeatureCatalog` (Paket `...common`) listet alle Funktionen in Nummernfolge an einer einzigen Stelle und liefert die Tastenliste je Version (4.3).
- **Konfiguration:** `<config>/hengst/settings.json` (aktives Profil, globale Schalter) und `<config>/hengst/profiles/<name>.json` (pro Modul: an/aus, Anker, Versatz, Skalierung, Stil, Moduloptionen). Format mit `"schema": 1`. Schreiben atomar (Temp-Datei, dann Umbenennen), höchstens alle 2 s und beim Schließen eines Bildschirms und bei `shutdown`. Kaputte Datei: Sicherung als `<name>.json.broken-<yyyyMMdd-HHmmss>`, dann Standardwerte. Unbekannte Module bleiben beim Speichern erhalten. Grenze zuerst: Ein schon ausgelieferter M0-Build verwirft beim Speichern unbekannte Wurzelschlüssel eines Profils, unbekannte Schlüssel in den Einträgen seiner vier Module und unbekannte Schlüssel in `settings.json`; er behält nur unbekannte Module und unbekannte Schlüssel in `options`. Das lässt sich nicht nachträglich beheben. Außerdem normalisiert er beim Schreiben eines Profils die Einträge seiner vier Module (cps, fps, keystrokes, coordinates), sobald er sie benutzt hat: `scale` wird auf 0,5 bis 3,0 begrenzt, ein ungültiger `anchor` wird zurückgesetzt, `x` und `y` müssen Ganzzahlen im `int`-Bereich sein, falsche Typen werden durch Standardwerte ersetzt, ein `options`, das kein Objekt ist, wird `{}`. Daraus folgt: spätere Versionen dürfen Bedeutung oder Wertebereich dieser bestehenden Felder nicht an Ort und Stelle erweitern; ein größerer Bereich braucht einen neuen Optionsschlüssel. Optionswerte, die `null` sind, verwirft M0 beim Lesen und beim Schreiben, nicht endliche Zahlen nur beim Schreiben; der JSON-Leser weist nicht endliche Zahlen ab, eine Datei mit einer solchen lässt sich nicht laden (und wird dann nie überschrieben). Deshalb schreibt der Mod ab M1a nie `null` oder nicht endliche Werte als Option, ein fehlender Schlüssel bedeutet den Standardwert. Darum gilt für neue persistierte Daten, solange M0-Builds in Gebrauch sind: nur als eigener Moduleintrag oder innerhalb von `options` eines Moduls, nichts Neues an der Wurzel eines Profils, nichts Neues direkt in einem Moduleintrag, nichts Neues in `settings.json`, dessen Verlust ein Spieler bemerken würde. Tastenbelegungen liegen in der `options.txt` des Spiels; das Spiel selbst verwirft nach einem M0-Lauf unbekannte Tastenzeilen (bekannte Einschränkung). Mit dieser Grenze gilt ab M1a: Unbekannte Schlüssel bleiben auf allen drei Ebenen erhalten (Wurzel eines Profils, Moduleintrag, `settings.json`), damit eine neuere Version nichts verliert, was eine noch neuere schrieb; wo ein M0-Build dazwischen lief, gilt nur, was oben als erhalten genannt ist. Die Schemanummer bleibt 1 bei additiven Änderungen (neue Module, neue Optionen; neue Felder nur innerhalb von `options` oder in einem eigenen Moduleintrag) und steigt nur bei inkompatiblen (ein bestehendes Feld ändert Bedeutung oder Namen); Dateien mit höherem Schema werden weiter nie überschrieben. Zahlen in unbekannten Daten laufen durch den eigenen JSON-Leser: Ganzzahlen im Bereich von `long` bleiben exakt, alles andere (Dezimalschreibweise, Exponent, Ganzzahl über `long`) wird zu `double` und kann Stellen über etwa 17 Ziffern verlieren; `1.0` bleibt `1.0`, `1` bleibt `1`, `1e2` wird zu `100.0`.
- **Erster Start und neue Module:** Beim ersten Start sind nur die vier M0-Module (fps, cps, keystrokes, coordinates) an, im Speicher; geschrieben wird erst bei der ersten Änderung. Jedes weitere HUD-Modul ohne Eintrag im geladenen Profil bekommt einen Eintrag an seinem Standardplatz, ausgeschaltet (Speicher, geschrieben beim nächsten Speichern). Eine defekte Datei, die als Sicherung beiseitegelegt wurde, zählt als erster Start; eine unlesbare oder eine mit höherem Schema nicht (dort wird nichts überschrieben und nichts eingeschaltet). Die Optionen eines Moduls stehen als `options` im Moduleintrag; die Zugriffe sind typisiert (Schalter, Ganzzahl mit Bereich, Dezimalzahl mit Bereich und Schrittweite, Auswahl aus festen Werten): ein fehlender Schlüssel, ein falscher Typ, NaN oder ein unbekannter Auswahlwert ergibt den Standardwert; eine Ganzzahl wird gerundet, eine Dezimalzahl auf ihre Schrittweite gerastet, beide auf den Bereich begrenzt.
- **Was die Funktionen von M1a speichern:** nur Moduleinträge und deren `options` (Schlüssel in 7.1). Die Stoppuhr speichert nichts: ihr Zustand (läuft, Zeit) lebt nur für die Sitzung im Speicher. Die Tastenbelegungen liegen in der `options.txt` des Spiels, nicht in `settings.json`.
- **Profile:** Name passt auf `[A-Za-z0-9 _-]{1,32}`; `default` gibt es immer und lässt sich nicht löschen. Neue Profile entstehen als Kopie ("Profil 2", "Profil 3", ...), ohne Texteingabe.
- **HUD-Layout:** Position = Anker (9 Punkte) + Versatz in GUI-Pixeln + Skalierung 0,5 bis 3,0. So bleibt ein Modul bei anderer GUI-Skalierung und Fenstergröße an seiner Ecke. Nach dem Ziehen wählt der Editor den Anker aus dem Bildschirmdrittel, in dem die Modulmitte liegt, und rechnet den Versatz um.
- **HUD-Editor:** Auswahl per Klick (oberstes Modul gewinnt), Ziehen, Einrasten an Bildschirmrand, Bildschirmmitte und Kanten anderer Module (4 px), Mausrad skaliert (±0,1), Pfeiltasten verschieben 1 px (mit Umschalt 10 px), Entf blendet aus, Esc schließt und speichert. Module bleiben immer vollständig im Bild.
- **Einstellungs-UI-Modell:** Liste der Features links mit Schalter, Optionen des gewählten Features rechts (Schalter, Schieber, Auswahl, Knopf), oben Profilwahl, Knopf "HUD bearbeiten". Gezeichnet über `Canvas`, damit alle drei Versionen dieselbe Oberfläche haben.
- **Zähllogik:** CPS über Ringpuffer (`long[64]`) je Maustaste, Fenster 1000 ms; FPS über Frame-Zähler mit Sekundenfenster; Geschwindigkeit, Uhr, Speicher und Stoppuhr seit M1a (7.1); Combo und Reichweite ab ihren Meilensteinen.
- **Server-Sperren:** Abschnitt 9.

## 6. Einstellungs-UI

- Taste (Standard Rechte Umschalttaste) öffnet die Einstellungen, eigene Taste (nicht belegt) öffnet direkt den HUD-Editor. Beide als normale Tastenbelegung in den Vanilla-Steuerungsoptionen, Kategorie "Hengst"; die Tasten öffnen die Bildschirme nur in einer Welt und ohne offenen Bildschirm (4.3). Funktionen mit eigenen Tasten tragen sie in derselben Kategorie (M1a: "Stoppuhr Start/Stopp" und "Stoppuhr zurücksetzen" der Stoppuhr, englisch "Stopwatch start/stop" und "Stopwatch reset", beide unbelegt; die Namen stehen in den Sprachdateien der Adapter).
- Ein Bildschirm, kein Fenster-im-Fenster. Links Features nach Gruppe (HUD, Optik, Komfort), jede Gruppe mit einer Überschrift; ist die Liste höher als der Platz, scrollt sie mit dem Mausrad (eine Zeile je Rastung) und zeigt rechts eine reine Bildlaufanzeige. Suche ab M6 (braucht Texteingabe).
- Rechts die Optionen der gewählten Funktion. Die Stil-Zeilen (Hintergrund, Schatten, Farbe) erscheinen für jedes HUD-Modul, unabhängig von seiner Gruppe (die Stoppuhr steht in der Gruppe Komfort und hat sie trotzdem). Hat eine Funktion einen Hinweis (`Feature.hintKey()`), steht er gedämpft und umbrochen über den Optionen: bei der Stoppuhr "Tasten unter Optionen > Steuerung belegen", weil ihre Tasten unbelegt sind.
- Texte der Oberfläche zeigen sich in der Spielsprache und wechseln spätestens eine Sekunde nach einem Sprachwechsel ohne Neustart; ein schon geöffneter Mod-Bildschirm behält seine Texte, bis er neu geöffnet wird.
- Gesperrte Features (Abschnitt 9) erscheinen ausgegraut mit Grund ("Auf diesem Server gesperrt: <host>").
- Kein eigener Look, der das Spiel imitiert oder ersetzt: dunkle Fläche, Vanilla-Schrift, Akzentfarbe aus den Einstellungen.

## 7. Feature × Version

Legende: **ja** / **nein** / **teils** (mit Grund). Hook-Angaben: **[B]** belegt aus dem Fabric-API-Quelltext des jeweiligen Zweigs [17] bzw. Legacy Yarn build.604 [7] oder Legacy-Fabric-API-Quelltext [18]; **[K]** Kandidat, vor dem Plan des Meilensteins per `genSources` am echten Spiel prüfen. "common" heißt: reine Logik ohne eigenen Hook, Daten über `GameView`/`InputView`, Anzeige über den gemeinsamen HUD-Hook.

Gemeinsame Hooks:
- **HUD:** 1.8.9 Mixin in `InGameHud.render(F)V` am ersten `GlStateManager.enableBlend()` zwischen `ClientWorld.getScoreboard()` und `ChatHud.render(I)V`, also nach Scoreboard-Seitenleiste und vor Chat und Spielerliste; gezeichnet wird mit abgeschaltetem Tiefentest, sonst verdecken Gegenstände in der Hotbar die Module [B, im Spiel geprüft 2026-11-21]. `HudRenderCallback` der Legacy Fabric API 1.13.5 wird nicht genutzt: es läuft erst nach Chat und Spielerliste und würde über beide zeichnen. 1.21.11 `HudElementRegistry.attachElementBefore(VanillaHudElements.CHAT, id, HudElement)` mit `render(GuiGraphics, DeltaTracker)` [B]; 26.3 dasselbe mit `extractRenderState(GuiGraphicsExtractor, DeltaTracker)` [B]. Ein einziges registriertes Element ruft `HengstCore.onHudRender`. Der Kern zeichnet nichts, wenn das HUD ausgeblendet ist (F1), der Spieler nicht in einer Welt ist, das Debug-Overlay (F3) offen ist oder einer der eigenen Bildschirme offen ist.
- **Tick:** `ClientTickEvents.END_CLIENT_TICK` in allen drei APIs [B].
- **Tasten:** 1.8.9 `KeyBindingHelper.registerKeyBinding(KeyBinding)` [B]; 1.21.11 `KeyBindingHelper.registerKeyBinding(KeyMapping)` [B]; 26.3 `KeyMappingHelper.registerKeyMapping(KeyMapping)` [B]. Welche Belegungen es gibt, bestimmt `FeatureCatalog.keys(GameVersion)` (4.3); die Adapter registrieren die Liste in einer Schleife und lesen je Tick `KeyMapping.consumeClick()`/`isDown()` bzw. `KeyBinding.wasPressed()`/`isPressed()`.
- **Frames:** `HengstCore.onFrame()` kommt aus einem Hook, der in jedem gerenderten Frame läuft, auch unter F1 und bei offenen Bildschirmen: 1.8.9 Mixin `MinecraftClient.runGameLoop()V` HEAD; 1.21.11 Mixin `GameRenderer.render(DeltaTracker, boolean)` HEAD; 26.3 Mixin `GameRenderer.render()` HEAD [B, im Spiel geprüft 2026-11-21]. Die HUD-Hooks der Versionen laufen unter F1 nicht, deshalb zählt `onHudRender` keine Frames.
- **Klicks:** 1.8.9 zwei Mixins, weil das Spiel die Mausereignisse in zwei Schleifen abholt: ohne offenen Bildschirm `KeyBinding.setKeyPressed(IZ)V` HEAD (statisch, Legacy-Yarn-Name; `setKeyBindState` ist der MCP-Name; Drücken und Loslassen, Maustasten kommen als `button - 100` an, Tastaturcodes werden verworfen), bei offenem Bildschirm `Screen.handleInput()V` am Aufruf von `handleMouse()`, denn diese Schleife leert die LWJGL-Warteschlange selbst. `KeyBinding.onKeyPressed(I)V` taugt nicht, es feuert nur beim Drücken [B, Semantik im Spiel geprüft 2026-11-21]. Modern Mixin `MouseHandler.onButton(JLnet/minecraft/client/input/MouseButtonInfo;I)V` HEAD; 26.3 liefert SDL-Tastennummern (1 links, 2 Mitte, 3 rechts), 1.21.11 GLFW-Nummern (0, 1, 2), beide über `InputConstants` abgebildet [B, im Spiel geprüft 2026-11-21].
- **Bildschirm öffnen:** 1.8.9 `MinecraftClient.setScreen(Screen)` [B]; 1.21.11 `Minecraft.setScreen(Screen)` [B]; 26.3 `Gui.setScreen(Screen)` [B].
- **Bildschirm zeichnen:** 1.8.9 `Screen.render(IIF)V`; 1.21.11 `Screen.render(GuiGraphics, int, int, float)` und `renderBackground(...)`; 26.3 `Screen.extractRenderState(GuiGraphicsExtractor, int, int, float)` und `extractBackground(...)` [B, geprüft 2026-11-21].
- **Text mit Schatten:** 1.8.9 `TextRenderer.draw(String, float, float, int, boolean)`; 1.21.11 `GuiGraphics.drawString(Font, String, int, int, int, boolean)`; 26.3 `GuiGraphicsExtractor.text(Font, String, int, int, int, boolean)` [B, geprüft 2026-11-21].
- **Tastenkategorie:** 1.21.11 und 26.3 `KeyMapping.Category.register(Identifier)` mit Beschriftung `key.category.hengst.main`; 1.8.9 Kategorie-Schlüssel `key.categories.hengst` im Konstruktor `KeyBinding(String, int, String)` [B, geprüft 2026-11-21].
- **Welt-Rendern:** 1.8.9 Mixin `GameRenderer.renderWorld(IFJ)V` [B]; 1.21.11 `WorldRenderEvents` [B]; 26.3 `LevelRenderEvents` [B].
- **Chat empfangen:** modern `ClientReceiveMessageEvents` [B]; 1.8.9 Mixin `ChatHud.addMessage(Text, int)` [B Name].

| Nr | Feature | 1.8.9 | 1.21.11 | 26.3 | Hook / Hinweis |
|---|---|---|---|---|---|
| 1 | FPS | ja | ja | ja | common: Frame-Zähler über `onFrame()`, Text einmal pro Sekunde |
| 2 | CPS | ja | ja | ja | Klick-Hook (oben); links/rechts getrennt |
| 3 | Keystrokes (WASD, Maus, Leertaste) | ja | ja | ja | `InputView.isDown` pro Frame: 1.8.9 `GameOptions.forwardKey ... attackKey/useKey/jumpKey` + `KeyBinding.isPressed()` [B]; modern `KeyMapping.isDown()` [B, geprüft 2026-11-21] |
| 4 | Rüstung mit Haltbarkeit | ja | ja | ja | Tick liest Rüstungsplätze: 1.8.9 `PlayerInventory.armor` [K]; modern `getItemBySlot(EquipmentSlot)` [K]; Item-Icon: 1.21.11 `GuiGraphics.renderItem` [K], 26.3 [K] |
| 5 | Trankeffekte | ja | ja | ja | 1.8.9 `getStatusEffectInstances()` [K]; modern `getActiveEffects()` [K]; Vanilla-Anzeige ausblenden: `VanillaHudElements.STATUS_EFFECTS` (1.21.11) / `MOB_EFFECTS` (26.3) [B]; 1.8.9 hat keine Vanilla-HUD-Anzeige |
| 6 | Koordinaten | ja | ja | ja | 1.8.9 Felder `Entity.x/y/z` [B]; modern `getX()/getY()/getZ()` [B, geprüft 2026-11-21]; Text im Tick |
| 7 | Richtung/Kompass | ja | ja | ja | `GameView.playerYaw()`: 1.8.9 Feld `ClientPlayerEntity.yaw` (`Entity.yaw`) [B, geprüft 2026-11-21]; modern `LocalPlayer.getYRot()` [B, geprüft 2026-11-21]; Beschreibung in 7.1 |
| 8 | Ping | ja | ja | ja | vom Server gemeldete Latenz des eigenen Listeneintrags (`GameView.pingMillis()`, -1 im Einzelspieler und ohne Eintrag): 1.8.9 `ClientPlayNetworkHandler.getPlayerListEntry(UUID)` und `PlayerListEntry.getLatency()` [B, geprüft 2026-11-21]; modern `ClientPacketListener.getPlayerInfo(UUID)` und `PlayerInfo.getLatency()` [B, geprüft 2026-11-21]; beide sind eine Tabellenabfrage und ein Feldlesen ohne Allokation; nicht an einem echten Server ausgeführt (7.3); ein Wert von 0 gilt als unbekannt und wird als „? ms“ gezeigt, weil ein Server die Latenz des eigenen Eintrags bis zur ersten Messung mit 0 meldet |
| 9 | Uhr | ja | ja | ja | common: `WallClock` (4.3), kein Spiel-Hook; Beschreibung in 7.1 |
| 10 | Server-Adresse | ja | ja | ja | `GameView.serverAddress()`: 1.8.9 `MinecraftClient.getCurrentServerEntry()` (`ServerInfo.address`) [B, geprüft 2026-11-21]; modern `Minecraft.getCurrentServer()` (`ServerData.ip`) [B, geprüft 2026-11-21]; null im Einzelspieler (`isInSingleplayer()` bzw. `hasSingleplayerServer()`) und ohne Welt; nicht an einem echten Server ausgeführt (7.3) |
| 11 | Speicher | ja | ja | ja | common (`GameView.memoryUsedBytes()`/`memoryMaxBytes()` über `Runtime`), gelesen alle 20 Ticks; Beschreibung in 7.1 |
| 12 | Combo-Zähler | ja | ja | ja | Treffer auf Ziel ohne eigenen Schaden: 1.8.9 Mixin `MinecraftClient.doAttack()V` [B]; modern `ClientPreAttackCallback` [B] bzw. `AttackEntityCallback` [K]; Schaden über `hurtTime` [K] |
| 13 | Reichweite | ja | ja | ja | Abstand Auge zu Trefferpunkt beim Angriff (Angriffs-Hook wie 12, `hitResult` [K]); **sperrbar** |
| 14 | Item-Zähler (Pfeile, Tränke, Totems) | teils | ja | ja | Inventar-Scan alle 10 Ticks; Totem gibt es erst ab 1.11, auf 1.8.9 nur Pfeile und Tränke |
| 15 | Geschwindigkeit | ja | ja | ja | common aus der Positionsänderung je Tick (`GameView.playerX/Y/Z()`) und der Weltinstanz (`worldIdentity()`: 1.8.9 `MinecraftClient.world`, modern `Minecraft.level`; neue Instanz bei Dimensionswechsel) [B, geprüft 2026-11-21]; Beschreibung in 7.1 |
| 16 | Resourcepack-Anzeige | ja | ja | ja | 1.8.9 `getResourcePackLoader()` [B]; modern `getResourcePackRepository().getSelectedPacks()` [K] |
| 17 | Scoreboard anpassen | ja | ja | ja | Größe/Lage: 1.8.9 Mixin `InGameHud.renderScoreboardObjective` [B]; modern `HudElementRegistry.replaceElement(VanillaHudElements.SCOREBOARD, ...)` mit Pose-Transform [B]; Zahlen ausblenden: 1.21.11 `Gui.displayScoreboardSidebar` [K], 26.3 `Hud` [K] |
| 18 | Bossbar anpassen | ja | ja | ja | 1.8.9 `InGameHud.renderBossBar()V` [B]; modern `replaceElement(BOSS_BAR)` [B] |
| 19 | Tabliste mit Ping-Zahlen | ja | ja | ja | 1.8.9 `PlayerListHud` Ping-Icon [K]; modern `PlayerTabOverlay.renderPingIcon` [K] |
| 20 | Sättigung/Hunger-Details | ja | ja | ja | Sättigung schickt der Server in allen Versionen mit; 1.8.9 `HungerManager.getSaturationLevel()` [K]; modern `FoodData.getSaturationLevel()` [K]; Anzeige `attachElementAfter(FOOD_BAR)` [B] |
| 21 | Stoppuhr/Timer | ja | ja | ja | common; zwei Tasten über `KeySpec` (4.3), beide unbelegt; Gruppe Komfort; Beschreibung in 7.1 |
| 22 | Cooldowns (Enderperle, Items) | nein | ja | ja | Item-Cooldowns gibt es erst ab 1.9; modern `getCooldowns()` [K] |
| 23 | Zoom | ja | ja | ja | 1.8.9 `GameRenderer.getFov(FZ)F` RETURN [B]; modern `GameRenderer` FOV [K]; Mausempfindlichkeit beim Zoom: 1.8.9 [K], modern `MouseHandler.turnPlayer` [K] |
| 24 | Freelook | ja | ja | ja | 1.8.9 `GameRenderer.transformCamera(F)V` [B] + Blickrichtung vom Spieler entkoppeln [K]; modern `Camera.setup` [K] + `Entity.turn(DD)V` [K]; **sperrbar** |
| 25 | Fullbright | ja | ja | ja | 1.8.9 `GameRenderer.updateLightmap(F)V` [B]; modern Lightmap [K] |
| 26 | Fadenkreuz-Editor | ja | ja | ja | 1.8.9 `InGameHud.showCrosshair()Z` [B] → eigenes Zeichnen; modern `replaceElement(CROSSHAIR)` [B] |
| 27 | Trefferfarbe | ja | ja | ja | Overlay-Farbe beim Treffer: 1.8.9 `LivingEntityRenderer` [K]; modern `LivingEntityRenderer.getOverlayCoords` [K] |
| 28 | Motion Blur | ja | ja | teils | 1.8.9 Akkumulations-Framebuffer am Ende von `GameRenderer.render` [K]; 1.21.11 Post-Effekt [K]; 26.3 hohes Risiko wegen neuem Render-Backend [B Hinweis], Entscheidung im M7-Plan |
| 29 | Block-Umrandung/Overlay | ja | ja | ja | 1.8.9 `WorldRenderer.drawBlockOutline` [K]; 1.21.11 `LevelRenderer.renderBlockOutline` [B]; 26.3 `LevelRenderer.submitBlockOutline` [B] |
| 30 | Alte Animationen (1.7) | ja | nein | nein | nur 1.8.9: `HeldItemRenderer.applySwordBlockTransformation`, `applyEquipAndSwingOffset` [B]; ab 1.9 fehlt Schwert-Blocken im Spiel, ein Nachbau wäre eine Fälschung des Zustands |
| 31 | Item-Physik | ja | ja | ja | `ItemEntityRenderer` [K] in allen Versionen |
| 32 | Partikel-Steuerung | ja | ja | ja | 1.8.9 `ParticleManager.addParticle` [K]; modern `ParticleEngine` (Klasse [B]) Methode [K] |
| 33 | Zeit ändern (nur Client) | ja | ja | ja | 1.8.9 `World.getTimeOfDay` [K]; modern `ClientLevel` Tageszeit [K] |
| 34 | Wetter ändern (nur Client) | ja | ja | ja | 1.8.9 `World.getRainGradient(F)F` [K]; modern `Level.getRainLevel(F)F` [K]; 26.3 `WeatherRenderState` (Klasse [B]) |
| 35 | Nametag-Optionen (eigener Name in F5) | ja | ja | ja | 1.8.9 `LivingEntityRenderer.hasLabel` [K]; modern `LivingEntityRenderer.shouldShowName` [K] |
| 36 | Hitbox-Optionen | ja | ja | ja | Vanilla-Hitboxen (F3+B) einfärben/filtern: 1.8.9 `EntityRenderDispatcher.renderHitbox` [K]; modern `EntityRenderDispatcher` (Klasse [B]) [K]; **sperrbar** |
| 37 | TNT-Countdown | ja | ja | ja | Zündzeit ist Vanilla-synchronisiert; 1.8.9 `TntEntityRenderer` [K]; modern `TntRenderer` + `PrimedTnt.getFuse()` [K] |
| 38 | Feuer, Schild, Kürbis niedriger/aus | teils | ja | ja | Schild gibt es erst ab 1.9; 1.8.9 Feuer `HeldItemRenderer.renderFireOverlay` [B], Kürbis `InGameHud.renderPumpkinBlur` [B]; 1.21.11 Feuer `ScreenEffectRenderer.renderScreenEffect` [B], Kürbis über `Gui.renderCameraOverlays` [B], Schild `ItemInHandRenderer` [K]; 26.3 alle [K] |
| 39 | Statisches FOV | ja | ja | ja | 1.8.9 `GameRenderer.updateMovementFovMultiplier` [B]; modern `AbstractClientPlayer.getFieldOfViewModifier` [K] |
| 40 | Glint-Farbe | ja | ja | teils | 1.8.9 `ItemRenderer` Glint [K]; 1.21.11 Glint-RenderType [K]; 26.3 Risiko wie 28 |
| 41 | Toggle Sprint/Sneak | ja | ja | ja | 1.8.9 eigene Umsetzung über `KeyBinding.setKeyPressed(IZ)V` im Tick [B]; modern nutzt die Vanilla-Option "Umschalten" für Sprint/Schleichen und ergänzt HUD-Anzeige und "immer sprinten" über `KeyMapping.setDown(Z)V` [K] |
| 42 | Chat (Zeitstempel, Stapeln, Suche, lange Historie) | ja | ja | ja | 1.8.9 `ChatHud.addMessage(Text, int)` [B]; modern `ChatComponent.addMessage` [K] (Klasse [B]); Historienlänge: Konstante 100 per `@ModifyConstant` [K]; Suche als Leiste über `ChatScreen` (modern `ScreenEvents` [B]) |
| 43 | Screenshot-Manager | ja | ja | ja | 1.8.9 `ScreenshotUtils.saveScreenshot` [K]; modern `Screenshot.grab` [K]; Galerie als `UiScreen` in common, Vorschaubilder als Textur je Version [K] |
| 44 | Waypoints | ja | ja | ja | Welt-Render-Hook (oben); Ablage je Server/Welt; nur selbst gesetzte Punkte, keine automatische Todesposition anderer; **sperrbar** |
| 45 | Minimap | ja | ja | ja | Farben aus geladenen Chunks (`MapColor` [K]), dynamische Textur [K]; keine Entities, kein Höhlenmodus; **sperrbar** |
| 46 | AutoGG / Auto-Text-Tasten | ja | ja | ja | Chat-Hook (oben) + Senden: 1.8.9 `ClientPlayerEntity.sendChatMessage(String)` [K]; modern `ClientPacketListener.sendChat(String)` [K]; Auslöser je Server aus Datei, höchstens 1 Nachricht pro 3 s |
| 47 | Nick-Hider | ja | ja | ja | eigener Name in Chat, Tab, Nametag ersetzen; modern über Chat-Event und `PlayerInfo` [K]; 1.8.9 [K]; Kosten im M6-Plan messen |
| 48 | Menü-Blur und GUI-Skalierung | ja | teils | teils | modern haben Menü-Unschärfe und GUI-Skalierung schon als Vanilla-Option: wir bieten Schnellzugriff [K]; 1.8.9 eigener Blur bei `Screen.renderBackground` [B Name] und Skalierung [K] |
| 49 | Chunk-Grenzen | ja | ja | ja | Vanilla F3+G gibt es ab 1.8; wir schalten es per eigener Taste und zeichnen schöner: 1.8.9 [K]; 1.21.11 `DebugRenderer` [K]; 26.3 `ClientDebugSubscriber`/`DebugRenderer` (Klassen [B]) [K] |
| 50 | HUD-Editor (Ziehen, Profile) | ja | ja | ja | common + Bildschirm-Hülle je Version |

Belegte Bildschirm-Hüllen: 1.8.9 `Screen.render(IIF)V`, `mouseClicked(III)V`, `mouseReleased(III)V`, `mouseDragged(IIIJ)V`, `keyPressed(CI)V`, `handleMouse()V` (Mausrad) [B]. 1.21.11 `mouseClicked(MouseButtonEvent, boolean)`, `mouseReleased(MouseButtonEvent)`, `mouseDragged(MouseButtonEvent, double, double)`, `mouseScrolled(double, double, double, double)` [B]; Render-Methode [K]. 26.3 Render über `extractRenderState(GuiGraphicsExtractor, int, int, float)` [K, abgeleitet aus `Screen.extractRenderStateWithTooltipAndSubtitles(GuiGraphicsExtractor;IIF)V` B].

### 7.1 Die sieben Funktionen von M1a im Einzelnen

Alle sieben sind einzeilige Textmodule auf dem HUD, auf 1.8.9, 1.21.11 und 26.3, und beim ersten Start aus (nur die vier M0-Module sind dann an). Die Optionen stehen unter `options` im Moduleintrag. Der Standardplatz gilt für ein Profil ohne Eintrag des Moduls: Anker, Versatz x, Versatz y in GUI-Pixeln. Die rechte Spalte beginnt bei y 52, damit sie unter den Effekt-Symbolen der modernen Versionen und unter Toasts liegt.

| Nr | ID | Gruppe | Optionen (Schlüssel: Art, Standard, Bereich) | Standardplatz |
|---|---|---|---|---|
| 7 | `direction` | HUD | `style`: Auswahl `cardinal` / `cardinal_degrees` / `degrees`, Standard `cardinal_degrees` | TOP_LEFT, 2, 44 |
| 8 | `ping` | HUD | keine | TOP_RIGHT, -2, 52 |
| 9 | `clock` | HUD | `format`: Auswahl `24h` / `12h`, Standard `24h`; `seconds`: Schalter, aus | TOP_RIGHT, -2, 66 |
| 10 | `server_address` | HUD | `show_port`: Schalter, aus | TOP_RIGHT, -2, 80 |
| 11 | `memory` | HUD | `style`: Auswahl `percent` / `used_max` / `both`, Standard `both` | TOP_RIGHT, -2, 94 |
| 15 | `speed` | HUD | `include_vertical`: Schalter, aus; `decimals`: Ganzzahl 0 bis 2, Standard 1 | TOP_LEFT, 2, 58 |
| 21 | `stopwatch` | Komfort | `tenths`: Schalter, an | TOP_LEFT, 2, 72 |

Die vier M0-Module behalten ihre Plätze (fps TOP_LEFT 2, 2; cps TOP_LEFT 2, 16; coordinates TOP_LEFT 2, 30; keystrokes BOTTOM_LEFT 2, -2). Ein Test prüft, dass sich die Standardplätze aller elf Module bei 320 x 240 Pixeln und den längsten Texten nicht überdecken.

- **7 Richtung.** Zeigt die Blickrichtung als Kompasskurs wie die Zeile "Facing" der F3-Anzeige: Yaw 0 blickt nach Süden, 180 nach Norden. Kurs `h = ((yaw + 180) mod 360 + 360) mod 360`, gezeigt als `round(h) mod 360` Grad (eine halbe Stufe rundet auf) und als Buchstaben der Achtelrichtung. `style`: `cardinal` zeigt nur Buchstaben ("N"), `cardinal_degrees` beides ("N 0°"), `degrees` nur Grad ("0°"). Buchstaben englisch N, NE, E, SE, S, SW, W, NW, deutsch N, NO, O, SO, S, SW, W, NW. Leer außerhalb einer Welt, "?" bei einem Yaw, der keine endliche Zahl ist. Die 360 Texte des gewählten Stils und der Sprache entstehen einmal beim Wechsel von Stil oder Sprache; ein Tick wählt nur einen aus.
- **8 Ping.** Zeigt die vom Server gemeldete Latenz des eigenen Spielers, "42 ms", dieselbe Zahl wie die Spielerliste (Tab). Sichtbar nur in einer Welt mit Serveradresse, sonst leer und ohne Größe. Der Wert wird beim Sichtbarwerden und danach alle 20 Ticks gelesen. Ein Wert von 0 oder weniger (nicht gemeldet, vom Server verborgen, noch nicht gemessen) erscheint als "? ms", ein Wert über 9999 als "9999 ms". Keine Farbe nach Schwelle, die Zahl ist der einzige Träger.
- **9 Uhr.** Zeigt die lokale Systemzeit: "09:05", mit `seconds` "09:05:07"; mit `format` `12h` "9:05 AM" (Stunde ohne führende Null, AM/PM aus den Sprachdateien). Die Zeit kommt nur aus der `WallClock`, nicht aus dem Spiel. Ein Tick liest `currentTimeMillis()` und vergleicht; der Versatz zur UTC wird erst an der nächsten Minuten- bzw. Sekundengrenze neu gefragt. Eine zurückgestellte Systemuhr zeigt die Anzeige deshalb im nächsten Tick, eine Sommerzeitumstellung an der nächsten Grenze; Mitternacht rollt die Anzeige um.
- **10 Server-Adresse.** Zeigt die Adresse des verbundenen Servers, wie sie eingetragen wurde (Groß-/Kleinschreibung bleibt). Ohne `show_port` entfällt ein Port: aus "[v6]:port" wird "[v6]", eine Adresse mit genau einem ":" verliert alles ab dem Doppelpunkt, alles andere (etwa ein nackter IPv6-Literal) bleibt. Das Paragraphenzeichen (Formatcode der Spielschrift) wird entfernt, damit eine eingetragene Adresse den HUD-Text nicht umfärben kann. Ist der gezeigte Text länger als 40 Zeichen, bleiben die ersten 19, "..", die letzten 19 (nie innerhalb eines Surrogatpaars), also höchstens 40 Zeichen. Sichtbar nur in einer Welt mit Adresse, sonst leer. Die Adresse wird nur angezeigt: nie aufgelöst, nie gesendet, nie protokolliert, nie gespeichert. Es gibt keine Option zum Ausblenden der Adresse; das Modul ist beim ersten Start aus.
- **11 Speicher.** Zeigt den belegten Heap der JVM, wie die F3-Anzeige: Megabyte (Bytes um 20 Bit nach rechts, abgerundet) und Prozent des Maximums (abgerundet). `style`: `percent` "RAM 41%", `used_max` "RAM 1234/3000 MB", `both` "RAM 41% 1234/3000 MB". Ist das Maximum nicht positiv oder so groß, dass `belegt * 100` überlaufen könnte (über `Long.MAX_VALUE / 100`, das schließt "ohne Grenze" ein), steht nur "RAM <belegt> MB". Gelesen beim ersten Tick und dann alle 20 Ticks; nach jedem Anwenden der Konfiguration wird ab dem nächsten Tick neu gelesen, ein neuer Stil gilt also sofort.
- **15 Geschwindigkeit.** Zeigt Blöcke pro Sekunde, "5.6 m/s": der Abstand der Positionen zweier Ticks, gemittelt über die letzten bis zu 10 Ticks, mal 20. Horizontal, mit `include_vertical` einschließlich der Höhe. `decimals` 0 bis 2 Nachkommastellen (gerundet, halbe Stufe auf). Der Mittelwert beginnt neu (die Anzeige zeigt dann 0) nach einem Sprung von mehr als 5 Blöcken in einem Tick, bei einer neuen Weltinstanz (Dimensionswechsel, neue Welt), bei einer nicht endlichen Position und bei jedem Anwenden der Konfiguration. Leer außerhalb einer Welt. Der größte gezeigte Wert ist 100 m/s.
- **21 Stoppuhr.** Zeigt eine Stoppuhr, "0:00.0", "1:05.3", ab einer Stunde "1:00:00.0" (Stunden nicht begrenzt, Zehntel abgerundet); ohne `tenths` entfällt ".t". Zwei Tasten in der Kategorie "Hengst", beide unbelegt: `stopwatch.toggle` startet und stoppt (ein Stopp behält die Zeit, der nächste Start macht dort weiter), `stopwatch.reset` stoppt und setzt auf null. Beides sind Drücken-Tasten: Loslassen tut nichts; der Kern stellt sie nur zu, solange die Funktion an ist, in einer Welt und ohne offenen Bildschirm. Die Zeit kommt aus der monotonen `TimeSource`, nie aus der Wanduhr; ein Verstellen der Systemuhr ändert sie nicht. Es ist Echtzeit: das Pausenmenü im Einzelspieler und das Verlassen oder Wechseln einer Welt halten eine laufende Stoppuhr nicht an. Der Zustand wird nicht gespeichert und bleibt beim Profilwechsel und beim Schließen der Einstellungen erhalten, außer wenn im neuen Profil die Stoppuhr ausgeschaltet ist: dann hält sie an und behält ihren Wert. Nur das Ausschalten der Funktion hält eine laufende Stoppuhr an (die Zeit bleibt, der Stopp gilt zum letzten Tick, höchstens 50 ms vor dem Ausschalten).

Gemeinsames Verhalten der Textmodule: Ein Text entsteht nur, wenn sich ein gezeigter Wert geändert hat (ein String je Änderung, im eingeschwungenen Zustand nichts). Ist der Text breiter, als der Bildschirm bietet (Breite des Bildschirms geteilt durch die Skalierung des Moduls, minus Polster), wird er beim Zeichnen in der Mitte mit ".." gekürzt (`TextFit.middle`); das Ergebnis ist zwischengespeichert nach Textreferenz und verfügbarer Breite und kostet im Frame nichts. Bei der Server-Adresse greift zuerst die Kürzung auf 40 Zeichen. Nach einem Sprachwechsel verwerfen die Module ihre zwischengespeicherten Textbreiten.

### 7.2 Entscheidungen von M1a, die den Plan geändert haben

- **Ping 0 gilt als unbekannt.** Der Plan zeigte 0 als "0 ms". Ein Server meldet die Latenz des eigenen Eintrags bis zur ersten Aktualisierung mit 0 (nach dem Beitritt bis zu etwa 30 Sekunden); 0 ist also keine Messung. Eine echte 0 ms gibt es praktisch nur auf localhost. Deshalb zeigt das Modul "? ms".
- **Geschwindigkeit: Sprungschwelle 5 Blöcke je Tick, größter Wert 100 m/s.** Der Plan sah 10 Blöcke je Tick vor. Damit liefen Chorusfrucht, Enderperle und ein Respawn in der Nähe als bis zu 160 m/s durch. Die größte rechtmäßige Bewegung ist die Endgeschwindigkeit eines Falls mit 3,92 Blöcken je Tick; die Schwelle liegt darüber, und ein Mittelwert über Schritte von höchstens 5 Blöcken kann 100 m/s nicht überschreiten.
- **Stoppuhr-Tasten sind unbelegt, die Einstellungen weisen darauf hin.** Eine Standardtaste kann auf einer der drei Versionen mit einer Vanilla-Belegung kollidieren (L ist auf 1.21.11 und 26.3 "Fortschritte"). Damit die Tasten auffindbar sind, zeigt die Optionsliste der Stoppuhr den Hinweis "Tasten unter Optionen > Steuerung belegen" (`Feature.hintKey()`).
- **Beide Stoppuhr-Tasten auf derselben physischen Taste:** auf 1.21.11 und 26.3 feuern beide Belegungen im selben Tick in Katalogreihenfolge (Start/Stopp, dann Zurücksetzen), die Stoppuhr endet also gestoppt auf null. Auf 1.8.9 siehe 7.3.
- **Die Stoppuhr steht in der Gruppe Komfort, und die Stil-Zeilen erscheinen für jedes HUD-Modul.** Der Plan nannte die Gruppe Komfort schon, die Einstellungen zeigten Hintergrund, Schatten und Farbe aber nur für die Gruppe HUD; die Stoppuhr zeichnet mit diesem Stil, also gilt die Bedingung jetzt "Gruppe HUD oder HUD-Modul". Offen beim Projektinhaber, noch nicht bestätigt: die Gruppe Komfort der Stoppuhr und die deutschen Kompassbuchstaben N/NO/O/SO.
- **Textmodule passen ihren Text an die Bildschirmbreite an.** Der Renderer hatte keine Breitenbegrenzung: eine 40 Zeichen lange Adresse mit breiten Zeichen oder eine Skalierung ab 1,5 hätte einen 320 Pixel breiten Bildschirm verlassen. Jetzt kürzt `TextHudModule` zu breiten Text in der Mitte (7.1).
- **Server-Adresse: auf 40 Zeichen gekürzt (19, "..", 19) vor dem Anpassen an die Breite, Formatzeichen entfernt, nie aufgelöst, gesendet oder protokolliert, keine Option zum Ausblenden** (das Modul ist beim ersten Start aus).
- **Sprachauflösung mit Familienrückfall.** Der Plan fiel bei einem unbekannten Code gleich auf Englisch zurück; ein Spieler mit `de_at` sähe Englisch. Jetzt: genauer Code, dann eine mitgelieferte Datei derselben Sprache (`de_at` zu `de_de`), dann `en_us` (4.2). Das Ergebnis wird je Code zwischengespeichert (höchstens 64 Codes; darüber wird nicht mehr zwischengespeichert), jede Datei höchstens einmal gelesen.
- **Das Modulbudget misst Grenzkosten** (Abschnitt 8.8): geändert gegenüber der ersten Fassung des Plans; ebenso wird das Budget der vier M0-Module am Median der Läufe beurteilt. Beides sind Regeln des Koordinators vom 22.11.2026, vorbehaltlich Bestätigung durch den Projektinhaber.
- **Ein fehlgeschlagener Schritt der Konfiguration verhindert die übrigen nicht.** `HengstCore.applyConfig` baut das HUD, meldet jeder unterstützten Funktion ihre Konfiguration und veröffentlicht den Funktionszustand in drei getrennt abgesicherten Schritten, in dieser Reihenfolge (der Plan nannte Aufbau, Veröffentlichung, Meldung). Die Meldung steht vor der Veröffentlichung, damit die Felder einer Funktion gesetzt sind, bevor Hooks anderer Threads den neuen Zustand sehen. Eine Funktion, deren Meldung eine Ausnahme wirft, ist in diesem Durchgang im Funktionszustand aus; schlägt der Aufbau des HUD fehl, bleibt das vorherige HUD stehen.

### 7.3 Bekannte Grenzen von M1a

Diese Punkte sind bekannt und nicht behoben; wo es heißt "nicht geprüft", ist es nicht geprüft.

**Nicht am echten Server geprüft.**
- Ping und Server-Adresse wurden in den automatischen Spieltests nicht gegen einen echten Server geprüft: im Einzelspieler zeigen beide nach Entwurf nichts, und ein Mehrspielerlauf war nicht Teil der Tests (der mitgelieferte Testserver der modernen Versionen verlangt `eula=true` im Testbuild, also die Zustimmung zur Mojang-EULA; diese Entscheidung ist beim Projektinhaber offen). Ungeprüft sind damit ein nicht leerer Wert, "? ms" beim Beitritt, die Formen der Adresse und der Serverwechsel. Ihre Kosten je Frame auf einem Server sind nicht gemessen (die Messung in 8.8 überspringt sie, sie zählen dort 0).

**Geschwindigkeit.**
- Eine vom Server gesetzte Fluggeschwindigkeit im Kreativmodus über 100 m/s (Befehl `/flyspeed` eines Administrators) liest sich als Sprung und zeigt 0.
- Bei zwei Nachkommastellen kann ein Wert auf einer Rundungsgrenze zwischen zwei Anzeigen wechseln (Schleichen, etwa 1,295 m/s, flackert zwischen 1.29 und 1.30).
- Nach einem Wechsel des Profils oder der Einstellungen zeigt die Geschwindigkeit einen Tick lang 0, Ping und Server-Adresse sind einen Tick lang leer (jedes Anwenden der Konfiguration setzt ihren Zustand zurück).

**Tasten.**
- Auf 1.8.9 empfängt von zwei Belegungen auf derselben physischen Taste nur eine die Eingabe; das ist Verhalten des Spiels.
- Die Tastenbelegungen speichert das Spiel in seiner `options.txt`. Nach einem Start mit dem ausgelieferten M0-Build verwirft das Spiel die Zeilen von Belegungen, die es nicht kennt (`key_key.hengst.stopwatch.*`); die Stoppuhr-Tasten sind dann neu zu belegen.

**Konfiguration.** Was ein M0-Build an einer M1a-Datei verwirft, steht in Abschnitt 5.

**Sprache und Schrift.**
- Ändert ein Ressourcenpaket nur die Schrift, ohne dass sich die Sprache ändert, bleiben zwischengespeicherte Textbreiten bis zum nächsten Text- oder Sprachwechsel veraltet; dafür gibt es auf keiner Version ein Signal.
- Die Sprache wird auf den modernen Versionen erst im ersten Tick gelesen: bei der Initialisierung der Mod ist der Sprachverwalter noch nicht bereit (`languageCode()` gibt dann `null`). Bis dahin gilt Englisch.

**Uhr.**
- Die Uhr liest die Zeitzone des Systems einmal, wenn die Mod lädt. Ändert sich die Zeitzone des Betriebssystems während einer Sitzung, folgt die Uhr nicht; Sommerzeitwechsel folgt sie, weil der Versatz für den jeweiligen Augenblick abgefragt wird.

**Server.**
- Ein Proxy-Netz, das hinter einer Adresse den Backend-Server wechselt, wird nicht als Serverwechsel erkannt.
- Die Serveradresse wird so verglichen, wie sie getippt wurde, Groß- und Kleinschreibung unterschieden. Eine spätere Sperr-Richtlinie muss sie selbst normalisieren (Abschnitt 9.2).
- Eine Funktion, die eine Server-Sperre abschaltet, obwohl sie in der Konfiguration eingeschaltet ist, bekommt heute keine Benachrichtigung "abgeschaltet" (`onConfigChanged` meldet nur die Konfiguration, nicht den Zustand): eine laufende Stoppuhr liefe unsichtbar weiter. Das ist mit den echten Sperrregeln in M4 zu definieren.

**Messung.**
- Auf 1.8.9 lief der Messrechner bei der Wiederholung auf dem untätigen Rechner zweimal in einem Zustand, in dem jeder Fall etwa 2,3-mal so lange dauerte, bei unveränderten 120 Bildern pro Sekunde und ohne sichtbaren anderen Prozess (Läufe vom 22. und 23.11.2026: Lauf 1 ab dem Fall „alle elf“, Läufe 5 und 6 durchgehend; drei von sechs Läufen betroffen, 8.8). Die Ursache ist nicht geklärt; die Werte zählen in der Beurteilung mit.
- Ein einzelner Aufruf der Tastenverteilung allokiert einmal je Spielstart 1192 Byte, etwa 67 bis 80 s nach dem Start; die Ursache ist nicht geklärt (8.8). Die Allokationsprüfung im Spieltest beginnt deshalb ihr geprüftes Fenster frühestens 90 s nach dem Start.

**Rückstand (bewusst angenommen).**
- `TextFit.middle` baut temporäre Strings, wenn sich ein zu breiter Text ändert (etwa 2 KB je Änderung bei einem langen Koordinatentext mit Skalierung 2). Das ist angenommen; eine Verbesserung wäre eine Messung mit Zeichenpositionen über die drei Adapter und steht im Rückstand.
- Feste Breite im ungünstigsten Fall für Module, deren Textbreite wechselt (Richtung): Rückstand; bis dahin ändert sich die Breite des Kastens mit dem Text.
- GL-Zustand auf 1.8.9 im HUD-Durchgang nur beim Wechsel zwischen Füllen und Text setzen (M1a-Plan, Task 17): nicht nötig, weil die vier M0-Module auf dem untätigen Rechner im Median der Läufe 19,2 µs kosten (Budget 50, 8.8); Kandidat, falls spätere Module das Budget auf 1.8.9 erreichen.

## 8. Laufzeitregeln

1. **Keine Allokation pro Frame im HUD-Pfad.** Texte entstehen im Tick (20 Hz) und nur, wenn sich der Wert geändert hat; `render` zeichnet zwischengespeicherte Strings. Positionen werden in einen wiederverwendeten `MutableRect` geschrieben. JUnit-Tests messen die Allokation mit `com.sun.management.ThreadMXBean.getThreadAllocatedBytes` und verlangen im eingeschwungenen Zustand, im Frame- und im Tick-Pfad, genau 0 Byte. Sie sind mit `@Tag("interpreted")` markiert und laufen unter `-Xint` (Gradle-Task `interpretedTest`, startet nach `test`), weil unter dem JIT eine Deoptimierung als einmalige Allokation des Messthreads erscheint. Ein geänderter Wert darf einen String kosten; der Test eines Moduls prüft dafür eine Schranke, die einen zweiten String nicht zulässt.
2. **Aktive Module als Array.** `HudRenderer` hält `HudModule[]` der eingeschalteten, unterstützten Module, neu gebaut nur bei Konfigurationsänderung. Keine Iteratoren, keine Streams, keine Lambdas mit Capture im Frame-Pfad.
3. **Abgeschaltet kostet nichts.** Abgeschaltete Module stehen nicht im Array. Mixins, die nicht entfernt werden können, prüfen als Erstes ein `static boolean` und kehren zurück; Mixins von Funktionen prüfen ab M1a `FeatureState.on(<nummer>)` (4.3).
4. **Mixins schmal.** `@Inject` an HEAD/RETURN oder `@WrapOperation`/`@ModifyExpressionValue` (MixinExtras ist im Fabric Loader seit 0.15.0 enthalten, https://github.com/FabricMC/fabric-loader/releases/tag/0.15.0; wir nutzen Loader 0.20.1 auf allen drei) statt `@Overwrite` und statt `@Redirect`. Kein Mixin, wo eine Fabric-API-Schnittstelle reicht. Jeder Mixin trägt `require = 1`, damit ein verschobenes Ziel beim Start auffällt statt still zu fehlen.
5. **Keine Reflection** außerhalb des Starts. Zugriffe auf private Felder über Accessor-Mixins.
6. **Kein Dateizugriff im Frame oder Tick.** Speichern gebündelt (Abschnitt 5). Einzige Ausnahme: Beim Wechsel der Spielsprache liest der Tick einmal die Sprachdatei aus dem Klassenpfad (`I18n.load`), höchstens einmal je Wechsel und nie im Frame-Pfad. Der Kern prüft die Sprache höchstens einmal je Sekunde und lädt nur, wenn sie sich geändert hat; jede mitgelieferte Datei wird höchstens einmal gelesen, das Ergebnis je Code gemerkt. Schlägt das Laden fehl, bleiben die alten Texte; nach drei Versuchen wird es bis zum nächsten Codewechsel aufgegeben (eine Logzeile).
7. **Keine eigenen Threads** in M0. Spätere Ausnahmen (Minimap-Kacheln, Screenshot-Vorschau) brauchen eine Begründung im Meilenstein-Plan.
8. **Kosten sichtbar.** Bei jedem Meilenstein wird im echten Spiel gemessen, was die Module pro gerendertem Frame kosten. Die Regel ist ein absolutes Budget. Ein abgeschaltetes Modul kostet nichts Messbares.

   *Gemessene Größe (seit M1a).* Die Messung je Modul im Spieltest erfasst die Zeit, die der Kern je Frame im Modul verbringt (Größe, Platzierung, Transformation und Zeichnen): auf 1.21.11 und 26.3 bis zur Abgabe der Zeichenaufträge, weil das Spiel später gesammelt zeichnet (Abgabezeit), auf 1.8.9 einschließlich der Aufrufe der festen Pipeline, weil dort sofort gezeichnet wird. Die beiden Werte sind nicht dieselbe Größe und werden nicht gegeneinander verglichen. Beide sind nicht der Bildratenvergleich der M0-Abschlussprüfung (26.3 etwa 26, 1.8.9 etwa 34 Mikrosekunden), der die Kosten des späteren Zeichnens mitzählt und je Meilenstein einmal wiederholt wird.

   *Budgets der gemessenen Größe (je Version).* Die vier M0-Module zusammen höchstens 50 µs, beurteilt am Median der Läufe-Mediane aus mindestens drei Läufen; jeder Lauf wird aufgeführt (Regel des Koordinators vom 22.11.2026, vorbehaltlich Bestätigung durch den Projektinhaber). Jedes Textmodul ab M1a höchstens 10 µs Grenzkosten (Median). Alle elf Module von M0 und M1a zusammen höchstens 120 µs (Median).

   *Grenzkosten.* Gemessen wird die eigene Zeit des Moduls in Frames, in denen die vier M0-Module eingeschaltet sind; es zeichnet nach ihnen. Das ist keine Differenz zweier Mediane (die steht als Gegenprobe im Log und ist unter Last verrauscht). Nicht erfasst werden: der Anteil des Moduls am späteren gesammelten Zeichnen auf den modernen Versionen, die Kosten der Größenabfrage übersprungener Module, und der Aufwand des Hooks außerhalb der Module (Hookzeit minus Summe der Modulzeiten; auf dem untätigen Rechner 0,9 bis 2,4 µs, unter Last bis 6,2 µs). Gegenprobe ohne den Zeitgeber je Modul: gepaarte, abwechselnde Fenster (siehe Methode).

   *Korrektur der Definition (22.11.2026, Regel des Koordinators, vorbehaltlich Bestätigung durch den Projektinhaber).* Der Plan für M1a definierte das Modulbudget zuerst als Kosten „jedes Textmodul allein“, höchstens 10 µs. Dieses Budget wurde fast überall verfehlt: in jedem gültigen Lauf auf jeder Version lag mindestens ein Modul allein darüber, in neun von dreizehn Läufen alle fünf gezeichneten (gültige Werte 7,5 bis 31,7 µs). Die Zeitmessung je Zeichenaufruf (acht Läufe, nur auf 1.21.11 und 26.3) zeigt warum: der erste Textaufruf eines Frames im Spiel selbst (vanilla) kostet im Median 7,9 bis 22,0 µs, jeder weitere 0,3 bis 3,8 µs. Diesen Betrag trägt das Modul, das im Frame zuerst Text zeichnet, nicht das einzelne Modul. Werte unter wenigen Mikrosekunden liegen nahe an der Auflösung des Zeitgebers (100 ns) und sind nur auf etwa 3 bis 30 Prozent genau. Für 1.8.9 ist dieselbe Erklärung angenommen, nicht gemessen. Deshalb gilt das Budget seitdem für die Grenzkosten; die Kosten „allein“ werden weiter gemessen und berichtet, sie sind kein Budget.

   *Methode.* Testhaken `HudRenderer.setRenderTimes` (in der ausgelieferten Mod nie gesetzt), Zeit mit `System.nanoTime()` vor der Größenabfrage und nach dem Zurücksetzen der Transformation; übersprungene Module (Größe 0) zählen 0. Je Fall 500 Frames Einschwingen, dann 2000 gemessene Frames; Median, 95. Perzentil und Maximum je Modul und für die Summe je Frame. Auflösung des Zeitgebers auf dem Messrechner 100 ns; ein leeres Paar von `nanoTime`-Aufrufen kostet im Median 0, im 95. Perzentil 100 bis 400 ns (nicht abgezogen). Die Bildratenbegrenzung des Spiels (Vsync, 120 Bilder pro Sekunde) bleibt; die Untätigkeitsbegrenzung der modernen Versionen („afk“: 30, später 10 Bilder pro Sekunde ohne Eingabe) wird für die Messung abgeschaltet und danach zurückgestellt, sonst misst der Test ein untätiges Spiel. Die beiden Diagnoseläufe (26.3 Lauf 5, 1.8.9 Lauf 4) liefen zusätzlich mit `-XX:+PrintCompilation`. Der Spieltest schlägt fehl, wenn alle elf über 120 µs, ein Grenzkostenwert über 20 µs oder die vier M0-Module über 100 µs liegen; die strenge Beurteilung gegen 50 und 10 µs steht als Warn- und Urteilszeile im Log und trifft der Mensch über die Läufe. Seit der Wiederholung (Gesamtprüfung von M1a) prüft der Test nach jedem Fall, dass der Spieler lebt (sonst schlägt der Lauf fehl), und misst als Gegenprobe der Grenzkosten gepaarte Fenster: Zeitgeber je Modul aus, abwechselnd A (vier M0-Module) und B (A plus ein Modul), 24 Paare, je Fenster 60 Frames Einschwingen und 120 gemessene, Median der Hookzeit je Fenster, Median der 24 Differenzen B minus A; gemessen für das billigste und das teuerste gezeichnete Modul des Laufs.

   *Messwerte vom 22.11.2026, untätiger Rechner.* Messrechner: Desktop-PC mit AMD Ryzen 9 5900X (12 Kerne, 24 Threads), 32 GB RAM, NVIDIA GeForce RTX 4070 SUPER, Windows 11 Pro; alle Spieltests laufen auf Java 25. Kein anderer Build und kein anderes Spiel liefen; im Hintergrund nur die Dateisynchronisation (etwa 5 bis 10 % der Maschine) und ein liegengebliebener Suchprozess (etwa 4 %, ein Kern). Die Bildrate lag in jedem Fenster an der Begrenzung (118 bis 120). Je Version mindestens fünf Läufe, nacheinander, mit Pausen dazwischen. Mediane in µs, in Klammern das 95. Perzentil; Grenzkosten und „allein“ in der Reihenfolge Richtung / Uhr / Speicher / Geschwindigkeit / Stoppuhr (Ping und Server-Adresse zeichnen im Einzelspieler nichts und zählen 0); „Paar“ ist die Gegenprobe (Median der gepaarten Differenzen, in Klammern die eigene Zeit desselben Moduls nach dem Zeitgeber). Es sind Messungen eines Rechners.

   | Version | Lauf | vier M0 (p95) | alle elf (p95) | alle aus (Hook) | Grenzkosten | allein (Information) | Paar |
   |---|---|---|---|---|---|---|---|
   | 1.8.9 | 1 | 18,9 (27,0) | 69,5 (115,5) langsam | 0,9 | 1,7 / 1,7 / 3,5 / 2,0 / 1,9 | 5,6 / 6,4 / 8,7 / 6,8 / 6,6 | Richtung 3,7 (1,7), Speicher 6,0 (3,5), langsam |
   | 1.8.9 | 2 | 19,3 (28,2) | 30,0 (36,4) | 1,3 | 1,7 / 1,6 / 3,4 / 1,8 / 1,8 | 5,9 / 6,0 / 7,9 / 5,1 / 6,6 | Uhr 1,6 (1,6), Speicher 3,8 (3,4) |
   | 1.8.9 | 3 | 18,2 (24,0) | 30,2 (38,5) | 1,0 | 1,6 / 1,7 / 3,5 / 2,0 / 1,9 | 5,5 / 5,4 / 8,5 / 5,8 / 5,3 | Richtung 1,35 (1,6), Speicher 3,6 (3,5) |
   | 1.8.9 | 4 | 19,0 (25,5) | 31,1 (42,3) | 1,0 | 1,7 / 1,6 / 3,5 / 2,0 / 1,9 | 5,3 / 5,6 / 7,9 / 6,4 / 6,3 | Uhr 1,55 (1,6), Speicher 3,8 (3,5) |
   | 1.8.9 | 5 | 43,6 (66,6) langsam | 65,0 (89,7) langsam | 0,8 | 4,3 / 4,1 / 6,0 / 4,8 / 4,5 | 11,1 / 10,1 / 12,7 / 11,2 / 10,1 | Uhr 4,2 (4,1), Speicher 6,45 (6,0) |
   | 1.8.9 | 6 | 44,5 (57,9) langsam | 61,8 (90,8) langsam | 0,5 | 4,0 / 4,0 / 5,8 / 4,1 / 4,1 | 11,6 / 10,0 / 12,4 / 10,4 / 10,5 | Richtung 3,9 (4,0), Speicher 5,6 (5,8) |
   | 26.3 | 1 | 24,7 (33,1) | 34,0 (44,4) | 0,6 | 2,1 / 2,3 / 3,6 / 2,4 / 2,0 | 9,7 / 8,7 / 9,1 / 9,0 / 8,4 | Stoppuhr 2,05 (2,0), Speicher 3,6 (3,6) |
   | 26.3 | 2 | 24,4 (32,7) | 36,4 (45,2) | 0,7 | 2,1 / 2,4 / 3,7 / 2,6 / 2,4 | 10,5 / 8,6 / 11,0 / 8,5 / 8,4 | Richtung 2,2 (2,1), Speicher 3,9 (3,7) |
   | 26.3 | 3 | 22,5 (29,3) | 36,1 (45,6) | 0,5 | 2,2 / 2,1 / 3,4 / 2,2 / 1,9 | 9,5 / 8,8 / 10,9 / 9,3 / 7,9 | Stoppuhr 2,1 (1,9), Speicher 4,0 (3,4) |
   | 26.3 | 4 | 25,0 (35,6) | 36,3 (46,7) | 0,6 | 2,4 / 2,3 / 3,5 / 2,4 / 2,2 | 8,8 / 8,3 / 8,7 / 9,0 / 7,2 | Stoppuhr 2,9 (2,2), Speicher 3,7 (3,5) |
   | 26.3 | 5 | 25,9 (35,4) | 34,5 (44,4) | 0,8 | 2,0 / 2,4 / 3,6 / 2,5 / 2,2 | 8,7 / 6,9 / 10,7 / 8,1 / 9,4 | Richtung 2,2 (2,0), Speicher 3,1 (3,6) |
   | 26.3 | 6 | 22,0 (32,1) | 35,2 (48,3) | 0,6 | 2,2 / 2,3 / 3,7 / 2,4 / 2,2 | 6,5 / 6,7 / 7,6 / 6,5 / 6,1 | Richtung 1,35 (2,2), Speicher 4,1 (3,7) |
   | 1.21.11 | 1 | 23,6 (32,7) | 38,4 (51,3) | 0,8 | 2,2 / 2,5 / 2,9 / 2,3 / 2,2 | 10,1 / 10,7 / 9,3 / 7,0 / 9,1 | Richtung 2,3 (2,2), Speicher 2,9 (2,9) |
   | 1.21.11 | 2 | 26,2 (38,2) | unter Last | unter Last | 2,4 / 2,4 / 3,4 / 2,8 / 2,5 | 10,3 / 9,7 / 12,4 / 10,0 / 9,5 | unter Last |
   | 1.21.11 | 6 | 19,5 (28,1) | 39,0 (48,1) | 0,8 | 2,3 / 2,4 / 3,1 / 2,4 / 2,2 | 5,4 / 8,4 / 13,3 / 11,6 / 8,9 | Stoppuhr 2,4 (2,2), Speicher 4,1 (3,1) |
   | 1.21.11 | 7 | 26,8 (34,8) | 38,2 (46,6) | 0,7 | 2,1 / 2,3 / 3,4 / 2,3 / 2,2 | 11,0 / 8,3 / 12,1 / 10,4 / 9,4 | Richtung 2,4 (2,1), Speicher 3,3 (3,4) |
   | 1.21.11 | 8 | 21,9 (30,2) | 30,7 (40,7) | 0,3 | 2,1 / 2,3 / 3,3 / 2,5 / 2,3 | 5,9 / 5,9 / 6,4 / 6,1 / 5,6 | Richtung 1,9 (2,1), Speicher 3,3 (3,3) |

   Hookzeit minus Summe der Module: 1.8.9 0,9 bis 1,8 µs, 26.3 0,9 bis 2,4 µs, 1.21.11 0,8 bis 2,5 µs. „Langsam“: auf 1.8.9 lief der Rechner zweimal in einem Zustand, in dem jeder Fall etwa 2,3-mal so lange dauerte, bei unveränderten 120 Bildern pro Sekunde und ohne sichtbaren anderen Prozess (Lauf 1 ab dem Fall „alle elf“, Läufe 5 und 6 durchgehend); die Ursache ist nicht geklärt (7.3). Diese Werte zählen mit. Auf 26.3 überschnitt sich in Lauf 3 eine kurze Dateisuche des Prüfers mit den ersten Fällen; die Werte fallen nicht auf und zählen mit.

   *Läufe auf 1.21.11.* Am ersten Messtag wurde der Rechner ab 20:46 anderweitig genutzt (ein Spiel des Inhabers): ab dem Fall „alle elf“ von Lauf 2 lief alles unter Last; Lauf 3 (vier M0 42,5 µs) ist deshalb nicht aufgeführt, Lauf 4 ist ungültig (fehlgeschlagene Prüfung der Tastenverteilung, siehe unten), Lauf 5 wurde abgebrochen. Die Läufe 6 bis 8 liefen am 23.11.2026 auf demselben, wieder untätigen Rechner. Damit liegen fünf untätige Läufe für die vier M0-Module und die Grenzkosten vor und vier für „alle elf“. Die Läufe 1.21.11 Nr. 6 bis 8, 26.3 Nr. 6 und 1.8.9 Nr. 6 liefen mit der geänderten Allokationsprüfung (unten).

   *Beurteilung (Regeln des Koordinators, vorbehaltlich Bestätigung durch den Projektinhaber).* Vier M0-Module, Median der Läufe-Mediane: 1.8.9 19,2 µs (sechs Läufe, Spanne 18,2 bis 44,5; kein Lauf über 50; das 95. Perzentil liegt in zwei Läufen darüber, 66,6 und 57,9, beide im langsamen Zustand), im Budget; 26.3 24,6 µs (sechs Läufe, 22,0 bis 25,9; kein Lauf über 50; 95. Perzentil höchstens 35,6), im Budget; 1.21.11 23,6 µs (fünf Läufe, 19,5 bis 26,8; kein Lauf über 50; 95. Perzentil höchstens 38,2), im Budget. Alle elf: 1.8.9 46,5 µs (30,0 bis 69,5; die drei hohen Werte im langsamen Zustand), 26.3 35,7 (34,0 bis 36,4), 1.21.11 38,3 (30,7 bis 39,0): im Budget von 120 µs. Grenzkosten je neuem Modul: 1.8.9 1,6 bis 6,0, 26.3 1,9 bis 3,7, 1.21.11 2,1 bis 3,4 µs: im Budget von 10 µs. Die Gegenprobe mit gepaarten Fenstern stimmt auf dem untätigen Rechner im Allgemeinen auf unter 1 µs mit dem Zeitgeber je Modul überein (größte Abweichung 1,0 µs: 1.21.11 Lauf 6 Speicher 4,1 gegen 3,1; im langsamen Zustand beide entsprechend höher); maßgeblich bleibt der Zeitgeber, weil er weniger rauscht und genau das Modul misst.

   *Frühere Zahlen unter Last.* Die vorläufigen Zahlen vom selben Tag (Rechner unter Last durch Builds anderer Agenten, Bildrate je Fenster 20 bis 120) stehen mit allen Läufen in der vorigen Fassung dieses Abschnitts (Commit 60ad758); dort lag der Median der vier M0-Module auf 1.8.9 bei 52,7 µs, auf 26.3 bei 27,9 und auf 1.21.11 bei 33,7. Sie sind durch die Werte oben ersetzt.

   *Offene Beobachtungen zur Allokation.* Der Frame-Aufruf des Kerns mit allen elf Modulen allokiert in jedem gültigen Lauf auf allen drei Versionen 0 Byte (300 Frames; auf den modernen Versionen allokieren die Zeichenaufrufe des Spiels selbst etwa 37 KB je Frame). Der Spieltest prüft das: ein Frame, in dem kein Modul einen neuen Text zeigt, muss 0 Byte haben.
   - 1192 Byte in der Tastenverteilung: weiter ungeklärt. Bei der Wiederholung auf 1.8.9 in keinem von fünf Läufen, auf 26.3 in einem von fünf (Aufruf 231 im ersten Fenster, 48 µs, Laufzeit der JVM 67,0 s), auf 1.21.11 zweimal (Aufruf 232 im ersten Fenster, 71,8 s; und Aufruf 60 im ZWEITEN Fenster, 93 µs, 79,8 s, womit die exakte Prüfung „zweites Fenster 0 Byte“ fehlschlug). Immer genau ein Aufruf, ohne Speicherbereinigung und ohne Fokuswechsel. Die Fundstellen liegen bei etwa derselben Laufzeit der JVM (67 bis 80 s) und fallen in das Fenster, das gerade offen ist: es ist ein einmaliges Ereignis je Spielstart, keine Allokation im eingeschwungenen Zustand, aber die exakte Prüfung ist damit nicht stabil. Vermutung, nicht belegt: der JIT stellt beim Verwerfen übersetzten Codes Objekte, die er wegoptimiert hatte, auf dem Heap wieder her. Der gemessene Aufruf enthält die Tastenabfragen des Spiels; der Beleg „0 Byte im Interpreter“ deckt nur den Kern. Regel des Koordinators vom 23.11.2026: das geprüfte zweite Fenster beginnt frühestens 90 s nach dem Start der JVM (der Test wartet bei Bedarf und schreibt die Wartezeit ins Log); darin bleibt die Prüfung exakt 0 Byte. In den fünf Läufen mit dieser Regel (1.21.11 Nr. 6 bis 8, 26.3 Nr. 6, 1.8.9 Nr. 6; Wartezeit 8 bis 23 s) trat die Allokation in keinem Fenster auf. Tritt sie nach 90 s auf, ist das ein Befund.
   - 896 Byte auf 1.8.9 (frühere Läufe unter Last, zweite zählende Zeichenrunde des Tests): seit 100 Vorlauf-Frames nicht mehr gesehen, auch in den fünf untätigen Läufen nicht.

   *Bildratenvergleich (Gegenprobe).* Ein Prozentwert beim FPS-Vergleich (gleiche Welt, gleiche Position, 30 s Mittel) hängt von der absoluten Bildrate ab und taugt nur als Gegenprobe. Messung der Abschlussprüfung von M0 (ohne Bildratenbegrenzung, vier Module an): auf 26.3 etwa 26 Mikrosekunden pro Frame (minus 4,1 %), auf 1.8.9 etwa 34 (minus 7,2 %); auf 1.21.11 nicht auflösbar, weil das Rauschen zwischen den Läufen (bis 8 %) größer war als der Unterschied (höchstens etwa 40 Mikrosekunden). Bei allen Modulen aus ließ sich auf keiner Version ein Kostenunterschied auflösen. Die frühere Regel (höchstens 2 % Bildratenverlust) wurde bei unbegrenzter Bildrate auf 26.3 und 1.8.9 überschritten und deshalb am 21.11.2026 durch den absoluten Wert ersetzt, per Entscheidung des Stellvertreters des Projektinhabers, vorbehaltlich dessen Bestätigung.

   *Nicht umgesetzt: GL-Zustand auf 1.8.9 nur bei einem Wechsel zwischen Füllen und Text setzen* (M1a-Plan, Task 17). Bedingung war ein Median der Läufe der vier M0-Module auf 1.8.9 über 50 µs auf dem untätigen Rechner; gemessen 19,2 µs aus sechs Läufen (kein Lauf darüber, das 95. Perzentil in zwei Läufen darüber). Die Optimierung bleibt im Rückstand (7.3).

## 9. Server-Sperren

### 9.1 Betroffene Features
24 Freelook, 45 Minimap, 44 Waypoints, 13 Reichweite, 36 Hitboxen. Weitere kommen nur per Entscheidung des Menschen dazu.

### 9.2 Mechanismus
- Datei `mod/common/src/main/resources/hengst/server-restrictions.json`, mitgeliefert und per PR erweiterbar:
  ```json
  { "schema": 1,
    "servers": [
      { "hosts": ["example.net"], "deny": ["freelook", "minimap"],
        "source": "https://example.net/rules", "checked": "2026-11-20" } ] }
  ```
  `hosts` passt auf den Host und alle Subdomains (`example.net` sperrt `mc.example.net`, nicht `notexample.net`). Jeder Eintrag braucht `source` (öffentliche Regelseite des Servers) und `checked`. Ohne Quelle wird ein PR nicht gemergt.
- Normalisierung der Adresse: Kleinbuchstaben, Port weg, abschließender Punkt weg, IPv6 in Klammern unterstützt. IP-Adressen passen nur auf exakt gleiche IP-Einträge.
- Die mitgelieferte Sperre ist bindend: der Spieler kann sie nicht aufheben. Grund: die Liste setzt Serverregeln um, ein Aufheben-Knopf würde sie wertlos machen.
- Zusätzlich je Server ein eigener Aus-Schalter für jedes sperrbare Feature (nur strenger, nie lockerer), gespeichert in `settings.json` unter dem normalisierten Host.
- Einzelspieler und LAN: keine Sperren.
- Prüfung beim Serverbeitritt und beim Wechsel; gesperrte Features gehen sofort aus (Rendering, Tasten, HUD), Status sichtbar in der UI. Seit M1a erkennt der Kern den Serverwechsel im Tick: er vergleicht `GameView.serverAddress()` mit dem letzten Wert (Beitritt, Verlassen, Wechsel von einem Server zum anderen; der Einzelspieler ist "kein Server"), meldet ihn `FeatureGate.setServer(String)` und ruft denselben Konfigurationsdurchgang wie ein Schließen der Einstellungen (jede Funktion bekommt `onConfigChanged`, das HUD wird neu gebaut, `FeatureState` neu veröffentlicht). Die Sperrliste speist ab M4 die Schnittstelle `RestrictionPolicy` (4.3); bis dahin gilt `RestrictionPolicy.NONE`. Grenzen des Vergleichs: Die Adresse zählt so, wie sie eingetragen wurde, Groß- und Kleinschreibung unterschieden (die Normalisierung der Adresse aus diesem Abschnitt gehört in die Richtlinie), und ein Proxy-Netz, das hinter einer Adresse den Backend-Server wechselt, fällt nicht auf. Eine Funktion, die die Richtlinie abschaltet, obwohl sie eingeschaltet ist, bekommt keine eigene Benachrichtigung (7.3).
- Die anfängliche Liste ist leer. Der M4-Plan recherchiert die Regeln der großen Server (z. B. Hypixel) und füllt sie mit Quellen. Heute ungeprüft, welche Server welche dieser Funktionen verbieten.

### 9.3 Testbarkeit
`ServerRestrictions` liegt in `common` und wird mit JUnit getestet (Host-Normalisierung, Subdomain-Treffer, Nicht-Treffer, IP, Port, kaputte Datei, unbekannte Feature-IDs werden ignoriert und geloggt).

### 9.4 Später möglich
Ein Server könnte über einen Plugin-Kanal selbst Features abschalten. Nicht in dieser Spezifikation; eigene Entscheidung, weil es Netzwerkcode in die Mod bringt.

## 10. Lizenz und Recht

- GPL-3.0-or-later wie das Repo; `fabric.mod.json` trägt `"license": "GPL-3.0-or-later"`.
- Abhängigkeiten zur Laufzeit: Fabric API (Apache-2.0), Legacy Fabric API (Apache-2.0, ungeprüft am Lizenztext des aktuellen Stands). Beide werden nicht in unsere Jar gebündelt, sondern vom Launcher als eigene Mod installiert.
- Keine Mojang-Assets in der Mod (keine kopierten Texturen, Schriften, Sounds). Eigene Icons als eigene PNGs.
- Hinweis "Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft." in `fabric.mod.json` (`description`) und im Einstellungsbildschirm.
- Kein "Minecraft" im Mod-Namen ("Hengst").
- Offene Clients (z. B. AxolotlClient, LGPL) dürfen als Ideenquelle dienen; Code-Übernahme nur mit Lizenzhinweis und nur, wenn die Lizenz mit GPL-3.0 verträglich ist. Geschlossene Clients: gar nichts.

## 11. Auslieferung durch den Launcher (später, kurz)

- CI baut `mod/` und hängt die drei Jars an ein GitHub-Release `mod-v<version>`.
- `data/companion-mod.json` im Repo nennt je Spielversion Jar-URL, SHA-512 und die nötige API (Fabric API bzw. Legacy Fabric API, über Modrinth aufgelöst, PLAN.md 4).
- Der Launcher legt die Jar beim Start einer Instanz mit eingeschalteter Option "Hengst-Mod" in `instances/<name>/mods/` (Hash-geprüft, PLAN.md 8) und entfernt sie, wenn die Option aus ist. Teil von Phase 7 laut PLAN.md 11.

## 12. Fahrplan

Jeder Meilenstein ist einzeln planbar und auslieferbar (alle drei Jars bauen, alle Tests grün, Prüfung im Spiel auf allen drei Versionen, wo das Feature existiert).

| Id | Titel | Features |
|---|---|---|
| M0 | Fundament | Build für 1.8.9, 1.21.11, 26.3; common-Kern, Konfiguration, Profile, HUD-Editor, Einstellungsbildschirm; 1 FPS, 2 CPS, 3 Keystrokes, 6 Koordinaten, 50 HUD-Editor. **Erledigt 2026-11-21**, über den Hengst-Launcher geprüft; der Bildratenvergleich von M0 ergab für die vier Module auf 26.3 etwa 26 und auf 1.8.9 etwa 34 Mikrosekunden pro Frame, auf 1.21.11 war der Unterschied nicht auflösbar (Messrauschen größer als der Unterschied); dieser Vergleich zählt die Kosten des späteren Zeichnens mit und ist nicht die Messgröße, die ab M1a je Modul gemessen wird. Diese Messgröße der vier M0-Module (Abgabe- bzw. Zeichenzeit, 8.8) liegt auf dem untätigen Rechner im Median der Läufe bei 19,2 µs (1.8.9), 24,6 µs (26.3) und 23,6 µs (1.21.11), also im Budget von 50 µs (Regel des Koordinators, vorbehaltlich Bestätigung durch den Projektinhaber) |
| M1a | Info-HUD, Grundlagen | Zustandstor für Features, Tastenbelegungen für Features, typisierte Optionszugriffe, vorwärtskompatible Konfiguration, Gruppenüberschriften in der Einstellungsliste, Messung der Frame-Kosten je Modul; 7 Richtung, 8 Ping, 9 Uhr, 10 Serveradresse, 11 Speicher, 15 Geschwindigkeit, 21 Stoppuhr |
| M1b | Info-HUD, Gegenstände | 4 Rüstungsstatus, 5 Trankeffekte, 16 Resourcepack-Anzeige, mit dem Zeichnen von Gegenständen und Symbolen |
| M2 | Vanilla-HUD anpassen | 17, 18, 19, 20, 26, 38 |
| M3 | Kampf und Bewegung | 12, 14, 22, 27, 30, 41 |
| M4 | Server-Sperren und Kamera | Sperrmechanismus (Abschnitt 9) mit recherchierter Liste; 13, 23, 24, 36, 39 |
| M5 | Welt-Optik | 25, 29, 31, 32, 33, 34, 37, 49 |
| M6 | Chat und Namen | 35, 42, 46, 47 (mit Texteingabe-Widget und Suche in den Einstellungen) |
| M7 | Render-Effekte | 28, 40, 48 |
| M8 | Karte und Bilder | 43, 44, 45 |

Reihenfolge nach Risiko und Nutzen: M1 bis M3 nutzen fast nur belegte Hooks und die M0-Infrastruktur; M4 bringt die Sperrlogik vor dem ersten sperrbaren Feature; M7 und M8 enthalten die Features mit eigenem Rendering, die auf 26.3 am meisten Unbekanntes haben.

## Quellen

1. https://meta.legacyfabric.net/v2/versions/loader/1.8.9 und https://meta.legacyfabric.net/v2/versions/intermediary/1.8.9 (abgerufen 20.11.2026)
2. https://meta.fabricmc.net/v2/versions/loader/1.21.11, https://meta.fabricmc.net/v2/versions/loader/26.3, https://meta.fabricmc.net/v2/versions/intermediary/26.3 (`0.0.0`), https://meta.fabricmc.net/v2/versions/yarn/26.3 (leer), https://meta.fabricmc.net/v2/versions/game (26.3 und 1.21.11 vorhanden)
3. https://github.com/Legacy-Fabric/fabric-example-mod (Zweig `master`: `gradle.properties`, `build.gradle`, Gradle 9.4.0)
4. https://repo.legacyfabric.net/legacyfabric/net/legacyfabric/legacy-looming/maven-metadata.xml (1.16.1; POM hängt an `fabric-loom:1.16-SNAPSHOT`)
5. https://maven.fabricmc.net/net/fabricmc/fabric-loom/maven-metadata.xml (stabil 1.19.4, 1.20.0-alpha.31), https://github.com/FabricMC/fabric-loom/releases/tag/1.19
6. https://github.com/FabricMC/fabric-example-mod Zweig `26.3` (`net.fabricmc.fabric-loom`, `release = 25`, Gradle 9.8.2, Fabric API 0.166.2+26.3)
7. https://maven.legacyfabric.net/net/legacyfabric/yarn/1.8.9+build.604/yarn-1.8.9+build.604-v2.jar (Mappings ausgewertet), https://meta.legacyfabric.net/v2/versions/yarn/1.8.9
8. https://github.com/FabricMC/fabric-example-mod Zweig `1.21.11` (`net.fabricmc.fabric-loom-remap`, `loom.officialMojangMappings()`, `release = 21`, Gradle 9.8.2)
9. https://piston-meta.mojang.com/mc/game/version_manifest_v2.json und die Versions-JSONs von 26.3, 1.21.11, 1.8.9 (`javaVersion`, `downloads`, Libraries)
10. https://docs.fabricmc.net/develop/loom/ (Plugin-IDs: `net.fabricmc.fabric-loom` für unverschleierte Versionen ab 26.1, `net.fabricmc.fabric-loom-remap` für 1.21.11 und älter, `fabric-loom` nur Altlast)
11. https://repo.legacyfabric.net/legacyfabric/net/legacyfabric/legacy-fabric-api/legacy-fabric-api/maven-metadata.xml (1.13.5+1.8.9)
12. https://api.modrinth.com/v2/project/legacy-fabric-api/version (Versionen für 1.8.9 mit Loader-Liste)
13. https://maven.fabricmc.net/net/fabricmc/fabric-api/fabric-api/maven-metadata.xml (0.141.6+1.21.11, 0.166.2+26.3)
14. https://github.com/FabricMC/fabric-loom/releases/tag/1.16 ("Update minimum Gradle version to 9.4")
15. https://fabricmc.net/2026/03/14/261.html (26.1: Plugin-Wechsel, `implementation` statt `modImplementation`, `jar` statt `remapJar`, Java 25 für die Gradle-JVM, Mod-ID `fabric` entfällt, keine Mod bis 1.21.11 läuft ohne Neukompilierung)
16. https://services.gradle.org/versions/current (9.9.2)
17. https://github.com/FabricMC/fabric-api Zweige `26.3` und `1.21.11` (API-Klassen, Mixins, Testmod `HudTests.java`)
18. https://github.com/Legacy-Fabric/fabric Zweig `port/ornithe-gen-2` (Standardzweig; `HudRenderCallback`, `ClientTickEvents`, `KeyBindingHelper`, `gradle.properties` mit `feather_build`, `osl_version`)
19. https://meta.ornithemc.net/v3/versions/feather/1.8.9, https://meta.ornithemc.net/v3/versions/intermediary/1.8.9, https://ornithemc.net/
20. https://maven.kikugie.dev/releases/dev/kikugie/stonecutter/maven-metadata.xml (0.10, lastUpdated 10.11.2026)
21. https://codeberg.org/stonecutter/stonecutter Zweig `0.10` (`README.md`, `settings/tree/TreeBuilder.kt`, `StonecutterSettingsExtension.kt`). Die Doku-Seite https://stonecutter.kikugie.dev/ war für automatisches Abrufen gesperrt.
22. https://codeberg.org/OxFaTech/DayOfMind-Additions/pulls/6 und https://codeberg.org/Mirsario/Minecraft-LoomBackCompat (nur über Suchergebnis gesehen, nicht im Detail geprüft)
23. https://maven.fabricmc.net/net/fabricmc/fabric-loom-remap/net.fabricmc.fabric-loom-remap.gradle.plugin/1.19.4/ und .../net.fabricmc.fabric-loom.gradle.plugin/1.19.4/ (beide POMs: `net.fabricmc:fabric-loom:1.19.4`)
