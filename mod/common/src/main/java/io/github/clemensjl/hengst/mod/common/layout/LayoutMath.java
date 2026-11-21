package io.github.clemensjl.hengst.mod.common.layout;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;

/** Turns a stored anchor plus offset into a pixel rectangle and back. Allocation free. */
public final class LayoutMath {
    private LayoutMath() {}

    /**
     * Resolves the module rectangle for the given screen. {@code w} and {@code h} are already scaled.
     * The result is clamped so the module is fully visible; a module larger than the screen starts at 0.
     */
    public static void resolve(ModuleConfig cfg, int w, int h, int screenW, int screenH, MutableRect out) {
        Anchor a = cfg.anchor();
        out.x = axis(a.fx(), cfg.offsetX(), w, screenW);
        out.y = axis(a.fy(), cfg.offsetY(), h, screenH);
        out.w = w;
        out.h = h;
    }

    /** Screen thirds: left/middle/right by column, top/middle/bottom by row. */
    public static Anchor anchorFor(int centerX, int centerY, int screenW, int screenH) {
        int col = third(centerX, screenW);
        int row = third(centerY, screenH);
        switch (row) {
            case 0:
                return col == 0 ? Anchor.TOP_LEFT : col == 1 ? Anchor.TOP_CENTER : Anchor.TOP_RIGHT;
            case 1:
                return col == 0 ? Anchor.MIDDLE_LEFT : col == 1 ? Anchor.CENTER : Anchor.MIDDLE_RIGHT;
            default:
                return col == 0 ? Anchor.BOTTOM_LEFT : col == 1 ? Anchor.BOTTOM_CENTER : Anchor.BOTTOM_RIGHT;
        }
    }

    /** Picks the anchor from the module centre and stores the offset so that resolve yields exactly x, y. */
    public static void reanchor(ModuleConfig cfg, int x, int y, int w, int h, int screenW, int screenH) {
        Anchor a = anchorFor(x + w / 2, y + h / 2, screenW, screenH);
        cfg.setAnchor(a);
        cfg.setOffsetX(x - base(a.fx(), w, screenW));
        cfg.setOffsetY(y - base(a.fy(), h, screenH));
    }

    private static int axis(float f, int offset, int size, int screen) {
        long pos = (long) base(f, size, screen) + offset;
        long max = Math.max(0, (long) screen - size);
        return (int) Math.max(0L, Math.min(max, pos));
    }

    private static int base(float f, int size, int screen) {
        return Math.round(f * (screen - size));
    }

    private static int third(int center, int screen) {
        long c3 = 3L * center;
        return c3 < screen ? 0 : c3 < 2L * screen ? 1 : 2;
    }
}
