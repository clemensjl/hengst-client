package io.github.clemensjl.hengst.mod.legacy;

import net.legacyfabric.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import org.lwjgl.input.Keyboard;

/**
 * The mod's two key bindings, in their own "Hengst" category of the vanilla controls screen (the category is the
 * translation key {@code key.categories.hengst}; KeyBinding's constructor adds it to the category set).
 */
final class LegacyKeys {
    static final String CATEGORY = "key.categories.hengst";
    private static KeyBinding settings;
    private static KeyBinding hudEditor;

    private LegacyKeys() {
    }

    /** From the client initializer. Legacy Fabric API adds the bindings to GameOptions.allKeys when it loads. */
    static void register() {
        settings = KeyBindingHelper.registerKeyBinding(
                new KeyBinding("key.hengst.settings", Keyboard.KEY_RSHIFT, CATEGORY));
        hudEditor = KeyBindingHelper.registerKeyBinding(
                new KeyBinding("key.hengst.hud_editor", Keyboard.KEY_NONE, CATEGORY));
    }

    /**
     * End of a client tick: hands every press counted since the last tick to the core, but only in a world. Presses
     * outside a world are drained so they do not fire later. KeyBinding.onKeyPressed counts a press only from the
     * tick's own keyboard loop, which runs while no screen is open (or one that passes events on).
     */
    static void dispatch(MinecraftClient client) {
        boolean inWorld = client.player != null && client.world != null;
        while (settings.wasPressed()) {
            if (inWorld) {
                HengstLegacyMod.core().onOpenSettingsKey();
            }
        }
        while (hudEditor.wasPressed()) {
            if (inWorld) {
                HengstLegacyMod.core().onOpenHudEditorKey();
            }
        }
    }
}
