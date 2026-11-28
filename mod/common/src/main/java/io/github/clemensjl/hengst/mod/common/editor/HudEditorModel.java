package io.github.clemensjl.hengst.mod.common.editor;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.config.Profile;
import io.github.clemensjl.hengst.mod.common.config.ProfileStore;
import io.github.clemensjl.hengst.mod.common.hud.HudModule;
import io.github.clemensjl.hengst.mod.common.layout.LayoutMath;
import io.github.clemensjl.hengst.mod.common.layout.MutableRect;
import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import io.github.clemensjl.hengst.mod.common.ui.ScreenClose;
import java.util.function.Consumer;

/**
 * State of the HUD editor: module rectangles, selection and the drag. Index {@code i} refers to the arrays given to
 * the constructor, a higher index is drawn later and so is on top. Hidden modules (disabled) are ignored.
 * Allocation free after construction. Not thread-safe: client thread only.
 */
public final class HudEditorModel {
    public static final int SNAP = 4;
    /** A module without size (for example empty text) is edited as a box of at least this size. */
    public static final int MIN_WIDTH = 20;
    public static final int MIN_HEIGHT = 10;

    private static final float SCALE_STEP = 0.1f;

    private final HudModule[] modules;
    private final ModuleConfig[] configs;
    private final MutableRect[] rects;
    // Unscaled size as reported by the module, 0 if it has none (or threw).
    private final int[] baseW;
    private final int[] baseH;
    private final boolean[] placeholder;
    // A module whose width or height threw is not asked again while the editor is open.
    private final boolean[] broken;
    private Consumer<String> warn;
    private int screenW;
    private int screenH;
    private int selected = -1;
    private boolean dragging;
    private int grabX;
    private int grabY;
    private boolean dirty;

    public HudEditorModel(HudModule[] modules, ModuleConfig[] configs) {
        this.modules = modules;
        this.configs = configs;
        int n = modules.length;
        rects = new MutableRect[n];
        for (int i = 0; i < n; i++) {
            rects[i] = new MutableRect();
        }
        baseW = new int[n];
        baseH = new int[n];
        placeholder = new boolean[n];
        broken = new boolean[n];
    }

    /** Where problems with modules are reported (logged once per module); none by default. */
    public void setWarningSink(Consumer<String> warn) {
        this.warn = warn;
    }

    public HudModule module(int index) {
        return modules[index];
    }

    public ModuleConfig config(int index) {
        return configs[index];
    }

    public int count() {
        return modules.length;
    }

    /** Whether the module is shown (not hidden in the editor). */
    public boolean visible(int index) {
        return configs[index].enabled();
    }

    /** True if the module reports no size, so the editor shows a minimum box for it. */
    public boolean placeholder(int index) {
        return placeholder[index];
    }

    /** Recomputes every rectangle for the canvas; call once per frame before drawing and hit testing. */
    public void layout(Canvas c) {
        screenW = c.width();
        screenH = c.height();
        for (int i = 0; i < modules.length; i++) {
            int w = 0;
            int h = 0;
            if (!broken[i]) {
                try {
                    w = modules[i].width(c, configs[i]);
                    h = modules[i].height(c, configs[i]);
                } catch (RuntimeException e) {
                    broken[i] = true;
                    w = 0;
                    h = 0;
                    if (warn != null) {
                        warn.accept("HUD module " + modules[i].id() + " failed to report its size in the editor: " + e);
                    }
                }
            }
            baseW[i] = w;
            baseH[i] = h;
            place(i);
        }
    }

    public MutableRect rect(int index) {
        return rects[index];
    }

    /** Highest visible index whose rectangle contains the point, else -1. */
    public int hit(int mx, int my) {
        for (int i = modules.length - 1; i >= 0; i--) {
            if (configs[i].enabled() && rects[i].contains(mx, my)) {
                return i;
            }
        }
        return -1;
    }

    /** Selects the module under the point (or clears the selection) and starts dragging it. */
    public void press(int mx, int my) {
        dragging = false;
        selected = hit(mx, my);
        if (selected >= 0) {
            grabX = mx - rects[selected].x;
            grabY = my - rects[selected].y;
            dragging = true;
        }
    }

    /** Moves the dragged module so the grabbed point stays under the cursor, snapped and clamped into the screen. */
    public void drag(int mx, int my) {
        if (!dragging || selected < 0) {
            return;
        }
        MutableRect r = rects[selected];
        int x = snap(mx - grabX, r.w, screenW, true);
        int y = snap(my - grabY, r.h, screenH, false);
        moveTo(selected, x, y);
    }

    public void release() {
        dragging = false;
    }

    public boolean dragging() {
        return dragging;
    }

    /** Changes the scale of the selected module by one step in the direction of the sign of {@code amount}. */
    public void scroll(double amount) {
        if (selected < 0 || placeholder[selected] || amount == 0 || Double.isNaN(amount)) {
            return;
        }
        ModuleConfig cfg = configs[selected];
        float before = cfg.scale();
        cfg.setScale(before + (amount > 0 ? SCALE_STEP : -SCALE_STEP));
        if (cfg.scale() != before) {
            dirty = true;
            place(selected);
        }
    }

    /** Ignored while a drag is running: the next drag event would override it. */
    public void nudge(int dx, int dy) {
        if (selected < 0 || dragging) {
            return;
        }
        MutableRect r = rects[selected];
        moveTo(selected, r.x + dx, r.y + dy);
    }

    public void hideSelected() {
        if (selected < 0) {
            return;
        }
        configs[selected].setEnabled(false);
        selected = -1;
        dragging = false;
        dirty = true;
    }

    public int selected() {
        return selected;
    }

    public boolean dirty() {
        return dirty;
    }

    /** The save step of the editor's close: writes the profile only if the layout changed, then marks it saved. */
    public ScreenClose.Save saver(ProfileStore store, String profileName, Profile profile) {
        return () -> {
            if (dirty) {
                store.save(profileName, profile);
                markSaved();
            }
        };
    }

    /** Called after the layout was written to disk. */
    public void markSaved() {
        dirty = false;
    }

    private void moveTo(int i, int x, int y) {
        MutableRect r = rects[i];
        int nx = Math.max(0, Math.min(Math.max(0, screenW - r.w), x));
        int ny = Math.max(0, Math.min(Math.max(0, screenH - r.h), y));
        int ox = r.x;
        int oy = r.y;
        if (nx == ox && ny == oy) {
            return; // the stored anchor and offsets stay as they are
        }
        LayoutMath.reanchor(configs[i], nx, ny, r.w, r.h, screenW, screenH);
        place(i);
        dirty = true;
    }

    /** Sizes rect i from the base size and the scale and resolves its position from the config. */
    private void place(int i) {
        float scale = configs[i].scale();
        int w = Math.round(baseW[i] * scale);
        int h = Math.round(baseH[i] * scale);
        boolean none = w <= 0 || h <= 0;
        placeholder[i] = none;
        if (none) {
            w = Math.max(w, MIN_WIDTH);
            h = Math.max(h, MIN_HEIGHT);
        }
        LayoutMath.resolve(configs[i], w, h, screenW, screenH, rects[i]);
    }

    /**
     * Returns the position of the dragged module along one axis after snapping: the nearest candidate (screen edges,
     * screen centre, edges of the other visible modules) within SNAP of the module's start, middle or end wins.
     */
    private int snap(int raw, int size, int screen, boolean xAxis) {
        int bestDist = SNAP + 1;
        int bestDelta = 0;
        // Screen edges and centre.
        for (int p = 0; p < 3; p++) {
            int at = raw + (p == 0 ? 0 : p == 1 ? size / 2 : size);
            for (int k = 0; k < 3; k++) {
                int target = k == 0 ? 0 : k == 1 ? screen : screen / 2;
                int d = Math.abs(target - at);
                if (d < bestDist) {
                    bestDist = d;
                    bestDelta = target - at;
                }
            }
            for (int j = 0; j < rects.length; j++) {
                if (j == selected || !configs[j].enabled()) {
                    continue;
                }
                MutableRect o = rects[j];
                int a = xAxis ? o.x : o.y;
                int b = a + (xAxis ? o.w : o.h);
                int da = Math.abs(a - at);
                if (da < bestDist) {
                    bestDist = da;
                    bestDelta = a - at;
                }
                int db = Math.abs(b - at);
                if (db < bestDist) {
                    bestDist = db;
                    bestDelta = b - at;
                }
            }
        }
        return bestDist <= SNAP ? raw + bestDelta : raw;
    }
}
