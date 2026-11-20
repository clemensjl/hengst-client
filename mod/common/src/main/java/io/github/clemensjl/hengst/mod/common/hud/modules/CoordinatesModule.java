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
 * Shows the player position as "X 12  Y 64  Z -30". Values are floored at the chosen number of decimals
 * (like the block position at 0), never rounded. Empty outside a world.
 */
public final class CoordinatesModule extends TextHudModule {
    private static final OptionSpec DECIMALS = OptionSpec.intRange("decimals", 0, 0, 2);
    private static final List<OptionSpec> OPTIONS = Collections.singletonList(DECIMALS);
    private static final long[] POW10 = {1L, 10L, 100L};

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
        double scale = POW10[decimals];
        long x = (long) Math.floor(game.playerX() * scale);
        long y = (long) Math.floor(game.playerY() * scale);
        long z = (long) Math.floor(game.playerZ() * scale);
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

    /** Sign first, then the whole part, then exactly {@code decimals} digits. */
    private static void append(StringBuilder b, long scaled, int decimals) {
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
