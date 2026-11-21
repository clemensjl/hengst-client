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
import io.github.clemensjl.hengst.mod.common.testutil.FakeWallClock;
import java.lang.management.ManagementFactory;
import java.nio.file.Path;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Tag;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/** Direction and speed through the real renderer: nothing is allocated unless a displayed value changed. */
@Tag("interpreted")
class InfoModulesAllocationTest {
    /**
     * One shown-value change builds one String and nothing else. The longest speed text "100.00 m/s" is 10 Latin-1
     * chars: a 24-byte String object (compressed oops: header 12, hash 4, coder 1, hashIsZero 1, value reference 4,
     * padding 2) plus a 32-byte byte[10] (16 header + 10, padded to 8) = 56 bytes; the texts of the test are shorter
     * (48 bytes measured). The bound is 56 + 16: a second String (at least 48 bytes) would break it on average.
     */
    private static final long MAX_BYTES_PER_CHANGE = 72;

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
    void speed_with_realistic_jittering_steps_allocates_nothing_while_the_shown_text_stays() {
        ModuleConfig cfg = ModuleConfig.defaults();
        cfg.setOption("decimals", 2);
        platform.playerX = 4321.123456;
        platform.playerZ = -765.4321;
        for (int i = 0; i < 3000; i++) {
            platform.playerX += 0.2158 + 1e-12 * (i % 7);
            speed.tick(ctx, cfg);
        }
        String shown = speed.text();
        assertEquals("4.32 m/s", shown);
        int n = 20000;
        int changes = 0;
        long before = allocated();
        for (int i = 0; i < n; i++) {
            platform.playerX += 0.2158 + 1e-12 * (i % 7);
            speed.tick(ctx, cfg);
            if (speed.text() != shown) {
                shown = speed.text();
                changes++;
            }
        }
        long bytes = allocated() - before;
        assertEquals(0, changes, "rounding noise flipped the shown text " + changes + " times");
        assertEquals(0, bytes, "allocated " + bytes + " bytes in " + n + " ticks");
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
    }

    // Clock and memory, through the real renderer.

    /**
     * One shown-value change builds one String and nothing else. Clock text "11:00" or "11:00:01": 5 or 8 Latin-1
     * chars, so a String is a 24-byte object (compressed oops: header 12, hash 4, coder 1, hashIsZero 1, value
     * reference 4, padded) plus its byte[] of 16 + L bytes rounded up to 8 = 24: 48 bytes. Memory text
     * "RAM 25% 1024/4096 MB" has 20 chars: 24 + 40 = 64 bytes. The bound per changing tick is that size plus 16 bytes
     * of slack, which is less than a second String of either size (48 and 64 bytes), so a second String fails it.
     */
    private static final long CLOCK_STRING = 24 + ((16 + 8 + 7) & ~7);

    private static final long MEMORY_STRING = 24 + ((16 + 20 + 7) & ~7);
    private static final long SLACK = 16;

    private FakeWallClock wall;
    private ClockModule clock;
    private MemoryModule memory;
    private HudRenderer clockRenderer;

    private void startClockAndMemory(boolean seconds) {
        wall = new FakeWallClock();
        wall.utcMillis = java.time.Instant.parse("2026-11-21T10:00:00Z").toEpochMilli();
        wall.offset = 3_600_000;
        platform.memoryMax = 4096L << 20;
        platform.memoryUsed = 1000L << 20;
        ctx = new HudContext(platform, time, wall, new FrameCounter(time), new CpsCounter(time));
        clock = new ClockModule();
        memory = new MemoryModule();
        FeatureRegistry reg = new FeatureRegistry();
        reg.register(clock);
        reg.register(memory);
        Profile p = new Profile();
        ModuleConfig c = p.module("clock");
        c.setEnabled(true);
        c.setAnchor(Anchor.TOP_LEFT);
        c.setOption("seconds", seconds);
        ModuleConfig m = p.module("memory");
        m.setEnabled(true);
        m.setAnchor(Anchor.BOTTOM_RIGHT);
        clockRenderer = new HudRenderer(reg, GameVersion.V1_8_9);
        clockRenderer.rebuild(p);
        for (int i = 0; i < 3000; i++) { // warm-up: class loading, resolution, first texts
            ctx.frames().onFrame();
            clockRenderer.tick(ctx);
            clockRenderer.render(canvas, ctx);
            wall.utcMillis += 50;
            if (i % 1000 == 999) {
                wall.utcMillis -= 50_000;
            }
        }
    }

    @Test
    void clock_and_memory_with_an_unchanged_text_allocate_nothing_in_frames_and_ticks() {
        startClockAndMemory(false);
        long base = java.time.Instant.parse("2026-11-21T10:00:00Z").toEpochMilli();
        int n = 20000;
        long frameBytes = 0;
        long tickBytes = 0;
        for (int i = 0; i < n; i++) {
            // time runs in 50 ms steps inside one minute; every 1000 ticks the system clock is set back (a recalculation
            // that finds the same minute)
            wall.utcMillis = base + (i % 1000) * 50L;
            long before = allocated();
            clockRenderer.tick(ctx);
            tickBytes += allocated() - before;
            before = allocated();
            ctx.frames().onFrame();
            clockRenderer.render(canvas, ctx);
            frameBytes += allocated() - before;
        }
        assertEquals("11:00", clock.text());
        assertEquals(0, frameBytes, "allocated " + frameBytes + " bytes in " + n + " frames");
        assertEquals(0, tickBytes, "allocated " + tickBytes + " bytes in " + n + " ticks");
        assertTrue(canvas.texts > 0);
    }

    @Test
    void a_clock_change_allocates_one_string_and_nothing_else() {
        startClockAndMemory(true);
        wall.utcMillis = java.time.Instant.parse("2026-11-21T10:00:00Z").toEpochMilli();
        clockRenderer.tick(ctx);
        long maxChange = 0;
        long quiet = 0;
        int changes = 0;
        String last = clock.text();
        for (int i = 0; i < 20000; i++) {
            wall.utcMillis += 50;
            long before = allocated();
            clockRenderer.tick(ctx);
            long used = allocated() - before;
            if (clock.text() != last) { // reference compare, no allocation
                last = clock.text();
                changes++;
                maxChange = Math.max(maxChange, used);
            } else {
                quiet += used;
            }
        }
        System.out.println("ALLOC clock bytes per change: max " + maxChange + " (one String of 8 chars = " + CLOCK_STRING + ")");
        assertEquals(1000, changes, "one change per second of 20 ticks");
        assertEquals(0, quiet, "ticks without a change allocated " + quiet + " bytes");
        assertTrue(maxChange <= CLOCK_STRING + SLACK, "a change allocated " + maxChange + " bytes, bound " + (CLOCK_STRING + SLACK));
    }

    @Test
    void a_memory_change_allocates_one_string_and_nothing_else() {
        startClockAndMemory(false);
        long maxChange = 0;
        long quiet = 0;
        int changes = 0;
        String last = memory.text();
        for (int i = 0; i < 20000; i++) {
            if (i % 20 == 19) {
                platform.memoryUsed = (1000L + (i / 20) % 3000) << 20; // 1000 to 3999 MB: always 4 digits, 24 to 97 percent
            }
            long before = allocated();
            clockRenderer.tick(ctx);
            long used = allocated() - before;
            if (memory.text() != last) {
                last = memory.text();
                changes++;
                maxChange = Math.max(maxChange, used);
            } else {
                quiet += used;
            }
        }
        System.out.println("ALLOC memory bytes per change: max " + maxChange + " (one String of 20 chars = " + MEMORY_STRING + ")");
        assertTrue(changes >= 900, "too few changes to prove anything: " + changes);
        assertEquals(0, quiet, "ticks without a change allocated " + quiet + " bytes");
        assertTrue(maxChange <= MEMORY_STRING + SLACK, "a change allocated " + maxChange + " bytes, bound " + (MEMORY_STRING + SLACK));
    }

    // Ping and server address, through the real renderer.

    /**
     * Ping text "1234 ms" (the shown values of the test have 4 digits, the longest text "9999 ms" has the same length)
     * is 7 Latin-1 chars: a 24-byte String object plus a byte[7] of 16 + 7 rounded up to 8 = 24 bytes: 48 bytes. Address
     * texts: "hostNN.example.net" is 18 chars: 24 + (16 + 18 rounded up to 8 = 40) = 64 bytes; dropping the port
     * ("hostNN.example.net:25565") is one substring, the only String built. The bound is that size plus 16 bytes of
     * slack, less than a second String of either size, so a second String fails it. An address that needs the middle
     * cut builds up to four short temporaries once per new address (a server join), which is not bounded here.
     */
    private static final long PING_STRING = 24 + ((16 + 7 + 7) & ~7);

    private static final long ADDRESS_STRING = 24 + ((16 + 18 + 7) & ~7);

    private PingModule ping;
    private ServerAddressModule address;
    private HudRenderer netRenderer;

    private void startNet(String addr, int pingMs) {
        platform.serverAddress = addr;
        platform.pingMillis = pingMs;
        ping = new PingModule();
        address = new ServerAddressModule();
        FeatureRegistry reg = new FeatureRegistry();
        reg.register(ping);
        reg.register(address);
        Profile p = new Profile();
        ModuleConfig a = p.module("ping");
        a.setEnabled(true);
        a.setAnchor(Anchor.TOP_LEFT);
        ModuleConfig b = p.module("server_address");
        b.setEnabled(true);
        b.setAnchor(Anchor.BOTTOM_RIGHT);
        netRenderer = new HudRenderer(reg, GameVersion.V1_8_9);
        netRenderer.rebuild(p);
        for (int i = 0; i < 3000; i++) {
            ctx.frames().onFrame();
            netRenderer.tick(ctx);
            netRenderer.render(canvas, ctx);
        }
    }

    @Test
    void ping_and_address_with_unchanged_values_allocate_nothing_in_frames_and_ticks() {
        String a1 = "play.example.net:25565";
        String a2 = new String(a1); // equal but a different instance: the equals path runs every interval
        startNet(a1, 42);
        int n = 20000;
        long frameBytes = 0;
        long tickBytes = 0;
        for (int i = 0; i < n; i++) {
            platform.serverAddress = (i & 1) == 0 ? a1 : a2;
            long before = allocated();
            netRenderer.tick(ctx);
            tickBytes += allocated() - before;
            before = allocated();
            ctx.frames().onFrame();
            netRenderer.render(canvas, ctx);
            frameBytes += allocated() - before;
        }
        assertEquals("42 ms", ping.text());
        assertEquals("play.example.net", address.text());
        assertEquals(0, frameBytes, "allocated " + frameBytes + " bytes in " + n + " frames");
        assertEquals(0, tickBytes, "allocated " + tickBytes + " bytes in " + n + " ticks");
        assertTrue(canvas.texts > 0);
    }

    @Test
    void an_unknown_ping_and_a_hidden_state_allocate_nothing() {
        startNet("play.example.net", -1);
        assertEquals("? ms", ping.text());
        long before = allocated();
        for (int i = 0; i < 20000; i++) {
            netRenderer.tick(ctx);
        }
        assertEquals(0, allocated() - before);
        platform.serverAddress = null;
        netRenderer.tick(ctx);
        assertEquals("", ping.text());
        before = allocated();
        for (int i = 0; i < 20000; i++) {
            netRenderer.tick(ctx);
            ctx.frames().onFrame();
            netRenderer.render(canvas, ctx);
        }
        assertEquals(0, allocated() - before);
    }

    @Test
    void a_ping_change_allocates_one_string_and_nothing_else() {
        startNet("play.example.net", 1000);
        long maxChange = 0;
        long quiet = 0;
        int changes = 0;
        String last = ping.text();
        for (int i = 0; i < 20000; i++) {
            if (i % 20 == 19) {
                platform.pingMillis = 1000 + (i / 20) % 9000; // always 4 digits
            }
            long before = allocated();
            netRenderer.tick(ctx);
            long used = allocated() - before;
            if (ping.text() != last) {
                last = ping.text();
                changes++;
                maxChange = Math.max(maxChange, used);
            } else {
                quiet += used;
            }
        }
        System.out.println("ALLOC ping bytes per change: max " + maxChange + " (one String of 7 chars = " + PING_STRING + ")");
        assertTrue(changes >= 900, "too few changes to prove anything: " + changes);
        assertEquals(0, quiet, "ticks without a change allocated " + quiet + " bytes");
        assertTrue(maxChange <= PING_STRING + SLACK, "a change allocated " + maxChange + " bytes, bound " + (PING_STRING + SLACK));
    }

    @Test
    void an_address_change_allocates_one_string_and_nothing_else() {
        String[] addresses = new String[64];
        for (int i = 0; i < addresses.length; i++) {
            addresses[i] = "host" + (10 + i) + ".example.net:25565";
        }
        startNet(addresses[0], 42);
        long maxChange = 0;
        long quiet = 0;
        int changes = 0;
        String last = address.text();
        for (int i = 0; i < 20000; i++) {
            if (i % 20 == 19) {
                platform.serverAddress = addresses[(i / 20) % addresses.length];
            }
            long before = allocated();
            netRenderer.tick(ctx);
            long used = allocated() - before;
            if (address.text() != last) {
                last = address.text();
                changes++;
                maxChange = Math.max(maxChange, used);
            } else {
                quiet += used;
            }
        }
        System.out.println("ALLOC address bytes per change: max " + maxChange + " (one String of 18 chars = " + ADDRESS_STRING + ")");
        assertTrue(changes >= 900, "too few changes to prove anything: " + changes);
        assertEquals(0, quiet, "ticks without a change allocated " + quiet + " bytes");
        assertTrue(maxChange <= ADDRESS_STRING + SLACK, "a change allocated " + maxChange + " bytes, bound " + (ADDRESS_STRING + SLACK));
    }

    // Stopwatch, through the real renderer.

    /**
     * One shown-value change builds one String and nothing else. The longest text of the test, "123:45:06.7", has 11
     * Latin-1 chars: a 24-byte String object (compressed oops: header 12, hash 4, coder 1, hashIsZero 1, value
     * reference 4, padded) plus a byte[11] of 16 + 11 rounded up to 8 = 32 bytes: 56 bytes. The bound is that size
     * plus 16 bytes of slack, which is less than a second String (at least 48 bytes), so a second String fails it.
     */
    private static final long STOPWATCH_STRING = 24 + ((16 + 11 + 7) & ~7);

    private StopwatchModule stopwatch;
    private HudRenderer stopwatchRenderer;

    private void startStopwatch(boolean tenths) {
        stopwatch = new StopwatchModule();
        FeatureRegistry reg = new FeatureRegistry();
        reg.register(stopwatch);
        Profile p = new Profile();
        ModuleConfig c = p.module("stopwatch");
        c.setEnabled(true);
        c.setAnchor(Anchor.TOP_LEFT);
        c.setOption("tenths", tenths);
        stopwatchRenderer = new HudRenderer(reg, GameVersion.V1_8_9);
        stopwatchRenderer.rebuild(p);
        for (int i = 0; i < 3000; i++) {
            ctx.frames().onFrame();
            stopwatchRenderer.tick(ctx);
            stopwatchRenderer.render(canvas, ctx);
        }
    }

    @Test
    void a_stopped_stopwatch_allocates_nothing_in_frames_and_ticks_at_zero_and_with_a_value() {
        startStopwatch(true);
        assertEquals("0:00.0", stopwatch.text());
        for (int round = 0; round < 2; round++) {
            int n = 20000;
            long frameBytes = 0;
            long tickBytes = 0;
            for (int i = 0; i < n; i++) {
                time.advanceMillis(50);
                long before = allocated();
                stopwatchRenderer.tick(ctx);
                tickBytes += allocated() - before;
                before = allocated();
                ctx.frames().onFrame();
                stopwatchRenderer.render(canvas, ctx);
                frameBytes += allocated() - before;
            }
            assertEquals(0, frameBytes, "allocated " + frameBytes + " bytes in " + n + " frames, round " + round);
            assertEquals(0, tickBytes, "allocated " + tickBytes + " bytes in " + n + " ticks, round " + round);
            if (round == 0) { // now with a stopped, non-zero value
                stopwatch.onKey(0, true, time.now);
                time.advanceMillis(123 * 3_600_000L + 45 * 60_000L + 6_700L);
                stopwatchRenderer.tick(ctx);
                stopwatch.onKey(0, true, time.now);
                stopwatchRenderer.tick(ctx);
                assertEquals("123:45:06.7", stopwatch.text());
            }
        }
        assertTrue(canvas.texts > 0);
    }

    @Test
    void a_running_stopwatch_allocates_one_string_per_shown_change_and_nothing_else() {
        startStopwatch(true);
        stopwatch.onKey(0, true, time.now);
        time.advanceMillis(123 * 3_600_000L + 45 * 60_000L + 6_000L);
        stopwatchRenderer.tick(ctx);
        long maxChange = 0;
        long quiet = 0;
        int changes = 0;
        String last = stopwatch.text();
        for (int i = 0; i < 20000; i++) {
            time.advanceMillis(50); // 20 ticks per second: a new tenth every second tick
            long before = allocated();
            stopwatchRenderer.tick(ctx);
            long used = allocated() - before;
            if (stopwatch.text() != last) { // reference compare, no allocation
                last = stopwatch.text();
                changes++;
                maxChange = Math.max(maxChange, used);
            } else {
                quiet += used;
            }
        }
        System.out.println("ALLOC stopwatch bytes per change: max " + maxChange + " (one String of 11 chars = " + STOPWATCH_STRING + ")");
        assertEquals(10000, changes, "one change per tenth of a second, 10 000 in 1000 seconds");
        assertEquals(0, quiet, "ticks without a change allocated " + quiet + " bytes");
        assertTrue(maxChange <= STOPWATCH_STRING + SLACK, "a change allocated " + maxChange + " bytes, bound " + (STOPWATCH_STRING + SLACK));
    }

    @Test
    void the_stopwatch_key_paths_allocate_nothing() {
        startStopwatch(true);
        long before = allocated();
        for (int i = 0; i < 20000; i++) {
            stopwatch.onKey(0, true, time.now);
            stopwatch.onKey(1, true, time.now);
            stopwatch.onKey(0, false, time.now);
            stopwatch.onKey(7, true, time.now);
        }
        assertEquals(0, allocated() - before);
    }

    /**
     * A new address that needs the cut, with a port: the module builds the shown text in one reused builder and makes
     * exactly one String of 40 Latin-1 chars: 24 + (16 + 40 = 56) = 80 bytes. The bound is that plus 16 bytes of slack
     * (96); a second String of any size (at least 48 bytes) breaks it, so does a port-free substring (51 chars, 88
     * bytes) built next to the result.
     */
    private static final long CUT_ADDRESS_STRING = 24 + (16 + 40);

    @Test
    void a_new_address_that_needs_the_cut_allocates_one_string_and_nothing_else() {
        String[] addresses = new String[64];
        for (int i = 0; i < addresses.length; i++) {
            addresses[i] = "aaaaaaaaaa.bbbbbbbbbb.cccccccccc.dddddddddd." + (10 + i) + "eee:25565"; // 55 chars, 49 without the port
        }
        startNet(addresses[0], 42);
        long maxChange = 0;
        long quiet = 0;
        int changes = 0;
        String last = address.text();
        for (int i = 0; i < 20000; i++) {
            if (i % 20 == 19) {
                platform.serverAddress = addresses[(i / 20) % addresses.length];
            }
            long before = allocated();
            netRenderer.tick(ctx);
            long used = allocated() - before;
            if (address.text() != last) {
                last = address.text();
                changes++;
                maxChange = Math.max(maxChange, used);
            } else {
                quiet += used;
            }
        }
        assertEquals(40, address.text().length());
        System.out.println("ALLOC cut address bytes per change: max " + maxChange + " (one String of 40 chars = " + CUT_ADDRESS_STRING + ")");
        assertTrue(changes >= 900, "too few changes to prove anything: " + changes);
        assertEquals(0, quiet, "ticks without a change allocated " + quiet + " bytes");
        assertTrue(maxChange <= CUT_ADDRESS_STRING + SLACK, "a change allocated " + maxChange + " bytes, bound " + (CUT_ADDRESS_STRING + SLACK));
    }

    /**
     * Fitting to the screen: a 40-char address of wide glyphs at scale 3 on a 320 px canvas is cut again at the first
     * frame after the text, the scale or the canvas changed (that cold step builds a few short temporaries, once);
     * every further frame draws the cached fitted String and allocates nothing.
     */
    @Test
    void a_fitted_text_allocates_nothing_after_the_first_frame() {
        platform.serverAddress = "WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW";
        ServerAddressModule m = new ServerAddressModule();
        FeatureRegistry reg = new FeatureRegistry();
        reg.register(m);
        Profile p = new Profile();
        ModuleConfig c = p.module("server_address");
        c.setEnabled(true);
        c.setAnchor(Anchor.TOP_RIGHT);
        c.setScale(3.0f);
        c.setBackground(true);
        HudRenderer r = new HudRenderer(reg, GameVersion.V1_8_9);
        r.rebuild(p);
        io.github.clemensjl.hengst.mod.common.testutil.WideGlyphCanvas wide =
                new io.github.clemensjl.hengst.mod.common.testutil.WideGlyphCanvas(320, 240);
        r.tick(ctx);
        long before = allocated();
        ctx.frames().onFrame();
        r.render(wide, ctx);
        long first = allocated() - before;
        for (int i = 0; i < 3000; i++) {
            ctx.frames().onFrame();
            r.tick(ctx);
            r.render(wide, ctx);
        }
        wide.reset();
        int n = 20000;
        long frameBytes = 0;
        long tickBytes = 0;
        for (int i = 0; i < n; i++) {
            before = allocated();
            r.tick(ctx);
            tickBytes += allocated() - before;
            before = allocated();
            ctx.frames().onFrame();
            r.render(wide, ctx);
            frameBytes += allocated() - before;
        }
        System.out.println("ALLOC fitted text first frame: " + first + " bytes");
        assertTrue(wide.lastText.contains(".."), wide.lastText);
        assertTrue(wide.maxRight <= 320 && wide.minLeft >= 0, "text reaches " + wide.minLeft + " to " + wide.maxRight);
        assertEquals(n, wide.texts);
        assertEquals(0, frameBytes, "allocated " + frameBytes + " bytes in " + n + " frames");
        assertEquals(0, tickBytes, "allocated " + tickBytes + " bytes in " + n + " ticks");
    }
}
