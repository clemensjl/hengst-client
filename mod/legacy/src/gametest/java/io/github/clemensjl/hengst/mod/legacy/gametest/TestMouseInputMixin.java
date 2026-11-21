package io.github.clemensjl.hengst.mod.legacy.gametest;

import net.minecraft.client.MouseInput;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Test run only: the game never grabs, warps or reads the system cursor, so the window on the user's desktop does
 * not capture their mouse and their mouse movements do not turn the camera. Mouse events then carry absolute
 * window coordinates, which the harness writes into LWJGL's queue.
 */
@Mixin(MouseInput.class)
abstract class TestMouseInputMixin {
    @Inject(method = "lockMouse()V", at = @At("HEAD"), cancellable = true)
    private void hengstTest$noGrab(CallbackInfo ci) {
        ci.cancel();
    }

    @Inject(method = "grabMouse()V", at = @At("HEAD"), cancellable = true)
    private void hengstTest$noWarp(CallbackInfo ci) {
        ci.cancel();
    }

    @Inject(method = "updateMouse()V", at = @At("HEAD"), cancellable = true)
    private void hengstTest$noTurn(CallbackInfo ci) {
        MouseInput self = (MouseInput) (Object) this;
        self.x = 0;
        self.y = 0;
        ci.cancel();
    }
}
