package io.github.clemensjl.hengst.mod.common.ui;

import io.github.clemensjl.hengst.mod.common.platform.Canvas;

public class ButtonWidget extends Widget {
    private final String label;
    private final Runnable action;

    public ButtonWidget(String label, Runnable action) {
        this.label = label;
        this.action = action;
    }

    @Override
    public void render(Canvas c, int mx, int my) {
        c.fill(x, y, x + w, y + h, inside(mx, my) ? BG_HOVER : BG);
        drawCentered(c, label);
    }

    @Override
    public boolean mouseClicked(int mx, int my, int button) {
        if (button != 0 || !inside(mx, my)) {
            return false;
        }
        action.run();
        return true;
    }
}
