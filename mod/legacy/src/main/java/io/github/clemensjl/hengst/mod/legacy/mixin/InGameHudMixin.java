package io.github.clemensjl.hengst.mod.legacy.mixin;

import io.github.clemensjl.hengst.mod.legacy.HengstLegacyMod;
import net.minecraft.client.gui.hud.InGameHud;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.Slice;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * The mod's HUD, inside {@code InGameHud.render(F)V} after the scoreboard and directly before the chat block
 * ({@code GlStateManager.enableBlend()}, {@code blendFuncSeparate}, {@code disableAlphaTest}, translate, then
 * {@code ChatHud.render(I)V}). Order in that method: vignette, pumpkin, portal, hotbar, crosshair, boss bar, status
 * bars, sleep fade, experience bar, held item name, debug overlay (F3), overlay message, title, scoreboard, then the
 * mod, then chat text and the player list. GameRenderer.render calls InGameHud.render only in a world, before the
 * current screen renders (so every screen and its dim is drawn over the mod), and skips it under F1 unless a screen
 * is open (the core hides the mod under F1 itself). The matrix stack is at the HUD base (no translate pending).
 *
 * <p>Why no event: Legacy Fabric API's {@code HudRenderCallback} (1.13.5) is injected at the first
 * {@code GlStateManager.color} after {@code ChatHud.render}, so it draws over the chat text and the player list.
 */
@Mixin(InGameHud.class)
abstract class InGameHudMixin {
    @Inject(method = "render(F)V",
            slice = @Slice(
                    from = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/hud/InGameHud;renderScoreboardObjective(Lnet/minecraft/scoreboard/ScoreboardObjective;Lnet/minecraft/client/util/Window;)V"),
                    to = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/hud/ChatHud;render(I)V")),
            at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/platform/GlStateManager;enableBlend()V", ordinal = 0),
            require = 1, allow = 1)
    private void hengst$beforeChat(float tickDelta, CallbackInfo ci) {
        if (!HengstLegacyMod.ENABLED) {
            return;
        }
        HengstLegacyMod.onHud(tickDelta);
    }
}
