package io.github.clemensjl.hengst.mod.common.hud.modules;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.feature.DefaultKey;
import io.github.clemensjl.hengst.mod.common.feature.FeatureGroup;
import io.github.clemensjl.hengst.mod.common.feature.KeySpec;
import io.github.clemensjl.hengst.mod.common.feature.OptionSpec;
import io.github.clemensjl.hengst.mod.common.hud.HudContext;
import io.github.clemensjl.hengst.mod.common.hud.TextHudModule;
import io.github.clemensjl.hengst.mod.common.layout.Anchor;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import java.util.Arrays;
import java.util.EnumSet;
import java.util.List;

/**
 * A stopwatch with two keys: {@code stopwatch.toggle} starts and stops (a stop keeps the time, the next start
 * continues from it), {@code stopwatch.reset} stops and sets the time to zero. Both keys are unbound by default and
 * are PRESS keys: a release does nothing, and the core delivers a press only while the feature is on, in a world and
 * without an open screen.
 *
 * <p>Time comes from the monotonic {@link io.github.clemensjl.hengst.mod.common.platform.TimeSource} only
 * ({@code nowNanos} of {@link #onKey}, {@code ctx.time()} in the tick), never from the wall clock, so setting the
 * system clock does not move it. It is real time: the single-player pause menu and leaving or switching a world do
 * not stop a running stopwatch. The module is ticked at 20 Hz while on, also outside a world.
 *
 * <p>Both keys on the same physical key: on the modern versions both bindings fire in the same tick in catalog order
 * (toggle, then reset), so the stopwatch ends stopped at zero; the settings hint asks for two different keys.
 *
 * <p>Text: under one hour {@code m:ss.t} ("0:00.0", "1:05.3", "59:59.9"), from one hour {@code h:mm:ss.t}
 * ("1:00:00.0", "24:00:00.0", "10000:00:00.0"; the hours are not capped and are computed in {@code long}, so a run
 * of years does not overflow); without the option {@code tenths} the {@code .t} is left out. Tenths are floored. The
 * text is rebuilt into one reused buffer only when the shown value (tenth, or second without tenths) or the option
 * changes: one String per shown change (10 per second while running with tenths), none while stopped.
 *
 * <p>State lives in memory for the session and is never saved. {@link #onConfigChanged} (profile switch, settings
 * closed, server change) does not reset it: it is user state, unlike a sample buffer. Only switching the feature off
 * stops a running stopwatch and keeps the elapsed value, so switching it on again shows the stopped value. The module
 * is not ticked while off and the change carries no clock, so the stop is taken at the last tick (at most 50 ms
 * before the switch-off). The stopwatch shows no game information and gives no advantage.
 */
public final class StopwatchModule extends TextHudModule {
    private static final OptionSpec TENTHS = OptionSpec.toggle("tenths", true);
    private static final List<OptionSpec> OPTIONS = Arrays.asList(TENTHS);
    private static final List<KeySpec> KEYS = Arrays.asList(
            new KeySpec("stopwatch.toggle", DefaultKey.NONE, KeySpec.Mode.PRESS),
            new KeySpec("stopwatch.reset", DefaultKey.NONE, KeySpec.Mode.PRESS));
    private static final int KEY_TOGGLE = 0;
    private static final int KEY_RESET = 1;
    private static final long NANOS_PER_TENTH = 100_000_000L;
    private static final long NANOS_PER_SECOND = 1_000_000_000L;
    private static final long NOTHING_SHOWN = -1L;

    // At most 19 digits of hours plus ":mm:ss.t": the buffer never grows.
    private final StringBuilder sb = new StringBuilder(32);
    private boolean running;
    private long startNanos;
    private long baseNanos;
    private long lastTickNanos;
    private long shownUnit = NOTHING_SHOWN;
    private boolean builtTenths;

    public StopwatchModule() {
        super("stopwatch", 21, FeatureGroup.COMFORT,
                EnumSet.of(GameVersion.V1_8_9, GameVersion.V1_21_11, GameVersion.V26_3));
    }

    @Override
    public Anchor defaultAnchor() {
        return Anchor.TOP_LEFT;
    }

    @Override
    public int defaultOffsetX() {
        return 2;
    }

    @Override
    public int defaultOffsetY() {
        return 72;
    }

    @Override
    public List<OptionSpec> options() {
        return OPTIONS;
    }

    /** The keys are unbound by default, so the settings tell the player where to bind them. */
    @Override
    public String hintKey() {
        return "hengst.hint.stopwatch";
    }

    @Override
    public List<KeySpec> keys() {
        return KEYS;
    }

    @Override
    public void onKey(int key, boolean down, long nowNanos) {
        if (!down) {
            return;
        }
        if (key == KEY_TOGGLE) {
            if (running) {
                baseNanos += Math.max(0L, nowNanos - startNanos);
                running = false;
            } else {
                startNanos = nowNanos;
                // The tick time has an arbitrary origin (nanoTime): before the first tick it is not comparable.
                lastTickNanos = nowNanos;
                running = true;
            }
        } else if (key == KEY_RESET) {
            running = false;
            baseNanos = 0L;
        }
    }

    /** Only a switch-off stops a running stopwatch; see the class comment. */
    @Override
    public void onConfigChanged(ModuleConfig cfg) {
        if (!cfg.enabled() && running) {
            baseNanos += Math.max(0L, lastTickNanos - startNanos);
            running = false;
        }
    }

    @Override
    public void tick(HudContext ctx, ModuleConfig cfg) {
        long now = ctx.time().nanoTime();
        lastTickNanos = now;
        boolean tenths = cfg.bool(TENTHS);
        long elapsed = running ? baseNanos + Math.max(0L, now - startNanos) : baseNanos;
        long unit = elapsed / (tenths ? NANOS_PER_TENTH : NANOS_PER_SECOND);
        if (unit == shownUnit && tenths == builtTenths) {
            return;
        }
        shownUnit = unit;
        builtTenths = tenths;
        long totalSeconds = elapsed / NANOS_PER_SECOND;
        long hours = totalSeconds / 3600;
        int minutes = (int) (totalSeconds / 60 % 60);
        int seconds = (int) (totalSeconds % 60);
        StringBuilder b = sb;
        b.setLength(0);
        if (hours > 0) {
            b.append(hours).append(':');
            two(b, minutes);
        } else {
            b.append(minutes);
        }
        b.append(':');
        two(b, seconds);
        if (tenths) {
            b.append('.').append((char) ('0' + (int) (elapsed / NANOS_PER_TENTH % 10)));
        }
        setText(b.toString());
    }

    private static void two(StringBuilder b, int v) {
        b.append((char) ('0' + v / 10)).append((char) ('0' + v % 10));
    }
}
