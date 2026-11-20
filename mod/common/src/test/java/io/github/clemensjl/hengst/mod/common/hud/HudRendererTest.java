package io.github.clemensjl.hengst.mod.common.hud;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotSame;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.config.Profile;
import io.github.clemensjl.hengst.mod.common.feature.FeatureRegistry;
import io.github.clemensjl.hengst.mod.common.layout.Anchor;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.testutil.CountingCanvas;
import io.github.clemensjl.hengst.mod.common.testutil.FakeCanvas;
import io.github.clemensjl.hengst.mod.common.testutil.FakePlatform;
import io.github.clemensjl.hengst.mod.common.testutil.FakeTime;
import io.github.clemensjl.hengst.mod.common.testutil.FixedSizeModule;
import java.lang.management.ManagementFactory;
import java.nio.file.Path;
import java.util.EnumSet;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class HudRendererTest {
    @TempDir Path dir;
    FakePlatform platform;
    FakeTime time;
    HudContext ctx;

    @BeforeEach
    void setUp() {
        platform = new FakePlatform(dir);
        platform.inWorld = true;
        time = new FakeTime();
        ctx = new HudContext(platform, time, new FrameCounter(time), new CpsCounter(time));
    }

    static final class Fixed extends TextHudModule {
        Fixed(String id, int nr) {
            super(id, nr, EnumSet.allOf(GameVersion.class));
        }

        @Override
        public void tick(HudContext ctx, ModuleConfig cfg) {
            setText("Hello");
        }
    }

    /** Rebuilds its text with a concatenation only when the underlying int changed. */
    static final class Counter extends TextHudModule {
        int value;
        int last = -1;
        int rebuilds;

        Counter() {
            super("counter", 1, EnumSet.allOf(GameVersion.class));
        }

        @Override
        public void tick(HudContext ctx, ModuleConfig cfg) {
            if (value != last) {
                last = value;
                rebuilds++;
                setText("N " + value);
            }
        }
    }

    private HudRenderer rendererWith(String id, ModuleConfig[] out) {
        FeatureRegistry reg = new FeatureRegistry();
        reg.register(new Fixed(id, 1));
        Profile p = new Profile();
        ModuleConfig a = p.module(id);
        a.setEnabled(true);
        out[0] = a;
        HudRenderer r = new HudRenderer(reg, GameVersion.V1_8_9);
        r.rebuild(p);
        r.tick(ctx);
        return r;
    }

    @Test
    void rebuild_takes_only_enabled_and_supported_in_number_order() {
        FeatureRegistry reg = new FeatureRegistry();
        reg.register(new Fixed("b", 6));
        reg.register(new Fixed("a", 1));
        reg.register(new FixedSizeModule("old", 3, 10, 10, EnumSet.of(GameVersion.V26_3)));
        Profile p = new Profile();
        p.module("a").setEnabled(true);
        p.module("b").setEnabled(true);
        p.module("old").setEnabled(true);
        HudRenderer r = new HudRenderer(reg, GameVersion.V1_8_9);
        r.rebuild(p);
        assertEquals(2, r.activeCount());
        assertEquals("a", r.activeModules()[0].id());
        assertEquals("b", r.activeModules()[1].id());
        assertSame(p.module("a"), r.activeConfigs()[0]);
        p.module("a").setEnabled(false);
        r.rebuild(p);
        assertEquals(1, r.activeCount());
    }

    @Test
    void render_places_scaled_module_at_resolved_rect() {
        FeatureRegistry reg = new FeatureRegistry();
        reg.register(new Fixed("a", 1));
        Profile p = new Profile();
        ModuleConfig a = p.module("a");
        a.setEnabled(true);
        a.setAnchor(Anchor.TOP_RIGHT);
        a.setOffsetX(-2);
        a.setOffsetY(2);
        a.setScale(2f);
        HudRenderer r = new HudRenderer(reg, GameVersion.V1_8_9);
        r.rebuild(p);
        FakeCanvas c = new FakeCanvas();
        r.tick(ctx);
        r.render(c, ctx);
        assertEquals("push 250.0 2.0 2.0", c.calls.get(0));
        assertEquals("fill 0 0 34 13 6f000000", c.calls.get(1));
        assertEquals("text Hello 2 2 ffffffff true", c.calls.get(2));
        assertEquals("pop", c.calls.get(c.calls.size() - 1));
    }

    @Test
    void render_skips_background_when_off() {
        ModuleConfig[] cfg = new ModuleConfig[1];
        HudRenderer r = rendererWith("a", cfg);
        cfg[0].setBackground(false);
        FakeCanvas c = new FakeCanvas();
        r.render(c, ctx);
        assertEquals(3, c.calls.size());
        assertEquals("text Hello 2 2 ffffffff true", c.calls.get(1));
    }

    @Test
    void render_draws_in_module_order() {
        FeatureRegistry reg = new FeatureRegistry();
        reg.register(new FixedSizeModule("second", 2, 20, 10));
        reg.register(new FixedSizeModule("first", 1, 10, 10));
        Profile p = new Profile();
        p.module("first").setEnabled(true);
        p.module("second").setEnabled(true);
        HudRenderer r = new HudRenderer(reg, GameVersion.V1_8_9);
        r.rebuild(p);
        FakeCanvas c = new FakeCanvas();
        r.render(c, ctx);
        assertEquals("fill 0 0 10 10 ffffffff", c.calls.get(1));
        assertEquals("fill 0 0 20 10 ffffffff", c.calls.get(4));
    }

    @Test
    void renders_nothing_when_hud_hidden() {
        HudRenderer r = rendererWith("a", new ModuleConfig[1]);
        platform.hudHidden = true;
        FakeCanvas c = new FakeCanvas();
        r.render(c, ctx);
        assertTrue(c.calls.isEmpty());
        platform.hudHidden = false;
        r.render(c, ctx);
        assertTrue(!c.calls.isEmpty());
    }

    @Test
    void renders_nothing_outside_a_world() {
        HudRenderer r = rendererWith("a", new ModuleConfig[1]);
        platform.inWorld = false;
        FakeCanvas c = new FakeCanvas();
        r.render(c, ctx);
        assertTrue(c.calls.isEmpty());
    }

    @Test
    void renders_nothing_while_a_screen_is_open() {
        HudRenderer r = rendererWith("a", new ModuleConfig[1]);
        platform.screenOpen = true;
        FakeCanvas c = new FakeCanvas();
        r.render(c, ctx);
        assertTrue(c.calls.isEmpty());
    }

    @Test
    void text_reference_kept_when_equal() {
        Fixed m = new Fixed("a", 1);
        ModuleConfig cfg = ModuleConfig.defaults();
        m.tick(ctx, cfg);
        String first = m.text();
        m.tick(ctx, cfg);
        assertSame(first, m.text());
        assertEquals("Hello", m.text());
        assertEquals(34, m.width(new FakeCanvas(), cfg));
        assertEquals(13, m.height(new FakeCanvas(), cfg));
    }

    @Test
    void text_is_rebuilt_only_when_the_value_changes_and_width_follows() {
        Counter m = new Counter();
        ModuleConfig cfg = ModuleConfig.defaults();
        FakeCanvas c = new FakeCanvas();
        m.tick(ctx, cfg);
        m.tick(ctx, cfg);
        m.tick(ctx, cfg);
        assertEquals(1, m.rebuilds);
        assertEquals("N 0", m.text());
        assertEquals(3 * 6 + 4, m.width(c, cfg));
        String before = m.text();
        m.value = 10;
        m.tick(ctx, cfg);
        assertEquals(2, m.rebuilds);
        assertNotSame(before, m.text());
        assertEquals(4 * 6 + 4, m.width(c, cfg));
    }

    @Test
    void text_width_is_measured_once_per_text() {
        final int[] measured = {0};
        FakeCanvas c = new FakeCanvas() {
            @Override
            public int textWidth(String text) {
                measured[0]++;
                return super.textWidth(text);
            }
        };
        Fixed m = new Fixed("a", 1);
        ModuleConfig cfg = ModuleConfig.defaults();
        m.tick(ctx, cfg);
        m.width(c, cfg);
        m.width(c, cfg);
        m.render(c, cfg, ctx);
        assertEquals(1, measured[0]);
    }

    @Test
    void hud_render_allocates_nothing_in_steady_state() {
        FeatureRegistry reg = new FeatureRegistry();
        Profile p = new Profile();
        for (int i = 1; i <= 4; i++) {
            reg.register(new Fixed("m" + i, i));
            ModuleConfig cfg = p.module("m" + i);
            cfg.setEnabled(true);
            cfg.setAnchor(Anchor.values()[i]);
            cfg.setScale(1.5f);
        }
        reg.register(new FixedSizeModule("box", 5, 30, 12));
        p.module("box").setEnabled(true);
        HudRenderer renderer = new HudRenderer(reg, GameVersion.V1_8_9);
        renderer.rebuild(p);
        CountingCanvas counting = new CountingCanvas();
        for (int i = 0; i < 2000; i++) {
            renderer.tick(ctx);
            renderer.render(counting, ctx);
        }
        com.sun.management.ThreadMXBean mx = (com.sun.management.ThreadMXBean) ManagementFactory.getThreadMXBean();
        long id = Thread.currentThread().getId();
        int frames = 20000;
        long before = mx.getThreadAllocatedBytes(id);
        for (int i = 0; i < frames; i++) {
            renderer.render(counting, ctx);
        }
        long allocated = mx.getThreadAllocatedBytes(id) - before;
        long beforeTicks = mx.getThreadAllocatedBytes(id);
        for (int i = 0; i < frames; i++) {
            renderer.tick(ctx);
        }
        long allocatedTicks = mx.getThreadAllocatedBytes(id) - beforeTicks;
        assertEquals(0, allocated, "allocated " + allocated + " bytes in " + frames + " frames");
        assertEquals(0, allocatedTicks, "allocated " + allocatedTicks + " bytes in " + frames + " ticks");
        assertTrue(counting.pushes > 0 && counting.pushes == counting.pops);
    }
}
