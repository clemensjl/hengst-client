package io.github.clemensjl.hengst.mod.common.testutil;

import io.github.clemensjl.hengst.mod.common.platform.Canvas;

/**
 * A canvas of a chosen size with per-character widths (9 px for "WM@" and the umlauts, 2 px for '.', 6 px otherwise: an
 * internationalized name typed as-is is about 9 px per character). It records only numbers and the last text, so it
 * allocates nothing: the left-most and right-most pixel any text or fill reaches, in canvas pixels after the transform.
 */
public final class WideGlyphCanvas implements Canvas {
    private final int width;
    private final int height;
    private float tx;
    private float ty;
    private float scale = 1f;
    public double minLeft = Double.MAX_VALUE;
    public double maxRight = -Double.MAX_VALUE;
    public double maxFillRight = -Double.MAX_VALUE;
    public int texts;
    public int fills;
    public String lastText;

    public WideGlyphCanvas(int width, int height) {
        this.width = width;
        this.height = height;
    }

    public void reset() {
        minLeft = Double.MAX_VALUE;
        maxRight = -Double.MAX_VALUE;
        maxFillRight = -Double.MAX_VALUE;
        texts = 0;
        fills = 0;
        lastText = null;
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
        return 9;
    }

    @Override
    public int textWidth(String text) {
        int w = 0;
        for (int i = 0; i < text.length(); i++) {
            char c = text.charAt(i);
            w += c == '.' ? 2 : (c == 'W' || c == 'M' || c == '@' || c == 'ü' || c == 'ä') ? 9 : 6;
        }
        return w;
    }

    @Override
    public void fill(int x1, int y1, int x2, int y2, int argb) {
        fills++;
        maxFillRight = Math.max(maxFillRight, tx + x2 * (double) scale);
    }

    @Override
    public void text(String text, int x, int y, int argb, boolean shadow) {
        texts++;
        lastText = text;
        minLeft = Math.min(minLeft, tx + x * (double) scale);
        maxRight = Math.max(maxRight, tx + (x + textWidth(text)) * (double) scale);
    }

    @Override
    public void pushTransform(float translateX, float translateY, float scale) {
        this.tx = translateX;
        this.ty = translateY;
        this.scale = scale;
    }

    @Override
    public void popTransform() {
        tx = 0;
        ty = 0;
        scale = 1f;
    }
}
