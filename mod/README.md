# Hengst companion mod

Fabric client mod for Minecraft 1.8.9 (Legacy Fabric), 1.21.11 and 26.3. Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft.

## Layout

- `common/`: plain Java 8 library, no game dependency, embedded into every mod jar (jar-in-jar).
- `modern/`: Fabric, one source tree for 1.21.11 and 26.3 via Stonecutter (`versions/<mc>/gradle.properties`).
- `legacy/`: Legacy Fabric for 1.8.9.
- This directory is a Gradle composite build that includes all three.

## Build

Requires JDK 25 as the Gradle JVM (`JAVA_HOME`). From this directory:

```
./gradlew :common:test      # unit tests of the common module
./gradlew buildAll          # all three jars into dist/
```

Single targets: `./gradlew :modern:1.21.11:build`, `./gradlew :modern:26.3:build`, `./gradlew :legacy:build`.

In-game test of the modern mod (starts the game in a window, not part of `build` or `buildAll`): `./gradlew -p modern :<node>:clientGameTests` with `<node>` `1.21.11` or `26.3`. It runs `runClientGameTest` (fresh run directory) and then `runClientGameTestRestart` (same directory, checks the saved layout); screenshots land in `modern/versions/<node>/build/run/clientGameTest/screenshots/`.

In-game test of the 1.8.9 mod: `./gradlew -p legacy gameTests` runs `runGameTest` (fresh run directory `legacy/build/run/gameTest`) and then `runGameTestRestart` (same directory, checks the saved layout). 1.8.9 has no client gametest API: a dev-only second mod (`legacy/src/gametest`, never in the jar) creates a flat world, feeds input events into LWJGL's queues and asserts on framebuffer screenshots in `legacy/build/run/gameTest/screenshots/`; a failed check exits the game with code 1. The game window opens on the desktop but never grabs the system cursor.

Production check (the built jar with Fabric API in Loom's production launch, intermediary names on 1.21.11): `./gradlew -p modern :<node>:runProductionClient -PwithGameTest` runs the same in-game test against the shipped jar; without `-PwithGameTest` it only starts the game.

Result: `dist/hengst-mod-0.1.0+1.8.9.jar`, `dist/hengst-mod-0.1.0+1.21.11.jar`, `dist/hengst-mod-0.1.0+26.3.jar`.

Unit tests of the adapters: `./gradlew -p legacy test`, `./gradlew -p modern :1.21.11:test`, `./gradlew -p modern :26.3:test`.

The Gradle wrapper must stay on Gradle 9.x: the legacy build's tooling (Legacy Looming) uses features Gradle reports as deprecated and incompatible with Gradle 10. javac's note that `--release 8` is obsolete is silenced on purpose (`-Xlint:-options` in `legacy/build.gradle`); Java 8 bytecode is the point of that build.

## Install into an instance

Put the jar for the game version into the instance's `mods/` folder together with the API mod it depends on:

| Jar | Loader | API mod |
|---|---|---|
| `hengst-mod-0.1.0+1.8.9.jar` | Legacy Fabric, loader 0.20.1 or newer, Java 8 or newer | Legacy Fabric API 1.13.5 or newer |
| `hengst-mod-0.1.0+1.21.11.jar` | Fabric, loader 0.20.1 or newer, Java 21 or newer | Fabric API 0.141.6+1.21.11 or newer |
| `hengst-mod-0.1.0+26.3.jar` | Fabric, loader 0.20.1 or newer, Java 25 or newer | Fabric API 0.166.2+26.3 or newer |

Use the Legacy Fabric API jar from Modrinth (it bundles the modules), not the `legacy-fabric-api` artifact from the Legacy Fabric Maven, which is an empty aggregator without them.

With the Hengst launcher: `hengst launch <version> --loader fabric` (or `legacy-fabric` for 1.8.9) creates the instance under `<home>/instances/<version>-<loader>/`; put the two jars into its `mods/` folder and start it again. Right Shift opens the settings; the HUD editor opens from the settings' Edit HUD button (its own key is unbound by default). The layout is saved in the instance's `config/hengst/profiles/default.json`.

## What was verified (M0, 2026-11-21)

The three `dist/` jars, installed as above into instances started by the Hengst launcher (release build with the `dev-offline` feature, offline account, loader 0.20.1 picked by the launcher; 1.8.9 on the launcher's Java 8 runtime `jre-legacy` 1.8.0_51, 1.21.11 on Java 21.0.7, 26.3 on Java 25.0.1, each the runtime the launcher selected):

- All three: the loader lists `hengst` with the nested common jar and the API mod; the log has `[hengst] loaded on <version>`; with `-Dmixin.debug.verbose=true` the log shows every mod mixin applied (1.8.9 and 1.21.11 to intermediary classes, for example `net.minecraft.class_371` and `net.minecraft.class_312`); no mixin warning or exception names the mod; the game closes cleanly with the window's close command.
- 1.8.9 and 1.21.11, driven by window messages and checked on the game's own F2 screenshots: the four modules in a world; CPS rises with clicks and falls to 0; W lights its key and the coordinates change; F1 hides and shows; F3 hides the mod HUD; Right Shift opens the settings with the legal notice; a module switched off and Done removes it; the HUD editor dims the screen and centres its hint; a dragged module stays where it was put after a restart. 1.8.9 also: a module over a filled hotbar slot stays visible, the German language shows the German key-binding names, closing the window while the HUD editor is open keeps the dragged layout.
- 26.3: SDL3 ignores posted keys without keyboard focus and the window cannot be captured, so the mod's own in-game test (`src/gametest`, pixel checks on framebuffer screenshots) was run inside the launcher-started game with the shipped jar, plus the test jar and Fabric's client test API module (`JAVA_TOOL_OPTIONS=-Dfabric.client.gametest`): all steps of both phases pass. Eight plain starts (seven with the mod, one without) reached the title screen and stopped cleanly; none crashed.
- A damaged profile file is moved aside (`default.json.broken-<time>`) with one warning and the default HUD is drawn (1.21.11 seen, 26.3 from the log).
- FPS cost, measured with vanilla's own counters (method and numbers in the Task 14 report): no cost resolved with all modules off; with the four modules on about 4 % on 26.3 and 7 % on 1.8.9 at 1500 to 2300 FPS, below the method's noise on 1.21.11.

Not verified: multiplayer (player list over the HUD, the server address, disconnecting while a mod screen is open); on 26.3 switching a module off in the settings (the in-game test does not do it, and posted keys, the settings key included, do not reach the game); scrolling the settings list with a real wheel notch in a launcher-started game (four rows do not overflow at this window size; the in-game tests cover it).
