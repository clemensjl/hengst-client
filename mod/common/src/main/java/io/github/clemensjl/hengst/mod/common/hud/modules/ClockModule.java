package io.github.clemensjl.hengst.mod.common.hud.modules;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.feature.OptionSpec;
import io.github.clemensjl.hengst.mod.common.hud.HudContext;
import io.github.clemensjl.hengst.mod.common.hud.TextHudModule;
import io.github.clemensjl.hengst.mod.common.layout.Anchor;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.platform.WallClock;
import java.util.Arrays;
import java.util.EnumSet;
import java.util.List;

/**
 * Shows the local wall-clock time ("09:05", "09:05:07", or "9:05 AM" in the 12-hour format). The time comes from the
 * context's {@link WallClock} only: local = utc + offset, split into hours, minutes and seconds by integer arithmetic
 * (floor modulo, so times before 1970 and negative offsets are right). A tick reads only {@code currentTimeMillis()}
 * and compares it; the offset is asked (once) and the text rebuilt only when the UTC time reaches the next minute or
 * second boundary determined at the last calculation, when it is earlier than at the last calculation (the system
 * clock was set back), and when an option or the language changes. The offset is therefore re-read at every boundary,
 * which follows a daylight saving change. The wall clock is not game information and gives no advantage.
 */
public final class ClockModule extends TextHudModule {
    private static final OptionSpec FORMAT = OptionSpec.choice("format", "24h", "24h", "12h");
    private static final OptionSpec SECONDS = OptionSpec.toggle("seconds", false);
    private static final List<OptionSpec> OPTIONS = Arrays.asList(FORMAT, SECONDS);
    private static final int TWELVE_HOUR = 1;
    private static final long MS_PER_SECOND = 1000L;
    private static final long MS_PER_MINUTE = 60_000L;
    private static final long MS_PER_HOUR = 3_600_000L;
    private static final long MS_PER_DAY = 86_400_000L;
    private static final int NOT_BUILT = -1;

    private final StringBuilder sb = new StringBuilder(16);
    private String am = "AM";
    private String pm = "PM";
    private long nextBoundaryUtc = Long.MIN_VALUE;
    private long lastUtc;
    private long shownUnit = -1L;
    private int builtFormat = NOT_BUILT;
    private boolean builtSeconds;
    private int builtGeneration = -1;

    public ClockModule() {
        super("clock", 9, EnumSet.of(GameVersion.V1_8_9, GameVersion.V1_21_11, GameVersion.V26_3));
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
        return 66;
    }

    @Override
    public List<OptionSpec> options() {
        return OPTIONS;
    }

    @Override
    public void tick(HudContext ctx, ModuleConfig cfg) {
        WallClock clock = ctx.wallClock();
        long utc = clock.currentTimeMillis();
        int format = cfg.choice(FORMAT);
        boolean seconds = cfg.bool(SECONDS);
        int generation = ctx.languageGeneration();
        boolean changed = format != builtFormat || seconds != builtSeconds || generation != builtGeneration;
        if (!changed && utc >= lastUtc && utc < nextBoundaryUtc) {
            return;
        }
        if (changed) {
            builtFormat = format;
            builtSeconds = seconds;
            builtGeneration = generation;
            am = ctx.i18n().get("hengst.clock.am");
            pm = ctx.i18n().get("hengst.clock.pm");
            shownUnit = -1L;
        }
        lastUtc = utc;
        long local = utc + clock.offsetMillis(utc);
        long unit = seconds ? MS_PER_SECOND : MS_PER_MINUTE;
        long dayMs = Math.floorMod(local, MS_PER_DAY);
        nextBoundaryUtc = utc + (unit - dayMs % unit);
        long unitOfDay = dayMs / unit;
        if (unitOfDay == shownUnit) {
            return;
        }
        shownUnit = unitOfDay;
        int hour = (int) (dayMs / MS_PER_HOUR);
        StringBuilder b = sb;
        b.setLength(0);
        if (format == TWELVE_HOUR) {
            b.append(hour % 12 == 0 ? 12 : hour % 12);
        } else {
            two(b, hour);
        }
        b.append(':');
        two(b, (int) (dayMs / MS_PER_MINUTE % 60));
        if (seconds) {
            b.append(':');
            two(b, (int) (dayMs / MS_PER_SECOND % 60));
        }
        if (format == TWELVE_HOUR) {
            b.append(' ').append(hour < 12 ? am : pm);
        }
        setText(b.toString());
    }

    private static void two(StringBuilder b, int v) {
        b.append((char) ('0' + v / 10)).append((char) ('0' + v % 10));
    }
}
