package io.github.clemensjl.hengst.mod.modern;

import io.github.clemensjl.hengst.mod.common.AdapterGuard;
import io.github.clemensjl.hengst.mod.common.FeatureCatalog;
import io.github.clemensjl.hengst.mod.common.HengstCore;
import io.github.clemensjl.hengst.mod.common.feature.DefaultKey;
import io.github.clemensjl.hengst.mod.common.feature.KeySpec;
import io.github.clemensjl.hengst.mod.common.platform.Platform;
import io.github.clemensjl.hengst.mod.common.platform.TimeSource;
import java.util.List;
import java.util.function.Supplier;
import java.util.function.ToIntFunction;
import java.util.function.UnaryOperator;
import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientLifecycleEvents;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.rendering.v1.hud.HudElementRegistry;
import net.fabricmc.fabric.api.client.rendering.v1.hud.VanillaHudElements;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.KeyMapping;
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
 *
 * <p>A common jar that is missing or damaged as a whole is not caught: this class refers to common types
 * ({@link #GUARD} is an {@link AdapterGuard}), so its class initialization fails before any of its code runs, and
 * Fabric treats a failing entrypoint as fatal. Do not try to catch that; common ships inside the mod jar, so it needs a
 * damaged jar.
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
        core = start(() -> FeatureCatalog.keys(ModernPlatform.gameVersion()), ModernKeys::code, ModernPlatform::new);
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
     * Registers the key bindings, then starts the core; null if either fails, and the mod stays inert (no listeners,
     * {@link #ENABLED} false). The registration runs common code (every feature's constructor and keys) and the API
     * mod's helper, which throws on a duplicate binding name: caught like the core start, logged once.
     */
    static HengstCore start(Supplier<List<KeySpec>> keys, ToIntFunction<DefaultKey> codes,
            Supplier<? extends Platform> platform) {
        return start(keys, codes, ModernKeys::registerMapping, platform);
    }

    /** {@link #start(Supplier, ToIntFunction, Supplier)} with the API mod's key helper replaceable, for the tests. */
    static HengstCore start(Supplier<List<KeySpec>> keys, ToIntFunction<DefaultKey> codes,
            UnaryOperator<KeyMapping> registrar, Supplier<? extends Platform> platform) {
        try {
            ModernKeys.register(keys.get(), codes, registrar);
        } catch (RuntimeException | LinkageError e) {
            LOGGER.error("[hengst] key bindings failed to register; the mod stays inactive", e);
            return null;
        }
        return GUARD.start(platform, TimeSource.SYSTEM);
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
            onHookFailure(SITE_HUD, e);
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
            onHookFailure(SITE_TICK, e);
        }
    }

    /** Every hook's catch: a linkage error switches the mod off (see {@link AdapterGuard#hookFailed}). Cold path. */
    static void onHookFailure(int site, Throwable e) {
        if (GUARD.hookFailed(site, e)) {
            ENABLED = false;
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

    /** Every physical mouse button transition, from the mouse mixin (which checks {@link #ENABLED} first). */
    public static void onMouseButton(int rawButton, boolean pressed) {
        try {
            int button = ModernInput.button(rawButton);
            if (button != ModernInput.OTHER_BUTTON) {
                core.onMouseButton(button, pressed);
            }
        } catch (RuntimeException | LinkageError e) {
            onHookFailure(SITE_MOUSE, e);
        }
    }
}
