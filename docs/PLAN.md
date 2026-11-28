# Hengst Client: Gesamtplan

Repo `clemensjl/hengst-client`, Logo macht Clemens. Stand: 31.10.2026, Status: freigegeben.

## 1. Ziel

Eine offene Alternative zu Lunar Client: ein Launcher, der jede Minecraft-Java-Version startet, auf jeder Version das Maximum an FPS herausholt, Mods und Modpacks von Modrinth und CurseForge installiert und dabei aufgeräumt bleibt. Keine Werbung, keine Telemetrie, kein Pflicht-Account außer dem Microsoft-Login.

Abgrenzung zu bestehenden Projekten:

| Projekt | Offen | Lunar-artige In-Game-Features | Ein-Klick-FPS über alle Versionen | Modrinth + CurseForge |
|---|---|---|---|---|
| Lunar Client (inkl. Badlion) | nein | ja | ja, nur eigene Versionen | nur Modrinth-Import |
| Modrinth App | ja (GPL-3) | nein | nein | nur Modrinth |
| Prism Launcher | ja (GPL-3) | nein | nein | ja |
| **Wir** | ja | ja (gebündelt) | ja (kuratierte Profile) | ja |

Die Lücke ist das Gesamtpaket: Prism-Funktionsumfang, Lunar-Komfort, offen.

## 2. Leitprinzipien

1. **Clean:** Eine Startseite mit großem Play-Knopf. Expertenoptionen hinter "Erweitert". Keine Werbung, kein Shop im Launcher.
2. **FPS zuerst:** Jede Instanz startet standardmäßig mit dem besten Performance-Profil für ihre Version. Abschaltbar.
3. **Offen und ehrlich:** GPL-Lizenz, reproduzierbare Builds, keine Telemetrie. Crash-Upload nur nach Klick.
4. **Respekt vor Mod-Autoren:** CurseForge-Weitergabesperren werden eingehalten, Downloads kommen immer von der Originalquelle.
5. **Regelkonform:** Kein Offline-Modus ohne gekauftes Konto, keine Cheats, Einhaltung der Minecraft-Nutzungsrichtlinien (kein "Minecraft" im Produktnamen).

## 3. Architektur

### Stack (Empfehlung)

- **Tauri 2 + Rust** für Kern und Fenster: kleine Installationsdatei (rund 10 MB statt 150 MB bei Electron), wenig RAM, während Minecraft läuft. Erfahrung aus KernLight vorhanden.
- **React + TypeScript + Tailwind + shadcn/ui** für die Oberfläche.
- **Java-Teil** nur für die optionale In-Game-Begleitmod (siehe 6).

### Monorepo-Aufbau

```
crates/
  launcher-core/   Versionsliste, Downloads, Assets, Libraries, Natives, Startargumente
  auth/            Microsoft-Login (Device Code + Auth Code mit PKCE), Token-Refresh, Ablage im Windows-Anmeldeinformationsspeicher
  java/            Java-Laufzeiten finden, herunterladen (Mojang-Runtime-Manifest, Fallback Adoptium), pro Version zuordnen
  loaders/         Fabric, Quilt, Legacy Fabric, Ornithe, Forge, NeoForge (inkl. Installer-Prozessoren)
  content/         Modrinth- und CurseForge-Anbieter hinter einer gemeinsamen Schnittstelle, Abhängigkeiten, Updates
  packs/           .mrpack, CurseForge-Zip, Prism/MultiMC-Import, Export
  perf/            Performance-Profile, JVM-Flags, Hardware-Erkennung
  instances/       Instanzen anlegen, kopieren, sichern, sperren
apps/
  desktop/         Tauri-App mit React-Oberfläche
  cli/             Kommandozeile (gleicher Kern, für Tests und Power-User)
mod/               optionale Begleitmod (Java, Fabric, mehrere Versionen)
data/              kuratierte Profile als JSON (per PR erweiterbar)
docs/
```

### Datenfluss beim Start

1. Instanz gewählt → Version + Loader + Mods aus `instance.json`.
2. `launcher-core` prüft Dateien gegen SHA1/SHA512, lädt fehlende parallel (begrenzt, mit Wiederholung, Mirror-Fallback).
3. `java` liefert die passende Laufzeit (8 / 16 / 17 / 21 / 25).
4. `perf` baut JVM-Flags und Speichergröße.
5. `auth` liefert gültiges Token (Refresh im Hintergrund).
6. Prozess startet, Log wird live gestreamt, bei Absturz läuft die Crash-Analyse.

### Speicherorte

`%APPDATA%\HengstClient\` mit `instances/`, `cache/` (geteilte Libraries und Assets, Duplikate per Hardlink), `java/`, `logs/`. Portabler Modus über eine Datei `portable.txt` neben der Exe.

## 4. Versionsabdeckung

"Alle Versionen" heißt: alles, was in Mojangs Versionsliste steht, ist startbar. Das schließt Classic, Alpha, Beta, alle Releases, Snapshots und die neuen Jahresversionen (26.x) ein. Wie viel FPS-Gewinn möglich ist, hängt von der Version ab:

| Bereich | Java | Loader | FPS-Profil (Kern) |
|---|---|---|---|
| 26.x | 25 | Fabric, NeoForge | Sodium, Lithium, FerriteCore, ImmediatelyFast, EntityCulling, ModernFix, Dynamic FPS, MoreCulling |
| 1.20.5 bis 1.21.11 | 21 | Fabric, Quilt, NeoForge, Forge | wie oben, dazu ScalableLux, Krypton |
| 1.18 bis 1.20.4 | 17 | Fabric, Quilt, Forge, NeoForge (ab 1.20.1) | Sodium/Embeddium, Lithium, FerriteCore, Starlight, EntityCulling |
| 1.17 | 16 | Fabric, Forge | Sodium, Lithium, Starlight |
| 1.13 bis 1.16.5 | 8 | Fabric (ab 1.14), Forge | Sodium (ab 1.16), Lithium, Phosphor/Starlight, FerriteCore |
| 1.12.2 | 8 | Forge, Legacy Fabric, Cleanroom | Celeritas/Vintagium, FoamFix, VanillaFix |
| 1.8.9 | 8 | Forge, Legacy Fabric, Ornithe | Patcher, Celeritas (prüfen), EntityCulling-Port; OptiFine nur per Hand (nicht weiterverteilbar) |
| 1.7.10 | 8 | Forge | Angelica (Sodium-Port mit Shadern), Hodgepodge, ArchaicFix |
| Classic bis 1.6 | 8 | keiner | nur JVM-Tuning; Fixes für tote Skin-/Sound-Server (lokaler Proxy wie bei Betacraft) |

Stand Phase 3: Der Launcher setzt heute Fabric, Quilt, Legacy Fabric, Forge und NeoForge um; Ornithe und Cleanroom fehlen noch.

Konkrete Mod-Versionen werden nicht hart codiert, sondern beim Anlegen über die Modrinth-/CurseForge-API zur Version aufgelöst. Die Profile in `data/` beschreiben nur "welche Projekte für welchen Bereich", plus bekannte Unverträglichkeiten.

## 5. FPS-Strategie

Drei Ebenen, jede einzeln abschaltbar:

1. **Mod-Profile** (größter Hebel, 2x bis 5x je nach Version): siehe Tabelle oben. Varianten "Max FPS", "Ausgewogen", "Vanilla" (nichts). Shader-Variante mit Iris/Angelica.
2. **JVM-Tuning:**
   - Speicher automatisch aus RAM und Mod-Anzahl (Vanilla 2 GB, kleines Pack 4 GB, großes 6 bis 8 GB). Zu viel RAM wird bewusst vermieden.
   - Java 21+: Generational ZGC oder abgestimmtes G1 (A/B-gemessen, Standard wird nach Messung festgelegt).
   - Java 8: G1 mit Client-tauglichen Flags.
   - Optional GraalVM als Laufzeit (Experten).
3. **System:**
   - Windows-Grafikvorgabe "Hohe Leistung" für die Java-Exe setzen (verhindert iGPU-Start auf Laptops).
   - Prozesspriorität "Höher als normal" optional.
   - Launcher geht während des Spiels in den Tray und gibt Speicher frei.

**Messbar machen:** Benchmark-Modus startet eine feste Szene (eigene Testwelt, festes Kameraskript über die Begleitmod) und misst Durchschnitt, 1%-Low und Frametime. Ergebnis pro Profil, damit "meiste FPS" belegt und nicht behauptet wird.

## 6. In-Game-Ebene (was Lunar ausmacht)

HUD-Module: FPS, CPS, Keystrokes, Rüstungsanzeige, Trankeffekte, Koordinaten, Richtung, Ping, Uhr, Server-Adresse, Speicher.
Komfort: Zoom, Freelook, Toggle Sprint/Sneak, Fullbright, Fadenkreuz-Editor, Trefferfarbe, Motion Blur, Chat-Verbesserungen (Zeitstempel, Stapeln, Suche), Nametag-Optionen, Screenshot-Manager, Waypoints (nur wo Server es erlauben).

Entscheidung 20.11.2026: eigene Begleitmod mit 50 Funktionen für 1.8.9, 1.21.11 und 26.3, von Grund auf geschrieben, kein Code aus geschlossenen Clients. Details und Fahrplan M0 bis M8 in `docs/begleitmod-spec.md`.

Server-Anti-Cheats (Hypixel etc.) erlauben HUD-Mods dieser Art; keine Funktion darf Spielvorteile über Vanilla-Info hinaus geben.

## 7. Launcher-Funktionen (Gesamtliste)

Priorität: **M** = MVP (v0.1), **1** = v1.0, **S** = später.

### Konten
- M: Microsoft-Login, mehrere Konten, schneller Wechsel
- 1: Skin hochladen/wechseln, Cape wählen (offizielle Mojang-API), Skin-Vorschau 3D
- 1: Offline-Konto nur, wenn mindestens ein gekauftes Konto angemeldet ist (wie Prism)

### Instanzen
- M: Anlegen mit Version + Loader + Profil, Starten, Log-Fenster
- M: Versionsfilter (Release, Snapshot, Beta, Alpha)
- 1: Kopieren, Gruppen, Symbole, Notizen, Spielzeit-Zähler
- 1: Desktop-Verknüpfung pro Instanz, Start direkt ohne Launcher-Fenster
- 1: Instanz-Backup und Wiederherstellung, Welt-Backups mit Zeitplan
- 1: Ordner-Import aus Prism, MultiMC, CurseForge-App, Modrinth App, offiziellem Launcher
- S: Instanz per Code teilen (Exportdatei + optional Link)

### Inhalte
- M: Gemeinsame Suche über Modrinth und CurseForge mit Filtern (Version, Loader, Kategorie)
- M: Installieren mit Abhängigkeiten, Aktivieren/Deaktivieren, Entfernen
- 1: Updates prüfen und gesammelt einspielen, Rollback auf vorige Version
- 1: Konflikterkennung (bekannte Unverträglichkeiten aus `data/`, doppelte Mods, falscher Loader)
- 1: Resourcepacks, Shaderpacks, Datapacks pro Welt
- 1: Hinweisdialog für CurseForge-Mods mit Weitergabesperre (Browser öffnet, Download wird erkannt und einsortiert)
- S: Lokaler Mod-Cache über Instanzen hinweg (Hardlinks, spart Platz)

### Modpacks
- M: .mrpack installieren
- 1: CurseForge-Modpacks installieren, Updates, Export als .mrpack und CF-Zip
- S: Eigene Packs mit Changelog veröffentlichen

### Server
- 1: Serverliste mit Ping und Spielerzahl, Quick-Join, Favoriten
- S: Empfohlene Server pro Version (nur wenn ohne Bezahlung)

### Diagnose
- M: Live-Log mit Filter
- 1: Crash-Analyse (bekannte Muster: falsche Java-Version, fehlende Abhängigkeit, Mixin-Konflikt) mit Klartext-Lösung
- 1: Log-Upload zu mclo.gs auf Knopfdruck
- 1: Benchmark-Modus (siehe 5)

### Komfort
- 1: Discord Rich Presence (abschaltbar)
- 1: Deep Links (`modrinth://`, eigenes Schema) zum Installieren aus dem Browser
- 1: Sprache Deutsch und Englisch, weitere per Crowdin/Weblate
- 1: Hell/Dunkel, Akzentfarbe
- 1: Auto-Update des Launchers (signiert, Tauri-Updater)
- 1: CLI mit gleichen Funktionen
- S: Screenshot-Galerie mit Teilen
- S: Instanz-Sync zwischen PCs (eigener Ordner oder Git, kein eigener Server)

### Plattformen
- M: Windows 10/11 x64
- 1: Linux (AppImage, Flatpak), macOS (Apple Silicon + Intel)
- S: Windows ARM

## 8. Sicherheit und Datenschutz

- Tokens nur im Betriebssystem-Schlüsselspeicher, nie in Klartextdateien oder Logs.
- Alle Downloads mit Hash-Prüfung. Mods nur von Modrinth/CurseForge/Maven-Quellen der Loader.
- Malware-Lage: 2023 gab es den "fractureiser"-Wurm über CurseForge-Mods. Gegenmaßnahme: Hash-Abgleich mit Plattformdaten, Warnung bei Mods ohne Plattform-Herkunft, optionaler Scan-Hinweis.
- Keine Telemetrie. Absturzberichte nur manuell.
- CurseForge-API-Key wird nicht im Klartext ins Repo gelegt (Build-Secret in GitHub Actions); Forks müssen eigenen Key beantragen.

### Spielausgabe und Schwärzung (Phase 4)

Der Kern (`game.rs`) liest stdout und stderr des Spiels zeilenweise und schwärzt jede Zeile, bevor sie den Lesethread verlässt, also bevor sie Live-Log, Launcher-Logdatei (`logs/game/<Instanz>-<Zeitstempel>.log`) oder Fehlermeldungen erreicht. Geheim ist nur das Minecraft-Zugriffstoken des Microsoft-Kontos.

- Erfasst: das Token im Klartext, JSON-maskiert (auch mit `\/` und in Gsons HTML-sicherer Form wie `\u003d`) und prozentkodiert (Hex in Groß- und Kleinbuchstaben), auch wenn es über zwei Lesevorgänge verteilt ankommt. Zerbricht ein Zeilenumbruch das Token, werden ein Präfix am Zeilenende, ein Suffix am Zeilenanfang und eine Zeile, die nur aus einem Innenstück besteht, ab 8 Byte geschwärzt; je Zeilenumbruch können höchstens 7 Byte auf jeder Seite stehen bleiben. Zeilen über 8 KiB werden erst geschwärzt, dann gekürzt, nie mitten im Token.
- Als XML-Entitäten (`&#x41;`) geschriebene Token in log4j-Ereignissen: der Parser entschlüsselt Nachricht und Attribute, danach wird der entschlüsselte Text ein zweites Mal geschwärzt, bevor er Live-Log oder Logdatei erreicht.
- Nicht erfasst: das Token in Base64, als XML-Entitäten in einfachen Textzeilen (dort wird nichts entschlüsselt, die Entitäten bleiben stehen), Teilstücke mitten in einer Zeile, ein Token, das in viele Stücke unter 8 Byte umbrochen ist, und Zeilenenden nur aus `\r` (sie gelten für die Bruchprüfung nicht als Zeilengrenze).
- Die XUID steht im Klartext in Live-Log und Logdateien (sie ist eine Kennung, kein Zugangsschlüssel, wie UUID und Spielername). Wer Logs an Fehlerberichte hängt, gibt sie damit preis.
- Das Spiel erbt die Umgebungsvariablen des Launchers ungeschwärzt.
- Die CLI (`hengst launch`) schwärzt nichts: Das Spiel schreibt dort direkt in die Konsole, seine Ausgabe geht ungeschwärzt hindurch. Geschwärzt werden nur die Zeilen, die die Desktop-App liest (Live-Log und Launcher-Logdatei). Wer Konsolenausgabe der CLI weitergibt, kann damit das Token weitergeben.
- Live-Log (Desktop-App): Im Speicher liegen nur die schon geschwärzten Zeilen des aktuellen und des vorigen Laufs, je Lauf höchstens 10.000 Zeilen und 16 MiB (die ältesten fallen zuerst weg; ein log4j-Ereignis kann bis 512 KiB groß sein), zusammen also rund 32 MiB. Die Web-Ansicht bekommt Bündel, deren JSON unter 8 KiB bleibt (also höchstens rund 46 Zeilen); eine längere Zeile kommt in Teilen. Tauri 2.12.7 gibt solche Nachrichten sofort an die Seite weiter und legt nichts in seiner Warteschlange ab, deshalb bleibt auch bei beliebig vielen Neuladungen nichts liegen. Jedes Bündel muss bestätigt werden; höchstens 16 sind unbestätigt unterwegs (zusammen also unter 128 KiB). Liest die Seite nicht mehr, bekommt sie nichts mehr, und was inzwischen aus dem Puffer fällt, meldet das nächste Bündel als ausgelassen. Ein Bündel, das voll war, zieht das nächste sofort nach; ein nicht volles wartet 100 ms. „Kopieren“ läuft einzeln (ein zweites gleichzeitiges wird abgelehnt), liefert höchstens 4 MiB, dann die neuesten Zeilen, mit den Uhrzeiten wie in der Launcher-Logdatei (Zeitzonenversatz vom Start des Laufs).
- Das Spiel läuft in einem eigenen Job-Objekt ohne kill-on-close und überlebt das Schließen des Launchers. Läuft der Launcher selbst in einem äußeren Job mit kill-on-close (etwa in manchen Terminals), endet das Spiel trotzdem mit dem Launcher.
- Schließen und Beenden, während ein Start gerade den Spielprozess anlegt (zwischen dem Festschreiben des Starts und dem Zustand „läuft“, der Prozess ist dabei noch angehalten), warten auf das Ergebnis: Läuft das Spiel danach, gilt das Übliche (Schließen versteckt das Fenster im Infobereich, Beenden fragt nach); scheitert der Start, schließt bzw. beendet sich die App. Nicht abzudecken ist ein hartes Ende des Launchers genau in diesem Moment (Absturz, Task-Manager): Dann kann der angehaltene Spielprozess in seinem Job liegen bleiben, nie laufen und die Laufsperre seiner Instanz halten, bis er von Hand beendet wird oder der PC neu startet.
- Desktop-App und CLI sperren `accounts.json` nicht gegeneinander. Melden sich beide gleichzeitig an oder ab (oder erneuert die eine gerade eine Anmeldung), kann der Schreibvorgang der einen den der anderen überschreiben. Bekannte Grenze; eine Dateisperre über Prozesse hinweg folgt in Phase 5.
- Spiel aus einer früheren Sitzung: Stürzt der Launcher ab oder wird er beendet, läuft das Spiel weiter. Es hält die Run-Lock-Datei seiner Instanz (`<Datenordner>/tmp/run-<Instanz>.lock`): Der Launcher öffnet sie ohne Freigabe für andere und gibt das Handle an den Java-Prozess weiter, bevor dieser seinen ersten Befehl ausführt; sein eigenes Handle schließt er. Windows schließt das Handle des Spiels erst, wenn der Java-Prozess endet. Solange die Datei so geöffnet ist, lehnen Starten und Löschen dieser Instanz ab („Läuft gerade“), auch in einer neu gestarteten Launcher-Sitzung. Eine liegengebliebene Datei (etwa nach einem Neustart des PCs) sperrt nichts: Es zählt nur, ob sie geöffnet ist. Kindprozesse des Spiels erben das Handle nicht und halten die Sperre nicht.
- Auch `hengst launch` (CLI) nimmt die Run-Lock-Datei vor dem Start und gibt sie direkt nach dem Start an den Java-Prozess weiter; bis dahin hält sie die CLI selbst. Läuft ein Spiel der Instanz, bricht die CLI mit „the game of instance … is running; close it first“ ab.
- Weiterhin nicht abgedeckt: Die neue Sitzung zeigt ein solches Spiel nicht als laufend an (Startseite und Instanzliste zeigen „Bereit“, erst der Klick auf Spielen oder Löschen meldet es), und sie kann es weder beenden noch sein Log zeigen. Zeigt ein von Hand gesetzter Java-Pfad auf ein Startprogramm, das Java neu startet und sich selbst beendet, endet die Sperre mit diesem Startprogramm und damit zu früh: Das eigentliche Spiel läuft dann ohne Sperre.

## 9. Recht und Plattformregeln

- **Microsoft/Mojang:** Neue Azure-Apps brauchen die Freigabe für die Minecraft-API, sonst 403. Antrag dauert Tage bis Wochen, daher sofort stellen.
- **Markenrecht:** "Minecraft" darf nicht im Namen stehen. Hinweis "Not an official Minecraft product" in App und README.
- **CurseForge:** API-Antrag bei Overwolf; Bedingungen (Weitergabesperre achten, keine Massendownloads, Quelle nennen) einhalten.
- **Modrinth:** offene API, eindeutiger User-Agent mit Kontakt, Rate-Limits beachten.
- **OptiFine:** darf nicht weiterverteilt werden, nur Hilfe beim manuellen Einbinden.
- **Lizenz:** GPL-3.0 (Empfehlung, hält Forks offen) oder MIT.
- **Website/Impressum:** sobald es eine Projektseite gibt, Impressum und Datenschutz nach österreichischem Recht (legal-at).

## 10. Qualität und Tests

- Rust: Unit-Tests pro Crate, Integrationstests mit aufgezeichneten API-Antworten (keine Live-Calls in CI).
- Launch-Matrix in CI (Windows-Runner, headless mit Mesa/llvmpipe): je eine Version aus jedem Bereich der Tabelle in 4 startet bis Titelbildschirm, Log wird auf Fehler geprüft.
- Oberfläche: Komponententests, Sicht- und Funktionsprüfung mit Agentfenster.
- Benchmark-Läufe vor jedem Release mit Ergebnistabelle im Release-Text.
- Das Testkit `hengst-testkit` und sein Programm `hengst-mock` sind reine Testwerkzeuge und gehören nie in ein Release: Release-Workflows packen nach ausdrücklichem Artefaktnamen (`hengst`, `hengst-desktop`), nie per Platzhalter über `target/release`.
- Code-Review pro PR (code-review-Skill), Security-Review vor v1.0.

## 11. Fahrplan

Stand 20.11.2026: Phase 0 und 1 erledigt (PR #1), Phase 2 (Login) umgesetzt (Microsoft-Login per Device Code, Mehrfachkonten, Refresh, Offline-Sperre im Kern) und gegen gemockte Dienstantworten getestet; eine echte Anmeldung wurde noch nicht geprüft. Sie bleibt offen, bis Mojang die Azure-App freigibt; bis dahin antworten die Minecraft-Dienste mit 403, Client-ID alternativ per HENGST_MS_CLIENT_ID.

Stand 21.11.2026: Phase 3 (Loader) umgesetzt: `hengst launch --loader` und `hengst loaders` für Fabric, Quilt, Legacy Fabric, Forge (ab 1.7.10, beide Installer-Formate) und NeoForge (ab 1.20.1). Echt gestartet auf Windows 11 (Loader meldet sich im Log, Texturen geladen, 15 s am Leben, kein Crash-Report; Offline-Entwicklungskonto, Ton aus): Fabric 1.21.11 und 26.3, Quilt 1.21.11, Legacy Fabric 1.8.9 und 1.12.2, Forge 1.7.10, 1.8.9, 1.12.2 (zwei Installer-Formate), 1.16.5, 1.20.1 und 26.3, NeoForge 1.20.1, 1.21.1 und 26.3. Nicht geprüft: Spielen, Mods, Serverbeitritt, alle übrigen Spielversionen. Forge vor 1.7.10 wird bewusst abgelehnt.

### Stand Phase 4 (28.11.2026)

**Umgesetzt** (Desktop-App mit Tauri 2 und React unter `apps/desktop`, Windows x64): Konten (Microsoft-Anmeldung im Browser mit PKCE oder per Code, mehrere Konten, Wechsel, Abmelden; Entwicklungskonto nur in Entwicklungs-Builds), Instanzen (Anlegen mit und ohne Loader, Liste, Löschen in den Papierkorb), Startseite (Spielen mit Fortschritt, Abbrechen, Beenden, Beenden erzwingen, Hinweis bei unerwartetem Ende), Infobereich (Fenster schließen versteckt es, solange ein Spiel läuft; Beenden fragt nach), Laufsperre gegen einen zweiten Start derselben Instanz, auch über einen Neustart der App und die CLI hinweg, Live-Log (Suche, Stufenfilter, die letzten zwei Läufe, Kopieren, Logdatei öffnen; Token schon im Kern geschwärzt), Einstellungen (Sprache, Design, Arbeitsspeicher, eigenes Java mit Pfadprüfung, Minimieren beim Start, Datenordner), Über-Bereich mit Drittlizenzen. Dazu `hengst launch --resolve-only` und eine CI mit Frontend-Tests und Debug-Build der App.

**Wie geprüft:**

- Automatisch: Rust-Tests des Workspace (870 mit allen Features, Dienste gemockt, `fake-java` statt Java), 880 Frontend-Tests (Vitest), Clippy und Formatprüfung. Das sind Tests gegen Mocks, keine Prüfung am echten Fenster.
- E2E über WebDriver gegen die echte App mit `hengst-mock`: die fünf Spec-Dateien der Phase 4a (16 Tests, Konten, Anmeldung, Instanzen) liefen am 23.11. grün, auf dem Stand vor 4b.
- In der echten App auf dem unsichtbaren Agent-Desktop angesehen: Anmeldeabläufe gegen den Mock, Instanzen anlegen, Startseite in ihren Ruhezuständen, Logansicht leer, Öffnen einer Logdatei.
- Echte Spielstarts aus der App (Task 22 Teil 2, 28.11.2026): Debug-Build mit `dev-offline` und echten Diensten, Entwicklungskonto, eigener Datenordner auf D:, Ton aus, alles auf dem Agent-Desktop. Vanilla 1.21.11 (erster Start mit allen Downloads etwa 75 s bis zu den geladenen Texturen), Fabric 1.21.11 (Loader 0.20.1, etwa 60 s) und Forge 1.20.1 (47.4.28, mit Installer-Schritten etwa 2 min) erreichten jeweils das Spielfenster mit geladenen Texturen, ohne Crash-Report; zusammen 1,47 GB Daten. Gesehen: Live-Log mit Zeilen während des Laufs; „Beenden“ schließt das Spiel über die Fensteranfrage (im Spiel-Log „Stopping!“, kein Prozess bleibt), Startseite „Das Spiel wurde beendet.“; ein von außen beendetes Spiel zeigt „Das Spiel wurde unerwartet beendet“ und im Log „Das Spiel wurde mit Code 1 beendet.“; Schließen des Fensters während das Spiel läuft versteckt es (Prozess lebt, kein sichtbares Fenster), nach dem Spielende kommt es zurück; Schließen kurz nach „Spielen“ (Vorbereitung) bricht ab und beendet die App ohne Spiel; Laufsperre: `hengst launch` (echt und mit `--dry-run`) lehnt ab, solange das Spiel aus der App läuft („the game of instance 1.21.11 is running; close it first“), nach dem Beenden nur des Launcher-Prozesses läuft das Spiel weiter, eine neue App-Sitzung meldet „Läuft gerade / Beende zuerst das Spiel.“, nach dem Spielende startet die Instanz wieder; es blieben keine WebView2-Prozesse zurück. Eigenes Java über den Windows-Dateidialog gewählt (die heruntergeladene Laufzeit, angezeigt als „Java 21.0.7“); ein gespeicherter Pfad zu einer gelöschten Datei: „Spielen“ meldet sofort „Java lässt sich nicht verwenden“, „Einstellungen öffnen“ zeigt „Erweitert“ schon offen mit „Diese Datei gibt es nicht mehr.“ und „Zurücksetzen“. Darstellung bei 1366 × 768 und in Mindestgröße 960 × 600 (100 % Skalierung des Agent-Desktops), Deutsch und Englisch: Startseite, Instanzliste, Anlegen-Dialog, Logansicht (auch mit einem Instanznamen aus 64 Zeichen), Einstellungen ohne abgeschnittene Texte oder überlappende Bedienelemente.
- Schwärzung an echter Ausgabe nur eingeschränkt geprüft: Das Entwicklungskonto hat das Token „0“, es gibt also kein echtes Geheimnis im Log; Forge maskiert das Token in seinen Argumentzeilen selbst. Die Schwärzung eines echten Microsoft-Tokens kennen weiter nur die Tests.
- E2E: Die neuen Specs aus Task 22 (Spielen mit `fake-java`, Live-Log unter Last, Einstellungen mit Neustart, Infobereich und Beenden, Laufsperre nach einem beendeten Launcher, Java auf einer Netzwerkfreigabe) sind geschrieben und typgeprüft, aber **noch nicht gelaufen**: agentfenster startet keine Skripte (`.ps1`) auf dem Agent-Desktop und kann in eine Konsole dort nicht tippen, und auf dem Desktop des Inhabers darf die Suite nicht laufen. Von den drei Wächterläufen lief nur der mit belegtem Port 4444 (Exit 1, „something already listens on port 4444“, kein Fenster, kein Treiber).

**Offen:**

- Rest von Task 22 Teil 2: die ganze E2E-Suite mit dem normalen und dem erzwungenen Wächterlauf (braucht einen Weg, die Suite auf dem Agent-Desktop zu starten, oder den freien Desktop des Inhabers); Infobereich-Symbol und Beenden-Dialog (auf dem Agent-Desktop gibt es keine Taskleiste, also kein Symbol und kein Menü); Schließen genau im Moment des Festschreibens eines Starts (Zeitfenster zu kurz für einen Handgriff); Darstellung bei 125 % Skalierung, Zoom 150 % und unter 880 CSS-Pixeln (Zoom-Tasten erreichen die WebView2 über agentfenster nicht, die Mindestgröße 960 × 600 liegt über der Schmal-Grenze); Tastensperre (F5, Strg+R, F12 …) im Release-Build; Titelleiste im Systemdesign; Schwärzung mit einem echten Microsoft-Token.
- Beim Projektinhaber: Redirect-URI `http://localhost` unter "Mobile and desktop applications" in der Azure-App eintragen; Mojang-Freigabe der App-ID (bis dahin ist keine echte Anmeldung geprüft); endgültiges Logo statt des Platzhalters `apps/desktop/brand/mark.svg`; Code-Signing (nicht in Phase 4, ungesignierte Builds lösen SmartScreen-Warnungen aus, siehe 12); Entscheidung über Push und Merge der Phase-Branches; Papierkorb-Sonderfälle (Netzlaufwerk, FAT32, Richtlinien) an Testvolumes.

| Phase | Inhalt | Ergebnis |
|---|---|---|
| 0 Vorbereitung | Name, Repo, Lizenz, Azure-Antrag, CurseForge-Antrag, CI-Gerüst | Leeres Repo mit Build und Test-Pipeline |
| 1 Kern | Versionsliste, Downloads, Java-Verwaltung, Vanilla-Start aller Bereiche (zuerst Offline-Test intern mit Dev-Token) | CLI startet jede Version |
| 2 Login | Microsoft-Login mit freigegebener App, Mehrfachkonten | Start mit echtem Konto |
| 3 Loader | Fabric, Quilt, NeoForge, Forge, Legacy Fabric | Modded-Start |
| 4 Oberfläche MVP | Startseite, Instanzen, Log, Einstellungen | **v0.1 MVP** (Windows) |
| 5 Inhalte | Modrinth- und CurseForge-Suche, Install, Updates, .mrpack | v0.2 |
| 6 FPS | Profile, JVM-Tuning, Benchmark | v0.3 mit Messwerten |
| 7 In-Game | Begleitmod M0 bis M8 (docs/begleitmod-spec.md), Auslieferung über den Launcher | v0.4 |
| 8 Ausbau | Server, Crash-Analyse, Backups, Skins, Import, Deep Links, i18n | v0.9 Beta |
| 9 Release | Linux/macOS, Signatur, Auto-Update, Website, Security-Review | **v1.0** |

Phasen 1 und 5 können parallel laufen (unabhängige Crates), ebenso 6 und 7.

## 12. Risiken

| Risiko | Folge | Gegenmittel |
|---|---|---|
| Azure-Freigabe dauert oder wird abgelehnt | Kein Login für Nutzer | Sofort beantragen; Entwicklung läuft mit eigenem Test-Token weiter |
| CurseForge-Key abgelehnt | Nur Modrinth | Modrinth-first bauen, CF als Anbieter-Modul nachrüsten |
| Forge-Installer ändern sich | Modded-Start bricht | Installer-Prozessoren generisch ausführen wie Prism, Tests pro Version |
| Performance-Mods hinken neuen Versionen hinterher | Weniger FPS direkt nach Updates | Profil fällt automatisch auf "was verfügbar ist" zurück, Hinweis in der UI |
| Umfang zu groß | Nie fertig | MVP strikt, Rest nach Priorität |
| Code-Signing fehlt | SmartScreen-Warnung | Später SignPath (gratis für OSS) beantragen |

## 13. Offene Entscheidungen

1. Name und GitHub-Ort: `clemensjl`, öffentlich. Name "Hengst Client" (31.10., "Swift Launcher" wegen Namenskollision verworfen)
2. Entschieden 31.10.: Eigenbau mit Tauri 2 und Rust
3. Entschieden 20.11.2026 (ersetzt 31.10.): eigene Begleitmod statt AxolotlClient, siehe docs/begleitmod-spec.md
4. Entschieden 31.10.: GPL-3.0
5. ~~Plattformen im MVP~~ Entschieden 31.10.: nur Windows im MVP, Linux/macOS zu v1.0
6. Entschieden 31.10.: Cosmetics mit eigenem Backend ja. Kommt nach v0.4 als eigene Phase (Cape/Hut-Server, Darstellung über Begleitmod, nur für Nutzer des Launchers sichtbar, kein Pay-to-win)
7. Entschieden 31.10.: vorerst nur Clemens
