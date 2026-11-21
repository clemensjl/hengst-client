package io.github.clemensjl.hengst.mod.common.ui;

import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import java.util.function.BooleanSupplier;
import java.util.function.Consumer;

/** A check box with an optional label; a click anywhere on the widget flips it. */
public class ToggleWidget extends Widget {
    private static final int BOX = 10;
    private static final int BOX_LEFT = 5;
    private static final int LABEL_LEFT = 20;

    private final String label;
    private final BooleanSupplier get;
    private final Consumer<Boolean> set;

    public ToggleWidget(String label, BooleanSupplier get, Consumer<Boolean> set) {
        this.label = label;
        this.get = get;
        this.set = set;
    }

    @Override
    public void render(Canvas c, int mx, int my) {
        c.fill(x, y, x + w, y + h, inside(mx, my) ? BG_HOVER : BG);
        int bx = x + BOX_LEFT;
        int by = y + (h - BOX) / 2;
        c.fill(bx, by, bx + BOX, by + BOX, 0xFFAAAAAA);
        c.fill(bx + 1, by + 1, bx + BOX - 1, by + BOX - 1, 0xFF202020);
        if (get.getAsBoolean()) {
            c.fill(bx + 2, by + 2, bx + BOX - 2, by + BOX - 2, 0xFF55FF55);
        }
        if (!label.isEmpty()) {
            String s = fitted(c, label, w - LABEL_LEFT - 2);
            c.text(s, x + LABEL_LEFT, y + (h - c.fontHeight()) / 2, TEXT, true);
        }
    }

    @Override
    public boolean mouseClicked(int mx, int my, int button) {
        if (button != 0 || !inside(mx, my)) {
            return false;
        }
        set.accept(!get.getAsBoolean());
        return true;
    }
}
