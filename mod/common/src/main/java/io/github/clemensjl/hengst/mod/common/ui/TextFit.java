package io.github.clemensjl.hengst.mod.common.ui;

import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import java.util.ArrayList;
import java.util.List;

/**
 * The one place that fits text into a pixel width with the canvas' own text measure. Cold paths: call when the text or
 * the width changes, never per frame. A text that already fits is returned as is, without allocating.
 */
public final class TextFit {
    private TextFit() {}

    /** The text itself if it fits {@code avail} pixels, else its longest prefix that fits; null gives "". */
    public static String prefix(Canvas c, String text, int avail) {
        if (text == null) {
            return "";
        }
        return prefix(c, text, avail, c.textWidth(text));
    }

    /**
     * Like {@link #prefix(Canvas, String, int)} for a caller that has measured the text already: {@code textWidth} is
     * the width of the whole (non-null) text, so a fitting text costs no further measurement. A cut never splits a
     * surrogate pair.
     */
    public static String prefix(Canvas c, String text, int avail, int textWidth) {
        int end = text.length();
        int tw = textWidth;
        while (tw > avail && end > 0) {
            end--;
            if (end > 0 && Character.isHighSurrogate(text.charAt(end - 1)) && Character.isLowSurrogate(text.charAt(end))) {
                end--;
            }
            tw = c.textWidth(text.substring(0, end));
        }
        return end == text.length() ? text : text.substring(0, end);
    }

    /**
     * The text itself if it fits, else its start and end joined by ".." keeping as many characters as fit (the extra
     * one of an odd count goes to the start); never splits a surrogate pair. Throws on null, unlike {@code prefix}.
     */
    public static String middle(Canvas c, String text, int avail) {
        if (c.textWidth(text) <= avail) {
            return text;
        }
        int len = text.length();
        for (int keep = len - 1; keep > 0; keep--) {
            int tail = keep / 2;
            int head = keep - tail;
            if (Character.isHighSurrogate(text.charAt(head - 1)) && Character.isLowSurrogate(text.charAt(head))) {
                head--;
            }
            int tailStart = len - tail;
            if (tail > 0 && Character.isHighSurrogate(text.charAt(tailStart - 1))
                    && Character.isLowSurrogate(text.charAt(tailStart))) {
                tailStart++;
            }
            String s = text.substring(0, head) + ".." + text.substring(tailStart);
            if (c.textWidth(s) <= avail) {
                return s;
            }
        }
        return "..";
    }

    /**
     * Greedy word wrap on spaces; a single word wider than the width gets a line of its own and is never broken (so
     * no surrogate pair is split either). Throws on null, unlike {@code prefix}.
     */
    public static String[] wrap(Canvas canvas, String text, int width) {
        List<String> lines = new ArrayList<String>();
        StringBuilder line = new StringBuilder();
        for (String word : text.split(" ")) {
            if (word.isEmpty()) {
                continue;
            }
            if (line.length() > 0 && canvas.textWidth(line + " " + word) > width) {
                lines.add(line.toString());
                line.setLength(0);
            }
            if (line.length() > 0) {
                line.append(' ');
            }
            line.append(word);
        }
        if (line.length() > 0) {
            lines.add(line.toString());
        }
        return lines.toArray(new String[0]);
    }
}
