package io.github.clemensjl.hengst.mod.common.platform;

import java.nio.file.Path;

public interface Platform {
    GameVersion version();

    GameView game();

    InputView input();

    /** The config folder inside the game directory. */
    Path configDir();

    /** Current game language, for example "de_de" (1.8.9 reports "de_DE"). */
    String languageCode();

    void logWarning(String message);

    /** Replaces the current screen. */
    void openScreen(UiScreen screen);

    /** Closes only the mod's own screen, and only if that screen is still the current one; otherwise does nothing. */
    void closeScreen();
}
