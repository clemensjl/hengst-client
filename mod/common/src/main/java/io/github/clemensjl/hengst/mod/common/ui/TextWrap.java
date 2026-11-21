package io.github.clemensjl.hengst.mod.common.ui;

import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import java.util.ArrayList;
import java.util.List;

/** Word wrapping with the canvas' own text measure. Cold path: call when the text or the width changes. */
public final class TextWrap {
    private TextWrap() {}

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
