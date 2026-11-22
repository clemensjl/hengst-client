package io.github.clemensjl.hengst.mod.legacy;

import io.github.clemensjl.hengst.mod.common.FeatureCatalog;
import io.github.clemensjl.hengst.mod.common.HengstCore;
import io.github.clemensjl.hengst.mod.common.feature.DefaultKey;
import io.github.clemensjl.hengst.mod.common.feature.KeySpec;
import java.util.List;
import java.util.function.ToIntFunction;
import java.util.function.UnaryOperator;
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
     * From the client initializer, before the core starts: one binding per key, in list order, so the index of a binding
     * is its index for {@link HengstCore#onFeatureKey}. Legacy Fabric API adds the bindings to GameOptions.allKeys when
     * it loads. All key codes are computed before the first binding is built, so an unmapped default key registers
     * nothing; the binding array is set only when every registration succeeded. Throws what a step throws; the caller
     * keeps the mod inert then. The registration step is {@link #REGISTRAR} in the game, replaceable for the adapter
     * tests.
     */
    static void register(List<KeySpec> keys, ToIntFunction<DefaultKey> codes, UnaryOperator<KeyBinding> registrar) {
        int[] keyCodes = new int[keys.size()];
        for (int i = 0; i < keyCodes.length; i++) {
            keyCodes[i] = codes.applyAsInt(keys.get(i).defaultKey());
        }
        KeyBinding[] built = new KeyBinding[keyCodes.length];
        for (int i = 0; i < built.length; i++) {
            built[i] = registrar.apply(new KeyBinding(keys.get(i).translationKey(), keyCodes[i], CATEGORY));
        }
        bindings = built;
    }

    /** The API mod's helper, the registration step in the game. */
    static final UnaryOperator<KeyBinding> REGISTRAR = KeyBindingHelper::registerKeyBinding;

    /**
     * The registered bindings in index order; empty until {@link #register} succeeded. The live array, for identity
     * checks in tests only; nothing may write to it.
     */
    static KeyBinding[] bindings() {
        return bindings;
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
        tick(HengstLegacyMod.core(), bindings, Display.isActive());
    }

    /**
     * One tick: the keys first, then the focus, so a release forced by a focus loss comes after any press counted in
     * the same tick and is final.
     */
    static void tick(HengstCore core, KeyBinding[] bindings, boolean focused) {
        dispatch(core, bindings);
        focus(core, bindings, focused);
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
