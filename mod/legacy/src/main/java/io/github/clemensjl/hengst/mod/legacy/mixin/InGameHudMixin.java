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
 * <p>Anchor. The slice starts at {@code this.client.world.getScoreboard()} (the only call of it in the method, owner
 * {@code ClientWorld} in the bytecode) and ends at {@code ChatHud.render(I)V}; inside it the first
 * {@code enableBlend()} is the one of the chat block. Between the anchor and the injection point vanilla only looks
 * up the team's sidebar objective and draws the sidebar ({@code renderScoreboardObjective}); before the anchor it
 * draws the title and the overlay message. So the draw position is the same as with the earlier anchor on
 * {@code renderScoreboardObjective}, which sidebar mods (and this mod's planned scoreboard feature) are likely to
 * redirect, while the scoreboard getter is not.
 *
 * <p>Trade-off, kept on purpose: with {@code require = 1} in a required mixin config, a mod that removes or redirects
 * one of the two slice anchors or the {@code enableBlend()} call makes the game fail at start with a mixin error naming
 * this class, instead of the mod staying inert without a HUD. A silent optional injection would hide that conflict.
 *
 * <p>Why no event: Legacy Fabric API's {@code HudRenderCallback} (1.13.5) is injected at the first
 * {@code GlStateManager.color} after {@code ChatHud.render}, so it draws over the chat text and the player list.
 */
@Mixin(InGameHud.class)
abstract class InGameHudMixin {
    @Inject(method = "render(F)V",
            slice = @Slice(
                    from = @At(value = "INVOKE", target = "Lnet/minecraft/client/world/ClientWorld;getScoreboard()Lnet/minecraft/scoreboard/Scoreboard;"),
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
