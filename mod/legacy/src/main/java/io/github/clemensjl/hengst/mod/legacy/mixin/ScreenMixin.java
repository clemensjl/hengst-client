package io.github.clemensjl.hengst.mod.legacy.mixin;

import io.github.clemensjl.hengst.mod.legacy.HengstLegacyMod;
import net.minecraft.client.gui.screen.Screen;
import org.lwjgl.input.Mouse;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Mouse buttons while a screen is open. MinecraftClient.tick calls {@code currentScreen.handleInput()} first, and
 * {@code Screen.handleInput()V} drains LWJGL's mouse queue itself ({@code while (Mouse.next()) this.handleMouse();});
 * the tick's own loop, where KeyBindingMixin sees the buttons, then finds the queue empty. So without this hook
 * every press and release made while a screen is open would be missing. The injection sits at the call of
 * {@code handleMouse()} inside that loop, once per event and before any screen subclass (twenty override
 * handleMouse) can change or swallow it; no subclass overrides handleInput. These are the only two
 * {@code Mouse.next()} loops of the game, so every LWJGL mouse event reaches exactly one of the two hooks.
 * Move and wheel events (button -1) are dropped.
 */
@Mixin(Screen.class)
abstract class ScreenMixin {
    @Inject(method = "handleInput()V",
            at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/screen/Screen;handleMouse()V"),
            require = 1, allow = 1)
    private void hengst$onMouseEvent(CallbackInfo ci) {
        if (!HengstLegacyMod.ENABLED) {
            return;
        }
        int button = Mouse.getEventButton();
        if (button >= 0) {
            HengstLegacyMod.onMouseButton(button, Mouse.getEventButtonState());
        }
    }
}
