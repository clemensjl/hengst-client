package io.github.clemensjl.hengst.mod.common.platform;

import java.nio.file.Path;

public interface Platform {
    GameVersion version();

    /** A long-lived object; must not allocate per call. */
    GameView game();

    /** A long-lived object; must not allocate per call. */
    InputView input();

    /** The config folder inside the game directory. */
    Path configDir();

    /**
     * Current game language, for example "de_de" (1.8.9 reports "de_DE"); null while the game has not loaded its
     * language yet (the core treats that as not read and reads again in the tick). Throw only on a real failure.
     */
    String languageCode();

    void logWarning(String message);

    /**
     * Replaces the current screen. Takes effect synchronously: when this returns, the screen is current (the core's
     * tick fallback relies on that).
     */
    void openScreen(UiScreen screen);

    /**
     * Closes only the mod's own screen, and only if that screen is still the current one; otherwise does nothing.
     * The adapter marks its host screen as removed BEFORE it forwards {@code closed()} to the {@link UiScreen}, so
     * this does nothing for a host that is removed or no longer current and never calls the game's set-screen
     * re-entrantly.
     */
    void closeScreen();
}
