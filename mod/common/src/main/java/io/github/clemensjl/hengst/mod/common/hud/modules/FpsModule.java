package io.github.clemensjl.hengst.mod.common.hud.modules;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.hud.HudContext;
import io.github.clemensjl.hengst.mod.common.hud.TextHudModule;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import java.util.EnumSet;

/** Shows the frames per second as "<n> FPS". */
public final class FpsModule extends TextHudModule {
    private final StringBuilder sb = new StringBuilder(16);
    private int last = -1;

    public FpsModule() {
        super("fps", 1, EnumSet.of(GameVersion.V1_8_9, GameVersion.V1_21_11, GameVersion.V26_3));
    }

    @Override
    public void tick(HudContext ctx, ModuleConfig cfg) {
        int fps = ctx.frames().fps();
        if (fps != last) {
            last = fps;
            sb.setLength(0);
            sb.append(fps).append(" FPS");
            setText(sb.toString());
        }
    }
}
