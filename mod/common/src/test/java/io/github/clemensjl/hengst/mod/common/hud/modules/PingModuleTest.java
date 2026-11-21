package io.github.clemensjl.hengst.mod.common.hud.modules;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.hud.CpsCounter;
import io.github.clemensjl.hengst.mod.common.hud.FrameCounter;
import io.github.clemensjl.hengst.mod.common.hud.HudContext;
import io.github.clemensjl.hengst.mod.common.testutil.FakeCanvas;
import io.github.clemensjl.hengst.mod.common.testutil.FakePlatform;
import io.github.clemensjl.hengst.mod.common.testutil.FakeTime;
import java.nio.file.Path;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class PingModuleTest {
    @TempDir Path dir;
    FakePlatform platform;
    HudContext ctx;
    PingModule m;
    ModuleConfig cfg;

    @BeforeEach
    void setUp() {
        platform = new FakePlatform(dir);
        platform.inWorld = true;
        platform.serverAddress = "mc.example.net";
        FakeTime time = new FakeTime();
        ctx = new HudContext(platform, time, new FrameCounter(time), new CpsCounter(time));
        m = new PingModule();
        cfg = ModuleConfig.defaults();
    }

    private void ticks(int n) {
        for (int i = 0; i < n; i++) {
            m.tick(ctx, cfg);
        }
    }

    @Test
    void ping_is_hidden_without_a_server() {
        platform.serverAddress = null;
        platform.pingMillis = 42;
        m.tick(ctx, cfg);
        assertEquals("", m.text());
        assertEquals(0, m.width(new FakeCanvas(), cfg));
        assertEquals(0, m.height(new FakeCanvas(), cfg));
        platform.serverAddress = "mc.example.net";
        m.tick(ctx, cfg);
        assertEquals("42 ms", m.text());
        platform.serverAddress = null;
        m.tick(ctx, cfg);
        assertEquals("", m.text());
        platform.serverAddress = "mc.example.net";
        platform.inWorld = false;
        m.tick(ctx, cfg);
        assertEquals("", m.text());
    }

    @Test
    void ping_is_sampled_once_per_second() {
        platform.pingMillis = 42;
        m.tick(ctx, cfg);
        platform.pingMillis = 57;
        ticks(19);
        assertEquals("42 ms", m.text());
        m.tick(ctx, cfg);
        assertEquals("57 ms", m.text());
    }

    @Test
    void ping_unknown_or_negative_shows_a_question_mark() {
        for (int unknown : new int[] {-1, -2, -1000, Integer.MIN_VALUE}) {
            m = new PingModule();
            platform.pingMillis = unknown;
            m.tick(ctx, cfg);
            assertEquals("? ms", m.text(), "ping " + unknown);
        }
    }

    @Test
    void ping_zero_is_shown() {
        platform.pingMillis = 0;
        m.tick(ctx, cfg);
        assertEquals("0 ms", m.text());
    }

    @Test
    void an_absurd_value_is_capped_at_9999() {
        platform.pingMillis = 9999;
        m.tick(ctx, cfg);
        assertEquals("9999 ms", m.text());
        platform.pingMillis = 10000;
        ticks(20);
        assertEquals("9999 ms", m.text());
        platform.pingMillis = Integer.MAX_VALUE;
        ticks(20);
        assertEquals("9999 ms", m.text());
    }

    @Test
    void the_value_follows_unknown_and_known_and_keeps_the_reference_while_unchanged() {
        platform.pingMillis = -1;
        m.tick(ctx, cfg);
        assertEquals("? ms", m.text());
        platform.pingMillis = 30;
        ticks(20);
        assertEquals("30 ms", m.text());
        String first = m.text();
        ticks(100);
        assertSame(first, m.text());
        platform.pingMillis = -1;
        ticks(20);
        assertEquals("? ms", m.text());
    }

    @Test
    void a_new_sample_is_taken_when_the_config_is_applied() {
        platform.pingMillis = 42;
        m.tick(ctx, cfg);
        platform.pingMillis = 57;
        ticks(5);
        assertEquals("42 ms", m.text());
        m.onConfigChanged(cfg);
        m.tick(ctx, cfg);
        assertEquals("57 ms", m.text());
    }

    @Test
    void the_value_is_read_at_once_when_the_module_becomes_visible_again() {
        platform.pingMillis = 42;
        m.tick(ctx, cfg);
        platform.serverAddress = null;
        m.tick(ctx, cfg);
        platform.serverAddress = "other.example.net";
        platform.pingMillis = 80;
        m.tick(ctx, cfg);
        assertEquals("80 ms", m.text());
    }
}
