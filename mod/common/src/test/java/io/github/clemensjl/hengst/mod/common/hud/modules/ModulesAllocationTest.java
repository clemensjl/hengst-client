package io.github.clemensjl.hengst.mod.common.hud.modules;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.config.Profile;
import io.github.clemensjl.hengst.mod.common.feature.FeatureRegistry;
import io.github.clemensjl.hengst.mod.common.hud.CpsCounter;
import io.github.clemensjl.hengst.mod.common.hud.FrameCounter;
import io.github.clemensjl.hengst.mod.common.hud.HudContext;
import io.github.clemensjl.hengst.mod.common.hud.HudModule;
import io.github.clemensjl.hengst.mod.common.hud.HudRenderer;
import io.github.clemensjl.hengst.mod.common.layout.Anchor;
import io.github.clemensjl.hengst.mod.common.platform.GameKey;
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
import io.github.clemensjl.hengst.mod.common.testutil.TestContexts;

/** The four real modules through the real renderer: nothing is allocated unless a displayed value changed. */
class ModulesAllocationTest {
    /** A new String with a short text: object header, fields and the backing array. Generous on purpose. */
    private static final long MAX_BYTES_PER_CHANGE = 128;

    private static final com.sun.management.ThreadMXBean MX =
            (com.sun.management.ThreadMXBean) ManagementFactory.getThreadMXBean();

    @TempDir Path dir;
    FakePlatform platform;
    FakeTime time;
    HudContext ctx;

    @BeforeEach
    void setUp() {
        platform = new FakePlatform(dir);
        platform.inWorld = true;
        platform.down.add(GameKey.FORWARD);
        platform.down.add(GameKey.ATTACK);
        time = new FakeTime();
        ctx = TestContexts.context(platform, time, new FrameCounter(time), new CpsCounter(time));
    }

    private static long allocated() {
        return MX.getThreadAllocatedBytes(Thread.currentThread().getId());
    }

    private HudRenderer rendererWithAllModules(FpsModule fps, CpsModule cps, KeystrokesModule keys, CoordinatesModule coords) {
        FeatureRegistry reg = new FeatureRegistry();
        reg.register(fps);
        reg.register(cps);
        reg.register(keys);
        reg.register(coords);
        Profile p = new Profile();
        HudModule[] all = {fps, cps, keys, coords};
        for (int i = 0; i < all.length; i++) {
            ModuleConfig c = p.module(all[i].id());
            c.setEnabled(true);
            c.setAnchor(Anchor.values()[i]);
            c.setScale(1.5f);
        }
        p.module("coordinates").setOption("decimals", 2);
        HudRenderer r = TestContexts.renderer(reg, GameVersion.V1_8_9);
        r.rebuild(p);
        return r;
    }

    @Test
    void real_modules_allocate_nothing_while_the_displayed_values_stay_the_same() {
        HudRenderer renderer = rendererWithAllModules(new FpsModule(), new CpsModule(), new KeystrokesModule(), new CoordinatesModule());
        platform.playerX = 10.25;
        platform.playerY = 64.0;
        platform.playerZ = -3.5;
        ctx.cps().onPress(0);
        ctx.cps().onPress(1);
        CountingCanvas canvas = new CountingCanvas();
        for (int i = 0; i < 3000; i++) {
            ctx.frames().onFrame();
            renderer.tick(ctx);
            renderer.render(canvas, ctx);
        }
        int frames = 20000;
        long before = allocated();
        for (int i = 0; i < frames; i++) {
            ctx.frames().onFrame();
            renderer.render(canvas, ctx);
        }
        long frameBytes = allocated() - before;
        before = allocated();
        for (int i = 0; i < frames; i++) {
            renderer.tick(ctx);
        }
        long tickBytes = allocated() - before;
        assertEquals(0, frameBytes, "allocated " + frameBytes + " bytes in " + frames + " frames");
        assertEquals(0, tickBytes, "allocated " + tickBytes + " bytes in " + frames + " ticks");
        assertTrue(canvas.fills > 0 && canvas.texts > 0);
    }

    @Test
    void real_modules_allocate_only_for_the_text_of_values_that_changed() {
        FpsModule fps = new FpsModule();
        CpsModule cps = new CpsModule();
        KeystrokesModule keys = new KeystrokesModule();
        CoordinatesModule coords = new CoordinatesModule();
        HudRenderer renderer = rendererWithAllModules(fps, cps, keys, coords);
        CountingCanvas canvas = new CountingCanvas();
        // 5 ms frames with a varying length, so the fps value differs from window to window; a tick every 10th frame
        // moves the player and every 7th tick clicks, so the coordinates and the cps counts keep changing.
        for (int round = 0; round < 2; round++) {
            long changes = 0;
            long before = allocated();
            String lastFps = fps.text();
            String lastCps = cps.text();
            String lastCoords = coords.text();
            for (int i = 0; i < 40000; i++) {
                time.advanceMillis(4 + (i / 200) % 3);
                ctx.frames().onFrame();
                if (i % 10 == 0) {
                    platform.playerX += 0.37;
                    platform.playerZ -= 0.21;
                    if ((i / 10) % 7 == 0) {
                        ctx.cps().onPress((i / 70) & 1);
                    }
                    renderer.tick(ctx);
                    if (fps.text() != lastFps) {
                        lastFps = fps.text();
                        changes++;
                    }
                    if (cps.text() != lastCps) {
                        lastCps = cps.text();
                        changes++;
                    }
                    if (coords.text() != lastCoords) {
                        lastCoords = coords.text();
                        changes++;
                    }
                }
                renderer.render(canvas, ctx);
            }
            long bytes = allocated() - before;
            if (round == 1) {
                assertTrue(changes > 500, "too few value changes to prove anything: " + changes);
                assertTrue(bytes <= changes * MAX_BYTES_PER_CHANGE,
                        "allocated " + bytes + " bytes for " + changes + " text changes");
            }
        }
    }

    private interface Change {
        void apply(int i);
    }

    /** Bytes per text change of one module, measured over many changes; printed for the report. */
    private long bytesPerChange(String name, HudModule module, ModuleConfig cfg, Change change) {
        for (int i = 0; i < 5000; i++) {
            change.apply(i);
            module.tick(ctx, cfg);
        }
        int n = 20000;
        long before = allocated();
        for (int i = 0; i < n; i++) {
            change.apply(5000 + i);
            module.tick(ctx, cfg);
        }
        long perChange = (allocated() - before) / n;
        System.out.println("ALLOC " + name + " bytes per change: " + perChange);
        return perChange;
    }

    @Test
    void each_value_change_costs_one_short_string_and_nothing_else() {
        final CpsCounter cpsCounter = ctx.cps();
        ModuleConfig cfg = ModuleConfig.defaults();
        FpsModule fps = new FpsModule();
        long fpsBytes = bytesPerChange("fps", fps, cfg, new Change() {
            @Override
            public void apply(int i) {
                // one full window per call with a different frame count each time
                for (int k = 0; k < 40 + i % 150; k++) {
                    ctx.frames().onFrame();
                }
                time.advanceMillis(1000);
                ctx.frames().onFrame();
            }
        });
        CpsModule cps = new CpsModule();
        long cpsBytes = bytesPerChange("cps", cps, cfg, new Change() {
            @Override
            public void apply(int i) {
                // a click on even calls, a silent second on odd ones: the text flips every call
                if ((i & 1) == 0) {
                    cpsCounter.onPress(0);
                } else {
                    time.advanceMillis(1000);
                }
            }
        });
        CoordinatesModule coords = new CoordinatesModule();
        ModuleConfig coordCfg = ModuleConfig.defaults();
        coordCfg.setOption("decimals", 2);
        long coordBytes = bytesPerChange("coordinates", coords, coordCfg, new Change() {
            @Override
            public void apply(int i) {
                platform.playerX = i * 0.25;
                platform.playerY = 64.0 + (i % 7);
                platform.playerZ = -i * 0.5;
            }
        });
        assertTrue(fpsBytes > 0 && fpsBytes <= MAX_BYTES_PER_CHANGE, "fps " + fpsBytes);
        assertTrue(cpsBytes > 0 && cpsBytes <= MAX_BYTES_PER_CHANGE, "cps " + cpsBytes);
        assertTrue(coordBytes > 0 && coordBytes <= MAX_BYTES_PER_CHANGE, "coordinates " + coordBytes);
    }

    @Tag("interpreted")
    @Test
    void keystrokes_with_a_too_wide_label_allocates_nothing_after_its_first_frame() {
        platform.labels.put(GameKey.FORWARD, "Button 4");
        KeystrokesModule keys = new KeystrokesModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        CountingCanvas canvas = new CountingCanvas();
        keys.tick(ctx, cfg);
        keys.render(canvas, cfg, ctx);
        long before = allocated();
        for (int i = 0; i < 1000; i++) {
            keys.render(canvas, cfg, ctx);
        }
        long used = allocated() - before;
        assertEquals(0, used, "allocated " + used + " bytes");
    }
}
