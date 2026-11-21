package io.github.clemensjl.hengst.mod.modern.mixin;

import io.github.clemensjl.hengst.mod.modern.HengstModernMod;
import net.minecraft.client.MouseHandler;
import net.minecraft.client.input.MouseButtonInfo;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Every physical mouse button press and release, before the game routes it to a screen or a key binding. No Fabric
 * API event fits: ScreenMouseEvents only fire while a screen is open, and there is no raw button event in game.
 */
@Mixin(MouseHandler.class)
abstract class MouseHandlerMixin {
    @Inject(method = "onButton", at = @At("HEAD"), require = 1)
    private void hengst$onButton(long handle, MouseButtonInfo rawButtonInfo, int action, CallbackInfo ci) {
        if (!HengstModernMod.ENABLED) {
            return;
        }
        // action: 1 press, 0 release (26.3 SDLEventHandler.handleMouseButtonEvent, 1.21.11 GLFW_PRESS/GLFW_RELEASE
        // through MouseHandler.setup). The button code is mapped through the node's InputConstants.
        HengstModernMod.onMouseButton(rawButtonInfo.button(), action == 1);
    }
}
