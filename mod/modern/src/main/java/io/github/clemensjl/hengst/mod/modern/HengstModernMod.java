package io.github.clemensjl.hengst.mod.modern;

import io.github.clemensjl.hengst.mod.common.HengstCore;
import io.github.clemensjl.hengst.mod.common.platform.Platform;
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

    /** Set once, as the last step of a successful start. Read first by every hook and mixin. */
    public static boolean ENABLED;
    private static HengstCore core;
    private static boolean hookFailureLogged;

    public static HengstCore core() {
        return core;
    }

    @Override
    public void onInitializeClient() {
        ModernPlatform platform = new ModernPlatform();
        LOGGER.info("[hengst] loaded on {}", platform.version().id());
        ModernKeys.register();
        core = startCore(platform);
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
     * {@link HengstCore#start}, or null after logging why. Failures of the core and of its classes (a broken or
     * missing common jar: NoClassDefFoundError, ExceptionInInitializerError) are caught; VM errors are not.
     */
    static HengstCore startCore(Platform platform) {
        try {
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
            hookFailed(e);
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
            hookFailed(e);
        }
    }

    private static void onStopping(Minecraft client) {
        if (!ENABLED) {
            return;
        }
        try {
            core.shutdown();
        } catch (RuntimeException | LinkageError e) {
            hookFailed(e);
        }
    }

    /** Once per rendered frame, from the frame mixin (which checks {@link #ENABLED} first). */
    public static void onFrame() {
        try {
            core.onFrame();
        } catch (RuntimeException | LinkageError e) {
            hookFailed(e);
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
            hookFailed(e);
        }
    }

    /** Cold path: nothing thrown by the mod reaches the game; the first failure is logged. */
    private static void hookFailed(Throwable e) {
        if (!hookFailureLogged) {
            hookFailureLogged = true;
            LOGGER.error("[hengst] a game hook failed; later hook failures are not logged", e);
        }
    }
}
