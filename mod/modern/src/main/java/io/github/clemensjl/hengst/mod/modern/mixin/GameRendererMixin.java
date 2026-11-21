package io.github.clemensjl.hengst.mod.modern.mixin;

import io.github.clemensjl.hengst.mod.modern.HengstModernMod;
import net.minecraft.client.renderer.GameRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Once per rendered frame: Minecraft.renderFrame calls GameRenderer.render() exactly once for every frame it draws,
 * with or without a world, under F1 and with screens open. No Fabric API event fits: the HUD element stops under
 * F1 and without a world, LevelRenderEvents need a world, screen events need a screen.
 *
 * <p>Target per node: 26.3 {@code GameRenderer.render()}, single caller {@code Minecraft.renderFrame(boolean)};
 * 1.21.11 {@code GameRenderer.render(DeltaTracker, boolean)}, single caller {@code Minecraft.runTick(boolean)}
 * (guarded only by the debug field {@code noRender}, which vanilla never sets). Both callers count the vanilla FPS
 * with {@code this.frames++} a few lines later. If a game update reshapes the target, the fallback with the same
 * meaning is that {@code frames++} site (an {@code @At("FIELD", opcode = PUTFIELD)} on {@code Minecraft.frames} in
 * the render-frame method).
 */
@Mixin(GameRenderer.class)
abstract class GameRendererMixin {
    //? if >=26.1 {
    @Inject(method = "render()V", at = @At("HEAD"), require = 1)
    //?} else {
    /*@Inject(method = "render(Lnet/minecraft/client/DeltaTracker;Z)V", at = @At("HEAD"), require = 1)
    *///?}
    private void hengst$onFrame(CallbackInfo ci) {
        if (!HengstModernMod.ENABLED) {
            return;
        }
        HengstModernMod.onFrame();
    }
}
