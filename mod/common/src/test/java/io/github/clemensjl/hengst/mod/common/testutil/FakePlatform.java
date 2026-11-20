package io.github.clemensjl.hengst.mod.common.testutil;

import io.github.clemensjl.hengst.mod.common.platform.GameKey;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.platform.GameView;
import io.github.clemensjl.hengst.mod.common.platform.InputView;
import io.github.clemensjl.hengst.mod.common.platform.Platform;
import io.github.clemensjl.hengst.mod.common.platform.UiScreen;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.EnumMap;
import java.util.EnumSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

public final class FakePlatform implements Platform, GameView, InputView {
    public GameVersion version = GameVersion.V1_8_9;
    public boolean inWorld;
    public boolean hudHidden;
    public boolean debugOverlayOpen;
    public boolean screenOpen;
    public double playerX;
    public double playerY;
    public double playerZ;
    public float playerYaw;
    public String serverAddress;
    public String languageCode = "en_us";
    public Set<GameKey> down = EnumSet.noneOf(GameKey.class);
    public Map<GameKey, String> labels = new EnumMap<GameKey, String>(GameKey.class);
    public List<String> warnings = new ArrayList<String>();
    public UiScreen openedScreen;
    public int closeCount;
    public Path configDir;

    public FakePlatform(Path configDir) {
        this.configDir = configDir;
    }

    @Override
    public GameVersion version() {
        return version;
    }

    @Override
    public GameView game() {
        return this;
    }

    @Override
    public InputView input() {
        return this;
    }

    @Override
    public Path configDir() {
        return configDir;
    }

    @Override
    public String languageCode() {
        return languageCode;
    }

    @Override
    public void logWarning(String message) {
        warnings.add(message);
    }

    @Override
    public void openScreen(UiScreen screen) {
        openedScreen = screen;
        screenOpen = true;
    }

    @Override
    public void closeScreen() {
        openedScreen = null;
        screenOpen = false;
        closeCount++;
    }

    @Override
    public boolean inWorld() {
        return inWorld;
    }

    @Override
    public double playerX() {
        return playerX;
    }

    @Override
    public double playerY() {
        return playerY;
    }

    @Override
    public double playerZ() {
        return playerZ;
    }

    @Override
    public float playerYaw() {
        return playerYaw;
    }

    @Override
    public boolean hudHidden() {
        return hudHidden;
    }

    @Override
    public boolean debugOverlayOpen() {
        return debugOverlayOpen;
    }

    @Override
    public boolean screenOpen() {
        return screenOpen;
    }

    @Override
    public String serverAddress() {
        return serverAddress;
    }

    @Override
    public boolean isDown(GameKey key) {
        return down.contains(key);
    }

    @Override
    public String keyLabel(GameKey key) {
        return labels.containsKey(key) ? labels.get(key) : key.name();
    }
}
