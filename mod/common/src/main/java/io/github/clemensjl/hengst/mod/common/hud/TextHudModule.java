package io.github.clemensjl.hengst.mod.common.hud;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import java.util.EnumSet;

/** A HUD module that shows one line of cached text on an optional background. Empty text has size 0 and draws nothing. */
public abstract class TextHudModule extends HudModule {
    protected static final int PAD = 2;
    private static final int BACKGROUND = 0x6F000000;

    private String text = "";
    private int textWidth = -1;

    protected TextHudModule(String id, int number, EnumSet<GameVersion> versions) {
        super(id, number, versions);
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
    }

    public final String text() {
        return text;
    }

    @Override
    public int width(Canvas c, ModuleConfig cfg) {
        if (textWidth < 0) {
            textWidth = c.textWidth(text);
        }
        return text.isEmpty() ? 0 : textWidth + 2 * PAD;
    }

    @Override
    public int height(Canvas c, ModuleConfig cfg) {
        return text.isEmpty() ? 0 : c.fontHeight() + 2 * PAD;
    }

    @Override
    public void render(Canvas c, ModuleConfig cfg, HudContext ctx) {
        if (text.isEmpty()) {
            return;
        }
        if (cfg.background()) {
            c.fill(0, 0, width(c, cfg), height(c, cfg), BACKGROUND);
        }
        c.text(text, PAD, PAD, cfg.textColor(), cfg.shadow());
    }
}
