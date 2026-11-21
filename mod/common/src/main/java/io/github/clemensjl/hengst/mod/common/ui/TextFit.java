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
        String s = text;
        int tw = c.textWidth(s);
        while (tw > avail && !s.isEmpty()) {
            s = s.substring(0, s.length() - 1);
            tw = c.textWidth(s);
        }
        return s;
    }

    /** The text itself if it fits, else its start and end joined by ".." keeping as many characters as fit. */
    public static String middle(Canvas c, String text, int avail) {
        if (c.textWidth(text) <= avail) {
            return text;
        }
        for (int keep = text.length() - 1; keep > 0; keep--) {
            int tail = keep / 2;
            int head = keep - tail;
            String s = text.substring(0, head) + ".." + text.substring(text.length() - tail);
            if (c.textWidth(s) <= avail) {
                return s;
            }
        }
        return "..";
    }

    /** Greedy word wrap; a single word wider than the width gets a line of its own. */
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
