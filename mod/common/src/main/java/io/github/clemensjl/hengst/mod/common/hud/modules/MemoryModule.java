package io.github.clemensjl.hengst.mod.common.hud.modules;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.feature.OptionSpec;
import io.github.clemensjl.hengst.mod.common.hud.HudContext;
import io.github.clemensjl.hengst.mod.common.hud.TextHudModule;
import io.github.clemensjl.hengst.mod.common.layout.Anchor;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.platform.GameView;
import java.util.Collections;
import java.util.EnumSet;
import java.util.List;

/**
 * Shows the JVM heap: used and maximum megabytes (bytes shifted right by 20, rounded down) and the percent of the
 * maximum (rounded down). The values are read through {@link GameView} (the F3 screen shows the same figures) on the
 * first tick and then every 20 ticks; a new style applies at the next reading. A maximum that is not positive, or so
 * large that {@code used * 100} could overflow (above {@code Long.MAX_VALUE / 100}, which includes "no limit"), is
 * shown as used megabytes only. Values are never negative. The text is rebuilt only when a shown number changed.
 */
public final class MemoryModule extends TextHudModule {
    private static final OptionSpec STYLE = OptionSpec.choice("style", "both", "percent", "used_max", "both");
    private static final List<OptionSpec> OPTIONS = Collections.singletonList(STYLE);
    private static final int PERCENT = 0;
    private static final int USED_MAX = 1;
    private static final int INTERVAL_TICKS = 20;
    private static final long NO_PERCENT_ABOVE = Long.MAX_VALUE / 100;
    private static final long NONE = -1L;

    private final StringBuilder sb = new StringBuilder(32);
    private int untilSample;
    private int shownStyle = -1;
    private long shownUsedMb = NONE;
    private long shownMaxMb = NONE;
    private long shownPercent = NONE;

    public MemoryModule() {
        super("memory", 11, EnumSet.of(GameVersion.V1_8_9, GameVersion.V1_21_11, GameVersion.V26_3));
    }

    @Override
    public Anchor defaultAnchor() {
        return Anchor.TOP_RIGHT;
    }

    @Override
    public int defaultOffsetX() {
        return -2;
    }

    @Override
    public int defaultOffsetY() {
        return 94;
    }

    @Override
    public List<OptionSpec> options() {
        return OPTIONS;
    }

    @Override
    public void tick(HudContext ctx, ModuleConfig cfg) {
        if (untilSample > 0) {
            untilSample--;
            return;
        }
        untilSample = INTERVAL_TICKS - 1;
        GameView game = ctx.platform().game();
        long used = Math.max(0L, game.memoryUsedBytes());
        long max = game.memoryMaxBytes();
        int style = cfg.choice(STYLE);
        long usedMb = used >> 20;
        long maxMb = NONE;
        long percent = NONE;
        if (max > 0L && max <= NO_PERCENT_ABOVE) {
            maxMb = max >> 20;
            percent = Math.min(used, max) * 100L / max;
        }
        if (style == shownStyle && usedMb == shownUsedMb && maxMb == shownMaxMb && percent == shownPercent) {
            return;
        }
        shownStyle = style;
        shownUsedMb = usedMb;
        shownMaxMb = maxMb;
        shownPercent = percent;
        StringBuilder b = sb;
        b.setLength(0);
        b.append("RAM ");
        if (percent == NONE) {
            b.append(usedMb).append(" MB");
        } else if (style == PERCENT) {
            b.append(percent).append('%');
        } else if (style == USED_MAX) {
            b.append(usedMb).append('/').append(maxMb).append(" MB");
        } else {
            b.append(percent).append("% ").append(usedMb).append('/').append(maxMb).append(" MB");
        }
        setText(b.toString());
    }
}
