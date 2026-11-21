package io.github.clemensjl.hengst.mod.common.hud.modules;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.I18n;
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

class DirectionModuleTest {
    @TempDir Path dir;
    FakePlatform platform;
    HudContext ctx;

    @BeforeEach
    void setUp() {
        platform = new FakePlatform(dir);
        platform.inWorld = true;
        FakeTime time = new FakeTime();
        ctx = new HudContext(platform, time, new FrameCounter(time), new CpsCounter(time));
    }

    @Test
    void direction_formats() {
        DirectionModule m = new DirectionModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        Object[][] cases = {{180f, "N 0°"}, {0f, "S 180°"}, {-90f, "E 90°"}, {90f, "W 270°"},
            {-135f, "NE 45°"}, {540f, "N 0°"}, {-720.4f, "S 180°"}, {202.4f, "N 22°"},
            {202.6f, "NE 23°"}, {179.6f, "N 0°"}};
        for (Object[] c : cases) {
            platform.playerYaw = (Float) c[0];
            m.tick(ctx, cfg);
            assertEquals(c[1], m.text(), "yaw " + c[0]);
        }
        cfg.setOption("style", "cardinal");
        platform.playerYaw = -135f;
        m.tick(ctx, cfg);
        assertEquals("NE", m.text());
        cfg.setOption("style", "degrees");
        m.tick(ctx, cfg);
        assertEquals("45°", m.text());
        platform.playerYaw = Float.NaN;
        m.tick(ctx, cfg);
        assertEquals("?", m.text());
    }

    @Test
    void the_eight_sectors_change_exactly_at_22_and_23_degrees_and_wrap_at_337_and_338() {
        DirectionModule m = new DirectionModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        cfg.setOption("style", "cardinal");
        String[] letters = {"N", "NE", "E", "SE", "S", "SW", "W", "NW"};
        for (int d = 0; d < 360; d++) {
            platform.playerYaw = d - 180f; // course d
            m.tick(ctx, cfg);
            assertEquals(letters[((2 * d + 45) / 90) % 8], m.text(), "course " + d);
        }
        platform.playerYaw = -180f + 22f;
        m.tick(ctx, cfg);
        assertEquals("N", m.text());
        platform.playerYaw = -180f + 23f;
        m.tick(ctx, cfg);
        assertEquals("NE", m.text());
        platform.playerYaw = -180f + 337f;
        m.tick(ctx, cfg);
        assertEquals("NW", m.text());
        platform.playerYaw = -180f + 338f;
        m.tick(ctx, cfg);
        assertEquals("N", m.text());
    }

    @Test
    void negative_and_large_yaws_are_normalized() {
        DirectionModule m = new DirectionModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        float[] yaws = {-0.0f, -1e-9f, -360f, -3600.5f, 7200f, 100000f, 3.0e38f, -3.0e38f};
        for (float yaw : yaws) {
            platform.playerYaw = yaw;
            m.tick(ctx, cfg);
            String t = m.text();
            int deg = Integer.parseInt(t.substring(t.indexOf(' ') + 1, t.length() - 1));
            assertTrue(deg >= 0 && deg < 360, "yaw " + yaw + " shows " + t);
        }
        platform.playerYaw = Float.POSITIVE_INFINITY;
        m.tick(ctx, cfg);
        assertEquals("?", m.text());
    }

    @Test
    void direction_letters_follow_the_language() {
        DirectionModule m = new DirectionModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        platform.playerYaw = -90f;
        m.tick(ctx, cfg);
        assertEquals("E 90°", m.text());
        ctx.setI18n(I18n.load("de_de"));
        m.tick(ctx, cfg);
        assertEquals("O 90°", m.text());
        platform.playerYaw = -135f;
        m.tick(ctx, cfg);
        assertEquals("NO 45°", m.text());
    }

    @Test
    void empty_outside_a_world_and_the_text_keeps_its_reference_while_unchanged() {
        DirectionModule m = new DirectionModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        platform.playerYaw = 10f;
        m.tick(ctx, cfg);
        String shown = m.text();
        platform.playerYaw = 10.1f;
        m.tick(ctx, cfg);
        assertSame(shown, m.text());
        platform.inWorld = false;
        m.tick(ctx, cfg);
        assertEquals("", m.text());
        platform.inWorld = true;
        m.tick(ctx, cfg);
        assertEquals(shown, m.text());
    }
}
