package io.github.clemensjl.hengst.mod.common.platform;

/** Drawing surface for one frame. Coordinates are in scaled GUI pixels. */
public interface Canvas {
    int width();

    int height();

    int fontHeight();

    int textWidth(String text);

    void fill(int x1, int y1, int x2, int y2, int argb);

    void text(String text, int x, int y, int argb, boolean shadow);

    void pushTransform(float translateX, float translateY, float scale);

    void popTransform();
}
