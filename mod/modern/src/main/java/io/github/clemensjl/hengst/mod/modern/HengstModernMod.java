package io.github.clemensjl.hengst.mod.modern;

import io.github.clemensjl.hengst.mod.common.HengstCore;
import io.github.clemensjl.hengst.mod.common.platform.TimeSource;
import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientLifecycleEvents;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.rendering.v1.hud.HudElementRegistry;
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

    /** Set once, as the last step of a successful start. Read first by every hook and mixin. */
    public static boolean ENABLED;
    private static HengstCore core;

    public static HengstCore core() {
        return core;
    }

    @Override
    public void onInitializeClient() {
        ModernPlatform platform = new ModernPlatform();
        LOGGER.info("[hengst] loaded on {}", platform.version().id());
        ModernKeys.register();
        try {
            core = HengstCore.start(platform, TimeSource.SYSTEM);
        } catch (RuntimeException e) {
            LOGGER.error("[hengst] core failed to start, the mod stays inactive", e);
            return;
        }
        HudElementRegistry.addLast(Identifier.fromNamespaceAndPath("hengst", "hud"), HengstModernMod::onHud);
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
        CANVAS.begin(graphics);
        core.onHudRender(CANVAS, delta.getGameTimeDeltaPartialTick(false));
    }

    private static void onEndTick(Minecraft client) {
        if (!ENABLED) {
            return;
        }
        ModernKeys.dispatch(client);
        core.onClientTick();
    }

    private static void onStopping(Minecraft client) {
        if (ENABLED) {
            core.shutdown();
        }
    }

    /** Once per rendered frame, from the frame mixin (which checks {@link #ENABLED} first). */
    public static void onFrame() {
        core.onFrame();
    }

    /** Every physical mouse button transition, from the mouse mixin (which checks {@link #ENABLED} first). */
    public static void onMouseButton(int rawButton, boolean pressed) {
        int button = ModernInput.button(rawButton);
        if (button != ModernInput.OTHER_BUTTON) {
            core.onMouseButton(button, pressed);
        }
    }
}
