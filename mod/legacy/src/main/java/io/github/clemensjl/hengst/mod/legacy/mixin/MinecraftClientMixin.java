package io.github.clemensjl.hengst.mod.legacy.mixin;

import io.github.clemensjl.hengst.mod.legacy.HengstLegacyMod;
import net.minecraft.client.MinecraftClient;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Once per rendered frame: {@code MinecraftClient.runGameLoop()V} is the frame method. Its single caller is
 * {@code MinecraftClient.run()}, which calls it once per loop iteration; every call ticks, renders through
 * {@code GameRenderer.render(FJ)V} (with or without a world, under F1 and with screens open), swaps the display and
 * counts the vanilla FPS with {@code this.fpsCounter++}. No Legacy Fabric API event fits: the HUD stops under F1 and
 * without a world, the tick events run at 20 Hz.
 *
 * <p>Fallback with the same meaning if a game update reshapes the method: the {@code fpsCounter++} site, an
 * {@code @At(value = "FIELD", target = "Lnet/minecraft/client/MinecraftClient;fpsCounter:I", opcode = PUTFIELD,
 * ordinal = 0)} in runGameLoop (ordinal 1 is the reset to 0 once a second).
 */
@Mixin(MinecraftClient.class)
abstract class MinecraftClientMixin {
    @Inject(method = "runGameLoop()V", at = @At("HEAD"), require = 1)
    private void hengst$onFrame(CallbackInfo ci) {
        if (!HengstLegacyMod.ENABLED) {
            return;
        }
        HengstLegacyMod.onFrame();
    }
}
