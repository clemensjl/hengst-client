package io.github.clemensjl.hengst.mod.common.layout;

/** Reusable pixel rectangle; filled by {@link LayoutMath#resolve} so the frame path allocates nothing. */
public final class MutableRect {
    public int x;
    public int y;
    public int w;
    public int h;

    /** Half-open: the left and top edges are inside, the right and bottom edges are not. */
    public boolean contains(int px, int py) {
        return px >= x && py >= y && px < x + w && py < y + h;
    }
}
