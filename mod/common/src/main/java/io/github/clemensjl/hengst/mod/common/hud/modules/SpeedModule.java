package io.github.clemensjl.hengst.mod.common.hud.modules;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.feature.OptionSpec;
import io.github.clemensjl.hengst.mod.common.hud.HudContext;
import io.github.clemensjl.hengst.mod.common.hud.TextHudModule;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.platform.GameView;
import java.util.Arrays;
import java.util.EnumSet;
import java.util.List;

/**
 * Shows the speed in blocks per second, from the distance between the positions of two ticks, averaged over the last
 * up to 10 ticks and multiplied by 20. Horizontal unless {@code include_vertical} is on. The average and the text
 * restart (the tick only remembers the position) after a jump of more than 5 blocks in one tick (teleport,
 * ender pearl, chorus fruit, respawn: terminal fall velocity is 3.92 blocks per tick, so the shown value never
 * exceeds 100 m/s), on a new
 * world instance (dimension change, new world), on a position that is not finite and whenever the core applies the
 * configuration (module switched off and on, profile switch, server change: the position from before would count as
 * one step). Empty outside a world and while the server asks for reduced debug info.
 */
public final class SpeedModule extends TextHudModule {
    private static final OptionSpec VERTICAL = OptionSpec.toggle("include_vertical", false);
    private static final OptionSpec DECIMALS = OptionSpec.intRange("decimals", 1, 0, 2);
    private static final List<OptionSpec> OPTIONS = Arrays.asList(VERTICAL, DECIMALS);
    private static final int WINDOW = 10;
    private static final double TICKS_PER_SECOND = 20.0;
    private static final double MAX_STEP_SQUARED = 5.0 * 5.0;
    private static final long[] POW10 = {1L, 10L, 100L};
    private static final long NONE = Long.MIN_VALUE;

    private final double[] distances = new double[WINDOW];
    private final StringBuilder sb = new StringBuilder(24);
    private int count;
    private int next;
    private boolean havePosition;
    private double lastX;
    private double lastY;
    private double lastZ;
    private Object world;
    private long shownScaled = NONE;
    private int shownDecimals = -1;

    public SpeedModule() {
        super("speed", 15, EnumSet.of(GameVersion.V1_8_9, GameVersion.V1_21_11, GameVersion.V26_3));
    }

    @Override
    public int defaultOffsetX() {
        return 2;
    }

    @Override
    public int defaultOffsetY() {
        return 58;
    }

    @Override
    public List<OptionSpec> options() {
        return OPTIONS;
    }

    /**
     * Called by the core on every apply (start, settings or HUD editor closed, profile switch, server change). Rule: always
     * forget the position and the average. An apply is rare and never happens while a slider moves, and the one
     * extra "0.0" it can cause is harmless against a spike after a gap in which the module was not ticked.
     */
    @Override
    public void onConfigChanged(ModuleConfig cfg) {
        restart(false);
        world = null;   // a switched-off module is not ticked: never keep the world alive (GameView.worldIdentity)
    }

    @Override
    public void tick(HudContext ctx, ModuleConfig cfg) {
        GameView game = ctx.platform().game();
        if (!game.inWorld() || game.reducedDebugInfo()) {          // vanilla's F3 hides it too
            restart(false);
            world = null;
            shownScaled = NONE;
            setText("");
            return;
        }
        boolean vertical = cfg.bool(VERTICAL);
        int decimals = cfg.integer(DECIMALS);
        double x = game.playerX();
        double y = game.playerY();
        double z = game.playerZ();
        Object w = game.worldIdentity();
        boolean finite = Math.abs(x) < Double.POSITIVE_INFINITY && Math.abs(y) < Double.POSITIVE_INFINITY
                && Math.abs(z) < Double.POSITIVE_INFINITY; // false for NaN too
        if (!finite) {
            restart(false);
        } else if (w != world || !havePosition) {
            world = w;
            restart(true);
            lastX = x;
            lastY = y;
            lastZ = z;
        } else {
            double dx = x - lastX;
            double dy = vertical ? y - lastY : 0.0;
            double dz = z - lastZ;
            double sq = dx * dx + dy * dy + dz * dz;
            lastX = x;
            lastY = y;
            lastZ = z;
            if (sq > MAX_STEP_SQUARED) {
                restart(true);
            } else {
                distances[next] = Math.sqrt(sq);
                next = (next + 1) % WINDOW;
                if (count < WINDOW) {
                    count++;
                }
            }
        }
        double sum = 0.0;
        for (int i = 0; i < count; i++) {
            sum += distances[i];
        }
        double perSecond = count == 0 ? 0.0 : sum / count * TICKS_PER_SECOND;
        long scaled = (long) (perSecond * POW10[decimals] + 0.5);
        if (scaled == shownScaled && decimals == shownDecimals) {
            return;
        }
        shownScaled = scaled;
        shownDecimals = decimals;
        StringBuilder b = sb;
        b.setLength(0);
        long pow = POW10[decimals];
        b.append(scaled / pow);
        if (decimals > 0) {
            long frac = scaled % pow;
            b.append('.');
            for (long p = pow / 10; p > 0; p /= 10) {
                b.append((char) ('0' + (frac / p) % 10));
            }
        }
        b.append(" m/s");
        setText(b.toString());
    }

    /** Empties the average; {@code positionKnown} says whether the tick has just stored a position. */
    private void restart(boolean positionKnown) {
        count = 0;
        next = 0;
        havePosition = positionKnown;
    }
}
