package io.github.clemensjl.hengst.mod.modern;

import com.mojang.blaze3d.platform.InputConstants;
//? if >=26.1 {
import net.fabricmc.fabric.api.client.keymapping.v1.KeyMappingHelper;
//?} else {
/*import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
*///?}
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.resources.Identifier;

/** The mod's two key bindings, in their own "Hengst" category of the vanilla controls screen. */
final class ModernKeys {
    private static KeyMapping settings;
    private static KeyMapping hudEditor;

    private ModernKeys() {
    }

    /** From the client initializer, before the game options are created. */
    static void register() {
        // Translation key of the category: key.category.hengst.main (Category.label()).
        KeyMapping.Category category = KeyMapping.Category.register(Identifier.fromNamespaceAndPath("hengst", "main"));
        settings = registerMapping(new KeyMapping("key.hengst.settings", InputConstants.KEY_RSHIFT, category));
        hudEditor = registerMapping(new KeyMapping("key.hengst.hud_editor", InputConstants.UNKNOWN.getValue(), category));
    }

    private static KeyMapping registerMapping(KeyMapping mapping) {
        //? if >=26.1 {
        return KeyMappingHelper.registerKeyMapping(mapping);
        //?} else {
        /*return KeyBindingHelper.registerKeyBinding(mapping);
        *///?}
    }

    /**
     * End of a client tick: hands every press counted since the last tick to the core, but only in a world. Presses
     * outside a world are drained so they do not fire later.
     */
    static void dispatch(Minecraft client) {
        boolean inWorld = client.player != null && client.level != null;
        while (settings.consumeClick()) {
            if (inWorld) {
                HengstModernMod.core().onOpenSettingsKey();
            }
        }
        while (hudEditor.consumeClick()) {
            if (inWorld) {
                HengstModernMod.core().onOpenHudEditorKey();
            }
        }
    }
}
