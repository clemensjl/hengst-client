package io.github.clemensjl.hengst.mod.common.hud.modules;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.I18n;
import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.hud.CpsCounter;
import io.github.clemensjl.hengst.mod.common.hud.FrameCounter;
import io.github.clemensjl.hengst.mod.common.hud.HudContext;
import io.github.clemensjl.hengst.mod.common.testutil.FakePlatform;
import io.github.clemensjl.hengst.mod.common.testutil.FakeTime;
import io.github.clemensjl.hengst.mod.common.testutil.FakeWallClock;
import java.nio.file.Path;
import java.time.Instant;
import java.util.Collections;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import io.github.clemensjl.hengst.mod.common.testutil.TestContexts;

class ClockModuleTest {
    @TempDir Path dir;
    FakeWallClock wall;
    HudContext ctx;

    @BeforeEach
    void setUp() {
        FakePlatform platform = new FakePlatform(dir);
        FakeTime time = new FakeTime();
        wall = new FakeWallClock();
        ctx = TestContexts.context(platform, time, wall, new FrameCounter(time), new CpsCounter(time));
    }

    static long utc(String iso) {
        return Instant.parse(iso).toEpochMilli();
    }

    @Test
    void clock_formats() {
        ClockModule m = new ClockModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        wall.utcMillis = utc("2026-11-21T08:05:07Z");
        wall.offset = 3_600_000;
        m.tick(ctx, cfg);
        assertEquals("09:05", m.text());
        cfg.setOption("seconds", true);
        m.tick(ctx, cfg);
        assertEquals("09:05:07", m.text());
        cfg.setOption("format", "12h");
        m.tick(ctx, cfg);
        assertEquals("9:05:07 AM", m.text());
        cfg.setOption("seconds", false);
        wall.offset = 0;
        wall.utcMillis = utc("2026-11-21T12:30:00Z");
        m.tick(ctx, cfg);
        assertEquals("12:30 PM", m.text());
        wall.utcMillis = utc("2026-11-21T00:30:00Z");
        m.tick(ctx, cfg);
        assertEquals("12:30 AM", m.text());
        wall.utcMillis = utc("2026-11-21T13:00:00Z");
        m.tick(ctx, cfg);
        assertEquals("1:00 PM", m.text());
    }

    @Test
    void midnight_and_noon_in_the_12_hour_mode() {
        ClockModule m = new ClockModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        cfg.setOption("format", "12h");
        wall.utcMillis = utc("2026-11-21T00:00:00Z");
        m.tick(ctx, cfg);
        assertEquals("12:00 AM", m.text());
        wall.utcMillis = utc("2026-11-21T12:00:00Z");
        m.tick(ctx, cfg);
        assertEquals("12:00 PM", m.text());
        wall.utcMillis = utc("2026-11-21T23:59:59Z");
        m.tick(ctx, cfg);
        assertEquals("11:59 PM", m.text());
        wall.utcMillis = utc("2026-11-21T11:59:59Z");
        m.tick(ctx, cfg);
        assertEquals("11:59 AM", m.text());
    }

    @Test
    void the_clock_rolls_over_midnight() {
        ClockModule m = new ClockModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        wall.utcMillis = utc("2026-11-21T23:59:59.900Z");
        m.tick(ctx, cfg);
        assertEquals("23:59", m.text());
        wall.utcMillis += 100;
        m.tick(ctx, cfg);
        assertEquals("00:00", m.text());
    }

    @Test
    void times_before_1970_and_negative_offsets_never_show_a_negative_unit() {
        ClockModule m = new ClockModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        cfg.setOption("seconds", true);
        wall.utcMillis = utc("1969-12-31T23:59:58Z");
        m.tick(ctx, cfg);
        assertEquals("23:59:58", m.text());
        wall.utcMillis = -1; // one millisecond before the epoch
        m.tick(ctx, cfg);
        assertEquals("23:59:59", m.text());
        wall.utcMillis = utc("1960-06-01T00:00:00Z");
        wall.offset = -5 * 3_600_000; // UTC-5, before 1970
        m.tick(ctx, cfg);
        assertEquals("19:00:00", m.text());
        wall.utcMillis = utc("2026-11-21T02:03:04Z");
        wall.offset = -5 * 3_600_000 - 30 * 60_000; // the local day is the previous one
        m.tick(ctx, cfg);
        assertEquals("20:33:04", m.text());
        cfg.setOption("format", "12h");
        m.tick(ctx, cfg);
        assertEquals("8:33:04 PM", m.text());
    }

    @Test
    void the_clock_follows_a_backward_system_clock_change_at_the_next_tick() {
        ClockModule m = new ClockModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        wall.utcMillis = utc("2026-11-21T10:00:30Z");
        m.tick(ctx, cfg);
        assertEquals("10:00", m.text());
        wall.utcMillis -= 3_600_000;
        m.tick(ctx, cfg);
        assertEquals("09:00", m.text());
        wall.utcMillis += 3 * 3_600_000;
        m.tick(ctx, cfg);
        assertEquals("12:00", m.text());
    }

    @Test
    void the_clock_follows_a_dst_offset_change() {
        ClockModule m = new ClockModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        wall.offset = 7_200_000;
        wall.switchAtUtc = utc("2026-10-25T01:00:00Z");
        wall.offsetAfterSwitch = 3_600_000;
        wall.utcMillis = utc("2026-10-25T00:59:30Z");
        m.tick(ctx, cfg);
        assertEquals("02:59", m.text());
        wall.utcMillis += 30_000;
        m.tick(ctx, cfg);
        assertEquals("02:00", m.text());
    }

    @Test
    void ticks_between_boundaries_do_not_ask_for_the_offset() {
        ClockModule m = new ClockModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        wall.utcMillis = utc("2026-11-21T10:00:00Z");
        m.tick(ctx, cfg);
        int calls = wall.offsetCalls;
        assertEquals(1, calls);
        for (int i = 0; i < 20; i++) {
            wall.utcMillis += 50;
            m.tick(ctx, cfg);
        }
        assertEquals(calls, wall.offsetCalls);
        wall.utcMillis = utc("2026-11-21T10:01:00Z");
        m.tick(ctx, cfg);
        assertEquals(calls + 1, wall.offsetCalls);
    }

    @Test
    void with_seconds_the_text_changes_every_second() {
        ClockModule m = new ClockModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        cfg.setOption("seconds", true);
        wall.utcMillis = utc("2026-11-21T10:00:00Z");
        m.tick(ctx, cfg);
        assertEquals("10:00:00", m.text());
        wall.utcMillis += 950;
        m.tick(ctx, cfg);
        assertEquals("10:00:00", m.text());
        wall.utcMillis += 50;
        m.tick(ctx, cfg);
        assertEquals("10:00:01", m.text());
    }

    @Test
    void the_am_pm_markers_come_from_the_language_files() {
        ClockModule m = new ClockModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        cfg.setOption("format", "12h");
        wall.utcMillis = utc("2026-11-21T15:00:00Z");
        m.tick(ctx, cfg);
        assertEquals("3:00 PM", m.text());
        ctx.setI18n(I18n.of(Collections.<String, Object>singletonMap("hengst.clock.pm", "nachm."),
                Collections.<String, Object>emptyMap()));
        m.tick(ctx, cfg);
        assertEquals("3:00 nachm.", m.text());
    }

    @Test
    void extreme_instants_do_not_crash_and_stay_in_range() {
        ModuleConfig cfg = ModuleConfig.defaults();
        cfg.setOption("seconds", true);
        for (long utc : new long[] {Long.MIN_VALUE, Long.MAX_VALUE}) {
            ClockModule m = new ClockModule();
            wall.utcMillis = utc;
            wall.offset = 0;
            m.tick(ctx, cfg);
            m.tick(ctx, cfg);
            String t = m.text();
            assertEquals(8, t.length(), "utc " + utc + " shows " + t);
            int h = Integer.parseInt(t.substring(0, 2));
            int min = Integer.parseInt(t.substring(3, 5));
            int s = Integer.parseInt(t.substring(6, 8));
            assertTrue(h < 24 && min < 60 && s < 60, "utc " + utc + " shows " + t);
        }
    }

    @Test
    void offsets_with_half_and_quarter_hours_and_negative_offsets() {
        Object[][] cases = {{19_800_000, "05:30"}, {20_700_000, "05:45"}, {45_900_000, "12:45"},
            {-43_200_000, "12:00"}};
        for (Object[] c : cases) {
            ClockModule m = new ClockModule();
            wall.utcMillis = 0;
            wall.offset = (Integer) c[0];
            m.tick(ctx, ModuleConfig.defaults());
            assertEquals(c[1], m.text(), "offset " + c[0]);
        }
    }

    @Test
    void one_tick_reads_the_instant_exactly_once() {
        ClockModule m = new ClockModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        wall.utcMillis = utc("2026-11-21T08:05:07Z");
        int before = wall.currentCalls;
        m.tick(ctx, cfg);
        assertEquals(1, wall.currentCalls - before);
        before = wall.currentCalls;
        m.tick(ctx, cfg); // unchanged: the early return path
        assertEquals(1, wall.currentCalls - before);
    }
}
