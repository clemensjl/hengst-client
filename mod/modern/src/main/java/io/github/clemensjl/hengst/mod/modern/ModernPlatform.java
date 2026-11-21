package io.github.clemensjl.hengst.mod.modern;

import io.github.clemensjl.hengst.mod.common.platform.GameKey;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.platform.GameView;
import io.github.clemensjl.hengst.mod.common.platform.InputView;
import io.github.clemensjl.hengst.mod.common.platform.Platform;
import io.github.clemensjl.hengst.mod.common.platform.UiScreen;
import java.nio.file.Path;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.Options;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.multiplayer.ServerData;
import net.minecraft.client.player.LocalPlayer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * The core's view of the game. Game and input views are created once; every call reads the live game state through
 * {@link Minecraft#getInstance()} (nothing is read in the constructor, which runs during client initialization).
 */
public final class ModernPlatform implements Platform {
    private static final Logger LOGGER = LoggerFactory.getLogger("hengst");

    private final Game game = new Game();
    private final Input input = new Input();

    @Override
    public GameVersion version() {
        // One branch per node: a new node gets no return here and fails to compile until it is mapped.
        //? if =26.3 {
        return GameVersion.V26_3;
        //?} else if =1.21.11 {
        /*return GameVersion.V1_21_11;
        *///?}
    }

    @Override
    public GameView game() {
        return game;
    }

    @Override
    public InputView input() {
        return input;
    }

    @Override
    public Path configDir() {
        return FabricLoader.getInstance().getConfigDir();
    }

    @Override
    public String languageCode() {
        return Minecraft.getInstance().getLanguageManager().getSelected();
    }

    @Override
    public void logWarning(String message) {
        LOGGER.warn(message);
    }

    @Override
    public void openScreen(UiScreen screen) {
        //? if >=26.1 {
        Minecraft.getInstance().gui.setScreen(new ModernScreenHost(screen));
        //?} else {
        /*Minecraft.getInstance().setScreen(new ModernScreenHost(screen));
        *///?}
    }

    /** Only the mod's own host, only while it is current and not being removed (no re-entrant set-screen). */
    @Override
    public void closeScreen() {
        Minecraft mc = Minecraft.getInstance();
        Screen current = currentScreen(mc);
        if (current instanceof ModernScreenHost && !((ModernScreenHost) current).isRemoved()) {
            //? if >=26.1 {
            mc.gui.setScreen(null);
            //?} else {
            /*mc.setScreen(null);
            *///?}
        }
    }

    /** 26.x keeps the current screen on Gui, 1.21.11 on Minecraft. */
    static Screen currentScreen(Minecraft mc) {
        //? if >=26.1 {
        return mc.gui.screen();
        //?} else {
        /*return mc.screen;
        *///?}
    }

    private static final class Game implements GameView {
        @Override
        public boolean inWorld() {
            Minecraft mc = Minecraft.getInstance();
            return mc.player != null && mc.level != null;
        }

        @Override
        public double playerX() {
            LocalPlayer p = Minecraft.getInstance().player;
            return p == null ? 0.0 : p.getX();
        }

        @Override
        public double playerY() {
            LocalPlayer p = Minecraft.getInstance().player;
            return p == null ? 0.0 : p.getY();
        }

        @Override
        public double playerZ() {
            LocalPlayer p = Minecraft.getInstance().player;
            return p == null ? 0.0 : p.getZ();
        }

        @Override
        public float playerYaw() {
            LocalPlayer p = Minecraft.getInstance().player;
            return p == null ? 0.0F : p.getYRot();
        }

        /** F1. In 26.x the flag lives on the HUD (Hud.isHidden), no longer in Options.hideGui. */
        @Override
        public boolean hudHidden() {
            //? if >=26.1 {
            return Minecraft.getInstance().gui.hud.isHidden();
            //?} else {
            /*return Minecraft.getInstance().options.hideGui;
            *///?}
        }

        /**
         * The F3 screen itself. Not {@code DebugScreenOverlay.showDebugScreen()}: that is also true when a debug entry
         * is set to "always on" (debug profiles, e.g. the Performance profile shows the FPS line without F3), which
         * would hide the mod HUD for good.
         */
        @Override
        public boolean debugOverlayOpen() {
            return Minecraft.getInstance().debugEntries.isOverlayVisible();
        }

        @Override
        public boolean screenOpen() {
            return currentScreen(Minecraft.getInstance()) != null;
        }

        @Override
        public String serverAddress() {
            Minecraft mc = Minecraft.getInstance();
            if (mc.hasSingleplayerServer()) {
                return null;
            }
            ServerData server = mc.getCurrentServer();
            return server == null ? null : server.ip;
        }
    }

    private static final class Input implements InputView {
        @Override
        public boolean isDown(GameKey key) {
            KeyMapping mapping = mapping(key);
            return mapping != null && mapping.isDown();
        }

        @Override
        public String keyLabel(GameKey key) {
            KeyMapping mapping = mapping(key);
            return mapping == null ? null : mapping.getTranslatedKeyMessage().getString();
        }

        private static KeyMapping mapping(GameKey key) {
            Options o = Minecraft.getInstance().options;
            if (o == null) {
                return null;
            }
            switch (key) {
                case FORWARD:
                    return o.keyUp;
                case BACK:
                    return o.keyDown;
                case LEFT:
                    return o.keyLeft;
                case RIGHT:
                    return o.keyRight;
                case JUMP:
                    return o.keyJump;
                case SNEAK:
                    return o.keyShift;
                case SPRINT:
                    return o.keySprint;
                case ATTACK:
                    return o.keyAttack;
                case USE:
                    return o.keyUse;
                default:
                    return null;
            }
        }
    }
}
