package io.github.clemensjl.hengst.mod.legacy;

import io.github.clemensjl.hengst.mod.common.FeatureCatalog;
import io.github.clemensjl.hengst.mod.common.HengstCore;
import io.github.clemensjl.hengst.mod.common.feature.DefaultKey;
import io.github.clemensjl.hengst.mod.common.feature.KeySpec;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import java.util.List;
import net.legacyfabric.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import org.lwjgl.input.Keyboard;
import org.lwjgl.opengl.Display;

/**
 * The core's key bindings ({@link FeatureCatalog#keys}), in their own "Hengst" category of the vanilla controls screen
 * (the category is the translation key {@code key.categories.hengst}; KeyBinding's constructor adds it to the category
 * set), and their per-tick report to the core.
 *
 * <p>What the game counts (MinecraftClient.tick, verified in Legacy Yarn build.604): {@code KeyBinding.onKeyPressed}
 * and {@code setKeyPressed} run from the tick's own keyboard loop, which reads LWJGL's queue only while no screen is
 * open (or one that passes events on), so typing in chat counts nothing; LWJGL's repeat events are off in the game, so
 * a held key is one press. Opening a screen calls {@code KeyBinding.releaseAllKeys()} (setScreen, through grabMouse).
 * The core ignores presses outside a world or with a screen open and a press of a key it already holds, and releases
 * held HOLD keys itself when a screen opens or the world is left; this class adds the release when the window loses
 * focus.
 */
final class LegacyKeys {
    static final String CATEGORY = "key.categories.hengst";

    /** Index like {@link HengstCore#keySpecs()}; built once by {@link #register}. */
    private static KeyBinding[] bindings = new KeyBinding[0];
    private static boolean windowFocused = true;

    private LegacyKeys() {
    }

    /**
     * From the client initializer, before the core starts: one binding per key of {@code version}, in catalog order, so
     * the index of a binding is its index for {@link HengstCore#onFeatureKey}. Legacy Fabric API adds the bindings to
     * GameOptions.allKeys when it loads.
     */
    static void register(GameVersion version) {
        List<KeySpec> keys = FeatureCatalog.keys(version);
        KeyBinding[] built = new KeyBinding[keys.size()];
        for (int i = 0; i < built.length; i++) {
            KeySpec spec = keys.get(i);
            built[i] = KeyBindingHelper.registerKeyBinding(
                    new KeyBinding(spec.translationKey(), code(spec.defaultKey()), CATEGORY));
        }
        bindings = built;
    }

    /** The LWJGL 2 key code for a default key. */
    static int code(DefaultKey key) {
        switch (key) {
            case NONE:
                return Keyboard.KEY_NONE;
            case RIGHT_SHIFT:
                return Keyboard.KEY_RSHIFT;
            default:
                throw new IllegalArgumentException("no key code for " + key);
        }
    }

    /** End of a client tick, before the core's tick. */
    static void dispatch(MinecraftClient client) {
        HengstCore core = HengstLegacyMod.core();
        focus(core, bindings, Display.isActive());
        dispatch(core, bindings);
    }

    /**
     * Per key, the contract of {@link HengstCore#onFeatureKey}: a press if at least one press was counted since the last
     * tick (all are drained), then a release if the key is no longer pressed. The core keeps the held state, so a
     * release of a key that is not held costs one array read. Indexed loop, allocation free.
     */
    static void dispatch(HengstCore core, KeyBinding[] bindings) {
        for (int i = 0; i < bindings.length; i++) {
            KeyBinding b = bindings[i];
            boolean pressed = false;
            while (b.wasPressed()) {
                pressed = true;
            }
            if (pressed) {
                core.onFeatureKey(i, true);
            }
            if (!b.isPressed()) {
                core.onFeatureKey(i, false);
            }
        }
    }

    /**
     * On the tick the window loses focus, every key is reported released, so no HOLD key stays held while the player is
     * in another window (LWJGL 2 on Windows also queues key releases on deactivation; this does not rely on it). A key
     * that still reads pressed afterwards counts again with its next press.
     */
    static void focus(HengstCore core, KeyBinding[] bindings, boolean focused) {
        if (windowFocused && !focused) {
            for (int i = 0; i < bindings.length; i++) {
                core.onFeatureKey(i, false);
            }
        }
        windowFocused = focused;
    }
}
