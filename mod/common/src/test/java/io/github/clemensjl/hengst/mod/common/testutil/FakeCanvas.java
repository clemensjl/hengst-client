package io.github.clemensjl.hengst.mod.common.testutil;

import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import java.util.ArrayList;
import java.util.List;

/** 320 x 240 canvas that logs every call. */
public class FakeCanvas implements Canvas {
    public final List<String> calls = new ArrayList<String>();

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
        calls.add("fill " + x1 + " " + y1 + " " + x2 + " " + y2 + " " + String.format("%08x", argb));
    }

    @Override
    public void text(String text, int x, int y, int argb, boolean shadow) {
        calls.add("text " + text + " " + x + " " + y + " " + String.format("%08x", argb) + " " + shadow);
    }

    @Override
    public void pushTransform(float translateX, float translateY, float scale) {
        calls.add("push " + String.valueOf(translateX) + " " + String.valueOf(translateY) + " " + String.valueOf(scale));
    }

    @Override
    public void popTransform() {
        calls.add("pop");
    }
}
