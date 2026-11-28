package io.github.clemensjl.hengst.mod.common;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.feature.DefaultKey;
import io.github.clemensjl.hengst.mod.common.feature.Feature;
import io.github.clemensjl.hengst.mod.common.feature.FeatureGroup;
import io.github.clemensjl.hengst.mod.common.feature.FeatureState;
import io.github.clemensjl.hengst.mod.common.feature.KeySpec;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
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
import java.util.Arrays;
import java.util.EnumSet;
import java.util.List;
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

    /** The in-game cost probe: with the per-module time array set the frame path still allocates nothing. */
    @Test void render_with_render_times_set_allocates_nothing() {
        core = start(); platform.inWorld = true;
        platform.playerX = 10.25; platform.playerY = 64; platform.playerZ = -3.5;
        long[] times = new long[core.renderer().activeCount()];
        core.renderer().setRenderTimes(times);
        CountingCanvas counting = new CountingCanvas();
        for (int i = 0; i < 3000; i++) { core.onClientTick(); frame(counting); }
        int calls = 20000;
        long before = allocated();
        for (int i = 0; i < calls; i++) { frame(counting); }
        long bytes = allocated() - before;
        core.renderer().setRenderTimes(null);
        System.out.println("HengstCore allocation with render times set, " + calls + " frames: " + bytes + " bytes");
        assertEquals(0, bytes, "frames with render times set allocated " + bytes + " bytes");
        assertEquals(4, times.length);
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

    /**
     * One FPS value change builds one String ("<n> FPS"). Measured after a warm-up with 300 changes (class loading and
     * resolution done), over 200 further distinct values of 3 digits. Bound: a String of length L is a 24-byte object
     * (compressed oops: header 12, hash 4, coder 1, hashIsZero 1, value reference 4, padded) plus its byte[] of
     * 16 + L bytes rounded up to 8; "301 FPS" has L = 7: 24 + 24 = 48. The bound allows 16 more bytes of slack.
     */
    @Test void an_fps_value_change_allocates_one_string_and_nothing_else() {
        core = start(); platform.inWorld = true;
        CountingCanvas counting = new CountingCanvas();
        StringBuilder first = new StringBuilder();
        for (int v = 1; v <= 300; v++) {
            long a = allocated();
            fpsChangeTo(v, counting);
            if (v <= 4) { first.append(' ').append(allocated() - a); }
        }
        System.out.println("FPS value change, bytes of the first four changes of this run (warm-up):" + first);
        int changes = 200;
        long min = Long.MAX_VALUE, max = 0, sum = 0, quiet = 0;
        for (int v = 301; v < 301 + changes; v++) {
            // build the window's frames first (no allocation, no time passing), then close it
            for (int i = 0; i < v - 1; i++) { core.onFrame(); }
            time.advanceMillis(1000);
            core.onFrame();
            long a = allocated();
            core.onClientTick();
            core.onHudRender(counting, 0f);
            long used = allocated() - a;
            min = Math.min(min, used); max = Math.max(max, used); sum += used;
            a = allocated();
            for (int i = 0; i < 3; i++) { core.onClientTick(); core.onHudRender(counting, 0f); core.onFrame(); }
            quiet += allocated() - a;
        }
        long perString = 24 + ((16 + 7 + 7) & ~7);
        System.out.println("FPS value change over " + changes + " distinct values (tick + render): min " + min + ", max " + max
                + ", avg " + sum / changes + " bytes; quiet ticks and frames: " + quiet + " bytes; one String of 7 chars = " + perString);
        assertEquals(0, quiet, "ticks and frames without a value change allocated " + quiet + " bytes");
        assertTrue(max <= perString + 16, "a value change allocated " + max + " bytes, bound " + (perString + 16));
    }

    private void fpsChangeTo(int v, Canvas c) {
        for (int i = 0; i < v - 1; i++) { core.onFrame(); }
        time.advanceMillis(1000);
        core.onFrame();
        core.onClientTick();
        core.onHudRender(c, 0f);
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
        // number 200 is outside the Integer cache: a boxed lookup over the catalog would allocate in the window below
        core.registerForTest(new io.github.clemensjl.hengst.mod.common.testutil.TestFeature("late", 200,
                java.util.EnumSet.allOf(io.github.clemensjl.hengst.mod.common.platform.GameVersion.class)));
        core.applyConfigForTest();
        assertEquals(2, core.renderer().activeCount());
        assertEquals(false, FeatureState.on(200));
        assertEquals(false, FeatureState.on(3));
        assertEquals(true, FeatureState.on(2));
        CountingCanvas counting = new CountingCanvas();
        for (int i = 0; i < 3000; i++) { core.onClientTick(); frame(counting); }
        int calls = 20000;
        boolean sink = false;
        long before = allocated();
        for (int i = 0; i < calls; i++) { frame(counting); sink ^= FeatureState.on(3) ^ FeatureState.on(2) ^ FeatureState.on(200) ^ FeatureState.on(-1) ^ FeatureState.on(999); }
        long renderBytes = allocated() - before;
        before = allocated();
        for (int i = 0; i < calls; i++) { core.onClientTick(); }
        long tickBytes = allocated() - before;
        before = allocated();
        for (int n = -5; n < 300; n++) { sink ^= FeatureState.on(n); }      // a scan of the whole number range
        assertEquals(0, allocated() - before);
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

    /** Counts instead of listing: nothing to allocate in the handler. */
    private static final class CountingKeyFeature extends Feature {
        int downs;
        int ups;
        private final List<KeySpec> keys;

        CountingKeyFeature(String id, int number, int count) {
            super(id, number, FeatureGroup.HUD, EnumSet.allOf(GameVersion.class));
            KeySpec[] specs = new KeySpec[count];
            for (int i = 0; i < count; i++) {
                specs[i] = new KeySpec("count.k" + i, DefaultKey.NONE, i == count - 1 ? KeySpec.Mode.HOLD : KeySpec.Mode.PRESS);
            }
            keys = Arrays.asList(specs);
        }

        @Override public List<KeySpec> keys() { return keys; }

        @Override public void onKey(int key, boolean down, long nowNanos) {
            if (down) { downs++; } else { ups++; }
        }
    }

    @Test void feature_keys_allocate_nothing() {
        core = start(); platform.inWorld = true;
        CountingKeyFeature f = new CountingKeyFeature("counting", 47, 1);
        core.activeProfile().module("counting").setEnabled(true);
        int hold = core.keySpecs().size();
        core.registerForTest(f);
        for (int i = 0; i < 3000; i++) { core.onFeatureKey(hold, true); core.onFeatureKey(hold, false); core.onClientTick(); }
        long before = allocated();
        for (int i = 0; i < 1000; i++) { core.onFeatureKey(hold, true); core.onFeatureKey(hold, false); }
        long pairs = allocated() - before;
        before = allocated();
        for (int i = 0; i < 1000; i++) { core.onClientTick(); }
        long idle = allocated() - before;
        core.onFeatureKey(hold, true);
        before = allocated();
        for (int i = 0; i < 1000; i++) { core.onFeatureKey(hold, true); core.onClientTick(); }
        long held = allocated() - before;
        before = allocated();
        for (int i = 0; i < 1000; i++) { core.onFeatureKey(-1, true); core.onFeatureKey(hold + 5, false); }
        long outOfRange = allocated() - before;
        System.out.println("HengstCore key allocation: 1000 press+release pairs " + pairs + " bytes, 1000 idle ticks " + idle
                + " bytes, 1000 ticks with a held key " + held + " bytes, 1000 out of range " + outOfRange + " bytes");
        assertEquals(0, pairs);
        assertEquals(0, idle);
        assertEquals(0, held);
        assertEquals(0, outOfRange);
        assertTrue(f.downs >= 4000 && f.ups >= 3999);
    }

    /**
     * Index 128 and up (outside the Integer cache), a feature that is off, and a release caused by a screen opening:
     * none of them may allocate. Mutations caught: boxing the key index in a map or list lookup; building anything
     * for a press of a disabled feature; allocating in the tick's release path.
     */
    @Test void high_key_indexes_off_features_and_screen_releases_allocate_nothing() {
        core = start(); platform.inWorld = true;
        CountingKeyFeature pad = new CountingKeyFeature("pad", 46, 130);
        CountingKeyFeature on = new CountingKeyFeature("counting", 47, 2);
        CountingKeyFeature off = new CountingKeyFeature("off", 48, 2);
        core.activeProfile().module("pad").setEnabled(true);
        core.activeProfile().module("counting").setEnabled(true);
        core.activeProfile().module("off").setEnabled(false);
        core.registerForTest(pad);
        int onBase = core.keySpecs().size();
        core.registerForTest(on);
        int offBase = core.keySpecs().size();
        core.registerForTest(off);
        int onHold = onBase + 1;
        int offHold = offBase + 1;
        assertTrue(onHold >= 128 && offHold >= 128);
        for (int i = 0; i < 3000; i++) {
            core.onFeatureKey(onHold, true); core.onFeatureKey(onHold, false);
            core.onFeatureKey(offHold, true); core.onFeatureKey(offHold, false);
            core.onClientTick();
        }
        long before = allocated();
        for (int i = 0; i < 1000; i++) {
            core.onFeatureKey(onHold, true); core.onFeatureKey(onHold, false);
            core.onFeatureKey(offHold, true); core.onFeatureKey(offHold, false);
        }
        long presses = allocated() - before;
        // a release caused by a screen opening: press, open, tick (release), close, real release
        for (int i = 0; i < 3; i++) {
            core.onFeatureKey(onHold, true); platform.screenOpen = true; core.onClientTick();
            platform.screenOpen = false; core.onFeatureKey(onHold, false); core.onClientTick();
        }
        int upsBefore = on.ups;
        before = allocated();
        for (int i = 0; i < 1000; i++) {
            core.onFeatureKey(onHold, true);
            platform.screenOpen = true; core.onClientTick();
            platform.screenOpen = false; core.onFeatureKey(onHold, false); core.onClientTick();
        }
        long screenReleases = allocated() - before;
        System.out.println("HengstCore key allocation at index >= 128: presses " + presses + " bytes, screen releases " + screenReleases + " bytes");
        assertEquals(0, presses);
        assertEquals(0, screenReleases);
        assertEquals(1000, on.ups - upsBefore);
        assertEquals(0, off.downs + off.ups);
    }

    private int keyIndex(String id) {
        for (int i = 0; i < core.keySpecs().size(); i++) {
            if (core.keySpecs().get(i).id().equals(id)) { return i; }
        }
        throw new AssertionError(id);
    }

    @Test void the_stopwatch_key_path_through_the_core_allocates_nothing_with_the_feature_on_and_off() {
        core = start(); platform.inWorld = true;
        int toggle = keyIndex("stopwatch.toggle"); int reset = keyIndex("stopwatch.reset");
        // on: the handler really runs (a press, a release, a reset press), 20000 rounds
        core.activeProfile().module("stopwatch").setEnabled(true); core.applyConfigForTest();
        CountingCanvas counting = new CountingCanvas();
        for (int i = 0; i < 3000; i++) { core.onFeatureKey(toggle, true); core.onFeatureKey(toggle, false); core.onFeatureKey(reset, true); core.onFeatureKey(reset, false); }
        core.onFeatureKey(toggle, true); core.onFeatureKey(toggle, false);
        time.advanceMillis(1_500); core.onClientTick();
        assertEquals("0:01.5", ((io.github.clemensjl.hengst.mod.common.hud.modules.StopwatchModule) core.registry().byId("stopwatch")).text());
        long before = allocated();
        for (int i = 0; i < 20000; i++) {
            core.onFeatureKey(toggle, true); core.onFeatureKey(toggle, false);
            core.onFeatureKey(reset, true); core.onFeatureKey(reset, false);
        }
        long onBytes = allocated() - before;
        // off: the core gates the press
        core.activeProfile().module("stopwatch").setEnabled(false); core.applyConfigForTest();
        before = allocated();
        for (int i = 0; i < 20000; i++) {
            core.onFeatureKey(toggle, true); core.onFeatureKey(toggle, false);
            core.onFeatureKey(reset, true); core.onFeatureKey(reset, false);
        }
        long offBytes = allocated() - before;
        System.out.println("stopwatch keys through the core: on " + onBytes + " bytes, off " + offBytes + " bytes in 20000 rounds");
        assertEquals(0, onBytes);
        assertEquals(0, offBytes);
        assertTrue(counting.texts == 0);
    }
}
