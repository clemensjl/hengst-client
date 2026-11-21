package io.github.clemensjl.hengst.mod.modern;

import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
//? if >=26.1 {
import net.minecraft.client.gui.GuiGraphicsExtractor;
//?} else {
/*import net.minecraft.client.gui.GuiGraphics;
*///?}
import org.joml.Matrix3x2fStack;

/**
 * The core's canvas over the game's GUI draw context. One instance per call site, rebound with {@link #begin} every
 * frame: nothing is allocated here per frame.
 */
public final class ModernCanvas implements Canvas {
    //? if >=26.1 {
    private GuiGraphicsExtractor g;
    //?} else {
    /*private GuiGraphics g;
    *///?}
    private Font font;

    /** Binds the canvas to this frame's draw context. */
    //? if >=26.1 {
    public void begin(GuiGraphicsExtractor graphics) {
    //?} else {
    /*public void begin(GuiGraphics graphics) {
    *///?}
        this.g = graphics;
        this.font = Minecraft.getInstance().font;
    }

    @Override
    public int width() {
        return g.guiWidth();
    }

    @Override
    public int height() {
        return g.guiHeight();
    }

    @Override
    public int fontHeight() {
        return font.lineHeight;
    }

    @Override
    public int textWidth(String text) {
        return font.width(text);
    }

    @Override
    public void fill(int x1, int y1, int x2, int y2, int argb) {
        g.fill(x1, y1, x2, y2, argb);
    }

    @Override
    public void text(String text, int x, int y, int argb, boolean shadow) {
        //? if >=26.1 {
        g.text(font, text, x, y, argb, shadow);
        //?} else {
        /*g.drawString(font, text, x, y, argb, shadow);
        *///?}
    }

    @Override
    public void pushTransform(float translateX, float translateY, float scale) {
        Matrix3x2fStack pose = g.pose();
        pose.pushMatrix();
        pose.translate(translateX, translateY);
        pose.scale(scale);
    }

    @Override
    public void popTransform() {
        g.pose().popMatrix();
    }
}
