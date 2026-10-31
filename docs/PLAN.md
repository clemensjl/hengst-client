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

Zwei Wege:

- **A: AxolotlClient bündeln** (offen, LGPL, Fabric/Legacy Fabric, deckt 1.8.9 bis aktuell mit HUD-System ab). Schnell, aber fremde Roadmap.
- **B: Eigene Begleitmod** (Fabric, Multi-Version per Stonecutter). Volle Kontrolle, deutlich mehr Arbeit, Pflege für jede neue Version.

Empfehlung: Start mit A, eigene Mod nur für Dinge, die A nicht kann (Benchmark-Skript, Launcher-Anbindung). Server-Anti-Cheats (Hypixel etc.) erlauben HUD-Mods dieser Art; keine Funktion darf Spielvorteile über Vanilla-Info hinaus geben.

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
- Code-Review pro PR (code-review-Skill), Security-Review vor v1.0.

## 11. Fahrplan

| Phase | Inhalt | Ergebnis |
|---|---|---|
| 0 Vorbereitung | Name, Repo, Lizenz, Azure-Antrag, CurseForge-Antrag, CI-Gerüst | Leeres Repo mit Build und Test-Pipeline |
| 1 Kern | Versionsliste, Downloads, Java-Verwaltung, Vanilla-Start aller Bereiche (zuerst Offline-Test intern mit Dev-Token) | CLI startet jede Version |
| 2 Login | Microsoft-Login mit freigegebener App, Mehrfachkonten | Start mit echtem Konto |
| 3 Loader | Fabric, Quilt, NeoForge, Forge, Legacy Fabric | Modded-Start |
| 4 Oberfläche MVP | Startseite, Instanzen, Log, Einstellungen | **v0.1 MVP** (Windows) |
| 5 Inhalte | Modrinth- und CurseForge-Suche, Install, Updates, .mrpack | v0.2 |
| 6 FPS | Profile, JVM-Tuning, Benchmark | v0.3 mit Messwerten |
| 7 In-Game | AxolotlClient-Bündel, Begleitmod-Grundlage | v0.4 |
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
3. Entschieden 31.10.: AxolotlClient bündeln, eigene Begleitmod nur für Lücken (Benchmark, Launcher-Anbindung, Cosmetics)
4. Entschieden 31.10.: GPL-3.0
5. ~~Plattformen im MVP~~ Entschieden 31.10.: nur Windows im MVP, Linux/macOS zu v1.0
6. Entschieden 31.10.: Cosmetics mit eigenem Backend ja. Kommt nach v0.4 als eigene Phase (Cape/Hut-Server, Darstellung über Begleitmod, nur für Nutzer des Launchers sichtbar, kein Pay-to-win)
7. Entschieden 31.10.: vorerst nur Clemens
