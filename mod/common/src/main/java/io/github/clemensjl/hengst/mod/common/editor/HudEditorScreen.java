package io.github.clemensjl.hengst.mod.common.editor;

import io.github.clemensjl.hengst.mod.common.I18n;
import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.hud.HudContext;
import io.github.clemensjl.hengst.mod.common.hud.HudModule;
import io.github.clemensjl.hengst.mod.common.layout.MutableRect;
import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import io.github.clemensjl.hengst.mod.common.platform.UiKey;
import io.github.clemensjl.hengst.mod.common.platform.UiScreen;

/**
 * The HUD editor screen. Drawn by common; the adapters only forward mouse and key events. It does not close itself:
 * {@code onClose} (see {@link HudEditorClose}) saves, rebuilds the renderer and closes the game screen.
 */
public final class HudEditorScreen implements UiScreen {
    private static final int DIM = 0x50000000;
    private static final int FRAME = 0x80FFFFFF;
    private static final int SELECTED = 0xFFFFAA00;
    private static final int LABEL = 0xFFFFFFFF;
    private static final int HINT_MARGIN = 4;

    private final HudEditorModel model;
    private final HudContext ctx;
    private final Runnable onClose;
    private final String hint;
    // Labels are built once when the editor opens, never per frame.
    private final String[] labels;
    // A module that threw while drawing is skipped for the rest of the session of this editor.
    private final boolean[] failed;
    private boolean closed;

    public HudEditorScreen(HudEditorModel model, HudContext ctx, I18n i18n, Runnable onClose) {
        this.model = model;
        this.ctx = ctx;
        this.onClose = onClose;
        this.hint = i18n.get("hengst.editor.hint");
        // The model only holds the arrays; the module names come from the features themselves.
        this.labels = new String[model.count()];
        this.failed = new boolean[model.count()];
        for (int i = 0; i < labels.length; i++) {
            labels[i] = i18n.get("hengst.feature." + model.module(i).id());
        }
    }

    @Override
    public void render(Canvas canvas, int mouseX, int mouseY, float partialTick) {
        model.layout(canvas);
        int sw = canvas.width();
        int sh = canvas.height();
        canvas.fill(0, 0, sw, sh, DIM);
        int selected = model.selected();
        for (int i = 0; i < model.count(); i++) {
            if (!model.visible(i)) {
                continue;
            }
            MutableRect r = model.rect(i);
            if (model.placeholder(i)) {
                canvas.text(labels[i], r.x + 1, r.y + 1, LABEL, true);
            } else if (!failed[i]) {
                drawModule(canvas, i, r);
            }
            frame(canvas, r, i == selected ? SELECTED : FRAME);
        }
        int hintX = Math.max(0, (sw - canvas.textWidth(hint)) / 2);
        canvas.text(hint, hintX, sh - canvas.fontHeight() - HINT_MARGIN, LABEL, true);
    }

    private void drawModule(Canvas canvas, int i, MutableRect r) {
        HudModule module = model.module(i);
        ModuleConfig cfg = model.config(i);
        canvas.pushTransform(r.x, r.y, cfg.scale());
        try {
            module.render(canvas, cfg, ctx);
        } catch (RuntimeException e) {
            failed[i] = true;
            ctx.platform().logWarning("HUD module " + module.id() + " failed in the editor: " + e);
        } finally {
            canvas.popTransform();
        }
    }

    private static void frame(Canvas canvas, MutableRect r, int argb) {
        int x2 = r.x + r.w;
        int y2 = r.y + r.h;
        canvas.fill(r.x, r.y, x2, r.y + 1, argb);
        canvas.fill(r.x, y2 - 1, x2, y2, argb);
        canvas.fill(r.x, r.y + 1, r.x + 1, y2 - 1, argb);
        canvas.fill(x2 - 1, r.y + 1, x2, y2 - 1, argb);
    }

    @Override
    public boolean mouseClicked(int mouseX, int mouseY, int button) {
        if (button != 0) {
            return false;
        }
        model.press(mouseX, mouseY);
        return true;
    }

    @Override
    public boolean mouseReleased(int mouseX, int mouseY, int button) {
        if (button != 0) {
            return false;
        }
        model.release();
        return true;
    }

    @Override
    public boolean mouseDragged(int mouseX, int mouseY, int button) {
        if (button != 0) {
            return false;
        }
        model.drag(mouseX, mouseY);
        return true;
    }

    @Override
    public boolean mouseScrolled(int mouseX, int mouseY, double amount) {
        model.scroll(amount);
        return true;
    }

    @Override
    public boolean keyPressed(UiKey key, boolean shift, boolean control) {
        int step = shift ? 10 : 1;
        switch (key) {
            case ESCAPE:
                model.release();
                runOnClose();
                return true;
            case DELETE:
                model.hideSelected();
                return true;
            case LEFT:
                model.nudge(-step, 0);
                return true;
            case RIGHT:
                model.nudge(step, 0);
                return true;
            case UP:
                model.nudge(0, -step);
                return true;
            case DOWN:
                model.nudge(0, step);
                return true;
            default:
                return false;
        }
    }

    @Override
    public void closed() {
        model.release();
        runOnClose();
    }

    private void runOnClose() {
        if (!closed) {
            closed = true;
            onClose.run();
        }
    }
}
