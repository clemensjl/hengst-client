package io.github.clemensjl.hengst.mod.legacy;

import com.mojang.blaze3d.platform.GlStateManager;
import io.github.clemensjl.hengst.mod.common.HengstCore;
import io.github.clemensjl.hengst.mod.common.platform.Platform;
import io.github.clemensjl.hengst.mod.common.platform.TimeSource;
import java.util.function.Supplier;
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
        LegacyKeys.register();
        core = startCore(LegacyPlatform::new);
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
            hookFailed(SITE_HUD, e);
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
            hookFailed(SITE_TICK, e);
        }
    }

    private static void onStopping(MinecraftClient client) {
        if (!ENABLED) {
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
            hookFailed(SITE_MOUSE, e);
        }
    }

    /** A mouse event's key-binding code ({@code Mouse.getEventButton() - 100}), from KeyBindingMixin. */
    public static void onKeyBindingMouseCode(int code, boolean pressed) {
        int lwjglButton = LegacyInput.buttonOfKeyCode(code);
        if (lwjglButton != LegacyInput.OTHER_BUTTON) {
            onMouseButton(lwjglButton, pressed);
        }
    }

    /**
     * Cold path; nothing thrown by the mod reaches the game. A linkage error (the game or Legacy Fabric API does not
     * have what the adapter was built against) would repeat on every call: the mod disables itself and logs once.
     * Other failures are logged once per site and exception class (the first four classes of a site).
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
