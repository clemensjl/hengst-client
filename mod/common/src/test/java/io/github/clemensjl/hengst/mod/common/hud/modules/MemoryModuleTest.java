package io.github.clemensjl.hengst.mod.common.hud.modules;

import static org.junit.jupiter.api.Assertions.assertEquals;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.hud.CpsCounter;
import io.github.clemensjl.hengst.mod.common.hud.FrameCounter;
import io.github.clemensjl.hengst.mod.common.hud.HudContext;
import io.github.clemensjl.hengst.mod.common.testutil.FakePlatform;
import io.github.clemensjl.hengst.mod.common.testutil.FakeTime;
import java.nio.file.Path;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class MemoryModuleTest {
    @TempDir Path dir;
    FakePlatform platform;
    HudContext ctx;

    @BeforeEach
    void setUp() {
        platform = new FakePlatform(dir);
        FakeTime time = new FakeTime();
        ctx = new HudContext(platform, time, new FrameCounter(time), new CpsCounter(time));
    }

    private void ticks(MemoryModule m, ModuleConfig cfg, int n) {
        for (int i = 0; i < n; i++) {
            m.tick(ctx, cfg);
        }
    }

    @Test
    void memory_formats_and_unknown_max() {
        MemoryModule m = new MemoryModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        platform.memoryUsed = 1024L << 20;
        platform.memoryMax = 4096L << 20;
        m.tick(ctx, cfg);
        assertEquals("RAM 25% 1024/4096 MB", m.text());
        cfg.setOption("style", "percent");
        ticks(m, cfg, 20);
        assertEquals("RAM 25%", m.text());
        cfg.setOption("style", "used_max");
        ticks(m, cfg, 20);
        assertEquals("RAM 1024/4096 MB", m.text());
        platform.memoryMax = Long.MAX_VALUE;
        ticks(m, cfg, 20);
        assertEquals("RAM 1024 MB", m.text());
    }

    @Test
    void without_a_usable_maximum_every_style_shows_the_used_megabytes() {
        MemoryModule m = new MemoryModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        platform.memoryUsed = 1024L << 20;
        for (long max : new long[] {Long.MAX_VALUE, 0L, -1L, Long.MIN_VALUE}) {
            platform.memoryMax = max;
            for (String style : new String[] {"percent", "used_max", "both"}) {
                cfg.setOption("style", style);
                ticks(m, cfg, 20);
                assertEquals("RAM 1024 MB", m.text(), "max " + max + " style " + style);
            }
        }
    }

    @Test
    void percent_and_megabytes_round_down_and_never_go_negative() {
        MemoryModule m = new MemoryModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        platform.memoryUsed = (1L << 20) - 1; // under 1 MB
        platform.memoryMax = 3L << 20;
        m.tick(ctx, cfg);
        assertEquals("RAM 33% 0/3 MB", m.text());
        platform.memoryUsed = -5;
        ticks(m, cfg, 20);
        assertEquals("RAM 0% 0/3 MB", m.text());
        platform.memoryUsed = 2L << 20;
        platform.memoryMax = (3L << 20) + 1;
        ticks(m, cfg, 20);
        assertEquals("RAM 66% 2/3 MB", m.text());
    }

    @Test
    void a_huge_maximum_does_not_overflow_the_percent() {
        MemoryModule m = new MemoryModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        cfg.setOption("style", "percent");
        platform.memoryMax = Long.MAX_VALUE - 1;
        platform.memoryUsed = Long.MAX_VALUE / 2;
        m.tick(ctx, cfg);
        // used * 100 would overflow: a maximum above Long.MAX_VALUE / 100 counts as no limit
        assertEquals("RAM " + (platform.memoryUsed >> 20) + " MB", m.text());
    }

    @Test
    void memory_is_sampled_once_per_second() {
        MemoryModule m = new MemoryModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        platform.memoryUsed = 1024L << 20;
        platform.memoryMax = 4096L << 20;
        m.tick(ctx, cfg);
        platform.memoryUsed = 2048L << 20;
        ticks(m, cfg, 19);
        assertEquals("RAM 25% 1024/4096 MB", m.text());
        m.tick(ctx, cfg);
        assertEquals("RAM 50% 2048/4096 MB", m.text());
    }

    @Test
    void a_config_apply_makes_the_next_tick_sample_again() {
        MemoryModule m = new MemoryModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        platform.memoryUsed = 1024L << 20;
        platform.memoryMax = 4096L << 20;
        m.tick(ctx, cfg); // sampled; the next 19 ticks would not sample
        assertEquals("RAM 25% 1024/4096 MB", m.text());
        platform.memoryUsed = 2048L << 20; // while the module is off
        m.tick(ctx, cfg);
        assertEquals("RAM 25% 1024/4096 MB", m.text());
        m.onConfigChanged(cfg);
        m.tick(ctx, cfg);
        assertEquals("RAM 50% 2048/4096 MB", m.text());
    }

    @Test
    void used_above_the_maximum_shows_100_percent() {
        MemoryModule m = new MemoryModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        platform.memoryUsed = 2000L << 20;
        platform.memoryMax = 1000L << 20;
        m.tick(ctx, cfg);
        assertEquals("RAM 100% 2000/1000 MB", m.text());
    }

    @Test
    void the_percent_guard_holds_exactly_at_a_hundredth_of_the_long_range() {
        MemoryModule m = new MemoryModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        long limit = Long.MAX_VALUE / 100;
        platform.memoryUsed = limit;
        platform.memoryMax = limit;
        m.tick(ctx, cfg);
        assertEquals("RAM 100% " + (limit >> 20) + "/" + (limit >> 20) + " MB", m.text());
        platform.memoryMax = limit + 1;
        m.onConfigChanged(cfg);
        m.tick(ctx, cfg);
        assertEquals("RAM " + (limit >> 20) + " MB", m.text());
    }
}
