package io.github.clemensjl.hengst.mod.common.ui;

import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import java.util.ArrayList;
import java.util.List;

/** A vertical list of equally tall rows. Rows not fully inside the list are neither drawn nor clickable. */
public final class ScrollList {
    private static final int NOTCH = 12;
    private static final int GAP = 2;

    private int x;
    private int y;
    private int w;
    private int h;
    private final List<Widget> widgets = new ArrayList<Widget>();
    private int rowHeight = 20;
    private int offset;
    private Widget pressed;

    public ScrollList(int x, int y, int w, int h) {
        setBounds(x, y, w, h);
    }

    public void setBounds(int x, int y, int w, int h) {
        this.x = x;
        this.y = y;
        this.w = w;
        this.h = h;
        clamp();
    }

    public void setWidgets(List<Widget> list, int rowHeight) {
        widgets.clear();
        widgets.addAll(list);
        this.rowHeight = rowHeight;
        pressed = null;
        clamp();
    }

    public int offset() {
        return offset;
    }

    /** One notch is 12 px; positive amounts scroll up (as the adapters deliver the wheel). */
    public void scroll(double amount) {
        offset -= (int) Math.round(amount * NOTCH);
        clamp();
    }

    boolean contains(int mx, int my) {
        return mx >= x && mx < x + w && my >= y && my < y + h;
    }

    List<Widget> widgets() {
        return widgets;
    }

    private void clamp() {
        int max = Math.max(0, widgets.size() * rowHeight - h);
        offset = Math.max(0, Math.min(max, offset));
    }

    private boolean place(int i) {
        Widget wd = widgets.get(i);
        int top = y + i * rowHeight - offset;
        wd.x = x;
        wd.y = top;
        wd.w = w;
        wd.h = rowHeight - GAP;
        return top >= y && top + rowHeight <= y + h;
    }

    public void render(Canvas c, int mx, int my) {
        for (int i = 0; i < widgets.size(); i++) {
            if (place(i)) {
                widgets.get(i).render(c, mx, my);
            }
        }
    }

    public boolean mouseClicked(int mx, int my, int button) {
        if (!contains(mx, my)) {
            return false;
        }
        for (int i = 0; i < widgets.size(); i++) {
            if (place(i) && widgets.get(i).mouseClicked(mx, my, button)) {
                pressed = widgets.get(i);
                return true;
            }
        }
        return false;
    }

    public boolean mouseDragged(int mx, int my) {
        return pressed != null && pressed.mouseDragged(mx, my);
    }

    public void mouseReleased() {
        if (pressed != null) {
            pressed.mouseReleased();
            pressed = null;
        }
    }
}
