package io.github.clemensjl.hengst.mod.common;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.feature.FeatureState;
import io.github.clemensjl.hengst.mod.common.hud.modules.CoordinatesModule;
import io.github.clemensjl.hengst.mod.common.hud.modules.FpsModule;
import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import io.github.clemensjl.hengst.mod.common.testutil.CountingCanvas;
import io.github.clemensjl.hengst.mod.common.testutil.FakePlatform;
import io.github.clemensjl.hengst.mod.common.testutil.FakeTime;
import java.lang.management.ManagementFactory;
import java.nio.file.Path;
import java.time.Clock;
import java.time.Instant;
import java.time.ZoneOffset;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Tag;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/**
 * Allocation of the facade's frame and tick hooks. Runs with -Xint (task interpretedTest): under the JIT the thread's
 * allocation counter shows one-off 600 to 1500 byte jumps that come from deoptimization, not from the code under
 * test; in the interpreter the count is exact.
 */
@Tag("interpreted")
class HengstCoreAllocationTest {
    private static final com.sun.management.ThreadMXBean MX =
            (com.sun.management.ThreadMXBean) ManagementFactory.getThreadMXBean();

    @TempDir Path dir;
    FakePlatform platform;
    FakeTime time;
    Clock clock;
    HengstCore core;

    @BeforeEach void setUp() {
        platform = new FakePlatform(dir); time = new FakeTime();
        clock = Clock.fixed(Instant.parse("2026-11-20T10:15:30Z"), ZoneOffset.UTC);
    }

    private HengstCore start() { return HengstCore.start(platform, time, clock); }

    private static long allocated() {
        return MX.getThreadAllocatedBytes(Thread.currentThread().getId());
    }

    private void frame(Canvas c) {
        core.onFrame();
        core.onHudRender(c, 0f);
    }

    @Test void render_and_tick_hooks_allocate_nothing_with_the_four_modules_active() {
        core = start(); platform.inWorld = true;
        platform.playerX = 10.25; platform.playerY = 64; platform.playerZ = -3.5;
        core.onMouseButton(0, true);
        assertEquals(4, core.renderer().activeCount());
        CountingCanvas counting = new CountingCanvas();
        for (int i = 0; i < 3000; i++) { core.onClientTick(); frame(counting); }
        int calls = 20000;
        long before = allocated();
        for (int i = 0; i < calls; i++) { frame(counting); }
        long renderBytes = allocated() - before;
        before = allocated();
        for (int i = 0; i < calls; i++) { core.onClientTick(); }
        long tickBytes = allocated() - before;
        System.out.println("HengstCore allocation over " + calls + " calls: frame+render " + renderBytes + " bytes, tick " + tickBytes + " bytes");
        assertEquals(0, renderBytes, "frame and render hooks allocated " + renderBytes + " bytes");
        assertEquals(0, tickBytes, "tick hook allocated " + tickBytes + " bytes");
        assertTrue(counting.texts > 0);
    }

    @Test void ticks_with_a_constant_server_and_language_allocate_nothing() {
        core = start(); platform.inWorld = true;
        platform.serverAddress = "mc.example.net"; platform.languageCode = "de_de";
        CountingCanvas counting = new CountingCanvas();
        for (int i = 0; i < 200; i++) { time.advanceMillis(50); core.onClientTick(); frame(counting); }
        int generation = core.context().languageGeneration();
        assertEquals(1, generation);                                     // the switch was seen while settling
        long before = allocated();
        for (int i = 0; i < 2000; i++) { time.advanceMillis(50); core.onClientTick(); frame(counting); }
        long bytes = allocated() - before;
        System.out.println("HengstCore server/language steady state, 2000 ticks with frames: " + bytes + " bytes");
        assertEquals(0, bytes);
        assertEquals(generation, core.context().languageGeneration());
    }

    @Test void ticks_with_an_equal_server_name_in_a_new_object_allocate_nothing() {
        core = start(); platform.inWorld = true;
        String a = "mc.example.net";
        String b = new String("mc.example.net");
        platform.serverAddress = a;
        for (int i = 0; i < 200; i++) { time.advanceMillis(50); core.onClientTick(); }
        long before = allocated();
        for (int i = 0; i < 2000; i++) { platform.serverAddress = (i & 1) == 0 ? b : a; time.advanceMillis(50); core.onClientTick(); }
        assertEquals(0, allocated() - before);
    }

    /** A disabled feature costs nothing: no module in the arrays, no allocation, and the state read allocates nothing. */
    @Test void disabled_features_cost_nothing_and_a_toggle_applies_without_a_restart() {
        core = start(); platform.inWorld = true;
        core.activeProfile().module("keystrokes").setEnabled(false);
        core.activeProfile().module("coordinates").setEnabled(false);
        core.applyConfigForTest();
        assertEquals(2, core.renderer().activeCount());
        assertEquals(false, FeatureState.on(3));
        assertEquals(true, FeatureState.on(2));
        CountingCanvas counting = new CountingCanvas();
        for (int i = 0; i < 3000; i++) { core.onClientTick(); frame(counting); }
        int calls = 20000;
        boolean sink = false;
        long before = allocated();
        for (int i = 0; i < calls; i++) { frame(counting); sink ^= FeatureState.on(3) ^ FeatureState.on(2) ^ FeatureState.on(-1) ^ FeatureState.on(999); }
        long renderBytes = allocated() - before;
        before = allocated();
        for (int i = 0; i < calls; i++) { core.onClientTick(); }
        long tickBytes = allocated() - before;
        System.out.println("HengstCore disabled-features allocation over " + calls + " calls: frame+render+state " + renderBytes
                + " bytes, tick " + tickBytes + " bytes (" + sink + ")");
        assertEquals(0, renderBytes);
        assertEquals(0, tickBytes);
        // switching one on again takes effect at the next applyConfig, without a restart
        core.activeProfile().module("coordinates").setEnabled(true);
        core.applyConfigForTest();
        assertEquals(3, core.renderer().activeCount());
        assertEquals(true, FeatureState.on(6));
        for (int i = 0; i < 3000; i++) { core.onClientTick(); frame(counting); }
        before = allocated();
        for (int i = 0; i < calls; i++) { frame(counting); }
        assertEquals(0, allocated() - before);
    }

    /** A value change builds one short string: header, fields and the backing array, with room to spare. */
    private static final long MAX_BYTES_PER_TEXT = 192;

    @Test void a_moving_player_and_a_running_clock_allocate_only_for_changed_texts() {
        core = start(); platform.inWorld = true;
        FpsModule fps = (FpsModule) core.registry().byId("fps");
        CoordinatesModule coords = (CoordinatesModule) core.registry().byId("coordinates");
        CountingCanvas counting = new CountingCanvas();
        long overhead = -allocated(); overhead += allocated();             // what two back-to-back readings cost
        long worst = 0;
        int changes = 0;
        int quiet = 0;
        for (int i = 0; i < 12000; i++) {
            time.advanceMillis(i / 200 % 2 == 0 ? 10 : 25);                // the frame time changes every 200 frames
            platform.playerX += 0.37;                                      // a new block now and then
            String fpsBefore = fps.text();
            String coordsBefore = coords.text();
            long a = allocated();
            core.onFrame();
            core.onHudRender(counting, 0f);
            if (i % 2 == 0) core.onClientTick();
            long used = allocated() - a - overhead;
            boolean changed = fps.text() != fpsBefore || coords.text() != coordsBefore;
            if (i < 2000) continue;                                        // warm-up
            if (changed) {
                changes++;
                worst = Math.max(worst, used);
            } else {
                quiet++;
                assertEquals(0, used, "frame " + i + " without a change allocated " + used + " bytes");
            }
        }
        System.out.println("HengstCore moving allocation: " + changes + " changing frames (worst " + worst
                + " bytes, bound " + MAX_BYTES_PER_TEXT * 2 + " for two texts), " + quiet + " quiet frames with 0 bytes each");
        assertTrue(changes > 50 && quiet > 1000);
        assertTrue(worst <= MAX_BYTES_PER_TEXT * 2, "worst changing frame allocated " + worst + " bytes");
    }

    /** Same drawing calls as CountingCanvas, other size. */
    private static final class SizedCanvas implements Canvas {
        private final int w;
        private final int h;

        SizedCanvas(int w, int h) { this.w = w; this.h = h; }
        @Override public int width() { return w; }
        @Override public int height() { return h; }
        @Override public int fontHeight() { return 9; }
        @Override public int textWidth(String text) { return 6 * text.length(); }
        @Override public void fill(int x1, int y1, int x2, int y2, int argb) { }
        @Override public void text(String text, int x, int y, int argb, boolean shadow) { }
        @Override public void pushTransform(float translateX, float translateY, float scale) { }
        @Override public void popTransform() { }
    }

    @Test void no_lazy_state_on_the_frame_path_after_rebuild_world_change_time_jump_and_resize() {
        core = start(); platform.inWorld = true;
        CountingCanvas counting = new CountingCanvas();
        for (int i = 0; i < 3000; i++) { core.onClientTick(); frame(counting); }
        long[] cold = new long[4];
        // 1) the first frames right after rebuild()
        core.renderer().rebuild(core.activeProfile());
        long b = allocated();
        for (int i = 0; i < 200; i++) frame(counting);
        cold[0] = allocated() - b;
        // 2) after inWorld false -> true
        platform.inWorld = false; core.onClientTick(); frame(counting);
        platform.inWorld = true; core.onClientTick();
        b = allocated();
        for (int i = 0; i < 200; i++) frame(counting);
        cold[1] = allocated() - b;
        // 3) after a time jump of more than a second (the window closes in a frame, the text rebuild is a tick matter)
        time.advanceMillis(5000);
        b = allocated();
        for (int i = 0; i < 200; i++) frame(counting);
        cold[2] = allocated() - b;
        // 4) after a canvas size change
        Canvas resized = new SizedCanvas(640, 480);
        b = allocated();
        for (int i = 0; i < 200; i++) frame(resized);
        cold[3] = allocated() - b;
        System.out.println("HengstCore cold-path allocation in bytes: after rebuild " + cold[0] + ", after world change "
                + cold[1] + ", after time jump " + cold[2] + ", after canvas resize " + cold[3]);
        for (int i = 0; i < cold.length; i++) {
            assertEquals(0, cold[i], "case " + i + " allocated " + cold[i] + " bytes on the frame path");
        }
    }
}
