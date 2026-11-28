package io.github.clemensjl.hengst.mod.modern;

import io.github.clemensjl.hengst.mod.common.platform.ScrollAccumulator;
import io.github.clemensjl.hengst.mod.common.platform.UiScreen;
//? if >=26.1 {
import net.minecraft.client.gui.GuiGraphicsExtractor;
//?} else {
/*import net.minecraft.client.gui.GuiGraphics;
*///?}
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.network.chat.Component;

/**
 * The game screen that hosts one of the core's screens. It draws nothing of its own (no background, no widgets),
 * forwards every input to the {@link UiScreen} and handles none itself, Escape included.
 */
public final class ModernScreenHost extends Screen {
    private final UiScreen ui;
    private final ModernCanvas canvas = new ModernCanvas();
    private final ScrollAccumulator scroll = new ScrollAccumulator(1.0);
    private boolean removed;

    public ModernScreenHost(UiScreen ui) {
        super(Component.empty());
        this.ui = ui;
    }

    /** True once the game removed this screen; {@link ModernPlatform#closeScreen()} then leaves it alone. */
    boolean isRemoved() {
        return removed;
    }

    //? if >=26.1 {
    @Override
    public void extractRenderState(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a) {
    //?} else {
    /*@Override
    public void render(GuiGraphics graphics, int mouseX, int mouseY, float a) {
    *///?}
        canvas.begin(graphics);
        // a is DeltaTracker.getGameTimeDeltaTicks() on both nodes (game ticks since the last frame), not a partial
        // tick; it is passed on as the partialTick argument, which the core ignores.
        ui.render(canvas, mouseX, mouseY, a);
    }

    /**
     * No vanilla background (blur, menu texture): the host draws only what the core's screen draws. Skipping the
     * super call also skips its deferred-subtitle flush; nothing is lost: the host is not an in-game UI screen, so
     * the game draws subtitles in the HUD pass instead of deferring them (1.21.11 Gui.render, 26.3
     * Hud.extractRenderState), and it flushes deferred subtitles again after the screen (1.21.11
     * GameRenderer.render, 26.3 Gui.extractRenderState). The mod's HUD is never deferred.
     */
    //? if >=26.1 {
    @Override
    public void extractBackground(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a) {
    //?} else {
    /*@Override
    public void renderBackground(GuiGraphics graphics, int mouseX, int mouseY, float a) {
    *///?}
    }

    @Override
    public boolean mouseClicked(MouseButtonEvent event, boolean doubleClick) {
        int button = ModernInput.button(event.button());
        return button != ModernInput.OTHER_BUTTON
                && ui.mouseClicked(ModernInput.guiPixel(event.x()), ModernInput.guiPixel(event.y()), button);
    }

    @Override
    public boolean mouseReleased(MouseButtonEvent event) {
        int button = ModernInput.button(event.button());
        return button != ModernInput.OTHER_BUTTON
                && ui.mouseReleased(ModernInput.guiPixel(event.x()), ModernInput.guiPixel(event.y()), button);
    }

    /** The event carries the absolute position; the deltas are not used. */
    @Override
    public boolean mouseDragged(MouseButtonEvent event, double dx, double dy) {
        int button = ModernInput.button(event.button());
        return button != ModernInput.OTHER_BUTTON
                && ui.mouseDragged(ModernInput.guiPixel(event.x()), ModernInput.guiPixel(event.y()), button);
    }

    /**
     * Vertical wheel as whole notches (+1 or -1 per call, positive away from the user): the game's sensitivity-scaled
     * and possibly fractional value is accumulated, so a touchpad does not scroll once per tiny event.
     */
    @Override
    public boolean mouseScrolled(double x, double y, double scrollX, double scrollY) {
        int notches = scroll.add(scrollY);
        int mx = ModernInput.guiPixel(x);
        int my = ModernInput.guiPixel(y);
        boolean handled = false;
        for (; notches > 0; notches--) {
            handled |= ui.mouseScrolled(mx, my, 1.0);
        }
        for (; notches < 0; notches++) {
            handled |= ui.mouseScrolled(mx, my, -1.0);
        }
        return handled;
    }

    /** Presses and key repeats (the game routes both here); Escape goes to the core's screen like any other key. */
    @Override
    public boolean keyPressed(KeyEvent event) {
        return ui.keyPressed(ModernInput.uiKey(event.key()), event.hasShiftDown(), event.hasControlDown());
    }

    @Override
    public boolean isPauseScreen() {
        return false;
    }

    /** Removal rule: mark removed first, then tell the core's screen, exactly once. */
    @Override
    public void removed() {
        if (removed) {
            return;
        }
        removed = true;
        scroll.reset();
        ui.closed();
    }
}
