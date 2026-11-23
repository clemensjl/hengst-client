package io.github.clemensjl.hengst.mod.common.hud.modules;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.hud.CpsCounter;
import io.github.clemensjl.hengst.mod.common.hud.FrameCounter;
import io.github.clemensjl.hengst.mod.common.hud.HudContext;
import io.github.clemensjl.hengst.mod.common.testutil.FakePlatform;
import io.github.clemensjl.hengst.mod.common.testutil.FakeTime;
import java.lang.ref.WeakReference;
import java.nio.file.Path;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import io.github.clemensjl.hengst.mod.common.testutil.TestContexts;

class SpeedModuleTest {
    @TempDir Path dir;
    FakePlatform platform;
    HudContext ctx;

    @BeforeEach
    void setUp() {
        platform = new FakePlatform(dir);
        platform.inWorld = true;
        FakeTime time = new FakeTime();
        ctx = TestContexts.context(platform, time, new FrameCounter(time), new CpsCounter(time));
    }

    @Test
    void speed_while_walking_with_each_number_of_decimals() {
        SpeedModule m = new SpeedModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        platform.worldIdentity = new Object();
        m.tick(ctx, cfg);
        for (int i = 0; i < 10; i++) {
            platform.playerX += 0.2155;
            m.tick(ctx, cfg);
        }
        assertEquals("4.3 m/s", m.text());
        cfg.setOption("decimals", 2L);
        m.tick(ctx, cfg); // a tick without movement: the average over 10 values, one of them 0
        assertEquals("3.88 m/s", m.text());
        cfg.setOption("decimals", 0L);
        platform.playerX += 0.2155;
        m.tick(ctx, cfg);
        assertEquals("4 m/s", m.text());
    }

    @Test
    void the_average_covers_only_the_ticks_seen_so_far() {
        SpeedModule m = new SpeedModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        platform.worldIdentity = new Object();
        m.tick(ctx, cfg);
        platform.playerZ -= 0.25;
        m.tick(ctx, cfg);
        assertEquals("5.0 m/s", m.text());
    }

    @Test
    void vertical_movement_counts_only_when_asked() {
        SpeedModule m = new SpeedModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        platform.worldIdentity = new Object();
        m.tick(ctx, cfg);
        platform.playerY -= 0.5;
        m.tick(ctx, cfg);
        assertEquals("0.0 m/s", m.text());
        cfg.setOption("include_vertical", true);
        platform.playerY -= 0.5;
        m.tick(ctx, cfg);
        assertEquals("5.0 m/s", m.text());
    }

    @Test
    void a_teleport_and_a_world_change_reset_the_average() {
        SpeedModule m = new SpeedModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        platform.worldIdentity = new Object();
        m.tick(ctx, cfg);
        for (int i = 0; i < 10; i++) {
            platform.playerX += 0.25;
            m.tick(ctx, cfg);
        }
        assertEquals("5.0 m/s", m.text());
        platform.playerX += 500;
        m.tick(ctx, cfg);
        assertEquals("0.0 m/s", m.text());
        platform.playerX += 0.25;
        m.tick(ctx, cfg);
        assertEquals("5.0 m/s", m.text());
        platform.worldIdentity = new Object();
        platform.playerX += 3;
        m.tick(ctx, cfg);
        assertEquals("0.0 m/s", m.text());
    }

    @Test
    void speed_is_empty_outside_a_world_and_resets_on_return() {
        SpeedModule m = new SpeedModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        platform.worldIdentity = new Object();
        m.tick(ctx, cfg);
        platform.playerX += 0.25;
        m.tick(ctx, cfg);
        platform.inWorld = false;
        m.tick(ctx, cfg);
        assertEquals("", m.text());
        platform.inWorld = true;
        platform.playerX += 0.25;
        m.tick(ctx, cfg);
        assertEquals("0.0 m/s", m.text());
    }

    @Test
    void a_step_of_exactly_five_blocks_counts_and_a_longer_one_does_not() {
        SpeedModule m = new SpeedModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        platform.worldIdentity = new Object();
        m.tick(ctx, cfg);
        platform.playerX += 5.0;
        m.tick(ctx, cfg);
        assertEquals("100.0 m/s", m.text());
        platform.playerX += 5.01;
        m.tick(ctx, cfg);
        assertEquals("0.0 m/s", m.text());
    }

    @Test
    void a_vertical_fall_over_five_blocks_resets_only_when_vertical_counts() {
        SpeedModule m = new SpeedModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        platform.worldIdentity = new Object();
        m.tick(ctx, cfg);
        platform.playerY -= 50;
        m.tick(ctx, cfg);
        assertEquals("0.0 m/s", m.text());
        cfg.setOption("include_vertical", true);
        platform.playerY -= 50;
        m.tick(ctx, cfg);
        assertEquals("0.0 m/s", m.text());
    }

    @Test
    void standing_still_shows_exactly_zero_also_after_moving() {
        SpeedModule m = new SpeedModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        cfg.setOption("decimals", 2L);
        platform.worldIdentity = new Object();
        platform.playerX = 123456.789;
        platform.playerZ = -98765.4321;
        m.tick(ctx, cfg);
        for (int i = 0; i < 10; i++) {
            platform.playerX += 0.1;
            platform.playerZ -= 0.07;
            m.tick(ctx, cfg);
        }
        for (int i = 0; i < 12; i++) {
            m.tick(ctx, cfg);
        }
        assertEquals("0.00 m/s", m.text());
    }

    @Test
    void a_position_that_is_not_finite_restarts_and_the_next_finite_tick_only_remembers() {
        SpeedModule m = new SpeedModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        platform.worldIdentity = new Object();
        m.tick(ctx, cfg);
        platform.playerX += 0.25;
        m.tick(ctx, cfg);
        assertEquals("5.0 m/s", m.text());
        platform.playerX = Double.NaN;
        m.tick(ctx, cfg);
        assertEquals("0.0 m/s", m.text());
        platform.playerX = 1000;
        m.tick(ctx, cfg);
        assertEquals("0.0 m/s", m.text());
        platform.playerX = 1000.25;
        m.tick(ctx, cfg);
        assertEquals("5.0 m/s", m.text());
    }

    @Test
    void the_first_tick_in_a_world_shows_zero_not_a_jump_from_the_origin() {
        SpeedModule m = new SpeedModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        platform.worldIdentity = new Object();
        platform.playerX = 8000;
        platform.playerZ = -3000;
        m.tick(ctx, cfg);
        assertEquals("0.0 m/s", m.text());
    }

    @Test
    void applying_the_config_forgets_the_position_so_a_gap_is_not_one_step() {
        SpeedModule m = new SpeedModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        platform.worldIdentity = new Object();
        m.tick(ctx, cfg);
        platform.playerX += 0.25;
        m.tick(ctx, cfg);
        assertEquals("5.0 m/s", m.text());
        platform.playerX += 4; // while the module was off: not ticked
        m.onConfigChanged(cfg);
        m.tick(ctx, cfg);
        assertEquals("0.0 m/s", m.text());
        platform.playerX += 0.25;
        m.tick(ctx, cfg);
        assertEquals("5.0 m/s", m.text());
        platform.playerX += 6; // beyond the limit: also reset without the apply
        m.tick(ctx, cfg);
        assertEquals("0.0 m/s", m.text());
    }

    @Test
    void a_switched_off_module_does_not_keep_the_world_alive() throws Exception {
        SpeedModule m = new SpeedModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        Object world = new Object();
        WeakReference<Object> ref = new WeakReference<Object>(world);
        platform.worldIdentity = world;
        m.tick(ctx, cfg);
        platform.playerX += 0.2;
        m.tick(ctx, cfg);
        cfg.setEnabled(false);
        m.onConfigChanged(cfg);          // switched off: never ticked again
        platform.worldIdentity = null;   // the player leaves the world
        platform.inWorld = false;
        world = null;
        for (int i = 0; i < 20 && ref.get() != null; i++) {
            System.gc();
            Thread.sleep(10);
        }
        assertNull(ref.get(), "the switched-off module still references the world");
    }

    @Test
    void speed_is_empty_while_the_server_asks_for_reduced_debug_info() {
        SpeedModule m = new SpeedModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        platform.worldIdentity = new Object();
        m.tick(ctx, cfg);
        platform.playerZ -= 0.25;
        m.tick(ctx, cfg);
        assertEquals("5.0 m/s", m.text());
        platform.reducedDebugInfo = true;
        platform.playerZ -= 0.25;
        m.tick(ctx, cfg);
        assertEquals("", m.text());
        platform.reducedDebugInfo = false;
        m.tick(ctx, cfg);           // starts again from this position
        assertEquals("0.0 m/s", m.text());
    }
}
