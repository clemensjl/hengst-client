package io.github.clemensjl.hengst.mod.legacy.gametest;

import io.github.clemensjl.hengst.mod.legacy.HengstLegacyMod;
import io.github.clemensjl.hengst.mod.legacy.LegacyGameTest;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Test run only: observes the mod's own hook entry points (reached from the mod's mixins), to record when the HUD
 * hook runs and every mouse button transition the two mouse mixins deliver.
 */
@Mixin(value = HengstLegacyMod.class, remap = false)
abstract class TestModProbeMixin {
    @Inject(method = "onHud(F)V", at = @At("HEAD"), remap = false)
    private static void hengstTest$hud(float tickDelta, CallbackInfo ci) {
        LegacyGameTest.stampHud();
    }

    @Inject(method = "onHud(F)V", at = @At("RETURN"), remap = false)
    private static void hengstTest$afterHud(float tickDelta, CallbackInfo ci) {
        LegacyGameTest.afterHud();
    }

    @Inject(method = "onMouseButton(IZ)V", at = @At("HEAD"), remap = false)
    private static void hengstTest$mouse(int lwjglButton, boolean pressed, CallbackInfo ci) {
        LegacyGameTest.recordMouse(lwjglButton, pressed);
    }
}
