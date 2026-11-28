package io.github.clemensjl.hengst.mod.common.ui;

import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import java.util.function.Consumer;
import java.util.function.Supplier;

/**
 * Cycles through values: left click forward, right click backward. The label is shown as "label: value"; a label
 * containing "%s" is used as a template for the value instead.
 */
public class ChoiceWidget extends Widget {
    private final String label;
    private final String[] values;
    private final Supplier<String> get;
    private final Consumer<String> set;
    private String lastValue;
    private int lastAvail = -1;
    private String display = "";
    private int displayWidth;

    public ChoiceWidget(String label, String[] values, Supplier<String> get, Consumer<String> set) {
        this.label = label;
        this.values = values.clone();
        this.get = get;
        this.set = set;
    }

    @Override
    public void render(Canvas c, int mx, int my) {
        c.fill(x, y, x + w, y + h, inside(mx, my) ? BG_HOVER : BG);
        String v = get.get();
        if (v == null) {
            v = "";
        }
        int avail = w - 4;
        if (lastValue == null || avail != lastAvail || !v.equals(lastValue)) {
            lastValue = v;
            lastAvail = avail;
            // Too narrow: drop the label part first, then shorten the value in the middle. The end of the value,
            // which tells similar values apart, is never the part that gets cut.
            String full = label.contains("%s") ? label.replace("%s", v) : label + ": " + v;
            display = c.textWidth(full) <= avail ? full : TextFit.middle(c, v, avail);
            displayWidth = c.textWidth(display);
        }
        c.text(display, x + (w - displayWidth) / 2, y + (h - c.fontHeight()) / 2, TEXT, true);
    }

    @Override
    public boolean mouseClicked(int mx, int my, int button) {
        if ((button != 0 && button != 1) || !inside(mx, my) || values.length == 0) {
            return false;
        }
        String cur = get.get();
        int idx = 0;
        for (int i = 0; i < values.length; i++) {
            if (values[i].equals(cur)) {
                idx = i;
                break;
            }
        }
        int next = button == 0 ? (idx + 1) % values.length : (idx - 1 + values.length) % values.length;
        set.accept(values[next]);
        return true;
    }
}
