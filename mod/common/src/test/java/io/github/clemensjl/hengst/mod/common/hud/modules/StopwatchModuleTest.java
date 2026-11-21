package io.github.clemensjl.hengst.mod.common.hud.modules;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.feature.DefaultKey;
import io.github.clemensjl.hengst.mod.common.feature.FeatureGroup;
import io.github.clemensjl.hengst.mod.common.feature.KeySpec;
import io.github.clemensjl.hengst.mod.common.hud.CpsCounter;
import io.github.clemensjl.hengst.mod.common.hud.FrameCounter;
import io.github.clemensjl.hengst.mod.common.hud.HudContext;
import io.github.clemensjl.hengst.mod.common.layout.Anchor;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.testutil.FakePlatform;
import io.github.clemensjl.hengst.mod.common.testutil.FakeTime;
import io.github.clemensjl.hengst.mod.common.testutil.FakeWallClock;
import java.nio.file.Path;
import java.util.List;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class StopwatchModuleTest {
    @TempDir Path dir;
    FakePlatform platform;
    FakeTime time;
    HudContext ctx;
    StopwatchModule m = new StopwatchModule();
    ModuleConfig cfg = ModuleConfig.defaults();

    @BeforeEach
    void setUp() {
        platform = new FakePlatform(dir);
        time = new FakeTime();
        ctx = new HudContext(platform, time, new FrameCounter(time), new CpsCounter(time));
    }

    @Test
    void identity_group_keys_and_default_place() {
        assertEquals("stopwatch", m.id());
        assertEquals(21, m.number());
        assertEquals(FeatureGroup.COMFORT, m.group());
        for (GameVersion v : GameVersion.values()) {
            assertTrue(m.supports(v), v.toString());
        }
        assertEquals(Anchor.TOP_LEFT, m.defaultAnchor());
        assertEquals(2, m.defaultOffsetX());
        assertEquals(72, m.defaultOffsetY());
        List<KeySpec> keys = m.keys();
        assertEquals(2, keys.size());
        assertEquals("key.hengst.stopwatch.toggle", keys.get(0).translationKey());
        assertEquals("key.hengst.stopwatch.reset", keys.get(1).translationKey());
        for (KeySpec k : keys) {
            assertEquals(DefaultKey.NONE, k.defaultKey());
            assertEquals(KeySpec.Mode.PRESS, k.mode());
        }
        assertEquals("tenths", m.options().get(0).key());
    }

    @Test
    void the_stopwatch_shows_zero_until_started() {
        m.tick(ctx, cfg);
        assertEquals("0:00.0", m.text());
        cfg.setOption("tenths", false);
        m.tick(ctx, cfg);
        assertEquals("0:00", m.text());
    }

    @Test
    void start_stop_and_continue() {
        m.onKey(0, true, time.now);
        time.advanceMillis(65_340);
        m.tick(ctx, cfg);
        assertEquals("1:05.3", m.text());
        m.onKey(0, true, time.now);
        time.advanceMillis(10_000);
        m.tick(ctx, cfg);
        assertEquals("1:05.3", m.text());
        m.onKey(0, true, time.now);
        time.advanceMillis(1_000);
        m.tick(ctx, cfg);
        assertEquals("1:06.3", m.text());
    }

    @Test
    void hours_and_reset() {
        m.onKey(0, true, time.now);
        time.advanceMillis(3_600_050);
        m.tick(ctx, cfg);
        assertEquals("1:00:00.0", m.text());
        cfg.setOption("tenths", false);
        m.tick(ctx, cfg);
        assertEquals("1:00:00", m.text());
        m.onKey(1, true, time.now);
        time.advanceMillis(2_000);
        m.tick(ctx, cfg);
        assertEquals("0:00", m.text());
    }

    @Test
    void releases_do_nothing() {
        m.onKey(0, false, time.now);
        time.advanceMillis(1_000);
        m.tick(ctx, cfg);
        assertEquals("0:00.0", m.text());
        m.onKey(1, false, time.now);
        m.onKey(0, true, time.now);
        m.onKey(0, false, time.now);
        time.advanceMillis(1_000);
        m.tick(ctx, cfg);
        assertEquals("0:01.0", m.text());
    }

    @Test
    void the_stopwatch_uses_the_monotonic_time_source() {
        FakeWallClock wall = new FakeWallClock();
        HudContext wctx = new HudContext(platform, time, wall, new FrameCounter(time), new CpsCounter(time));
        m.onKey(0, true, time.now);
        time.advanceMillis(2_000);
        wall.utcMillis -= 3_600_000;
        m.tick(wctx, cfg);
        assertEquals("0:02.0", m.text());
        time.advanceMillis(1_000);
        wall.utcMillis += 7_200_000;
        m.tick(wctx, cfg);
        assertEquals("0:03.0", m.text());
        assertEquals(0, wall.currentCalls);
        assertEquals(0, wall.offsetCalls);
    }

    @Test
    void tenths_are_floored_and_the_text_changes_only_with_the_shown_value() {
        m.onKey(0, true, time.now);
        time.advanceMillis(99);
        m.tick(ctx, cfg);
        assertEquals("0:00.0", m.text());
        String before = m.text();
        time.advanceMillis(1);
        m.tick(ctx, cfg);
        assertEquals("0:00.1", m.text());
        before = m.text();
        time.advanceMillis(50);
        m.tick(ctx, cfg);
        assertSame(before, m.text());
        time.advanceMillis(849);
        m.tick(ctx, cfg);
        assertEquals("0:00.9", m.text());
        time.advanceMillis(1);
        m.tick(ctx, cfg);
        assertEquals("0:01.0", m.text());
    }

    @Test
    void without_tenths_the_text_changes_once_per_second() {
        cfg.setOption("tenths", false);
        m.onKey(0, true, time.now);
        time.advanceMillis(999);
        m.tick(ctx, cfg);
        assertEquals("0:00", m.text());
        String before = m.text();
        time.advanceMillis(500);
        m.tick(ctx, cfg);
        assertEquals("0:01", m.text());
        before = m.text();
        time.advanceMillis(400);
        m.tick(ctx, cfg);
        assertSame(before, m.text());
    }

    @Test
    void format_boundaries() {
        m.onKey(0, true, time.now);
        time.advanceMillis(599_900);
        m.tick(ctx, cfg);
        assertEquals("9:59.9", m.text());
        time.advanceMillis(100);
        m.tick(ctx, cfg);
        assertEquals("10:00.0", m.text());
        time.advanceMillis(2_999_900);
        m.tick(ctx, cfg);
        assertEquals("59:59.9", m.text());
        time.advanceMillis(100);
        m.tick(ctx, cfg);
        assertEquals("1:00:00.0", m.text());
    }

    @Test
    void long_runs_format_without_overflow_or_truncation() {
        m.onKey(0, true, time.now);
        time.advanceMillis(99 * 60_000L);
        m.tick(ctx, cfg);
        assertEquals("1:39:00.0", m.text());
        time.advanceMillis(24 * 3_600_000L - 99 * 60_000L + 59_999L);
        m.tick(ctx, cfg);
        assertEquals("24:00:59.9", m.text());
        time.advanceMillis(100 * 3_600_000L + 1L);
        m.tick(ctx, cfg);
        assertEquals("124:01:00.0", m.text());
        time.advanceMillis(10_000L * 3_600_000L);
        cfg.setOption("tenths", false);
        m.tick(ctx, cfg);
        assertEquals("10124:01:00", m.text());
        time.advanceMillis(3_000L * 24 * 3_600_000L); // 3000 days: far beyond any int in milliseconds
        m.tick(ctx, cfg);
        assertEquals("82124:01:00", m.text());
    }

    @Test
    void reset_while_running_stops_and_zeroes() {
        m.onKey(0, true, time.now);
        time.advanceMillis(3_000);
        m.tick(ctx, cfg);
        assertEquals("0:03.0", m.text());
        m.onKey(1, true, time.now);
        time.advanceMillis(5_000);
        m.tick(ctx, cfg);
        assertEquals("0:00.0", m.text());
        m.onKey(0, true, time.now);
        time.advanceMillis(2_000);
        m.tick(ctx, cfg);
        assertEquals("0:02.0", m.text());
    }

    @Test
    void reset_while_stopped_keeps_zero_and_unknown_keys_do_nothing() {
        m.onKey(1, true, time.now);
        m.onKey(2, true, time.now);
        m.onKey(-1, true, time.now);
        m.onKey(99, true, time.now);
        time.advanceMillis(1_000);
        m.tick(ctx, cfg);
        assertEquals("0:00.0", m.text());
    }

    @Test
    void a_stopped_value_survives_a_config_apply_and_a_running_one_keeps_running() {
        cfg.setEnabled(true);
        m.onKey(0, true, time.now);
        time.advanceMillis(2_000);
        m.tick(ctx, cfg);
        m.onConfigChanged(cfg); // profile switch or settings closed: the stopwatch is user state
        time.advanceMillis(3_000);
        m.tick(ctx, cfg);
        assertEquals("0:05.0", m.text());
        m.onKey(0, true, time.now);
        m.onConfigChanged(cfg);
        time.advanceMillis(3_000);
        m.tick(ctx, cfg);
        assertEquals("0:05.0", m.text());
    }

    @Test
    void disabling_stops_at_the_last_tick_and_keeps_the_value() {
        m.onKey(0, true, time.now);
        time.advanceMillis(5_000);
        m.tick(ctx, cfg); // the last tick before the feature is switched off
        time.advanceMillis(40);
        cfg.setEnabled(false);
        m.onConfigChanged(cfg);
        time.advanceMillis(100_000); // the module is not ticked while off
        cfg.setEnabled(true);
        m.onConfigChanged(cfg);
        time.advanceMillis(100_000);
        m.tick(ctx, cfg);
        assertEquals("0:05.0", m.text()); // stopped, not running invisibly
        m.onKey(0, true, time.now);
        time.advanceMillis(1_000);
        m.tick(ctx, cfg);
        assertEquals("0:06.0", m.text()); // and it continues from the kept value
    }

    @Test
    void a_key_before_the_first_tick_counts_from_the_key() {
        time.advanceMillis(777);
        m.onKey(0, true, time.now);
        time.advanceMillis(1_000);
        m.tick(ctx, cfg);
        assertEquals("0:01.0", m.text());
    }

    @Test
    void a_key_time_after_the_tick_time_never_shows_negative_time() {
        m.onKey(0, true, time.now + 5_000_000L); // a key time a few milliseconds ahead of the next tick
        m.tick(ctx, cfg);
        assertEquals("0:00.0", m.text());
    }

    @Test
    void the_width_follows_the_text() {
        m.tick(ctx, cfg);
        assertEquals(4 + 6 * 6, m.width(new io.github.clemensjl.hengst.mod.common.testutil.FakeCanvas(), cfg));
    }
}
