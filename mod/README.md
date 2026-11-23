# Hengst companion mod

Fabric client mod for Minecraft 1.8.9 (Legacy Fabric), 1.21.11 and 26.3. Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft.

## Layout

- `common/`: plain Java 8 library, no game dependency, embedded into every mod jar (jar-in-jar).
- `modern/`: Fabric, one source tree for 1.21.11 and 26.3 via Stonecutter (`versions/<mc>/gradle.properties`).
- `legacy/`: Legacy Fabric for 1.8.9.
- This directory is a Gradle composite build that includes all three.

Inside `common/src/main/java/io/github/clemensjl/hengst/mod/common/`:

- `HengstCore`: the one object the adapters talk to (hooks for frame, tick, HUD, mouse, keys, shutdown). `FeatureCatalog` lists every feature in number order and the key bindings per game version. `AdapterGuard` is the fault isolation the adapters share. `I18n` loads the language files.
- `feature/`: `Feature` (id, number, group, supported versions, options, keys, hint), `OptionSpec`, `KeySpec`, `DefaultKey`, `FeatureGate`, `FeatureState` (`FeatureState.on(number)`, what mixins of later features read first), `RestrictionPolicy`.
- `hud/`: `HudRenderer`, `HudModule`, `TextHudModule` (one line of cached text, fitted to the screen width), `HudContext`, the counters, and `hud/modules/` with one class per HUD module.
- `config/`: settings and profile files (JSON, atomic writes, forward-compatible); `json/`: the small JSON reader and writer (no Gson).
- `ui/` and `editor/`: the settings screen and the HUD editor, drawn through `Canvas`; `ui/TextFit` is the one place that cuts text to a pixel width.
- `platform/`: the interfaces an adapter implements (`Platform`, `GameView`, `InputView`, `Canvas`, `WallClock`, `TimeSource`) and `ScrollAccumulator`.
- Language files: `common/src/main/resources/hengst/lang/en_us.json` and `de_de.json` (all UI texts). The names of the key bindings are the exception: the game translates them itself, so they live in the adapters' language files (`modern/src/main/resources/assets/hengst/lang/*.json`, `legacy/src/main/resources/assets/hengst/lang/*.lang`).

The specification is [../docs/begleitmod-spec.md](../docs/begleitmod-spec.md) (German); section 4.3 lists the interfaces, section 7 the features, section 8 the runtime rules.

## Build

Requires JDK 25 as the Gradle JVM (`JAVA_HOME`). From this directory:

```
./gradlew -p common test    # unit tests of the common module, then the interpreted allocation tests
./gradlew buildAll          # unit tests of common, then all three jars into dist/
```

`test` of `common` finishes with `interpretedTest`: the tests tagged `interpreted` (allocation tests, they must count exact bytes) run in a second JVM with `-Xint`. Run a single class while iterating with `./gradlew -p common test --tests '<fully qualified class>'`. Only one Gradle build at a time, never next to a running game (`org.gradle.workers.max=1` is set on purpose); stop the daemon when done with `./gradlew --stop`.

Single targets: `./gradlew :modern:1.21.11:build`, `./gradlew :modern:26.3:build`, `./gradlew :legacy:build`.

In-game test of the modern mod (starts the game in a window, not part of `build` or `buildAll`): `./gradlew -p modern :<node>:clientGameTests` with `<node>` `1.21.11` or `26.3`. It runs `runClientGameTest` (fresh run directory) and then `runClientGameTestRestart` (same directory, checks the saved layout); screenshots land in `modern/versions/<node>/build/run/clientGameTest/screenshots/`.

In-game test of the 1.8.9 mod: `./gradlew -p legacy gameTests` runs `runGameTest` (fresh run directory `legacy/build/run/gameTest`) and then `runGameTestRestart` (same directory, checks the saved layout). 1.8.9 has no client gametest API: a dev-only second mod (`legacy/src/gametest`, never in the jar) creates a flat world, feeds input events into LWJGL's queues and asserts on framebuffer screenshots in `legacy/build/run/gameTest/screenshots/`; a failed check exits the game with code 1. The game window opens on the desktop but never grabs the system cursor.

The three in-game suites are `-p modern :26.3:clientGameTests`, `-p modern :1.21.11:clientGameTests` and `-p legacy gameTests`. Run them one at a time: each starts a game window, and two games or a game next to a Gradle build compete for memory. Each suite runs the seven M1a modules (and the M0 modules) one by one in a real world and checks the core's text and the framebuffer pixels (for ping and server address only that nothing is drawn in single player), and it measures the frame cost per module (spec 8.8).

Production check (the built jar with Fabric API in Loom's production launch, intermediary names on 1.21.11): `./gradlew -p modern :<node>:runProductionClient -PwithGameTest` runs the same in-game test against the shipped jar; without `-PwithGameTest` it only starts the game.

Result: `dist/hengst-mod-0.1.0+1.8.9.jar`, `dist/hengst-mod-0.1.0+1.21.11.jar`, `dist/hengst-mod-0.1.0+26.3.jar`.

Unit tests of the adapters: `./gradlew -p legacy test`, `./gradlew -p modern :1.21.11:test`, `./gradlew -p modern :26.3:test`.

`buildAll` also scans the three jars in `dist/` (nested jars included) and fails if any entry belongs to a test: a path containing `gametest` or `testutil`, or a class named `Test...` or `...Test`. The in-game test code under `src/gametest` and the test helpers must never ship.

Known Windows nuisance: `buildAll` or a test task can fail with "Unable to delete directory ... test-results ... binary". Delete the empty `binary` directories under the affected `build/test-results` and run the task again; it is not a code error.

The Gradle wrapper must stay on Gradle 9.x: the legacy build's tooling (Legacy Looming) uses features Gradle reports as deprecated and incompatible with Gradle 10. javac's note that `--release 8` is obsolete is silenced on purpose (`-Xlint:-options` in `legacy/build.gradle`); Java 8 bytecode is the point of that build.

## Adding a HUD module

The rules behind this list are in the specification, section 8 (no allocation per frame, nothing disabled costs anything). Use `ClockModule` or `PingModule` as a model.

1. Subclass `TextHudModule` in `common/.../hud/modules/` (id, number from spec section 7, supported versions; the constructor with a `FeatureGroup` for another group than HUD). Persisted names (id, option keys) are fixed once released: a profile file stores them.
2. List it in `FeatureCatalog.create()`, in number order. That one entry registers the feature, its settings row, its default profile entry (off) and its keys.
3. Declare options as `OptionSpec` constants (`toggle`, `intRange`, `floatRange`, `choice`) and return them from `options()`. Read them in `tick` with the typed accessors `cfg.bool`, `cfg.integer`, `cfg.decimal`, `cfg.choice` and keep the values in primitive fields. Never read options in `render`, `width` or `height`.
4. Build the text in `tick`, into a reused `StringBuilder`, and call `setText` only when a shown value changed: one String per change, nothing otherwise. `render` is inherited and draws the cached text.
5. State that must not survive a switch-off, a profile switch or a server change (positions, averages, samples) is reset in `onConfigChanged(ModuleConfig)`: the core calls it for every supported feature at start, after the settings or the HUD editor close, on a profile switch and on a server change; a module is not ticked while it is off. Anything cached from a text measurement is dropped in `onLanguageChanged()` (the core calls it after a language change; `TextHudModule` already drops its own widths). That hook only drops measurements: a module that builds its texts from the language file must also compare `ctx.languageGeneration()` in `tick` and rebuild its texts when it changed, as `DirectionModule` and `ClockModule` do.
6. Set the default place with `defaultAnchor()`, `defaultOffsetX()`, `defaultOffsetY()` (the first start creates the entry there, switched off). Extend `default_places_of_the_eleven_modules_do_not_overlap_at_320_by_240` in `ModulesTest` with the new module and its longest text.
7. Add an allocation test to the `@Tag("interpreted")` classes (`InfoModulesAllocationTest`): exactly 0 bytes in frames and ticks while the text does not change, and for a change a bound that one String of the module's text fits under and a second String would break. Add the usual behaviour tests next to `ModulesTest` and the module tests in `hud/modules/`.
8. Language keys, in both `en_us.json` and `de_de.json` (German with real umlauts; the test `both_language_files_have_the_same_keys` fails if the two files do not have the same keys, it does not check the umlauts): `hengst.feature.<id>` (the row name), `hengst.option.<id>.<option>` for each option (fallback `hengst.option.<option>`), `hengst.choice.<id>.<value>` for each choice value, and any texts the module shows through `I18n`.
9. Add a step for the module to the in-game suites (`modern/src/gametest`, `legacy/src/gametest`) and run all three, one at a time.

Keys and hints: a feature that needs key bindings returns them from `keys()` as `KeySpec(id, DefaultKey, Mode)` (`PRESS` gets only the press, `HOLD` also the release) and handles `onKey(int index, boolean down, long nowNanos)`; the index is the position in that list. Nothing else is registered by hand: `FeatureCatalog.keys(version)` lists the keys per game version and both adapters register that list. Each key needs its name `key.hengst.<id>` in the adapters' language files (modern `.json`, legacy `.lang`, English and German; `ModernKeysTest` and `LegacyKeysTest` check it), and a `DefaultKey` value other than `NONE` or `RIGHT_SHIFT` needs a mapping in `ModernKeys.code` and `LegacyKeys`. The core delivers a press only while the feature is on, in a world and without an open screen. Keys that ship unbound should say where to bind them: return a language key from `hintKey()` (`hengst.hint.<id>`, in both common language files); the settings show it above the options.

A feature that is not a HUD module and wants to switch a mixin on and off reads `FeatureState.on(<number>)` as the first statement of the mixin.

## Install into an instance

Put the jar for the game version into the instance's `mods/` folder together with the API mod it depends on:

| Jar | Loader | API mod |
|---|---|---|
| `hengst-mod-0.1.0+1.8.9.jar` | Legacy Fabric, loader 0.20.1 or newer, Java 8 or newer | Legacy Fabric API 1.13.5 or newer |
| `hengst-mod-0.1.0+1.21.11.jar` | Fabric, loader 0.20.1 or newer, Java 21 or newer | Fabric API 0.141.6+1.21.11 or newer |
| `hengst-mod-0.1.0+26.3.jar` | Fabric, loader 0.20.1 or newer, Java 25 or newer | Fabric API 0.166.2+26.3 or newer |

Use the Legacy Fabric API jar from Modrinth (it bundles the modules), not the `legacy-fabric-api` artifact from the Legacy Fabric Maven, which is an empty aggregator without them.

With the Hengst launcher: `hengst launch <version> --loader fabric` (or `legacy-fabric` for 1.8.9) creates the instance under `<home>/instances/<version>-<loader>/`; put the two jars into its `mods/` folder and start it again. Right Shift opens the settings; the HUD editor opens from the settings' Edit HUD button (its own key is unbound by default). The layout is saved in the instance's `config/hengst/profiles/default.json`.

Both keys open their screen only in a world with no other screen open. The stopwatch has two key bindings, "Stopwatch start/stop" and "Stopwatch reset", both unbound by default: bind them under Options > Controls in the category "Hengst" (the stopwatch's options in the settings say so). Do not put both on the same physical key: on 1.21.11 and 26.3 the stopwatch then toggles and resets in the same tick and ends stopped at zero, and on 1.8.9 only one of the two bindings receives the key.

HUD modules added in M1a, all off at first start (enable them in the settings): Direction, Ping, Clock, Server address, Memory, Speed, Stopwatch (the stopwatch sits in the group Comfort, the others in HUD). Ping and Server address show something only on a multiplayer server. Coordinates, Direction and Speed show nothing while the server asks for reduced debug info (the game rule `reducedDebugInfo`, `reduced_debug_info` from 1.21.11) or the game's own reduced-debug-info option is set: the game's F3 screen hides the absolute position and the facing then (a chunk-relative position stays), and the mod hides Coordinates entirely, as well as Direction and Speed.

## Known limitations

- The game stores key bindings in its own `options.txt`. After a start with the released M0 build the game drops the lines of bindings it does not know (`key_key.hengst.stopwatch.*`), so the stopwatch keys have to be bound again.
- A released M0 build drops unknown root keys of a profile, unknown keys in the entries of its four modules (fps, cps, keystrokes, coordinates) and unknown keys in `settings.json` when it writes them back, and it normalises those four entries. M1a therefore stores new data only as a module entry or inside a module's `options`, and the stopwatch stores nothing (its state lives for the session). The exact rules are in the specification, section 5.
- Ping and Server address were not checked against a real server in the automated in-game tests (in single player they show nothing by design); their cost per frame on a server is not measured.
- A server-set creative fly speed above 100 m/s reads as a teleport to the Speed module and shows 0. At two decimals a speed on a rounding boundary can flicker between two values (sneaking, about 1.295 m/s).
- After a profile or settings change the speed shows 0 for one tick, and ping and server address are blank for one tick.
- The clock reads the system time zone once, when the mod loads; a time zone change of the operating system during a session is not followed (daylight saving changes are).
- A font change through a resource pack without a language change leaves cached text widths stale until the next text or language change (no platform signal exists).
- Proxy networks that switch the backend server behind one address are not detected as a server change. The server address is compared as typed, case-sensitive.
- A feature that a server restriction would switch off while it is enabled in the configuration gets no "disabled" notification yet (a running stopwatch would keep running invisibly); to be defined with the restriction rules in M4.
- On the modern versions the game language is read at the first tick, not at mod initialisation (the language manager is not ready yet); English texts until then.
- A text that is wider than the screen is cut in the middle; the box of a module whose text width varies (Direction) changes its width with the text.

## What was verified (M1a)

Checked on 2026-11-22 and 2026-11-23, one machine (AMD Ryzen 9 5900X, 32 GB, Windows 11 Pro).

In-game suites on the development classes (`clientGameTests` on 1.21.11 and 26.3, `gameTests` on 1.8.9; both phases green on commit 09348d9 on all three versions, 2026-11-23: 26.3 49 + 5 checks, 1.21.11 49 + 5, 1.8.9 54 + 5; functional runs only, the machine was not idle, so their cost numbers are not used): every M0 and M1a module in a real world, checked by the core's text and by framebuffer pixels (for ping and server address only that nothing is drawn in single player), the settings with groups, hint and scrolling, the stopwatch keys, the controls screen with the translated binding names in English and German, a language switch in a running world, Coordinates, Direction and Speed empty while the game rule for reduced debug info is on and back when it is off, the saved layout after a restart, zero allocation in steady-state frames and in the key dispatch (asserted window from 90 s after game start), and the cost per module.

Cost per module per frame (spec 8.8; time the core spends per module, submit time on 1.21.11 and 26.3, including the GL calls on 1.8.9; median of the runs' medians on an otherwise idle machine, in microseconds):

| Version | Runs | Four M0 modules (budget 50) | All eleven (budget 120) | Marginal per new module (budget 10) |
|---|---|---|---|---|
| 1.8.9 | 6 | 19.2 (18.2 to 44.5) | 46.5 (30.0 to 69.5) | 1.6 to 6.0 |
| 1.21.11 | 5 (all eleven: 4) | 23.6 (19.5 to 26.8) | 38.3 (30.7 to 39.0) | 2.1 to 3.4 |
| 26.3 | 6 | 24.6 (22.0 to 25.9) | 35.7 (34.0 to 36.4) | 1.9 to 3.7 |

The budget definitions (marginal cost, median of runs) are rulings that the project owner has not confirmed yet. A cross-check with paired alternating windows (no per-module timer) agrees within about 1 microsecond outside the 1.8.9 slow state and outside the runs under load (1.21.11 run 2's paired windows are not counted). The 1.8.9 all-eleven median of 46.5 lies between two clusters of runs (30.0 to 31.1 normal, 61.8 to 69.5 in the slow state).

The shipped jars from `buildAll` (sizes and SHA-256 in the Task 19 report; each contains `LICENSE_hengst`, the jar scan found no test entry), installed into fresh instances started by the Hengst launcher (`hengst launch <version> --loader fabric|legacy-fabric --offline Dev`, a `dev-offline` build of the CLI, loader 0.20.1, Fabric API 0.141.6 / 0.166.2, Legacy Fabric API 1.13.5; Java 8 on 1.8.9, 21 on 1.21.11, 25 on 26.3 as picked by the launcher; sound muted):

- All three: the loader lists `hengst` with its nested common jar; the log has `[hengst] loaded on <version>`; with `-Dmixin.debug.verbose=true` every mod mixin is applied (1.21.11 and 1.8.9 to intermediary classes); no warning or exception from the mod; the game closes cleanly with the window's close command. `options.txt` has the four key lines `key_key.hengst.settings`, `.hud_editor`, `.stopwatch.toggle`, `.stopwatch.reset` (on 1.8.9 after the options were saved once; the game writes the file only then). The mod writes `config/hengst/profiles/default.json` (keys `schema`, `modules`; no paths, no account data) once something changed; a start without any change writes no config file.
- 1.21.11 and 26.3: the in-game suite above was also run inside the launcher-started game against the shipped jar, with the test jar and Fabric's client game-test module added (`JAVA_TOOL_OPTIONS=-Dfabric.client.gametest`): both phases pass (48 + 5 checks). This is the shipped jar plus a separate test driver jar, not the shipped jar alone.
- 1.21.11 and 1.8.9, the shipped jar alone, driven by window messages and checked on the game's F2 screenshots: a profile written by the released M0 build is loaded with its positions (FPS middle right, CPS on the hotbar on 1.8.9, Coordinates off); switching Direction on in the settings draws it and rewrites the file with the four M0 entries unchanged and the seven new entries added; after a restart Direction is still on and the other six new modules, switched on in the file before the start, draw plausible text (direction, `0.0 m/s`, `0:00.0`, the time, `RAM ...`; ping and server address nothing in single player); the controls screen lists the category Hengst with the four translated binding names. 1.8.9 also: the first start draws exactly the four M0 modules.
- 26.3: SDL3 takes no posted keys and the window cannot be captured, so the M0 profile was checked only by a start: no warning, the file is left unchanged (it is rewritten on the first change, which the suite covers with the current format).

Not verified: multiplayer (ping and server address with a value, a server change, the player list over the HUD): it needs a server, and running the server software means accepting Mojang's EULA, which is the owner's decision. Measurements on other hardware. The cause of a state on 1.8.9 in which the measured cases with modules took about 1.5 to 2.6 times as long (the empty hook not; three of six runs: 1, 5, 6) and of a single 1192-byte allocation in the key dispatch about 67 to 80 s after game start (spec 7.3, 8.8). On 26.3 a real key press or screen capture in a launcher-started game outside the in-game suite.

Also not verified yet:
- The failure path of the guarded hook registration at startup (a renamed API member) is covered by unit tests only; no game was started with a broken API jar. That Speed releases the world reference when switched off is covered by a unit test only.
- The Server address module on Realms: it may show the raw host address of the Realms server (the module is off by default; the address is never logged or stored).
- The 1.8.9 suite against the shipped jar: on the shipped 1.8.9 jar the stopwatch (bind, start, stop, reset), the language switch, and the settings groups and scroll bar at 320 x 240 were not exercised (only on the development classes).
- On 26.3 without the test jar: the rewrite of an M0 profile and the seven modules drawn from a pre-seeded profile.
- The frame-rate comparison without the mod, with the four M0 modules and with all eleven (plan task 19, step 2.7): not yet measured.

## What was verified (M0, 2026-11-21)

The three `dist/` jars, installed as above into instances started by the Hengst launcher (release build with the `dev-offline` feature, offline account, loader 0.20.1 picked by the launcher; 1.8.9 on the launcher's Java 8 runtime `jre-legacy` 1.8.0_51, 1.21.11 on Java 21.0.7, 26.3 on Java 25.0.1, each the runtime the launcher selected):

- All three: the loader lists `hengst` with the nested common jar and the API mod; the log has `[hengst] loaded on <version>`; with `-Dmixin.debug.verbose=true` the log shows every mod mixin applied (1.8.9 and 1.21.11 to intermediary classes, for example `net.minecraft.class_371` and `net.minecraft.class_312`); no mixin warning or exception names the mod; the game closes cleanly with the window's close command.
- 1.8.9 and 1.21.11, driven by window messages and checked on the game's own F2 screenshots: the four modules in a world; CPS rises with clicks and falls to 0; W lights its key and the coordinates change; F1 hides and shows; F3 hides the mod HUD; Right Shift opens the settings with the legal notice; a module switched off and Done removes it; the HUD editor dims the screen and centres its hint; a dragged module stays where it was put after a restart. 1.8.9 also: a module over a filled hotbar slot stays visible, the German language shows the German key-binding names, closing the window while the HUD editor is open keeps the dragged layout.
- 26.3: SDL3 ignores posted keys without keyboard focus and the window cannot be captured, so the mod's own in-game test (`src/gametest`, pixel checks on framebuffer screenshots) was run inside the launcher-started game with the shipped jar, plus the test jar and Fabric's client test API module (`JAVA_TOOL_OPTIONS=-Dfabric.client.gametest`): all steps of both phases pass. Eight plain starts (seven with the mod, one without) reached the title screen and stopped cleanly; none crashed.
- A damaged profile file is moved aside (`default.json.broken-<time>`) with one warning and the default HUD is drawn (1.21.11 seen, 26.3 from the log).
- FPS cost, measured with vanilla's own counters (method and numbers in the Task 14 report): no cost resolved with all modules off; with the four modules on about 4 % on 26.3 and 7 % on 1.8.9 at 1500 to 2300 FPS, below the method's noise on 1.21.11.

Not verified: multiplayer (player list over the HUD, the server address, disconnecting while a mod screen is open); on 26.3 switching a module off in the settings (the in-game test does not do it, and posted keys, the settings key included, do not reach the game); scrolling the settings list with a real wheel notch in a launcher-started game (four rows do not overflow at this window size; the in-game tests cover it).
