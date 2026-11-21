package io.github.clemensjl.hengst.mod.modern;

import com.mojang.blaze3d.platform.InputConstants;
import io.github.clemensjl.hengst.mod.common.FeatureCatalog;
import io.github.clemensjl.hengst.mod.common.HengstCore;
import io.github.clemensjl.hengst.mod.common.feature.DefaultKey;
import io.github.clemensjl.hengst.mod.common.feature.KeySpec;
import java.util.List;
import java.util.function.ToIntFunction;
import java.util.function.UnaryOperator;
//? if >=26.1 {
import net.fabricmc.fabric.api.client.keymapping.v1.KeyMappingHelper;
//?} else {
/*import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
*///?}
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.resources.Identifier;

/**
 * The core's key bindings ({@link FeatureCatalog#keys}), in their own "Hengst" category of the vanilla controls screen,
 * and their per-tick report to the core.
 *
 * <p>What the game counts (KeyboardHandler.keyPress, verified on 1.21.11 and 26.3): {@code KeyMapping.click} and
 * {@code set(key, true)} run only while no screen is open, so typing in chat or any other screen counts nothing; a key
 * repeat (GLFW_REPEAT on 1.21.11, the repeat action -1 on 26.3) counts as a further click. Opening a screen calls
 * {@code KeyMapping.releaseAll()} (1.21.11 Minecraft.setScreen, 26.3 Gui.setScreen). The core ignores presses
 * outside a world or with a screen open and a press of a key it already holds, and releases held HOLD keys itself
 * when a screen opens or the world is left; this class adds the release when the window loses focus.
 */
final class ModernKeys {
    /** Translation key of the category: key.category.hengst.main (Category.label()). Registered once. */
    static final KeyMapping.Category CATEGORY =
            KeyMapping.Category.register(Identifier.fromNamespaceAndPath("hengst", "main"));

    /** Index like {@link HengstCore#keySpecs()}; built once by {@link #register}. */
    private static KeyMapping[] mappings = new KeyMapping[0];
    private static boolean windowFocused = true;

    private ModernKeys() {
    }

    /**
     * {@code InputConstants.UNKNOWN.getValue()}, written out: reading that field runs InputConstants' static initializer,
     * which on 1.21.11 loads LWJGL's GLFW natives, also in the unit tests. ModernKeysTest checks the value against the
     * game's key table, the in-game test against the field itself.
     */
    //? if >=26.1 {
    static final int UNKNOWN_KEY = 0;
    //?} else {
    /*static final int UNKNOWN_KEY = -1;
    *///?}

    /**
     * From the client initializer, before the game options are created and before the core starts: one mapping per key,
     * in list order, so the index of a mapping is its index for {@link HengstCore#onFeatureKey}. All key codes are
     * computed before the first mapping is built, so an unmapped default key registers nothing; the mapping array is
     * set only when every registration succeeded. Throws what a step throws; the caller keeps the mod inert then.
     */
    static void register(List<KeySpec> keys, ToIntFunction<DefaultKey> codes) {
        register(keys, codes, ModernKeys::registerMapping);
    }

    /** {@link #register(List, ToIntFunction)} with the API mod's helper replaceable, for the adapter tests. */
    static void register(List<KeySpec> keys, ToIntFunction<DefaultKey> codes, UnaryOperator<KeyMapping> registrar) {
        int[] keyCodes = new int[keys.size()];
        for (int i = 0; i < keyCodes.length; i++) {
            keyCodes[i] = codes.applyAsInt(keys.get(i).defaultKey());
        }
        KeyMapping[] built = new KeyMapping[keyCodes.length];
        for (int i = 0; i < built.length; i++) {
            built[i] = registrar.apply(new KeyMapping(keys.get(i).translationKey(), keyCodes[i], CATEGORY));
        }
        mappings = built;
    }

    /** The registered mappings in index order; empty until {@link #register} succeeded. For tests. */
    static KeyMapping[] mappings() {
        return mappings;
    }

    /** This version's key code for a default key: GLFW key codes on 1.21.11, SDL scancodes on 26.3. */
    static int code(DefaultKey key) {
        switch (key) {
            case NONE:
                return UNKNOWN_KEY;
            case RIGHT_SHIFT:
                return InputConstants.KEY_RSHIFT;
            default:
                throw new IllegalArgumentException("no key code for " + key);
        }
    }

    static KeyMapping registerMapping(KeyMapping mapping) {
        //? if >=26.1 {
        return KeyMappingHelper.registerKeyMapping(mapping);
        //?} else {
        /*return KeyBindingHelper.registerKeyBinding(mapping);
        *///?}
    }

    /** End of a client tick, before the core's tick. */
    static void dispatch(Minecraft client) {
        tick(HengstModernMod.core(), mappings, client.isWindowActive());
    }

    /**
     * One tick: the keys first, then the focus, so a release forced by a focus loss comes after any press counted in
     * the same tick and is final.
     */
    static void tick(HengstCore core, KeyMapping[] mappings, boolean focused) {
        dispatch(core, mappings);
        focus(core, mappings, focused);
    }

    /**
     * Per key, the contract of {@link HengstCore#onFeatureKey}: a press if at least one click was counted since the last
     * tick (all clicks are drained), then a release if the key is no longer down. The core keeps the held state, so a
     * release of a key that is not held costs one array read. Indexed loop, allocation free.
     */
    static void dispatch(HengstCore core, KeyMapping[] mappings) {
        for (int i = 0; i < mappings.length; i++) {
            KeyMapping m = mappings[i];
            boolean clicked = false;
            while (m.consumeClick()) {
                clicked = true;
            }
            if (clicked) {
                core.onFeatureKey(i, true);
            }
            if (!m.isDown()) {
                core.onFeatureKey(i, false);
            }
        }
    }

    /**
     * On the tick the window loses focus, every key is reported released, so no HOLD key stays held while the player is
     * in another window (GLFW and SDL also send key releases on focus loss; this does not rely on it). A key that
     * still reads down afterwards counts again with its next click.
     */
    static void focus(HengstCore core, KeyMapping[] mappings, boolean focused) {
        if (windowFocused && !focused) {
            for (int i = 0; i < mappings.length; i++) {
                core.onFeatureKey(i, false);
            }
        }
        windowFocused = focused;
    }
}
