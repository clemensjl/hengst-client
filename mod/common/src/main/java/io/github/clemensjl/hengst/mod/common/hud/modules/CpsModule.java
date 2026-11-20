package io.github.clemensjl.hengst.mod.common.hud.modules;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.feature.OptionSpec;
import io.github.clemensjl.hengst.mod.common.hud.HudContext;
import io.github.clemensjl.hengst.mod.common.hud.TextHudModule;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import java.util.Collections;
import java.util.EnumSet;
import java.util.List;

/** Shows clicks per second as "<left> | <right> CPS", or "<left> CPS" without the right button. */
public final class CpsModule extends TextHudModule {
    private static final OptionSpec SHOW_RIGHT = OptionSpec.toggle("show_right", true);
    private static final List<OptionSpec> OPTIONS = Collections.singletonList(SHOW_RIGHT);

    private final StringBuilder sb = new StringBuilder(24);
    private int lastLeft = -1;
    private int lastRight = -1;
    private boolean lastShowRight;

    public CpsModule() {
        super("cps", 2, EnumSet.allOf(GameVersion.class));
    }

    @Override
    public List<OptionSpec> options() {
        return OPTIONS;
    }

    @Override
    public void tick(HudContext ctx, ModuleConfig cfg) {
        boolean showRight = (Boolean) cfg.option(SHOW_RIGHT);
        int left = ctx.cps().cps(0);
        int right = showRight ? ctx.cps().cps(1) : 0;
        if (left != lastLeft || right != lastRight || showRight != lastShowRight) {
            lastLeft = left;
            lastRight = right;
            lastShowRight = showRight;
            sb.setLength(0);
            sb.append(left);
            if (showRight) {
                sb.append(" | ").append(right);
            }
            sb.append(" CPS");
            setText(sb.toString());
        }
    }
}
