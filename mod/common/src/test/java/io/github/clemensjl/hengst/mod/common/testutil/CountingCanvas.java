package io.github.clemensjl.hengst.mod.common.testutil;

import io.github.clemensjl.hengst.mod.common.platform.Canvas;

/** Same size as {@link FakeCanvas} but only counts calls in int fields, so it allocates nothing. */
public final class CountingCanvas implements Canvas {
    public int fills;
    public int texts;
    public int pushes;
    public int pops;

    @Override
    public int width() {
        return 320;
    }

    @Override
    public int height() {
        return 240;
    }

    @Override
    public int fontHeight() {
        return 9;
    }

    @Override
    public int textWidth(String text) {
        return 6 * text.length();
    }

    @Override
    public void fill(int x1, int y1, int x2, int y2, int argb) {
        fills++;
    }

    @Override
    public void text(String text, int x, int y, int argb, boolean shadow) {
        texts++;
    }

    @Override
    public void pushTransform(float translateX, float translateY, float scale) {
        pushes++;
    }

    @Override
    public void popTransform() {
        pops++;
    }
}
