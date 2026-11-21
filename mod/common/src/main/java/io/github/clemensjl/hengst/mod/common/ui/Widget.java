package io.github.clemensjl.hengst.mod.common.ui;

import io.github.clemensjl.hengst.mod.common.platform.Canvas;

/** A rectangle that draws itself and reacts to the mouse. Positions are set by the owner (screen or list). */
public abstract class Widget {
    static final int TEXT = 0xFFFFFFFF;
    static final int BG = 0xFF303030;
    static final int BG_HOVER = 0xFF454545;
    static final int ACCENT = 0xFF3A6EA5;

    public int x;
    public int y;
    public int w;
    public int h;

    // Fitted text: computed when the source text (by reference) or the available width changes, never per frame.
    private String fitSrc;
    private int fitAvail = -1;
    private String fitOut = "";
    private int fitOutWidth;

    public abstract void render(Canvas c, int mx, int my);

    /** True if the click was handled inside the widget. */
    public boolean mouseClicked(int mx, int my, int button) {
        return false;
    }

    public boolean mouseDragged(int mx, int my) {
        return false;
    }

    public void mouseReleased() {}

    final boolean inside(int mx, int my) {
        return mx >= x && mx < x + w && my >= y && my < y + h;
    }

    /** The text, or its longest prefix that fits {@code avail} pixels. */
    final String fitted(Canvas c, String text, int avail) {
        if (text != fitSrc || avail != fitAvail) {
            String s = TextFit.prefix(c, text, avail);
            int tw = c.textWidth(s);
            fitSrc = text;
            fitAvail = avail;
            fitOut = s;
            fitOutWidth = tw;
        }
        return fitOut;
    }

    final int fittedWidth() {
        return fitOutWidth;
    }

    final void drawCentered(Canvas c, String text) {
        String s = fitted(c, text, w - 4);
        c.text(s, x + (w - fitOutWidth) / 2, y + (h - c.fontHeight()) / 2, TEXT, true);
    }
}
