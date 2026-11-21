package io.github.clemensjl.hengst.mod.legacy;

import io.github.clemensjl.hengst.mod.common.platform.GameKey;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.platform.GameView;
import io.github.clemensjl.hengst.mod.common.platform.InputView;
import io.github.clemensjl.hengst.mod.common.platform.Platform;
import io.github.clemensjl.hengst.mod.common.platform.UiScreen;
import java.nio.file.Path;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.MinecraftClient;
import net.minecraft.entity.player.ClientPlayerEntity;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.network.ServerInfo;
import net.minecraft.client.option.GameOptions;
import net.minecraft.client.option.KeyBinding;
import org.apache.logging.log4j.LogManager;
import org.apache.logging.log4j.Logger;

/**
 * The core's view of the game. Game and input views are created once; every call reads the live game state through
 * {@link MinecraftClient#getInstance()} (nothing is read in the constructor, which runs during client initialization).
 */
public final class LegacyPlatform implements Platform {
    private static final Logger LOGGER = LogManager.getLogger("hengst");

    private final Game game = new Game();
    private final Input input = new Input();

    @Override
    public GameVersion version() {
        return GameVersion.V1_8_9;
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

    /** {@code GameOptions.language}, for example "en_US" or "de_DE" (the core's I18n lowercases it). */
    @Override
    public String languageCode() {
        GameOptions o = MinecraftClient.getInstance().options;
        return o == null ? "en_US" : o.language;
    }

    @Override
    public void logWarning(String message) {
        LOGGER.warn(message);
    }

    /** Synchronous: MinecraftClient.setScreen makes the host current before it returns. */
    @Override
    public void openScreen(UiScreen screen) {
        MinecraftClient.getInstance().setScreen(new LegacyScreenHost(screen));
    }

    /**
     * Only the mod's own host, only while it is current and not being removed. MinecraftClient.setScreen calls
     * {@code removed()} on the old screen while it is still current, so the removed flag is what keeps this from
     * calling setScreen re-entrantly.
     */
    @Override
    public void closeScreen() {
        MinecraftClient mc = MinecraftClient.getInstance();
        Screen current = mc.currentScreen;
        if (current instanceof LegacyScreenHost && !((LegacyScreenHost) current).isRemoved()) {
            mc.setScreen(null);
        }
    }

    private static final class Game implements GameView {
        @Override
        public boolean inWorld() {
            MinecraftClient mc = MinecraftClient.getInstance();
            return mc.player != null && mc.world != null;
        }

        @Override
        public double playerX() {
            ClientPlayerEntity p = MinecraftClient.getInstance().player;
            return p == null ? 0.0 : p.x;
        }

        @Override
        public double playerY() {
            ClientPlayerEntity p = MinecraftClient.getInstance().player;
            return p == null ? 0.0 : p.y;
        }

        @Override
        public double playerZ() {
            ClientPlayerEntity p = MinecraftClient.getInstance().player;
            return p == null ? 0.0 : p.z;
        }

        @Override
        public float playerYaw() {
            ClientPlayerEntity p = MinecraftClient.getInstance().player;
            return p == null ? 0.0F : p.yaw;
        }

        /** F1 (MinecraftClient.tick toggles it on key 59). */
        @Override
        public boolean hudHidden() {
            GameOptions o = MinecraftClient.getInstance().options;
            return o != null && o.hudHidden;
        }

        /** F3 (key 61; InGameHud.render draws the DebugHud while it is set). */
        @Override
        public boolean debugOverlayOpen() {
            GameOptions o = MinecraftClient.getInstance().options;
            return o != null && o.debugEnabled;
        }

        @Override
        public boolean screenOpen() {
            return MinecraftClient.getInstance().currentScreen != null;
        }

        /**
         * The address typed for the current server: ConnectScreen sets the entry, MinecraftClient.connect(null)
         * clears it when the world is left. The world check covers the connecting screens, where the entry is
         * already set but no world exists yet.
         */
        @Override
        public String serverAddress() {
            MinecraftClient mc = MinecraftClient.getInstance();
            if (mc.world == null || mc.isInSingleplayer()) {
                return null;
            }
            ServerInfo server = mc.getCurrentServerEntry();
            return server == null ? null : server.address;
        }
    }

    private static final class Input implements InputView {
        @Override
        public boolean isDown(GameKey key) {
            KeyBinding binding = binding(key);
            return binding != null && binding.isPressed();
        }

        /** "W", "SPACE", "Button 1" ({@code GameOptions.getFormattedNameForKeyCode}); allocates, called rarely. */
        @Override
        public String keyLabel(GameKey key) {
            KeyBinding binding = binding(key);
            return binding == null ? null : GameOptions.getFormattedNameForKeyCode(binding.getCode());
        }

        private static KeyBinding binding(GameKey key) {
            GameOptions o = MinecraftClient.getInstance().options;
            if (o == null) {
                return null;
            }
            switch (key) {
                case FORWARD:
                    return o.forwardKey;
                case BACK:
                    return o.backKey;
                case LEFT:
                    return o.leftKey;
                case RIGHT:
                    return o.rightKey;
                case JUMP:
                    return o.jumpKey;
                case SNEAK:
                    return o.sneakKey;
                case SPRINT:
                    return o.sprintKey;
                case ATTACK:
                    return o.attackKey;
                case USE:
                    return o.useKey;
                default:
                    return null;
            }
        }
    }
}
