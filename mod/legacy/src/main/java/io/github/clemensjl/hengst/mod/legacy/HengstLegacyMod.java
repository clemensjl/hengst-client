package io.github.clemensjl.hengst.mod.legacy;

import com.mojang.blaze3d.platform.GlStateManager;
import io.github.clemensjl.hengst.mod.common.AdapterGuard;
import io.github.clemensjl.hengst.mod.common.HengstCore;
import io.github.clemensjl.hengst.mod.common.platform.TimeSource;
import net.fabricmc.api.ClientModInitializer;
import net.legacyfabric.fabric.api.client.event.lifecycle.v1.ClientLifecycleEvents;
import net.legacyfabric.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import org.apache.logging.log4j.LogManager;
import org.apache.logging.log4j.Logger;

/**
 * Client entrypoint: starts the core and wires the game's hooks to it. If the core fails to start, the mod stays
 * inert: {@link #ENABLED} remains false and every hook returns at once.
 */
public final class HengstLegacyMod implements ClientModInitializer {
    private static final Logger LOGGER = LogManager.getLogger("hengst");
    private static final LegacyCanvas HUD_CANVAS = new LegacyCanvas();

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
        LegacyKeys.register();
        core = GUARD.start(LegacyPlatform::new, TimeSource.SYSTEM);
        if (core == null) {
            return;
        }
        // HUD: InGameHudMixin, not Legacy Fabric API's HudRenderCallback (that one fires after the chat and the
        // player list, see the mixin).
        ClientTickEvents.END_CLIENT_TICK.register(HengstLegacyMod::onEndTick);
        // Legacy Fabric API injects this at the HEAD of MinecraftClient.stop(), which run() calls on the client
        // thread when the game loop ends (window closed, quit button), before Display.destroy and System.exit.
        ClientLifecycleEvents.CLIENT_STOPPING.register(HengstLegacyMod::onStopping);
        ENABLED = true;
    }

    /**
     * From the HUD mixin (which checks {@link #ENABLED} first), inside InGameHud.render with the HUD projection set
     * up. The canvas size is the scaled size vanilla's HUD uses, computed without allocating a Window.
     *
     * <p>Depth: the hotbar item icons are drawn earlier in the same pass at z 100 and more with the depth test on and
     * write depth, the mod draws at z 0, so a module over a filled slot would be hidden behind the item. The depth
     * test is therefore off while the mod draws and on again afterwards, the state vanilla has at this point (the
     * in-game test checks it with {@code glIsEnabled}) and keeps for the chat and player list.
     */
    public static void onHud(float tickDelta) {
        try {
            MinecraftClient mc = MinecraftClient.getInstance();
            int scale = LegacyInput.guiScale(mc.width, mc.height, mc.options.guiScale, mc.forcesUnicodeFont());
            HUD_CANVAS.begin(mc.textRenderer, LegacyInput.guiSize(mc.width, scale),
                    LegacyInput.guiSize(mc.height, scale));
            GlStateManager.disableDepthTest();
            core.onHudRender(HUD_CANVAS, tickDelta);
            GlStateManager.enableDepthTest();
        } catch (RuntimeException | LinkageError e) {
            if (GUARD.hookFailed(SITE_HUD, e)) {
                ENABLED = false;
            }
        }
    }

    private static void onEndTick(MinecraftClient client) {
        if (!ENABLED) {
            return;
        }
        try {
            LegacyKeys.dispatch(client);
            core.onClientTick();
        } catch (RuntimeException | LinkageError e) {
            if (GUARD.hookFailed(SITE_TICK, e)) {
                ENABLED = false;
            }
        }
    }

    private static void onStopping(MinecraftClient client) {
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

    /**
     * Every physical mouse button transition, from the two mouse mixins (which check {@link #ENABLED} first): the
     * LWJGL button number ({@code Mouse.getEventButton()}, 0 left, 1 right, 2 middle) and its state.
     */
    public static void onMouseButton(int lwjglButton, boolean pressed) {
        try {
            int button = LegacyInput.button(lwjglButton);
            if (button != LegacyInput.OTHER_BUTTON) {
                core.onMouseButton(button, pressed);
            }
        } catch (RuntimeException | LinkageError e) {
            if (GUARD.hookFailed(SITE_MOUSE, e)) {
                ENABLED = false;
            }
        }
    }

    /** A mouse event's key-binding code ({@code Mouse.getEventButton() - 100}), from KeyBindingMixin. */
    public static void onKeyBindingMouseCode(int code, boolean pressed) {
        int lwjglButton = LegacyInput.buttonOfKeyCode(code);
        if (lwjglButton != LegacyInput.OTHER_BUTTON) {
            onMouseButton(lwjglButton, pressed);
        }
    }
}
