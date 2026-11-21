package io.github.clemensjl.hengst.mod.legacy.gametest;

import io.github.clemensjl.hengst.mod.legacy.LegacyGameTest;
import net.minecraft.client.gui.hud.ChatHud;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Test run only: stamps when vanilla starts drawing the chat lines (draw-order check). */
@Mixin(ChatHud.class)
abstract class TestChatHudMixin {
    @Inject(method = "render(I)V", at = @At("HEAD"))
    private void hengstTest$chat(int ticks, CallbackInfo ci) {
        LegacyGameTest.stampChat();
    }
}
