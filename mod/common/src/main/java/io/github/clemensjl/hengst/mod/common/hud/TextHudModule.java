package io.github.clemensjl.hengst.mod.common.hud;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.feature.FeatureGroup;
import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.ui.TextFit;
import java.util.EnumSet;

/**
 * A HUD module that shows one line of cached text on an optional background. Empty text has size 0 and draws nothing.
 * A text wider than the screen offers (canvas width divided by the module's scale, minus the padding) is drawn cut in
 * the middle with {@link TextFit#middle}; {@link #width} reports the fitted width. The fitted text is cached by the
 * text reference and the available width, so it is built once per change and costs nothing per frame.
 */
public abstract class TextHudModule extends HudModule {
    protected static final int PAD = 2;
    private static final int BACKGROUND = 0x6F000000;

    private String text = "";
    private int textWidth = -1;
    private String fitted = "";
    private String fittedFrom;
    private int fittedAvail;
    private int fittedWidth;

    protected TextHudModule(String id, int number, EnumSet<GameVersion> versions) {
        super(id, number, versions);
    }

    protected TextHudModule(String id, int number, FeatureGroup group, EnumSet<GameVersion> versions) {
        super(id, number, group, versions);
    }

    /** Keeps the current reference when the text is equal, otherwise stores it and drops the cached width. */
    protected final void setText(String text) {
        String t = text == null ? "" : text;
        if (!t.equals(this.text)) {
            this.text = t;
            textWidth = -1;
        }
    }

    @Override
    public void onLanguageChanged() {
        textWidth = -1;
        fittedFrom = null;
    }

    /** Brings {@link #fitted} up to date for the canvas and the scale; a no-op while text and space are unchanged. */
    private void fit(Canvas c, ModuleConfig cfg) {
        int avail = (int) (c.width() / cfg.scale()) - 2 * PAD;
        if (fittedFrom == text && fittedAvail == avail) {
            return;
        }
        fittedFrom = text;
        fittedAvail = avail;
        if (text.isEmpty()) {
            fitted = text;
            fittedWidth = 0;
            return;
        }
        if (textWidth < 0) {
            textWidth = c.textWidth(text);
        }
        if (textWidth <= avail) {
            fitted = text;
            fittedWidth = textWidth;
        } else {
            fitted = TextFit.middle(c, text, avail);
            fittedWidth = c.textWidth(fitted);
        }
    }

    public final String text() {
        return text;
    }

    @Override
    public int width(Canvas c, ModuleConfig cfg) {
        fit(c, cfg);
        return fitted.isEmpty() ? 0 : fittedWidth + 2 * PAD;
    }

    @Override
    public int height(Canvas c, ModuleConfig cfg) {
        fit(c, cfg);
        return fitted.isEmpty() ? 0 : c.fontHeight() + 2 * PAD;
    }

    @Override
    public void render(Canvas c, ModuleConfig cfg, HudContext ctx) {
        int w = width(c, cfg);
        if (w == 0) {
            return;
        }
        if (cfg.background()) {
            c.fill(0, 0, w, height(c, cfg), BACKGROUND);
        }
        c.text(fitted, PAD, PAD, cfg.textColor(), cfg.shadow());
    }
}
