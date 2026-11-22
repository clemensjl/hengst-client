package io.github.clemensjl.hengst.mod.common.testutil;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.hud.HudContext;
import io.github.clemensjl.hengst.mod.common.hud.HudModule;
import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import java.util.EnumSet;

/** HUD module with a fixed size whose render advances a {@link FakeTime} by a fixed amount and draws nothing. */
public final class TimedModule extends HudModule {
    private final int width;
    private final int height;
    private final FakeTime time;
    private final long renderNanos;

    public TimedModule(String id, int number, int width, int height, FakeTime time, long renderNanos) {
        super(id, number, EnumSet.allOf(GameVersion.class));
        this.width = width;
        this.height = height;
        this.time = time;
        this.renderNanos = renderNanos;
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
        time.now += renderNanos;
    }
}
