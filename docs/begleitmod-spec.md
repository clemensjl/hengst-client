# Begleitmod: Spezifikation

Stand 20.11.2026. Ersetzt PLAN.md Entscheidung 13.3 ("AxolotlClient bündeln"): Hengst Client bekommt eine eigene Client-Mod mit 50 Funktionen, von Grund auf geschrieben, gleichzeitig für die Spielversionen **1.8.9, 1.21.11 und 26.3**. Ort im Repo: `mod/`. Lizenz wie das Repo: GPL-3.0-or-later.

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
- 1.21.11 baut mit Mojang-Mappings und wird beim Packen nach Intermediary remappt (mit Refmap). Weil beide Versionen im Quelltext Mojang-Namen nutzen, ist geteilter Quelltext möglich; die Unterschiede sind echte Umbenennungen von Mojang (Beispiele unten).
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
- Übersetzungen: `en_us` und `de_de`. Alle Texte der Mod-Oberfläche liegen als `hengst/lang/<code>.json` in `common` und werden von `common` selbst geladen (Sprache über `Platform.languageCode()`, Rückfall `en_us`). Nur die Namen der Tastenbelegungen, die das Spiel im Steuerungsmenü selbst übersetzt, stehen zusätzlich in den Sprachdateien der Adapter (modern `.json`, 1.8.9 `.lang`).

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
    String serverAddress();       // null: Einzelspieler oder Hauptmenü
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
    String languageCode();                // aktuelle Spielsprache, z. B. "de_de"
    void logWarning(String message);
    void openScreen(UiScreen screen);     // ersetzt den aktuellen Bildschirm
    void closeScreen();
}

public interface TimeSource {
    long nanoTime();
    TimeSource SYSTEM = System::nanoTime;
}
```

Einstiegspunkte von den Adaptern in `common` (Klasse `io.github.clemensjl.hengst.mod.common.HengstCore`):

```java
public static HengstCore start(Platform platform, TimeSource time);
public void onClientTick();                               // Ende jedes Client-Ticks, 20 Hz
public void onHudRender(Canvas canvas, float partialTick); // einmal pro Frame
public void onMouseButton(int button, boolean pressed);    // 0 links, 1 rechts, 2 Mitte
public void onOpenSettingsKey();
public void onOpenHudEditorKey();
public void shutdown();                                    // speichert offene Änderungen
```

Spätere Meilensteine ergänzen Einstiegspunkte (`onServerJoin(String)`, `onDisconnect()`, `onChatMessage(...)`, `onAttack(...)`) und Schnittstellen (`WorldView`, `ItemView`), jeweils im Plan des Meilensteins mit genauer Signatur.

### 4.4 Neue Spielversion

Eine neue 26.x-Version wird ein weiterer Stonecutter-Knoten in `modern`. Fällt sie zu weit ab, wird der älteste modern-Knoten fallen gelassen. 1.8.9 bleibt eigener Build. Jede neue Version ist ein eigener Plan mit dem Prüfschritt aus M0 Task 13.

## 5. Kernlogik in `common`

- **Modul-Registry:** jedes Feature ist ein `Feature` mit ID (`fps`, `cps`, ...), Nummer aus Abschnitt 7 und `supports(GameVersion)` aus der Matrix. HUD-Funktionen sind `HudModule` (Unterklasse). Nicht unterstützte Features erscheinen in der UI nicht.
- **Konfiguration:** `<config>/hengst/settings.json` (aktives Profil, globale Schalter) und `<config>/hengst/profiles/<name>.json` (pro Modul: an/aus, Anker, Versatz, Skalierung, Stil, Moduloptionen). Format mit `"schema": 1`. Schreiben atomar (Temp-Datei, dann Umbenennen), höchstens alle 2 s und beim Schließen eines Bildschirms und bei `shutdown`. Kaputte Datei: Sicherung als `<name>.json.broken-<yyyyMMdd-HHmmss>`, dann Standardwerte. Unbekannte Module bleiben beim Speichern erhalten.
- **Profile:** Name passt auf `[A-Za-z0-9 _-]{1,32}`; `default` gibt es immer und lässt sich nicht löschen. Neue Profile entstehen als Kopie ("Profil 2", "Profil 3", ...), ohne Texteingabe.
- **HUD-Layout:** Position = Anker (9 Punkte) + Versatz in GUI-Pixeln + Skalierung 0,5 bis 3,0. So bleibt ein Modul bei anderer GUI-Skalierung und Fenstergröße an seiner Ecke. Nach dem Ziehen wählt der Editor den Anker aus dem Bildschirmdrittel, in dem die Modulmitte liegt, und rechnet den Versatz um.
- **HUD-Editor:** Auswahl per Klick (oberstes Modul gewinnt), Ziehen, Einrasten an Bildschirmrand, Bildschirmmitte und Kanten anderer Module (4 px), Mausrad skaliert (±0,1), Pfeiltasten verschieben 1 px (mit Umschalt 10 px), Entf blendet aus, Esc schließt und speichert. Module bleiben immer vollständig im Bild.
- **Einstellungs-UI-Modell:** Liste der Features links mit Schalter, Optionen des gewählten Features rechts (Schalter, Schieber, Auswahl, Knopf), oben Profilwahl, Knopf "HUD bearbeiten". Gezeichnet über `Canvas`, damit alle drei Versionen dieselbe Oberfläche haben.
- **Zähllogik:** CPS über Ringpuffer (`long[64]`) je Maustaste, Fenster 1000 ms; FPS über Frame-Zähler mit Sekundenfenster; Combo, Timer, Speed, Reichweite ab ihren Meilensteinen.
- **Server-Sperren:** Abschnitt 9.

## 6. Einstellungs-UI

- Taste (Standard Rechte Umschalttaste) öffnet die Einstellungen, eigene Taste (nicht belegt) öffnet direkt den HUD-Editor. Beide als normale Tastenbelegung in den Vanilla-Steuerungsoptionen, Kategorie "Hengst".
- Ein Bildschirm, kein Fenster-im-Fenster. Links Features nach Gruppe (HUD, Optik, Komfort), mit Suche ab M6 (braucht Texteingabe).
- Gesperrte Features (Abschnitt 9) erscheinen ausgegraut mit Grund ("Auf diesem Server gesperrt: <host>").
- Kein eigener Look, der das Spiel imitiert oder ersetzt: dunkle Fläche, Vanilla-Schrift, Akzentfarbe aus den Einstellungen.

## 7. Feature × Version

Legende: **ja** / **nein** / **teils** (mit Grund). Hook-Angaben: **[B]** belegt aus dem Fabric-API-Quelltext des jeweiligen Zweigs [17] bzw. Legacy Yarn build.604 [7] oder Legacy-Fabric-API-Quelltext [18]; **[K]** Kandidat, vor dem Plan des Meilensteins per `genSources` am echten Spiel prüfen. "common" heißt: reine Logik ohne eigenen Hook, Daten über `GameView`/`InputView`, Anzeige über den gemeinsamen HUD-Hook.

Gemeinsame Hooks:
- **HUD:** 1.8.9 `HudRenderCallback.EVENT` (Legacy Fabric API, `onHudRender(client, tickDelta)`) [B]; 1.21.11 `HudElementRegistry.addLast(id, HudElement)` mit `render(GuiGraphics, DeltaTracker)` [B]; 26.3 dasselbe mit `extractRenderState(GuiGraphicsExtractor, DeltaTracker)` [B]. Ein einziges registriertes Element ruft `HengstCore.onHudRender`.
- **Tick:** `ClientTickEvents.END_CLIENT_TICK` in allen drei APIs [B].
- **Tasten:** 1.8.9 `KeyBindingHelper.registerKeyBinding(KeyBinding)` [B]; 1.21.11 `KeyBindingHelper.registerKeyBinding(KeyMapping)` [B]; 26.3 `KeyMappingHelper.registerKeyMapping(KeyMapping)` [B].
- **Klicks:** 1.8.9 Mixin `KeyBinding.onKeyPressed(I)V` (statisch, Maustasten kommen dort als `button - 100` an) [B Name, K Semantik]; modern Mixin `MouseHandler.onButton(JLnet/minecraft/client/input/MouseButtonInfo;I)V` HEAD [B].
- **Bildschirm öffnen:** 1.8.9 `MinecraftClient.setScreen(Screen)` [B]; 1.21.11 `Minecraft.setScreen(Screen)` [B]; 26.3 `Gui.setScreen(Screen)` [B].
- **Welt-Rendern:** 1.8.9 Mixin `GameRenderer.renderWorld(IFJ)V` [B]; 1.21.11 `WorldRenderEvents` [B]; 26.3 `LevelRenderEvents` [B].
- **Chat empfangen:** modern `ClientReceiveMessageEvents` [B]; 1.8.9 Mixin `ChatHud.addMessage(Text, int)` [B Name].

| Nr | Feature | 1.8.9 | 1.21.11 | 26.3 | Hook / Hinweis |
|---|---|---|---|---|---|
| 1 | FPS | ja | ja | ja | common: Frame-Zähler im HUD-Hook, Text einmal pro Sekunde |
| 2 | CPS | ja | ja | ja | Klick-Hook (oben); links/rechts getrennt |
| 3 | Keystrokes (WASD, Maus, Leertaste) | ja | ja | ja | `InputView.isDown` pro Frame: 1.8.9 `GameOptions.forwardKey ... attackKey/useKey/jumpKey` + `KeyBinding.isPressed()` [B]; modern `KeyMapping.isDown()` [K] |
| 4 | Rüstung mit Haltbarkeit | ja | ja | ja | Tick liest Rüstungsplätze: 1.8.9 `PlayerInventory.armor` [K]; modern `getItemBySlot(EquipmentSlot)` [K]; Item-Icon: 1.21.11 `GuiGraphics.renderItem` [K], 26.3 [K] |
| 5 | Trankeffekte | ja | ja | ja | 1.8.9 `getStatusEffectInstances()` [K]; modern `getActiveEffects()` [K]; Vanilla-Anzeige ausblenden: `VanillaHudElements.STATUS_EFFECTS` (1.21.11) / `MOB_EFFECTS` (26.3) [B]; 1.8.9 hat keine Vanilla-HUD-Anzeige |
| 6 | Koordinaten | ja | ja | ja | 1.8.9 Felder `Entity.x/y/z` [B]; modern `getX()/getY()/getZ()` [K]; Text im Tick |
| 7 | Richtung/Kompass | ja | ja | ja | 1.8.9 `Entity.yaw` [B]; modern `getYRot()` [K] |
| 8 | Ping | ja | ja | ja | vom Server gemeldete Latenz: 1.8.9 `PlayerListEntry.getLatency()` [K]; modern `PlayerInfo.getLatency()` [K] |
| 9 | Uhr | ja | ja | ja | common |
| 10 | Server-Adresse | ja | ja | ja | 1.8.9 `MinecraftClient.getCurrentServerEntry()` [B]; modern `Minecraft.getCurrentServer()` [K] |
| 11 | Speicher | ja | ja | ja | common (`Runtime`), Text einmal pro Sekunde |
| 12 | Combo-Zähler | ja | ja | ja | Treffer auf Ziel ohne eigenen Schaden: 1.8.9 Mixin `MinecraftClient.doAttack()V` [B]; modern `ClientPreAttackCallback` [B] bzw. `AttackEntityCallback` [K]; Schaden über `hurtTime` [K] |
| 13 | Reichweite | ja | ja | ja | Abstand Auge zu Trefferpunkt beim Angriff (Angriffs-Hook wie 12, `hitResult` [K]); **sperrbar** |
| 14 | Item-Zähler (Pfeile, Tränke, Totems) | teils | ja | ja | Inventar-Scan alle 10 Ticks; Totem gibt es erst ab 1.11, auf 1.8.9 nur Pfeile und Tränke |
| 15 | Geschwindigkeit | ja | ja | ja | common aus Positionsänderung je Tick |
| 16 | Resourcepack-Anzeige | ja | ja | ja | 1.8.9 `getResourcePackLoader()` [B]; modern `getResourcePackRepository().getSelectedPacks()` [K] |
| 17 | Scoreboard anpassen | ja | ja | ja | Größe/Lage: 1.8.9 Mixin `InGameHud.renderScoreboardObjective` [B]; modern `HudElementRegistry.replaceElement(VanillaHudElements.SCOREBOARD, ...)` mit Pose-Transform [B]; Zahlen ausblenden: 1.21.11 `Gui.displayScoreboardSidebar` [K], 26.3 `Hud` [K] |
| 18 | Bossbar anpassen | ja | ja | ja | 1.8.9 `InGameHud.renderBossBar()V` [B]; modern `replaceElement(BOSS_BAR)` [B] |
| 19 | Tabliste mit Ping-Zahlen | ja | ja | ja | 1.8.9 `PlayerListHud` Ping-Icon [K]; modern `PlayerTabOverlay.renderPingIcon` [K] |
| 20 | Sättigung/Hunger-Details | ja | ja | ja | Sättigung schickt der Server in allen Versionen mit; 1.8.9 `HungerManager.getSaturationLevel()` [K]; modern `FoodData.getSaturationLevel()` [K]; Anzeige `attachElementAfter(FOOD_BAR)` [B] |
| 21 | Stoppuhr/Timer | ja | ja | ja | common |
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

## 8. Laufzeitregeln

1. **Keine Allokation pro Frame im HUD-Pfad.** Texte entstehen im Tick (20 Hz) und nur, wenn sich der Wert geändert hat; `render` zeichnet zwischengespeicherte Strings. Positionen werden in einen wiederverwendeten `MutableRect` geschrieben. Ein JUnit-Test misst die Allokation von 1000 Frames mit `com.sun.management.ThreadMXBean.getThreadAllocatedBytes` und verlangt weniger als 1024 Bytes.
2. **Aktive Module als Array.** `HudRenderer` hält `HudModule[]` der eingeschalteten, unterstützten Module, neu gebaut nur bei Konfigurationsänderung. Keine Iteratoren, keine Streams, keine Lambdas mit Capture im Frame-Pfad.
3. **Abgeschaltet kostet nichts.** Abgeschaltete Module stehen nicht im Array. Mixins, die nicht entfernt werden können, prüfen als Erstes ein `static boolean` und kehren zurück.
4. **Mixins schmal.** `@Inject` an HEAD/RETURN oder `@WrapOperation`/`@ModifyExpressionValue` (MixinExtras ist im Fabric Loader seit 0.15.0 enthalten, https://github.com/FabricMC/fabric-loader/releases/tag/0.15.0; wir nutzen Loader 0.20.1 auf allen drei) statt `@Overwrite` und statt `@Redirect`. Kein Mixin, wo eine Fabric-API-Schnittstelle reicht. Jeder Mixin trägt `require = 1`, damit ein verschobenes Ziel beim Start auffällt statt still zu fehlen.
5. **Keine Reflection** außerhalb des Starts. Zugriffe auf private Felder über Accessor-Mixins.
6. **Kein Dateizugriff im Frame oder Tick.** Speichern gebündelt (Abschnitt 5).
7. **Keine eigenen Threads** in M0. Spätere Ausnahmen (Minimap-Kacheln, Screenshot-Vorschau) brauchen eine Begründung im Meilenstein-Plan.
8. **Kosten sichtbar.** Bei jedem Meilenstein wird im echten Spiel die FPS mit und ohne Mod verglichen (gleiche Welt, gleiche Position, 30 s Mittel); mehr als 2 % Verlust bei eingeschalteten M0-Modulen ist ein Fehler.

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
- Prüfung beim Serverbeitritt und beim Wechsel; gesperrte Features gehen sofort aus (Rendering, Tasten, HUD), Status sichtbar in der UI.
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
| M0 | Fundament | Build für 1.8.9, 1.21.11, 26.3; common-Kern, Konfiguration, Profile, HUD-Editor, Einstellungsbildschirm; 1 FPS, 2 CPS, 3 Keystrokes, 6 Koordinaten, 50 HUD-Editor |
| M1 | Info-HUD | 4, 5, 7, 8, 9, 10, 11, 15, 16, 21 |
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
