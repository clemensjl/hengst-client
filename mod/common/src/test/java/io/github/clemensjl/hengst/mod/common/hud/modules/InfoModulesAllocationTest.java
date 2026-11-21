package io.github.clemensjl.hengst.mod.common.hud.modules;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.config.Profile;
import io.github.clemensjl.hengst.mod.common.feature.FeatureRegistry;
import io.github.clemensjl.hengst.mod.common.hud.CpsCounter;
import io.github.clemensjl.hengst.mod.common.hud.FrameCounter;
import io.github.clemensjl.hengst.mod.common.hud.HudContext;
import io.github.clemensjl.hengst.mod.common.hud.HudRenderer;
import io.github.clemensjl.hengst.mod.common.layout.Anchor;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.testutil.CountingCanvas;
import io.github.clemensjl.hengst.mod.common.testutil.FakePlatform;
import io.github.clemensjl.hengst.mod.common.testutil.FakeTime;
import java.lang.management.ManagementFactory;
import java.nio.file.Path;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Tag;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/** Direction and speed through the real renderer: nothing is allocated unless a displayed value changed. */
@Tag("interpreted")
class InfoModulesAllocationTest {
    /** A new String with a short text: object header, fields and the backing array. Generous on purpose. */
    private static final long MAX_BYTES_PER_CHANGE = 128;

    private static final com.sun.management.ThreadMXBean MX =
            (com.sun.management.ThreadMXBean) ManagementFactory.getThreadMXBean();

    @TempDir Path dir;
    FakePlatform platform;
    FakeTime time;
    HudContext ctx;
    DirectionModule direction;
    SpeedModule speed;
    HudRenderer renderer;
    CountingCanvas canvas;

    @BeforeEach
    void setUp() {
        platform = new FakePlatform(dir);
        platform.inWorld = true;
        platform.worldIdentity = new Object();
        time = new FakeTime();
        ctx = new HudContext(platform, time, new FrameCounter(time), new CpsCounter(time));
        direction = new DirectionModule();
        speed = new SpeedModule();
        FeatureRegistry reg = new FeatureRegistry();
        reg.register(direction);
        reg.register(speed);
        Profile p = new Profile();
        ModuleConfig d = p.module("direction");
        d.setEnabled(true);
        d.setAnchor(Anchor.TOP_LEFT);
        ModuleConfig s = p.module("speed");
        s.setEnabled(true);
        s.setAnchor(Anchor.BOTTOM_RIGHT);
        s.setOption("decimals", 2);
        renderer = new HudRenderer(reg, GameVersion.V1_8_9);
        renderer.rebuild(p);
        canvas = new CountingCanvas();
    }

    private static long allocated() {
        return MX.getThreadAllocatedBytes(Thread.currentThread().getId());
    }

    private void settle() {
        for (int i = 0; i < 3000; i++) {
            ctx.frames().onFrame();
            renderer.tick(ctx);
            renderer.render(canvas, ctx);
        }
    }

    @Test
    void standing_player_allocates_nothing_in_frames_and_ticks() {
        platform.playerYaw = -135f;
        platform.playerX = 10.5;
        settle();
        int n = 20000;
        long before = allocated();
        for (int i = 0; i < n; i++) {
            ctx.frames().onFrame();
            renderer.render(canvas, ctx);
        }
        long frameBytes = allocated() - before;
        before = allocated();
        for (int i = 0; i < n; i++) {
            renderer.tick(ctx);
        }
        long tickBytes = allocated() - before;
        assertEquals(0, frameBytes, "allocated " + frameBytes + " bytes in " + n + " frames");
        assertEquals(0, tickBytes, "allocated " + tickBytes + " bytes in " + n + " ticks");
        assertTrue(canvas.texts > 0);
    }

    @Test
    void direction_while_turning_allocates_nothing() {
        platform.playerYaw = 0f;
        settle();
        int n = 20000;
        String last = direction.text();
        int changes = 0;
        long before = allocated();
        for (int i = 0; i < n; i++) {
            platform.playerYaw += 7f;
            renderer.tick(ctx);
            if (direction.text() != last) { // reference compare, no allocation
                last = direction.text();
                changes++;
            }
        }
        long bytes = allocated() - before;
        assertEquals(0, bytes, "allocated " + bytes + " bytes in " + n + " ticks");
        assertTrue(changes > n / 2, "the text must keep changing: " + changes);
    }

    @Test
    void speed_while_moving_at_a_constant_speed_allocates_nothing() {
        settle();
        for (int i = 0; i < 3000; i++) {
            platform.playerX += 0.25;
            renderer.tick(ctx);
        }
        assertEquals("5.00 m/s", speed.text());
        int n = 20000;
        long before = allocated();
        for (int i = 0; i < n; i++) {
            platform.playerX += 0.25;
            renderer.tick(ctx);
        }
        long bytes = allocated() - before;
        assertEquals(0, bytes, "allocated " + bytes + " bytes in " + n + " ticks");
        assertEquals("5.00 m/s", speed.text());
    }

    @Test
    void speed_allocates_one_short_string_per_value_change() {
        ModuleConfig cfg = ModuleConfig.defaults();
        cfg.setOption("decimals", 2);
        double step = 0.0;
        for (int i = 0; i < 5000; i++) {
            step = 0.01 * (i % 40);
            platform.playerX += step;
            speed.tick(ctx, cfg);
        }
        int n = 20000;
        int changes = 0;
        String last = speed.text();
        long before = allocated();
        for (int i = 0; i < n; i++) {
            platform.playerX += 0.01 * (i % 40);
            speed.tick(ctx, cfg);
            if (speed.text() != last) {
                last = speed.text();
                changes++;
            }
        }
        long bytes = allocated() - before;
        assertTrue(changes > 1000, "too few value changes to prove anything: " + changes);
        assertTrue(bytes <= changes * MAX_BYTES_PER_CHANGE, "allocated " + bytes + " bytes for " + changes + " changes");
        System.out.println("ALLOC speed bytes per change: " + bytes / Math.max(1, changes));
    }
}
