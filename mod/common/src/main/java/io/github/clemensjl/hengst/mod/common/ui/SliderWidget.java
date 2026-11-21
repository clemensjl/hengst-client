package io.github.clemensjl.hengst.mod.common.ui;

import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import java.util.function.DoubleConsumer;
import java.util.function.DoubleSupplier;

/** A slider; click and drag set the value from the x position, snapped to the step and kept inside [min, max]. */
public class SliderWidget extends Widget {
    private final String label;
    private final float min;
    private final float max;
    private final float step;
    private final DoubleSupplier get;
    private final DoubleConsumer set;
    private boolean dragging;
    private double lastValue = Double.NaN;
    private String composed = "";

    public SliderWidget(String label, float min, float max, float step, DoubleSupplier get, DoubleConsumer set) {
        this.label = label;
        this.min = min;
        this.max = max;
        this.step = step;
        this.get = get;
        this.set = set;
    }

    @Override
    public void render(Canvas c, int mx, int my) {
        double v = get.getAsDouble();
        if (Double.compare(v, lastValue) != 0) {
            lastValue = v;
            composed = label + ": " + format(v);
        }
        c.fill(x, y, x + w, y + h, inside(mx, my) || dragging ? BG_HOVER : BG);
        double frac = max > min ? Math.max(0.0, Math.min(1.0, (v - min) / (max - min))) : 0.0;
        c.fill(x, y, x + (int) (frac * w), y + h, ACCENT);
        drawCentered(c, composed);
    }

    private String format(double v) {
        if (step >= 1f && step == Math.floor(step)) {
            return Long.toString(Math.round(v));
        }
        return Double.toString(Math.round(v * 100.0) / 100.0);
    }

    @Override
    public boolean mouseClicked(int mx, int my, int button) {
        if (button != 0 || !inside(mx, my)) {
            return false;
        }
        dragging = true;
        apply(mx);
        return true;
    }

    @Override
    public boolean mouseDragged(int mx, int my) {
        if (!dragging) {
            return false;
        }
        apply(mx);
        return true;
    }

    @Override
    public void mouseReleased() {
        dragging = false;
    }

    private void apply(int mx) {
        double frac = w <= 0 ? 0.0 : Math.max(0.0, Math.min(1.0, (mx - x) / (double) w));
        double v = min + frac * ((double) max - min);
        if (step > 0f) {
            double n = Math.round((v - min) / step);
            n = Math.min(n, Math.floor(((double) max - min) / step + 1e-6));
            v = min + n * (double) step;
        }
        set.accept(Math.max(min, Math.min(max, v)));
    }
}
