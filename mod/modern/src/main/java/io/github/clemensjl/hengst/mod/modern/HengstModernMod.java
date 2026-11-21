package io.github.clemensjl.hengst.mod.modern;

import io.github.clemensjl.hengst.mod.common.HengstCore;
import io.github.clemensjl.hengst.mod.common.platform.Platform;
import io.github.clemensjl.hengst.mod.common.platform.TimeSource;
import java.util.function.Supplier;
import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientLifecycleEvents;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.rendering.v1.hud.HudElementRegistry;
import net.fabricmc.fabric.api.client.rendering.v1.hud.VanillaHudElements;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.Minecraft;
//? if >=26.1 {
import net.minecraft.client.gui.GuiGraphicsExtractor;
//?} else {
/*import net.minecraft.client.gui.GuiGraphics;
*///?}
import net.minecraft.resources.Identifier;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * Client entrypoint: starts the core and wires the game's hooks to it. If the core fails to start, the mod stays
 * inert: {@link #ENABLED} remains false and every hook returns at once.
 */
public final class HengstModernMod implements ClientModInitializer {
    private static final Logger LOGGER = LoggerFactory.getLogger("hengst");
    private static final ModernCanvas CANVAS = new ModernCanvas();

    /** The mod's HUD element in Fabric's HUD registry. */
    public static final Identifier HUD_ID = Identifier.fromNamespaceAndPath("hengst", "hud");

    // Hook sites for failure logging.
    static final int SITE_HUD = 0;
    static final int SITE_TICK = 1;
    static final int SITE_STOP = 2;
    static final int SITE_FRAME = 3;
    static final int SITE_MOUSE = 4;
    private static final String[] SITE_NAMES = {"HUD", "client tick", "client stop", "frame", "mouse button"};
    private static final int MAX_KINDS = 4;
    private static final Class<?>[][] LOGGED_KINDS = new Class<?>[SITE_NAMES.length][MAX_KINDS];
    private static final int[] LOGGED_COUNT = new int[SITE_NAMES.length];

    /** Set once, as the last step of a successful start. Read first by every hook and mixin. */
    public static boolean ENABLED;
    private static HengstCore core;

    public static HengstCore core() {
        return core;
    }

    @Override
    public void onInitializeClient() {
        ModernKeys.register();
        core = startCore(ModernPlatform::new);
        if (core == null) {
            return;
        }
        // In the main HUD pass, directly before the chat (verified on 1.21.11 Gui.render and 26.3
        // Hud.extractRenderState): above hotbar, effects, boss bar, scoreboard, action bar and title; below the chat
        // text, the player list, any screen and its dim, toasts and the F3 overlay. Never deferred. Inherits the
        // chat's render condition (not under F1), which the core applies anyway.
        HudElementRegistry.attachElementBefore(VanillaHudElements.CHAT, HUD_ID, HengstModernMod::onHud);
        ClientTickEvents.END_CLIENT_TICK.register(HengstModernMod::onEndTick);
        ClientLifecycleEvents.CLIENT_STOPPING.register(HengstModernMod::onStopping);
        ENABLED = true;
    }

    /**
     * Creates the platform, logs the load line and starts the core; null after logging why. Caught: runtime
     * exceptions and linkage errors raised while the platform and the core are created (a common class that fails
     * its static initializer or references something missing). Not caught: VM errors. A common jar that is missing
     * as a whole cannot be caught here: this class itself refers to common types, so the game reports the
     * entrypoint as failed before this method runs (common ships inside the mod jar, so that needs a damaged jar).
     */
    static HengstCore startCore(Supplier<? extends Platform> platformFactory) {
        try {
            Platform platform = platformFactory.get();
            LOGGER.info("[hengst] loaded on {}", platform.version().id());
            return HengstCore.start(platform, TimeSource.SYSTEM);
        } catch (RuntimeException | LinkageError e) {
            LOGGER.error("[hengst] core failed to start; the mod stays inactive and its key bindings do nothing", e);
            return null;
        }
    }

    //? if >=26.1 {
    private static void onHud(GuiGraphicsExtractor graphics, DeltaTracker delta) {
    //?} else {
    /*private static void onHud(GuiGraphics graphics, DeltaTracker delta) {
    *///?}
        if (!ENABLED) {
            return;
        }
        try {
            CANVAS.begin(graphics);
            core.onHudRender(CANVAS, delta.getGameTimeDeltaPartialTick(false));
        } catch (RuntimeException | LinkageError e) {
            hookFailed(SITE_HUD, e);
        }
    }

    private static void onEndTick(Minecraft client) {
        if (!ENABLED) {
            return;
        }
        try {
            ModernKeys.dispatch(client);
            core.onClientTick();
        } catch (RuntimeException | LinkageError e) {
            hookFailed(SITE_TICK, e);
        }
    }

    private static void onStopping(Minecraft client) {
        stopCore(core);
    }

    /**
     * Writes the core's pending changes whenever a core exists, also after a hook failure cleared {@link #ENABLED}:
     * the last unsaved changes must not be lost because the mod switched itself off. Fenced on its own.
     */
    static void stopCore(HengstCore core) {
        if (core == null) {
            return;
        }
        try {
            core.shutdown();
        } catch (RuntimeException | LinkageError e) {
            hookFailed(SITE_STOP, e);
        }
    }

    /** Once per rendered frame, from the frame mixin (which checks {@link #ENABLED} first). */
    public static void onFrame() {
        try {
            core.onFrame();
        } catch (RuntimeException | LinkageError e) {
            hookFailed(SITE_FRAME, e);
        }
    }

    /** Every physical mouse button transition, from the mouse mixin (which checks {@link #ENABLED} first). */
    public static void onMouseButton(int rawButton, boolean pressed) {
        try {
            int button = ModernInput.button(rawButton);
            if (button != ModernInput.OTHER_BUTTON) {
                core.onMouseButton(button, pressed);
            }
        } catch (RuntimeException | LinkageError e) {
            hookFailed(SITE_MOUSE, e);
        }
    }

    /**
     * Cold path; nothing thrown by the mod reaches the game. A linkage error (the game or Fabric API does not have
     * what the adapter was built against) would repeat on every call: the mod disables itself and logs once. Other
     * failures are logged once per site and exception class (the first four classes of a site).
     */
    static void hookFailed(int site, Throwable e) {
        if (e instanceof LinkageError) {
            if (ENABLED) {
                ENABLED = false;
                LOGGER.error("[hengst] the {} hook does not match this game; the mod is disabled", SITE_NAMES[site], e);
            }
            return;
        }
        Class<?> kind = e.getClass();
        int n = LOGGED_COUNT[site];
        for (int i = 0; i < n; i++) {
            if (LOGGED_KINDS[site][i] == kind) {
                return;
            }
        }
        if (n == MAX_KINDS) {
            return;
        }
        LOGGED_KINDS[site][n] = kind;
        LOGGED_COUNT[site] = n + 1;
        LOGGER.error("[hengst] the {} hook failed; further {} there are not logged", SITE_NAMES[site],
                kind.getSimpleName(), e);
    }
}
