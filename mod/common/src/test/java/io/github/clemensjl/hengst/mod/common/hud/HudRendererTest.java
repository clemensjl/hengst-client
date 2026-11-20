package io.github.clemensjl.hengst.mod.common.hud;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotSame;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.config.Profile;
import io.github.clemensjl.hengst.mod.common.feature.FeatureRegistry;
import io.github.clemensjl.hengst.mod.common.layout.Anchor;
import io.github.clemensjl.hengst.mod.common.platform.Canvas;
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
            super("counter", 6, EnumSet.allOf(GameVersion.class));
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

    /** Throws in the chosen method. */
    static final class Thrower extends HudModule {
        boolean inTick;
        boolean inRender;
        boolean inWidth;
        int calls;

        Thrower(String id, int nr) {
            super(id, nr, EnumSet.allOf(GameVersion.class));
        }

        @Override
        public void tick(HudContext ctx, ModuleConfig cfg) {
            calls++;
            if (inTick) {
                throw new IllegalStateException("tick boom");
            }
        }

        @Override
        public int width(Canvas c, ModuleConfig cfg) {
            if (inWidth) {
                throw new IllegalStateException("width boom");
            }
            return 10;
        }

        @Override
        public int height(Canvas c, ModuleConfig cfg) {
            return 10;
        }

        @Override
        public void render(Canvas c, ModuleConfig cfg, HudContext ctx) {
            if (inRender) {
                throw new IllegalStateException("render boom");
            }
            c.fill(0, 0, 10, 10, 0xFFFFFFFF);
        }
    }

    private HudRenderer sandwich(Thrower bad, Profile[] out) {
        FeatureRegistry reg = new FeatureRegistry();
        reg.register(new FixedSizeModule("a", 1, 10, 10));
        reg.register(bad);
        reg.register(new FixedSizeModule("c", 3, 10, 10));
        Profile p = new Profile();
        p.module("a").setEnabled(true);
        p.module(bad.id()).setEnabled(true);
        p.module("c").setEnabled(true);
        out[0] = p;
        HudRenderer r = new HudRenderer(reg, GameVersion.V1_8_9);
        r.rebuild(p);
        return r;
    }

    private static int count(FakeCanvas c, String prefix) {
        int n = 0;
        for (String s : c.calls) {
            if (s.startsWith(prefix)) {
                n++;
            }
        }
        return n;
    }

    @Test
    void module_throwing_in_render_does_not_break_the_others() {
        Thrower bad = new Thrower("bad", 2);
        bad.inRender = true;
        HudRenderer r = sandwich(bad, new Profile[1]);
        FakeCanvas c = new FakeCanvas();
        for (int i = 0; i < 50; i++) {
            r.tick(ctx);
            r.render(c, ctx);
        }
        assertEquals(count(c, "push"), count(c, "pop"));
        assertEquals(1 + 50 * 2, count(c, "push"));
        assertEquals(100, count(c, "fill"));
        assertEquals(1, platform.warnings.size());
        assertTrue(platform.warnings.get(0).contains("bad"));
        assertTrue(platform.warnings.get(0).contains("IllegalStateException"));
        assertTrue(platform.warnings.get(0).contains("render boom"));
    }

    @Test
    void module_throwing_in_width_is_skipped_without_push() {
        Thrower bad = new Thrower("bad", 2);
        bad.inWidth = true;
        HudRenderer r = sandwich(bad, new Profile[1]);
        FakeCanvas c = new FakeCanvas();
        for (int i = 0; i < 20; i++) {
            r.render(c, ctx);
        }
        assertEquals(40, count(c, "push"));
        assertEquals(40, count(c, "pop"));
        assertEquals(1, platform.warnings.size());
    }

    @Test
    void module_throwing_in_tick_is_skipped_in_tick_and_render() {
        Thrower bad = new Thrower("bad", 2);
        bad.inTick = true;
        HudRenderer r = sandwich(bad, new Profile[1]);
        FakeCanvas c = new FakeCanvas();
        for (int i = 0; i < 20; i++) {
            r.tick(ctx);
            r.render(c, ctx);
        }
        assertEquals(1, bad.calls);
        assertEquals(40, count(c, "push"));
        assertEquals(count(c, "push"), count(c, "pop"));
        assertEquals(1, platform.warnings.size());
        assertTrue(platform.warnings.get(0).contains("tick boom"));
    }

    @Test
    void failed_module_is_tried_again_after_rebuild() {
        Thrower bad = new Thrower("bad", 2);
        bad.inRender = true;
        Profile[] p = new Profile[1];
        HudRenderer r = sandwich(bad, p);
        FakeCanvas c = new FakeCanvas();
        r.render(c, ctx);
        r.render(c, ctx);
        assertEquals(1, platform.warnings.size());
        bad.inRender = false;
        r.rebuild(p[0]);
        c.calls.clear();
        r.render(c, ctx);
        assertEquals(3, count(c, "push"));
        assertEquals(1, platform.warnings.size());
        bad.inRender = true;
        r.rebuild(p[0]);
        r.render(c, ctx);
        assertEquals(2, platform.warnings.size());
    }

    @Test
    void empty_text_module_has_no_size_and_draws_nothing() {
        Fixed m = new Fixed("a", 1);
        ModuleConfig cfg = ModuleConfig.defaults();
        FakeCanvas c = new FakeCanvas();
        assertEquals(0, m.width(c, cfg));
        assertEquals(0, m.height(c, cfg));
        m.render(c, cfg, ctx);
        assertTrue(c.calls.isEmpty());
    }

    @Test
    void renderer_skips_push_for_zero_size_module() {
        FeatureRegistry reg = new FeatureRegistry();
        reg.register(new Fixed("a", 1));
        Profile p = new Profile();
        p.module("a").setEnabled(true);
        HudRenderer r = new HudRenderer(reg, GameVersion.V1_8_9);
        r.rebuild(p);
        FakeCanvas c = new FakeCanvas();
        r.render(c, ctx);
        assertTrue(c.calls.isEmpty());
        r.tick(ctx);
        r.render(c, ctx);
        assertEquals(1, count(c, "push"));
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
    void renders_while_a_vanilla_screen_is_open() {
        HudRenderer r = rendererWith("a", new ModuleConfig[1]);
        platform.screenOpen = true;
        FakeCanvas c = new FakeCanvas();
        r.render(c, ctx);
        assertTrue(!c.calls.isEmpty());
    }

    @Test
    void renders_nothing_while_suppressed() {
        HudRenderer r = rendererWith("a", new ModuleConfig[1]);
        r.setSuppressed(true);
        FakeCanvas c = new FakeCanvas();
        r.render(c, ctx);
        assertTrue(c.calls.isEmpty());
        r.setSuppressed(false);
        r.render(c, ctx);
        assertTrue(!c.calls.isEmpty());
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
        reg.register(new Counter());
        p.module("counter").setEnabled(true);
        HudRenderer renderer = new HudRenderer(reg, GameVersion.V1_8_9);
        renderer.rebuild(p);
        platform.screenOpen = true;
        CountingCanvas counting = new CountingCanvas();
        for (int i = 0; i < 2000; i++) {
            renderer.tick(ctx);
            renderer.render(counting, ctx);
        }
        com.sun.management.ThreadMXBean mx = (com.sun.management.ThreadMXBean) ManagementFactory.getThreadMXBean();
        int frames = 20000;
        long before = mx.getCurrentThreadAllocatedBytes();
        for (int i = 0; i < frames; i++) {
            renderer.render(counting, ctx);
        }
        long allocated = mx.getCurrentThreadAllocatedBytes() - before;
        long beforeTicks = mx.getCurrentThreadAllocatedBytes();
        for (int i = 0; i < frames; i++) {
            renderer.tick(ctx);
        }
        long allocatedTicks = mx.getCurrentThreadAllocatedBytes() - beforeTicks;
        assertEquals(0, allocated, "allocated " + allocated + " bytes in " + frames + " frames");
        assertEquals(0, allocatedTicks, "allocated " + allocatedTicks + " bytes in " + frames + " ticks");
        assertTrue(counting.pushes > 0 && counting.pushes == counting.pops);
    }
}
