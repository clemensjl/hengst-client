package io.github.clemensjl.hengst.mod.legacy;

import com.mojang.blaze3d.platform.GlStateManager;
import io.github.clemensjl.hengst.mod.common.AdapterGuard;
import io.github.clemensjl.hengst.mod.common.FeatureCatalog;
import io.github.clemensjl.hengst.mod.common.HengstCore;
import io.github.clemensjl.hengst.mod.common.feature.DefaultKey;
import io.github.clemensjl.hengst.mod.common.feature.KeySpec;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.platform.Platform;
import io.github.clemensjl.hengst.mod.common.platform.TimeSource;
import java.util.List;
import java.util.function.Supplier;
import java.util.function.ToIntFunction;
import java.util.function.UnaryOperator;
import net.fabricmc.api.ClientModInitializer;
import net.legacyfabric.fabric.api.client.event.lifecycle.v1.ClientLifecycleEvents;
import net.legacyfabric.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import org.apache.logging.log4j.LogManager;
import org.apache.logging.log4j.Logger;

/**
 * Client entrypoint: starts the core and wires the game's hooks to it. If the core fails to start, the mod stays
 * inert: {@link #ENABLED} remains false and every hook returns at once.
 *
 * <p>A common jar that is missing or damaged as a whole is not caught: this class refers to common types
 * ({@link #GUARD} is an {@link AdapterGuard}), so its class initialization fails before any of its code runs, and
 * Fabric treats a failing entrypoint as fatal. Do not try to catch that; common ships inside the mod jar, so it needs a
 * damaged jar.
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
        core = start(() -> FeatureCatalog.keys(GameVersion.V1_8_9), LegacyKeys::code, LegacyPlatform::new);
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
     * Registers the key bindings, then starts the core; null if either fails, and the mod stays inert (no listeners,
     * {@link #ENABLED} false). The registration runs common code (every feature's constructor and keys) and the API
     * mod's helper, which throws on a duplicate binding name: caught like the core start, logged once.
     */
    static HengstCore start(Supplier<List<KeySpec>> keys, ToIntFunction<DefaultKey> codes,
            Supplier<? extends Platform> platform) {
        return start(keys, codes, LegacyKeys.REGISTRAR, platform);
    }

    /** {@link #start(Supplier, ToIntFunction, Supplier)} with the API mod's key helper replaceable, for the tests. */
    static HengstCore start(Supplier<List<KeySpec>> keys, ToIntFunction<DefaultKey> codes,
            UnaryOperator<KeyBinding> registrar, Supplier<? extends Platform> platform) {
        try {
            LegacyKeys.register(keys.get(), codes, registrar);
        } catch (RuntimeException | LinkageError e) {
            LOGGER.error("[hengst] key bindings failed to register; the mod stays inactive", e);
            return null;
        }
        return GUARD.start(platform, TimeSource.SYSTEM);
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
            onHookFailure(SITE_HUD, e);
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
            onHookFailure(SITE_TICK, e);
        }
    }

    /** Every hook's catch: a linkage error switches the mod off (see {@link AdapterGuard#hookFailed}). Cold path. */
    static void onHookFailure(int site, Throwable e) {
        if (GUARD.hookFailed(site, e)) {
            ENABLED = false;
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
            onHookFailure(SITE_STOP, e);
        }
    }

    /** Once per rendered frame, from the frame mixin (which checks {@link #ENABLED} first). */
    public static void onFrame() {
        try {
            core.onFrame();
        } catch (RuntimeException | LinkageError e) {
            onHookFailure(SITE_FRAME, e);
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
            onHookFailure(SITE_MOUSE, e);
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
