package io.github.clemensjl.hengst.mod.legacy.mixin;

import io.github.clemensjl.hengst.mod.legacy.HengstLegacyMod;
import net.minecraft.client.option.KeyBinding;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Mouse buttons while no screen takes the input. MinecraftClient.tick reads LWJGL's mouse queue
 * ({@code while (Mouse.next())}) only when no screen is open or the screen passes events on, and calls the static
 * {@code KeyBinding.setKeyPressed(IZ)V} for every event with {@code Mouse.getEventButton() - 100} and the button
 * state, before it looks up a binding: so HEAD sees every press and release, bound or not. Move and wheel events
 * arrive as -101 and keyboard events with codes from 0 up; both are dropped. (The spec's
 * {@code setKeyBindState} is the MCP name; Legacy Yarn calls it {@code setKeyPressed}. {@code onKeyPressed(I)V} fires
 * on presses only.) With a screen open the screen drains the queue first: see ScreenMixin.
 */
@Mixin(KeyBinding.class)
abstract class KeyBindingMixin {
    @Inject(method = "setKeyPressed(IZ)V", at = @At("HEAD"), require = 1)
    private static void hengst$onSetKeyPressed(int code, boolean pressed, CallbackInfo ci) {
        if (!HengstLegacyMod.ENABLED) {
            return;
        }
        if (code < 0) {
            HengstLegacyMod.onKeyBindingMouseCode(code, pressed);
        }
    }
}
