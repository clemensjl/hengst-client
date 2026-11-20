package io.github.clemensjl.hengst.mod.common.testutil;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.hud.HudContext;
import io.github.clemensjl.hengst.mod.common.hud.HudModule;
import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import java.util.EnumSet;

/** HUD module with a fixed size that fills its rectangle white. */
public final class FixedSizeModule extends HudModule {
    private final int width;
    private final int height;

    public FixedSizeModule(String id, int number, int width, int height) {
        this(id, number, width, height, EnumSet.allOf(GameVersion.class));
    }

    public FixedSizeModule(String id, int number, int width, int height, EnumSet<GameVersion> versions) {
        super(id, number, versions);
        this.width = width;
        this.height = height;
    }

    @Override
    public int width(Canvas c, ModuleConfig cfg) {
        return width;
    }

    @Override
    public int height(Canvas c, ModuleConfig cfg) {
        return height;
    }

    @Override
    public void render(Canvas c, ModuleConfig cfg, HudContext ctx) {
        c.fill(0, 0, width, height, 0xFFFFFFFF);
    }
}
