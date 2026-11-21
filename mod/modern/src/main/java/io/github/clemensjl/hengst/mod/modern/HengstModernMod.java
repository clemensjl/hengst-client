package io.github.clemensjl.hengst.mod.modern;

import io.github.clemensjl.hengst.mod.common.AdapterGuard;
import io.github.clemensjl.hengst.mod.common.HengstCore;
import io.github.clemensjl.hengst.mod.common.platform.TimeSource;
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
    static final AdapterGuard GUARD = new AdapterGuard(
            new String[] {"HUD", "client tick", "client stop", "frame", "mouse button"}, new AdapterGuard.Log() {
                @Override
                public void info(String message) {
                    LOGGER.info(message);
                }

                @Override
                public void error(String message, Throwable error) {
                    LOGGER.error(message, error);
                }
            });

    /** Set once, as the last step of a successful start. Read first by every hook and mixin. */
    public static boolean ENABLED;
    private static HengstCore core;

    public static HengstCore core() {
        return core;
    }

    @Override
    public void onInitializeClient() {
        ModernKeys.register();
        core = GUARD.start(ModernPlatform::new, TimeSource.SYSTEM);
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
            if (GUARD.hookFailed(SITE_HUD, e)) {
                ENABLED = false;
            }
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
            if (GUARD.hookFailed(SITE_TICK, e)) {
                ENABLED = false;
            }
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
            if (GUARD.hookFailed(SITE_STOP, e)) {
                ENABLED = false;
            }
        }
    }

    /** Once per rendered frame, from the frame mixin (which checks {@link #ENABLED} first). */
    public static void onFrame() {
        try {
            core.onFrame();
        } catch (RuntimeException | LinkageError e) {
            if (GUARD.hookFailed(SITE_FRAME, e)) {
                ENABLED = false;
            }
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
            if (GUARD.hookFailed(SITE_MOUSE, e)) {
                ENABLED = false;
            }
        }
    }
}
