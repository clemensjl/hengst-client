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

Production check (the built jar with Fabric API in Loom's production launch, intermediary names on 1.21.11): `./gradlew -p modern :<node>:runProductionClient -PwithGameTest` runs the same in-game test against the shipped jar; without `-PwithGameTest` it only starts the game.

Result: `dist/hengst-mod-0.1.0+1.8.9.jar`, `dist/hengst-mod-0.1.0+1.21.11.jar`, `dist/hengst-mod-0.1.0+26.3.jar`.
