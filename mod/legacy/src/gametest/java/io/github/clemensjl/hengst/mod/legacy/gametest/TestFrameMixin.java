package io.github.clemensjl.hengst.mod.legacy.gametest;

import io.github.clemensjl.hengst.mod.legacy.LegacyGameTest;
import net.minecraft.client.MinecraftClient;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Test run only: counts finished frames, so the harness can wait until a change has been drawn. */
@Mixin(MinecraftClient.class)
abstract class TestFrameMixin {
    @Inject(method = "runGameLoop()V", at = @At("TAIL"))
    private void hengstTest$frameDone(CallbackInfo ci) {
        LegacyGameTest.frames++;
    }
}
