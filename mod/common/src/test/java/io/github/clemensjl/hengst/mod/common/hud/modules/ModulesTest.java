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
    void coordinates_with_two_decimals_truncate() {
        platform.inWorld = true;
        platform.playerX = 1.239;
        platform.playerY = 64.0;
        platform.playerZ = -0.5;
        cfg.setOption("decimals", 2);
        CoordinatesModule coords = new CoordinatesModule();
        coords.tick(ctx, cfg);
        assertEquals("X 1.23  Y 64.00  Z -0.50", coords.text());
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
        assertEquals("X 0.00  Y 0.01  Z 0.50", coords(0.004, 0.019, 0.5, 2));
        assertEquals("X 0  Y 0  Z 0", coords(0.999, 0.1, 0.0001, 0));
        // floor, like the block position: a hair below zero belongs to the next lower step
        assertEquals("X -1  Y 0  Z 0", coords(-0.001, 0.0, 0.0, 0));
        assertEquals("X -0.01  Y 0.00  Z 0.00", coords(-0.001, 0.0, 0.0, 2));
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
        assertEquals(58, k.width(c, cfg));
        assertEquals(58, k.height(c, cfg));
        cfg.setOption("show_space", Boolean.TRUE);
        cfg.setOption("show_mouse", Boolean.FALSE);
        assertEquals(50, k.height(c, cfg));
        cfg.setOption("show_space", Boolean.FALSE);
        assertEquals(58, k.width(c, cfg));
        assertEquals(38, k.height(c, cfg));
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
}
