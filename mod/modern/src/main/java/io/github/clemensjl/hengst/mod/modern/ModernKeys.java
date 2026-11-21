package io.github.clemensjl.hengst.mod.modern;

import com.mojang.blaze3d.platform.InputConstants;
import io.github.clemensjl.hengst.mod.common.FeatureCatalog;
import io.github.clemensjl.hengst.mod.common.HengstCore;
import io.github.clemensjl.hengst.mod.common.feature.DefaultKey;
import io.github.clemensjl.hengst.mod.common.feature.KeySpec;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import java.util.List;
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
     * From the client initializer, before the game options are created and before the core starts: one mapping per key
     * of {@code version}, in catalog order, so the index of a mapping is its index for {@link HengstCore#onFeatureKey}.
     */
    static void register(GameVersion version) {
        List<KeySpec> keys = FeatureCatalog.keys(version);
        KeyMapping[] built = new KeyMapping[keys.size()];
        for (int i = 0; i < built.length; i++) {
            KeySpec spec = keys.get(i);
            built[i] = registerMapping(new KeyMapping(spec.translationKey(), code(spec.defaultKey()), CATEGORY));
        }
        mappings = built;
    }

    /** This version's key code for a default key: GLFW key codes on 1.21.11, SDL scancodes on 26.3. */
    static int code(DefaultKey key) {
        switch (key) {
            case NONE:
                return InputConstants.UNKNOWN.getValue();
            case RIGHT_SHIFT:
                return InputConstants.KEY_RSHIFT;
            default:
                throw new IllegalArgumentException("no key code for " + key);
        }
    }

    private static KeyMapping registerMapping(KeyMapping mapping) {
        //? if >=26.1 {
        return KeyMappingHelper.registerKeyMapping(mapping);
        //?} else {
        /*return KeyBindingHelper.registerKeyBinding(mapping);
        *///?}
    }

    /** End of a client tick, before the core's tick. */
    static void dispatch(Minecraft client) {
        HengstCore core = HengstModernMod.core();
        focus(core, mappings, client.isWindowActive());
        dispatch(core, mappings);
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
