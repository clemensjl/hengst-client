package io.github.clemensjl.hengst.mod.legacy;

import com.mojang.blaze3d.platform.GlStateManager;
import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawableHelper;

/**
 * The core's canvas over 1.8.9's fixed-function GUI drawing. One instance per call site, rebound with {@link #begin}
 * every frame: nothing is allocated here per frame.
 *
 * <p>GL state. Each call leaves the state that vanilla's own {@code DrawableHelper.fill} leaves behind and that the
 * HUD and screen code is written against: blending off, 2D texturing on, colour white (1, 1, 1, 1). In detail:
 * <ul>
 * <li>{@link #fill}: {@code DrawableHelper.fill} itself enables blending with {@code blendFuncSeparate(770, 771, 1,
 * 0)}, disables texturing, sets the colour to the fill colour and draws, then enables texturing and disables
 * blending. The canvas then resets the colour, which vanilla leaves tinted.</li>
 * <li>{@link #text}: enables blending with the same blend function (the font renderer does not, so alpha would only
 * be cut by the alpha test), draws through {@code TextRenderer.draw(String, float, float, int, boolean)}, which
 * enables the alpha test and sets the colour, then disables blending and resets the colour. The alpha test stays
 * on, as the font renderer leaves it.</li>
 * <li>Transforms push and pop the model-view matrix ({@code GlStateManager.pushMatrix/popMatrix}).</li>
 * </ul>
 * At the HUD hook, vanilla enables blending, sets the same blend function and disables the alpha test itself right
 * after the mod's draw (before the chat), so nothing of this leaks into the chat or the player list.
 *
 * <p>Alpha. ARGB colours; alpha is honoured through blending, with two limits of 1.8.9's fixed-function path:
 * the font renderer treats a text colour with alpha below 4 ({@code (color & 0xFC000000) == 0}) as fully opaque, so
 * such text is skipped here instead of drawn opaque; and with the alpha test on ({@code alphaFunc(GL_GREATER, 0.1)}
 * during the HUD, always for text) fragments with alpha 25 or less (at most 0.1) are discarded, so text and, where
 * the test is on, fills that faint are invisible. A fill with alpha 0 is skipped.
 */
public final class LegacyCanvas implements Canvas {
    private TextRenderer font;
    private int width;
    private int height;

    /** Binds the canvas to this frame's font and scaled GUI size. */
    public void begin(TextRenderer font, int width, int height) {
        this.font = font;
        this.width = width;
        this.height = height;
    }

    @Override
    public int width() {
        return width;
    }

    @Override
    public int height() {
        return height;
    }

    @Override
    public int fontHeight() {
        return font.fontHeight;
    }

    @Override
    public int textWidth(String text) {
        return font.getStringWidth(text);
    }

    @Override
    public void fill(int x1, int y1, int x2, int y2, int argb) {
        if ((argb >>> 24) == 0) {
            return;
        }
        DrawableHelper.fill(x1, y1, x2, y2, argb);
        GlStateManager.color(1.0F, 1.0F, 1.0F, 1.0F);
    }

    @Override
    public void text(String text, int x, int y, int argb, boolean shadow) {
        if ((argb & 0xFC000000) == 0) {
            return;
        }
        GlStateManager.enableBlend();
        GlStateManager.blendFuncSeparate(770, 771, 1, 0);
        font.draw(text, (float) x, (float) y, argb, shadow);
        GlStateManager.disableBlend();
        GlStateManager.color(1.0F, 1.0F, 1.0F, 1.0F);
    }

    @Override
    public void pushTransform(float translateX, float translateY, float scale) {
        GlStateManager.pushMatrix();
        GlStateManager.translate(translateX, translateY, 0.0F);
        GlStateManager.scale(scale, scale, 1.0F);
    }

    @Override
    public void popTransform() {
        GlStateManager.popMatrix();
    }
}
