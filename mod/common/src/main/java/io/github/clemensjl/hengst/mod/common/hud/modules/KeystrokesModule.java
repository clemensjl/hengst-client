package io.github.clemensjl.hengst.mod.common.hud.modules;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.feature.OptionSpec;
import io.github.clemensjl.hengst.mod.common.hud.HudContext;
import io.github.clemensjl.hengst.mod.common.hud.HudModule;
import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import io.github.clemensjl.hengst.mod.common.platform.GameKey;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.platform.InputView;
import java.util.Arrays;
import java.util.EnumSet;
import java.util.List;

/** W / A S D, optionally the mouse buttons and the space bar; a pressed key is drawn highlighted. */
public final class KeystrokesModule extends HudModule {
    private static final OptionSpec SHOW_MOUSE = OptionSpec.toggle("show_mouse", true);
    private static final OptionSpec SHOW_SPACE = OptionSpec.toggle("show_space", true);
    private static final List<OptionSpec> OPTIONS = Arrays.asList(SHOW_MOUSE, SHOW_SPACE);

    private static final int KEY = 18;
    private static final int GAP = 2;
    private static final int STEP = KEY + GAP;
    private static final int WIDTH = 3 * KEY + 2 * GAP;
    private static final int MOUSE_W = 28;
    private static final int SPACE_H = 10;
    private static final int PRESSED_FILL = 0xB0FFFFFF;
    private static final int PRESSED_TEXT = 0xFF000000;
    private static final int IDLE_FILL = 0x6F000000;
    private static final int LABEL_REFRESH_TICKS = 20;

    // Label slots 0..3 are W, A, S, D read from the platform; 4 and 5 are the fixed mouse labels.
    private static final GameKey[] KEYS = {GameKey.FORWARD, GameKey.LEFT, GameKey.BACK, GameKey.RIGHT};
    private static final int LMB = 4;
    private static final int RMB = 5;

    private final String[] labels = {"", "", "", "", "LMB", "RMB"};
    // What is drawn: the label, or its longest prefix that fits the cell; and that text's width. Refitted when a label changes.
    private final String[] shown = {"", "", "", "", "LMB", "RMB"};
    private final int[] labelWidths = {-1, -1, -1, -1, -1, -1};
    private int refreshIn;

    public KeystrokesModule() {
        super("keystrokes", 3, EnumSet.allOf(GameVersion.class));
    }

    @Override
    public List<OptionSpec> options() {
        return OPTIONS;
    }

    @Override
    public void tick(HudContext ctx, ModuleConfig cfg) {
        if (--refreshIn > 0) {
            return;
        }
        refreshIn = LABEL_REFRESH_TICKS;
        InputView input = ctx.platform().input();
        for (int i = 0; i < KEYS.length; i++) {
            String label = input.keyLabel(KEYS[i]);
            if (label == null) {
                label = "";
            }
            if (!label.equals(labels[i])) {
                labels[i] = label;
                labelWidths[i] = -1;
            }
        }
    }

    @Override
    public int width(Canvas c, ModuleConfig cfg) {
        return WIDTH;
    }

    @Override
    public int height(Canvas c, ModuleConfig cfg) {
        int h = 2 * KEY + GAP;
        if ((Boolean) cfg.option(SHOW_MOUSE)) {
            h += KEY + GAP;
        }
        if ((Boolean) cfg.option(SHOW_SPACE)) {
            h += SPACE_H + GAP;
        }
        return h;
    }

    @Override
    public void render(Canvas c, ModuleConfig cfg, HudContext ctx) {
        InputView input = ctx.platform().input();
        cell(c, cfg, input, GameKey.FORWARD, 0, STEP, 0, KEY, KEY);
        cell(c, cfg, input, GameKey.LEFT, 1, 0, STEP, KEY, KEY);
        cell(c, cfg, input, GameKey.BACK, 2, STEP, STEP, KEY, KEY);
        cell(c, cfg, input, GameKey.RIGHT, 3, 2 * STEP, STEP, KEY, KEY);
        int y = 2 * STEP;
        if ((Boolean) cfg.option(SHOW_MOUSE)) {
            cell(c, cfg, input, GameKey.ATTACK, LMB, 0, y, MOUSE_W, KEY);
            cell(c, cfg, input, GameKey.USE, RMB, WIDTH - MOUSE_W, y, MOUSE_W, KEY);
            y += STEP;
        }
        if ((Boolean) cfg.option(SHOW_SPACE)) {
            c.fill(0, y, WIDTH, y + SPACE_H, input.isDown(GameKey.JUMP) ? PRESSED_FILL : IDLE_FILL);
        }
    }

    private void cell(Canvas c, ModuleConfig cfg, InputView input, GameKey key, int slot, int x, int y, int w, int h) {
        boolean down = input.isDown(key);
        c.fill(x, y, x + w, y + h, down ? PRESSED_FILL : IDLE_FILL);
        if (labelWidths[slot] < 0) {
            fit(c, slot, w);
        }
        String text = shown[slot];
        if (!text.isEmpty()) {
            c.text(text, x + (w - labelWidths[slot]) / 2, y + (h - c.fontHeight()) / 2,
                    down ? PRESSED_TEXT : cfg.textColor(), cfg.shadow() && !down);
        }
    }

    /** Runs once per label change (needs the canvas for measuring); allocates only when it has to cut. */
    private void fit(Canvas c, int slot, int cellWidth) {
        String text = labels[slot];
        int tw = c.textWidth(text);
        while (tw > cellWidth && !text.isEmpty()) {
            text = text.substring(0, text.length() - 1);
            tw = c.textWidth(text);
        }
        shown[slot] = text;
        labelWidths[slot] = tw;
    }
}
