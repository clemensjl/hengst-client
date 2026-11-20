package io.github.clemensjl.hengst.mod.common.hud.modules;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.feature.OptionSpec;
import io.github.clemensjl.hengst.mod.common.hud.HudContext;
import io.github.clemensjl.hengst.mod.common.hud.TextHudModule;
import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.platform.GameView;
import java.util.Collections;
import java.util.EnumSet;
import java.util.List;

/**
 * Shows the player position as "X 12  Y 64  Z -30". Without decimals the values are floored like the block position;
 * with decimals they are rounded to the nearest step, half away from zero. An axis that is not a sane number
 * (NaN, infinite, beyond 1e15) shows "?". Empty outside a world.
 */
public final class CoordinatesModule extends TextHudModule {
    private static final OptionSpec DECIMALS = OptionSpec.intRange("decimals", 0, 0, 2);
    private static final List<OptionSpec> OPTIONS = Collections.singletonList(DECIMALS);
    private static final long[] POW10 = {1L, 10L, 100L};
    private static final double MAX_ABS = 1.0e15;
    private static final long INVALID = Long.MIN_VALUE;

    private final StringBuilder sb = new StringBuilder(48);
    private boolean shown;
    private int lastDecimals = -1;
    private long lastX;
    private long lastY;
    private long lastZ;

    public CoordinatesModule() {
        super("coordinates", 6, EnumSet.allOf(GameVersion.class));
    }

    @Override
    public List<OptionSpec> options() {
        return OPTIONS;
    }

    @Override
    public void tick(HudContext ctx, ModuleConfig cfg) {
        GameView game = ctx.platform().game();
        if (!game.inWorld()) {
            shown = false;
            setText("");
            return;
        }
        int decimals = (Integer) cfg.option(DECIMALS);
        long x = scaled(game.playerX(), decimals);
        long y = scaled(game.playerY(), decimals);
        long z = scaled(game.playerZ(), decimals);
        if (shown && decimals == lastDecimals && x == lastX && y == lastY && z == lastZ) {
            return;
        }
        shown = true;
        lastDecimals = decimals;
        lastX = x;
        lastY = y;
        lastZ = z;
        StringBuilder b = sb;
        b.setLength(0);
        b.append("X ");
        append(b, x, decimals);
        b.append("  Y ");
        append(b, y, decimals);
        b.append("  Z ");
        append(b, z, decimals);
        setText(b.toString());
    }

    /** The value in steps of 10^-decimals as a long, or {@link #INVALID}. Never "negative zero": a magnitude that rounds to 0 has no sign. */
    private static long scaled(double v, int decimals) {
        if (!(Math.abs(v) <= MAX_ABS)) {
            return INVALID;
        }
        if (decimals == 0) {
            return (long) Math.floor(v);
        }
        long magnitude = Math.round(Math.abs(v) * POW10[decimals]);
        return v < 0 ? -magnitude : magnitude;
    }

    /** Sign first, then the whole part, then exactly {@code decimals} digits. */
    private static void append(StringBuilder b, long scaled, int decimals) {
        if (scaled == INVALID) {
            b.append('?');
            return;
        }
        long abs = scaled;
        if (scaled < 0) {
            b.append('-');
            abs = -scaled;
        }
        long pow = POW10[decimals];
        b.append(abs / pow);
        if (decimals > 0) {
            long frac = abs % pow;
            b.append('.');
            for (long p = pow / 10; p > 0; p /= 10) {
                b.append((char) ('0' + (frac / p) % 10));
            }
        }
    }

    @Override
    public int width(Canvas c, ModuleConfig cfg) {
        return text().isEmpty() ? 0 : super.width(c, cfg);
    }
}
