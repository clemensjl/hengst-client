package io.github.clemensjl.hengst.mod.common.hud.modules;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.feature.OptionSpec;
import io.github.clemensjl.hengst.mod.common.hud.CpsCounter;
import io.github.clemensjl.hengst.mod.common.hud.FrameCounter;
import io.github.clemensjl.hengst.mod.common.hud.HudContext;
import io.github.clemensjl.hengst.mod.common.platform.GameKey;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.testutil.FakeCanvas;
import io.github.clemensjl.hengst.mod.common.testutil.FakePlatform;
import io.github.clemensjl.hengst.mod.common.testutil.FakeTime;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.List;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class ModulesTest {
    @TempDir Path dir;
    FakePlatform platform;
    FakeTime time;
    HudContext ctx;
    ModuleConfig cfg;

    @BeforeEach
    void setUp() {
        platform = new FakePlatform(dir);
        time = new FakeTime();
        cfg = ModuleConfig.defaults();
        ctx = new HudContext(platform, time, new FrameCounter(time), new CpsCounter(time));
    }

    private String coords(double x, double y, double z, int decimals) {
        platform.inWorld = true;
        platform.playerX = x;
        platform.playerY = y;
        platform.playerZ = z;
        cfg.setOption("decimals", decimals);
        CoordinatesModule m = new CoordinatesModule();
        m.tick(ctx, cfg);
        return m.text();
    }

    // identity

    @Test
    void ids_numbers_and_versions_are_the_persisted_ones() {
        assertIdentity(new FpsModule(), "fps", 1);
        assertIdentity(new CpsModule(), "cps", 2);
        assertIdentity(new KeystrokesModule(), "keystrokes", 3);
        assertIdentity(new CoordinatesModule(), "coordinates", 6);
        assertIdentity(new DirectionModule(), "direction", 7);
        assertIdentity(new SpeedModule(), "speed", 15);
        assertIdentity(new PingModule(), "ping", 8);
        assertIdentity(new ClockModule(), "clock", 9);
        assertIdentity(new ServerAddressModule(), "server_address", 10);
        assertIdentity(new MemoryModule(), "memory", 11);
        assertIdentity(new StopwatchModule(), "stopwatch", 21);
    }

    private static void assertIdentity(io.github.clemensjl.hengst.mod.common.feature.Feature f, String id, int nr) {
        assertEquals(id, f.id());
        assertEquals(nr, f.number());
        for (GameVersion v : GameVersion.values()) {
            assertTrue(f.supports(v), id + " on " + v);
        }
    }

    @Test
    void option_specs_match_the_persisted_keys_and_defaults() {
        List<OptionSpec> cps = new CpsModule().options();
        assertEquals(1, cps.size());
        assertEquals("show_right", cps.get(0).key());
        assertEquals(OptionSpec.Kind.TOGGLE, cps.get(0).kind());
        assertEquals(Boolean.TRUE, cps.get(0).defaultValue());

        List<OptionSpec> keys = new KeystrokesModule().options();
        assertEquals(2, keys.size());
        assertEquals("show_mouse", keys.get(0).key());
        assertEquals(Boolean.TRUE, keys.get(0).defaultValue());
        assertEquals("show_space", keys.get(1).key());
        assertEquals(Boolean.TRUE, keys.get(1).defaultValue());

        List<OptionSpec> coords = new CoordinatesModule().options();
        assertEquals(1, coords.size());
        OptionSpec d = coords.get(0);
        assertEquals("decimals", d.key());
        assertEquals(OptionSpec.Kind.INT_RANGE, d.kind());
        assertEquals(0, d.defaultValue());
        assertEquals(0f, d.min());
        assertEquals(2f, d.max());

        List<OptionSpec> direction = new DirectionModule().options();
        assertEquals(1, direction.size());
        OptionSpec style = direction.get(0);
        assertEquals("style", style.key());
        assertEquals(OptionSpec.Kind.CHOICE, style.kind());
        assertEquals("cardinal_degrees", style.defaultValue());
        assertEquals("cardinal", style.value(0));
        assertEquals("cardinal_degrees", style.value(1));
        assertEquals("degrees", style.value(2));

        List<OptionSpec> speed = new SpeedModule().options();
        assertEquals(2, speed.size());
        assertEquals("include_vertical", speed.get(0).key());
        assertEquals(OptionSpec.Kind.TOGGLE, speed.get(0).kind());
        assertEquals(Boolean.FALSE, speed.get(0).defaultValue());
        assertEquals("decimals", speed.get(1).key());
        assertEquals(OptionSpec.Kind.INT_RANGE, speed.get(1).kind());
        assertEquals(1, speed.get(1).defaultValue());
        assertEquals(0f, speed.get(1).min());
        assertEquals(2f, speed.get(1).max());

        assertTrue(new FpsModule().options().isEmpty());
    }

    // fps

    @Test
    void fps_text_follows_the_counter_and_keeps_its_reference_while_unchanged() {
        FpsModule fps = new FpsModule();
        fps.tick(ctx, cfg);
        assertEquals("0 FPS", fps.text());
        for (int i = 0; i < 144; i++) {
            ctx.frames().onFrame();
        }
        time.advanceMillis(1000);
        ctx.frames().onFrame();
        fps.tick(ctx, cfg);
        assertEquals("144 FPS", fps.text());
        String first = fps.text();
        fps.tick(ctx, cfg);
        fps.tick(ctx, cfg);
        assertSame(first, fps.text());
    }

    // cps

    @Test
    void cps_text_with_and_without_right() {
        for (int i = 0; i < 3; i++) {
            ctx.cps().onPress(0);
        }
        ctx.cps().onPress(1);
        CpsModule cps = new CpsModule();
        cps.tick(ctx, cfg);
        assertEquals("3 | 1 CPS", cps.text());
        cfg.setOption("show_right", Boolean.FALSE);
        cps.tick(ctx, cfg);
        assertEquals("3 CPS", cps.text());
        cfg.setOption("show_right", Boolean.TRUE);
        cps.tick(ctx, cfg);
        assertEquals("3 | 1 CPS", cps.text());
    }

    @Test
    void cps_text_shows_the_buffer_size_at_most_and_zero_after_a_second() {
        for (int i = 0; i < 100; i++) {
            ctx.cps().onPress(0);
        }
        CpsModule cps = new CpsModule();
        cps.tick(ctx, cfg);
        assertEquals("64 | 0 CPS", cps.text());
        time.advanceMillis(1000);
        cps.tick(ctx, cfg);
        assertEquals("0 | 0 CPS", cps.text());
    }

    @Test
    void cps_text_reference_is_kept_while_the_counts_are_unchanged() {
        ctx.cps().onPress(0);
        CpsModule cps = new CpsModule();
        cps.tick(ctx, cfg);
        String first = cps.text();
        cps.tick(ctx, cfg);
        assertSame(first, cps.text());
    }

    // coordinates

    @Test
    void coordinates_floor_and_reuse_text() {
        platform.playerX = -0.5;
        platform.playerY = 64.9;
        platform.playerZ = 12.0;
        platform.inWorld = true;
        CoordinatesModule coords = new CoordinatesModule();
        coords.tick(ctx, cfg);
        assertEquals("X -1  Y 64  Z 12", coords.text());
        String first = coords.text();
        platform.playerX = -0.7;
        coords.tick(ctx, cfg);
        assertSame(first, coords.text());
    }

    @Test
    void coordinates_with_two_decimals_round() {
        platform.inWorld = true;
        platform.playerX = 1.239;
        platform.playerY = 64.0;
        platform.playerZ = -0.5;
        cfg.setOption("decimals", 2);
        CoordinatesModule coords = new CoordinatesModule();
        coords.tick(ctx, cfg);
        assertEquals("X 1.24  Y 64.00  Z -0.50", coords.text());
    }

    @Test
    void coordinates_negative_values() {
        assertEquals("X -13  Y -1  Z -30", coords(-12.25, -0.5, -29.5, 0));
        assertEquals("X -12.25  Y -0.50  Z -29.50", coords(-12.25, -0.5, -29.5, 2));
    }

    @Test
    void coordinates_negative_zero_never_prints_a_minus() {
        assertEquals("X 0  Y 0  Z 0", coords(-0.0, 0.0, -0.0, 0));
        assertEquals("X 0.00  Y 0.00  Z 0.00", coords(-0.0, 0.0, -0.0, 2));
    }

    @Test
    void coordinates_near_zero() {
        assertEquals("X 0.00  Y 0.02  Z 0.50", coords(0.004, 0.019, 0.5, 2));
        assertEquals("X 0  Y 0  Z 0", coords(0.999, 0.1, 0.0001, 0));
        // without decimals: floor, like the block position; with decimals: rounded, and no sign on a rounded zero
        assertEquals("X -1  Y 0  Z 0", coords(-0.001, 0.0, 0.0, 0));
        assertEquals("X 0.00  Y 0.00  Z 0.00", coords(-0.001, 0.0, 0.0, 2));
    }

    @Test
    void coordinates_round_with_decimals_and_floor_without() {
        assertEquals("X 0.29  Y 0.29  Z 0.29", coords(0.29, 0.29, 0.29, 2));
        assertEquals("X 124.00  Y 124.00  Z 124.00", coords(123.999, 123.999, 123.999, 2));
        assertEquals("X 0.0  Y 0.0  Z 0.0", coords(-0.04, -0.04, -0.04, 1));
        assertEquals("X -1  Y -1  Z -1", coords(-0.5, -0.5, -0.5, 0));
        assertEquals("X -0.5  Y -0.5  Z -0.5", coords(-0.5, -0.5, -0.5, 1));
        assertEquals("X 0.01  Y 0.01  Z 0.01", coords(0.005, 0.005, 0.005, 2));
        assertEquals("X -0.01  Y -0.01  Z -0.01", coords(-0.005, -0.005, -0.005, 2));
        assertEquals("X 30000000.00  Y 30000000.00  Z 30000000.00", coords(29999999.999, 29999999.999, 29999999.999, 2));
    }

    @Test
    void coordinates_show_a_placeholder_for_axes_that_are_not_sane_numbers() {
        assertEquals("X ?  Y 64  Z ?", coords(1e20, 64.0, Double.NaN, 0));
        assertEquals("X ?  Y ?  Z ?", coords(Double.POSITIVE_INFINITY, Double.NEGATIVE_INFINITY, -1e16, 2));
        assertEquals("X ?  Y 1.00  Z ?", coords(Double.NaN, 1.0, 1e20, 2));
        assertEquals("X 1000000000000000  Y -1000000000000000  Z 0", coords(1e15, -1e15, 0.0, 0));
    }

    @Test
    void coordinates_recover_after_a_placeholder() {
        platform.inWorld = true;
        platform.playerX = Double.NaN;
        CoordinatesModule m = new CoordinatesModule();
        m.tick(ctx, cfg);
        assertEquals("X ?  Y 0  Z 0", m.text());
        String first = m.text();
        m.tick(ctx, cfg);
        assertSame(first, m.text());
        platform.playerX = 5;
        m.tick(ctx, cfg);
        assertEquals("X 5  Y 0  Z 0", m.text());
    }

    @Test
    void coordinates_large_values() {
        assertEquals("X 30000000  Y 319  Z -30000000", coords(30000000.0, 319.5, -30000000.0, 0));
        assertEquals("X 30000000.00  Y 319.50  Z -29999999.75", coords(30000000.0, 319.5, -29999999.75, 2));
    }

    @Test
    void coordinates_decimals_at_minimum_and_maximum_and_clamped_beyond() {
        assertEquals("X 1  Y 2  Z 3", coords(1.75, 2.5, 3.25, 0));
        assertEquals("X 1.75  Y 2.50  Z 3.25", coords(1.75, 2.5, 3.25, 2));
        assertEquals("X 1.75  Y 2.50  Z 3.25", coords(1.75, 2.5, 3.25, 7));
        assertEquals("X 1  Y 2  Z 3", coords(1.75, 2.5, 3.25, -4));
    }

    @Test
    void coordinates_changing_the_decimals_rebuilds_even_if_the_scaled_values_match() {
        platform.inWorld = true;
        CoordinatesModule m = new CoordinatesModule();
        m.tick(ctx, cfg);
        assertEquals("X 0  Y 0  Z 0", m.text());
        cfg.setOption("decimals", 2);
        m.tick(ctx, cfg);
        assertEquals("X 0.00  Y 0.00  Z 0.00", m.text());
    }

    @Test
    void coordinates_pad_the_fraction_with_zeros() {
        assertEquals("X 5.06  Y 5.00  Z 5.50", coords(5.0625, 5.0, 5.5, 2));
    }

    @Test
    void coordinates_are_empty_with_zero_width_outside_a_world() {
        FakeCanvas canvas = new FakeCanvas();
        platform.inWorld = true;
        platform.playerX = 12;
        CoordinatesModule m = new CoordinatesModule();
        m.tick(ctx, cfg);
        assertEquals("X 12  Y 0  Z 0", m.text());
        assertEquals(14 * 6 + 4, m.width(canvas, cfg));
        platform.inWorld = false;
        m.tick(ctx, cfg);
        assertEquals("", m.text());
        assertEquals(0, m.width(canvas, cfg));
        platform.inWorld = true;
        m.tick(ctx, cfg);
        assertEquals("X 12  Y 0  Z 0", m.text());
        assertEquals(14 * 6 + 4, m.width(canvas, cfg));
    }

    // keystrokes

    private void labels() {
        platform.labels.put(GameKey.FORWARD, "W");
        platform.labels.put(GameKey.LEFT, "A");
        platform.labels.put(GameKey.BACK, "S");
        platform.labels.put(GameKey.RIGHT, "D");
    }

    @Test
    void keystrokes_size_follows_options() {
        KeystrokesModule k = new KeystrokesModule();
        FakeCanvas c = new FakeCanvas();
        assertEquals(58, k.width(c, cfg));
        assertEquals(70, k.height(c, cfg));
        cfg.setOption("show_space", Boolean.FALSE);
        k.tick(ctx, cfg);
        assertEquals(58, k.width(c, cfg));
        assertEquals(58, k.height(c, cfg));
        cfg.setOption("show_space", Boolean.TRUE);
        cfg.setOption("show_mouse", Boolean.FALSE);
        k.tick(ctx, cfg);
        assertEquals(50, k.height(c, cfg));
        cfg.setOption("show_space", Boolean.FALSE);
        k.tick(ctx, cfg);
        assertEquals(58, k.width(c, cfg));
        assertEquals(38, k.height(c, cfg));
    }

    @Test
    void keystrokes_height_and_render_use_the_options_cached_by_the_last_tick() {
        KeystrokesModule k = new KeystrokesModule();
        FakeCanvas c = new FakeCanvas();
        k.tick(ctx, cfg);
        k.render(c, cfg, ctx);
        int full = c.calls.size();
        cfg.setOption("show_mouse", Boolean.FALSE);
        cfg.setOption("show_space", Boolean.FALSE);
        // Not ticked yet: neither height nor render read the config, they still show the cached state.
        assertEquals(70, k.height(c, cfg));
        c.calls.clear();
        k.render(c, cfg, ctx);
        assertEquals(full, c.calls.size());
        k.tick(ctx, cfg);
        assertEquals(38, k.height(c, cfg));
        c.calls.clear();
        k.render(c, cfg, ctx);
        assertEquals(full - 5, c.calls.size());            // two mouse cells with their labels, the space bar
    }

    @Test
    void keystrokes_draws_pressed_key_highlighted() {
        platform.down.add(GameKey.FORWARD);
        KeystrokesModule k = new KeystrokesModule();
        FakeCanvas c = new FakeCanvas();
        k.tick(ctx, cfg);
        k.render(c, cfg, ctx);
        assertTrue(c.calls.contains("fill 20 0 38 18 b0ffffff"));
        assertTrue(c.calls.contains("fill 0 20 18 38 6f000000"));
    }

    @Test
    void keystrokes_layout_with_nothing_pressed() {
        labels();
        KeystrokesModule k = new KeystrokesModule();
        FakeCanvas c = new FakeCanvas();
        k.tick(ctx, cfg);
        k.render(c, cfg, ctx);
        assertEquals(Arrays.asList(
                "fill 20 0 38 18 6f000000", "text W 26 4 ffffffff true",
                "fill 0 20 18 38 6f000000", "text A 6 24 ffffffff true",
                "fill 20 20 38 38 6f000000", "text S 26 24 ffffffff true",
                "fill 40 20 58 38 6f000000", "text D 46 24 ffffffff true",
                "fill 0 40 28 58 6f000000", "text LMB 5 44 ffffffff true",
                "fill 30 40 58 58 6f000000", "text RMB 35 44 ffffffff true",
                "fill 0 60 58 70 6f000000"), c.calls);
    }

    @Test
    void keystrokes_layout_with_everything_pressed() {
        labels();
        platform.down.addAll(Arrays.asList(GameKey.values()));
        KeystrokesModule k = new KeystrokesModule();
        FakeCanvas c = new FakeCanvas();
        k.tick(ctx, cfg);
        k.render(c, cfg, ctx);
        assertEquals(Arrays.asList(
                "fill 20 0 38 18 b0ffffff", "text W 26 4 ff000000 false",
                "fill 0 20 18 38 b0ffffff", "text A 6 24 ff000000 false",
                "fill 20 20 38 38 b0ffffff", "text S 26 24 ff000000 false",
                "fill 40 20 58 38 b0ffffff", "text D 46 24 ff000000 false",
                "fill 0 40 28 58 b0ffffff", "text LMB 5 44 ff000000 false",
                "fill 30 40 58 58 b0ffffff", "text RMB 35 44 ff000000 false",
                "fill 0 60 58 70 b0ffffff"), c.calls);
    }

    @Test
    void keystrokes_mouse_and_space_follow_their_own_keys() {
        labels();
        platform.down.add(GameKey.USE);
        KeystrokesModule k = new KeystrokesModule();
        FakeCanvas c = new FakeCanvas();
        k.tick(ctx, cfg);
        k.render(c, cfg, ctx);
        assertTrue(c.calls.contains("fill 0 40 28 58 6f000000"));
        assertTrue(c.calls.contains("fill 30 40 58 58 b0ffffff"));
        assertTrue(c.calls.contains("fill 0 60 58 70 6f000000"));
        platform.down.clear();
        platform.down.add(GameKey.JUMP);
        c.calls.clear();
        k.render(c, cfg, ctx);
        assertTrue(c.calls.contains("fill 0 60 58 70 b0ffffff"));
        assertTrue(c.calls.contains("fill 30 40 58 58 6f000000"));
    }

    @Test
    void keystrokes_rows_are_left_out_when_their_options_are_off() {
        labels();
        cfg.setOption("show_mouse", Boolean.FALSE);
        KeystrokesModule k = new KeystrokesModule();
        FakeCanvas c = new FakeCanvas();
        k.tick(ctx, cfg);
        k.render(c, cfg, ctx);
        assertEquals(9, c.calls.size());
        assertEquals("fill 0 40 58 50 6f000000", c.calls.get(8));
        cfg.setOption("show_space", Boolean.FALSE);
        k.tick(ctx, cfg);
        c.calls.clear();
        k.render(c, cfg, ctx);
        assertEquals(8, c.calls.size());
    }

    @Test
    void keystrokes_uses_the_text_color_and_shadow_of_the_config_when_released() {
        labels();
        cfg.setTextColor(0xFF00FF00);
        cfg.setShadow(false);
        KeystrokesModule k = new KeystrokesModule();
        FakeCanvas c = new FakeCanvas();
        k.tick(ctx, cfg);
        k.render(c, cfg, ctx);
        assertEquals("text W 26 4 ff00ff00 false", c.calls.get(1));
    }

    @Test
    void keystrokes_labels_come_from_the_platform_and_refresh_every_20_ticks() {
        labels();
        platform.labels.put(GameKey.FORWARD, "Z");
        KeystrokesModule k = new KeystrokesModule();
        FakeCanvas c = new FakeCanvas();
        k.tick(ctx, cfg);
        k.render(c, cfg, ctx);
        assertEquals("text Z 26 4 ffffffff true", c.calls.get(1));
        platform.labels.put(GameKey.FORWARD, "Up");
        for (int i = 0; i < 19; i++) {
            k.tick(ctx, cfg);
        }
        c.calls.clear();
        k.render(c, cfg, ctx);
        assertEquals("text Z 26 4 ffffffff true", c.calls.get(1));
        k.tick(ctx, cfg);
        c.calls.clear();
        k.render(c, cfg, ctx);
        assertEquals("text Up 23 4 ffffffff true", c.calls.get(1));
    }

    @Test
    void keystrokes_long_label_is_cut_to_the_longest_prefix_that_fits_the_cell() {
        labels();
        platform.labels.put(GameKey.FORWARD, "Button 4");
        KeystrokesModule k = new KeystrokesModule();
        FakeCanvas c = new FakeCanvas();
        k.tick(ctx, cfg);
        k.render(c, cfg, ctx);
        // 18 px cell, 6 px per char: "But" is the longest prefix that fits, centred at 20 + (18 - 18) / 2
        assertEquals("text But 20 4 ffffffff true", c.calls.get(1));
        c.calls.clear();
        k.render(c, cfg, ctx);
        assertEquals("text But 20 4 ffffffff true", c.calls.get(1));
    }

    @Test
    void keystrokes_label_that_fits_exactly_is_not_cut() {
        labels();
        platform.labels.put(GameKey.FORWARD, "Up");
        platform.labels.put(GameKey.LEFT, "Tab");
        KeystrokesModule k = new KeystrokesModule();
        FakeCanvas c = new FakeCanvas();
        k.tick(ctx, cfg);
        k.render(c, cfg, ctx);
        assertEquals("text Up 23 4 ffffffff true", c.calls.get(1));
        assertEquals("text Tab 0 24 ffffffff true", c.calls.get(3));
    }

    @Test
    void keystrokes_null_and_empty_labels_draw_no_text() {
        labels();
        platform.labels.put(GameKey.FORWARD, null);
        platform.labels.put(GameKey.LEFT, "");
        KeystrokesModule k = new KeystrokesModule();
        FakeCanvas c = new FakeCanvas();
        k.tick(ctx, cfg);
        k.render(c, cfg, ctx);
        int texts = 0;
        for (String call : c.calls) {
            if (call.startsWith("text ")) {
                texts++;
            }
        }
        assertEquals(4, texts);   // S, D, LMB, RMB
        assertEquals("fill 20 0 38 18 6f000000", c.calls.get(0));
        assertEquals("fill 0 20 18 38 6f000000", c.calls.get(1));
    }

    @Test
    void keystrokes_label_that_changes_is_fitted_again() {
        labels();
        platform.labels.put(GameKey.FORWARD, "Button 4");
        KeystrokesModule k = new KeystrokesModule();
        FakeCanvas c = new FakeCanvas();
        k.tick(ctx, cfg);
        platform.labels.put(GameKey.FORWARD, "W");
        for (int i = 0; i < 20; i++) {
            k.tick(ctx, cfg);
        }
        k.render(c, cfg, ctx);
        assertEquals("text W 26 4 ffffffff true", c.calls.get(1));
    }

    @Test
    void keystrokes_label_width_is_measured_once_per_label() {
        labels();
        final int[] measured = {0};
        FakeCanvas c = new FakeCanvas() {
            @Override
            public int textWidth(String text) {
                measured[0]++;
                return super.textWidth(text);
            }
        };
        KeystrokesModule k = new KeystrokesModule();
        k.tick(ctx, cfg);
        k.render(c, cfg, ctx);
        k.render(c, cfg, ctx);
        k.render(c, cfg, ctx);
        assertEquals(6, measured[0]);
    }

    @Test
    void clock_and_memory_options_defaults_and_places() {
        List<OptionSpec> clock = new ClockModule().options();
        assertEquals(2, clock.size());
        assertEquals("format", clock.get(0).key());
        assertEquals(OptionSpec.Kind.CHOICE, clock.get(0).kind());
        assertEquals("24h", clock.get(0).defaultValue());
        assertEquals("seconds", clock.get(1).key());
        assertEquals(OptionSpec.Kind.TOGGLE, clock.get(1).kind());
        assertEquals(Boolean.FALSE, clock.get(1).defaultValue());
        List<OptionSpec> memory = new MemoryModule().options();
        assertEquals(1, memory.size());
        assertEquals("style", memory.get(0).key());
        assertEquals("both", memory.get(0).defaultValue());
        for (io.github.clemensjl.hengst.mod.common.hud.HudModule m : new io.github.clemensjl.hengst.mod.common.hud.HudModule[] {new ClockModule(), new MemoryModule()}) {
            assertEquals(io.github.clemensjl.hengst.mod.common.layout.Anchor.TOP_RIGHT, m.defaultAnchor());
            assertEquals(-2, m.defaultOffsetX());
        }
        assertEquals(66, new ClockModule().defaultOffsetY());
        assertEquals(94, new MemoryModule().defaultOffsetY());
    }

    @Test
    void ping_and_server_address_options_defaults_and_places() {
        assertTrue(new PingModule().options().isEmpty());
        List<OptionSpec> sa = new ServerAddressModule().options();
        assertEquals(1, sa.size());
        assertEquals("show_port", sa.get(0).key());
        assertEquals(OptionSpec.Kind.TOGGLE, sa.get(0).kind());
        assertEquals(Boolean.FALSE, sa.get(0).defaultValue());
        for (io.github.clemensjl.hengst.mod.common.hud.HudModule m : new io.github.clemensjl.hengst.mod.common.hud.HudModule[] {new PingModule(), new ServerAddressModule()}) {
            assertEquals(io.github.clemensjl.hengst.mod.common.layout.Anchor.TOP_RIGHT, m.defaultAnchor());
            assertEquals(-2, m.defaultOffsetX());
        }
        assertEquals(52, new PingModule().defaultOffsetY());
        assertEquals(80, new ServerAddressModule().defaultOffsetY());
    }

    @Test
    void default_places_of_the_eleven_modules_do_not_overlap_at_320_by_240() {
        assertDefaultPlacesApart("en_us");
        assertDefaultPlacesApart("de_de");
    }

    /** Worst-case texts: direction "NW 337°" (the longest course text), speed "100.00 m/s", coordinates with 2 decimals. */
    private void assertDefaultPlacesApart(String language) {
        platform.inWorld = true;
        platform.worldIdentity = new Object();
        platform.playerYaw = 157f; // course 337: "NW 337°"
        platform.down.add(GameKey.FORWARD);
        platform.playerX = 12345.6789;
        platform.playerY = 120.5;
        platform.playerZ = -12345.6789;
        platform.serverAddress = "WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW"; // 60 chars, shown as 19 W, .., 19 W
        platform.pingMillis = 12345; // shown as "9999 ms"
        platform.memoryUsed = 16384L << 20; // worst case: "RAM 100% 16384/16384 MB"
        platform.memoryMax = 16384L << 20;
        io.github.clemensjl.hengst.mod.common.testutil.FakeWallClock wall =
                new io.github.clemensjl.hengst.mod.common.testutil.FakeWallClock();
        wall.utcMillis = java.time.Instant.parse("2026-11-21T23:58:58Z").toEpochMilli(); // "11:58:58 PM"
        HudContext wallCtx = new HudContext(platform, time, wall, new FrameCounter(time), new CpsCounter(time));
        wallCtx.setI18n(io.github.clemensjl.hengst.mod.common.I18n.load(language));
        io.github.clemensjl.hengst.mod.common.hud.HudModule[] all = {new FpsModule(), new CpsModule(),
            new CoordinatesModule(), new DirectionModule(), new SpeedModule(), new KeystrokesModule(),
            new ClockModule(), new MemoryModule(), new PingModule(), new ServerAddressModule(), new StopwatchModule()};
        FakeCanvas canvas = new FakeCanvas();
        io.github.clemensjl.hengst.mod.common.layout.MutableRect[] rects =
                new io.github.clemensjl.hengst.mod.common.layout.MutableRect[all.length];
        for (int i = 0; i < all.length; i++) {
            ModuleConfig c = ModuleConfig.defaults();
            c.setAnchor(all[i].defaultAnchor());
            c.setOffsetX(all[i].defaultOffsetX());
            c.setOffsetY(all[i].defaultOffsetY());
            c.setOption("decimals", 2);
            c.setOption("seconds", true);
            c.setOption("format", "12h");
            c.setOption("show_port", true);
            if (all[i] instanceof StopwatchModule) { // the longest text a stopwatch shows in practice: 123 hours
                ((StopwatchModule) all[i]).onKey(0, true, time.now);
                time.advanceMillis(123 * 3_600_000L + 45 * 60_000L + 6_700L);
            }
            all[i].tick(wallCtx, c);
            if (all[i] instanceof StopwatchModule) {
                assertEquals("123:45:06.7", ((StopwatchModule) all[i]).text());
            }
            if (all[i] instanceof SpeedModule) {
                platform.playerX += 5; // the largest counted step: 100 m/s
                all[i].tick(wallCtx, c);
                assertEquals("100.00 m/s", ((SpeedModule) all[i]).text());
            }
            if (all[i] instanceof PingModule) {
                assertEquals("9999 ms", ((PingModule) all[i]).text());
            }
            if (all[i] instanceof ServerAddressModule) {
                assertEquals(40, ((ServerAddressModule) all[i]).text().length());
            }
            if (all[i] instanceof DirectionModule) {
                assertEquals("NW 337°", ((DirectionModule) all[i]).text());
            }
            rects[i] = new io.github.clemensjl.hengst.mod.common.layout.MutableRect();
            io.github.clemensjl.hengst.mod.common.layout.LayoutMath.resolve(c, all[i].width(canvas, c),
                    all[i].height(canvas, c), canvas.width(), canvas.height(), rects[i]);
            assertTrue(rects[i].w > 0 && rects[i].h > 0, all[i].id() + " has a size");
        }
        for (int i = 0; i < all.length; i++) {
            for (int j = i + 1; j < all.length; j++) {
                io.github.clemensjl.hengst.mod.common.layout.MutableRect a = rects[i];
                io.github.clemensjl.hengst.mod.common.layout.MutableRect b = rects[j];
                boolean apart = a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y;
                assertTrue(apart, all[i].id() + " overlaps " + all[j].id());
            }
        }
    }
}
