package io.github.clemensjl.hengst.mod.modern;

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
        ui.render(canvas, mouseX, mouseY, a);
    }

    /** No vanilla background (blur, menu texture): the host draws only what the core's screen draws. */
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

    /** Vertical wheel as the game delivers it: positive away from the user, one notch is 1 at default sensitivity. */
    @Override
    public boolean mouseScrolled(double x, double y, double scrollX, double scrollY) {
        return ui.mouseScrolled(ModernInput.guiPixel(x), ModernInput.guiPixel(y), scrollY);
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
        ui.closed();
    }
}
