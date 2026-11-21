package io.github.clemensjl.hengst.mod.common.hud;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.feature.Feature;
import io.github.clemensjl.hengst.mod.common.feature.FeatureGroup;
import io.github.clemensjl.hengst.mod.common.layout.Anchor;
import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import java.util.EnumSet;

/** A feature drawn on the HUD. {@link #render} draws from (0, 0); the renderer positions and scales it. */
public abstract class HudModule extends Feature {
    protected HudModule(String id, int number, EnumSet<GameVersion> versions) {
        super(id, number, FeatureGroup.HUD, versions);
    }

    /** Where the module sits when a profile has no entry for it. */
    public Anchor defaultAnchor() {
        return Anchor.TOP_LEFT;
    }

    public int defaultOffsetX() {
        return 0;
    }

    public int defaultOffsetY() {
        return 0;
    }

    /** Called at 20 Hz while the module is active, also outside a world: tolerate that. Rebuild cached text here. */
    public void tick(HudContext ctx, ModuleConfig cfg) {}

    /**
     * The language changed, which in the game comes with a resource reload: drop everything cached from a text
     * measurement (widths, cut text), the next draw measures again. Runs in the tick, also for modules that are off.
     */
    public void onLanguageChanged() {}

    /** Unscaled width in GUI pixels. */
    public abstract int width(Canvas c, ModuleConfig cfg);

    /** Unscaled height in GUI pixels. */
    public abstract int height(Canvas c, ModuleConfig cfg);

    /**
     * Must not allocate. Read options in {@link #tick} through the typed accessors
     * ({@link ModuleConfig#bool}, {@link ModuleConfig#integer}, {@link ModuleConfig#decimal},
     * {@link ModuleConfig#choice}) and cache them in primitive fields.
     */
    public abstract void render(Canvas c, ModuleConfig cfg, HudContext ctx);
}
