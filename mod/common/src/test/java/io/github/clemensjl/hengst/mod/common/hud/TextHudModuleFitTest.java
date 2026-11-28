package io.github.clemensjl.hengst.mod.common.hud;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.config.Profile;
import io.github.clemensjl.hengst.mod.common.feature.FeatureRegistry;
import io.github.clemensjl.hengst.mod.common.hud.modules.ServerAddressModule;
import io.github.clemensjl.hengst.mod.common.layout.Anchor;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.testutil.FakePlatform;
import io.github.clemensjl.hengst.mod.common.testutil.FakeTime;
import io.github.clemensjl.hengst.mod.common.testutil.WideGlyphCanvas;
import java.nio.file.Path;
import java.util.EnumSet;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import io.github.clemensjl.hengst.mod.common.testutil.TestContexts;

/** The generic fit of {@link TextHudModule}: a text wider than the screen offers is drawn cut in the middle. */
class TextHudModuleFitTest {
    private static final int[] WIDTHS = {320, 427, 480};
    private static final float[] SCALES = {1.0f, 1.5f, 3.0f};
    private static final Anchor[] ANCHORS = {Anchor.TOP_LEFT, Anchor.TOP_RIGHT, Anchor.BOTTOM_RIGHT, Anchor.CENTER};

    /** A module whose text is set from the outside, to prove the fit is not specific to one module. */
    private static final class Plain extends TextHudModule {
        Plain() {
            super("plain", 97, EnumSet.of(GameVersion.V1_8_9));
        }

        void show(String s) {
            setText(s);
        }
    }

    @TempDir Path dir;
    FakePlatform platform;
    HudContext ctx;

    @BeforeEach
    void setUp() {
        platform = new FakePlatform(dir);
        platform.inWorld = true;
        platform.worldIdentity = new Object();
        FakeTime time = new FakeTime();
        ctx = TestContexts.context(platform, time, new FrameCounter(time), new CpsCounter(time));
    }

    private static String repeat(char c, int n) {
        StringBuilder b = new StringBuilder();
        for (int i = 0; i < n; i++) {
            b.append(c);
        }
        return b.toString();
    }

    private WideGlyphCanvas draw(HudModule module, String id, int width, float scale, Anchor anchor) {
        return draw(module, id, width, scale, anchor,
                anchor == Anchor.TOP_RIGHT || anchor == Anchor.BOTTOM_RIGHT ? -2 : 2, anchor == Anchor.BOTTOM_RIGHT ? -2 : 2);
    }

    private WideGlyphCanvas draw(HudModule module, String id, int width, float scale, Anchor anchor, int offsetX,
            int offsetY) {
        FeatureRegistry reg = new FeatureRegistry();
        reg.register(module);
        Profile p = new Profile();
        ModuleConfig c = p.module(id);
        c.setEnabled(true);
        c.setAnchor(anchor);
        c.setOffsetX(offsetX);
        c.setOffsetY(offsetY);
        c.setScale(scale);
        c.setBackground(true);
        HudRenderer r = TestContexts.renderer(reg, GameVersion.V1_8_9);
        r.rebuild(p);
        WideGlyphCanvas canvas = new WideGlyphCanvas(width, 240);
        r.tick(ctx);
        r.render(canvas, ctx);
        return canvas;
    }

    @Test
    void a_wide_address_never_leaves_the_screen_at_any_gui_width_and_scale() {
        platform.serverAddress = repeat('W', 300) + ":25565";
        for (int width : WIDTHS) {
            for (float scale : SCALES) {
                for (Anchor anchor : ANCHORS) {
                    WideGlyphCanvas canvas = draw(new ServerAddressModule(), "server_address", width, scale, anchor);
                    String where = "width " + width + " scale " + scale + " " + anchor;
                    assertEquals(1, canvas.texts, where);
                    assertTrue(canvas.minLeft >= 0, where + " left " + canvas.minLeft);
                    assertTrue(canvas.maxRight <= width, where + " right " + canvas.maxRight);
                    assertTrue(canvas.maxFillRight <= width + 0.5, where + " background " + canvas.maxFillRight);
                }
            }
        }
    }

    @Test
    void a_too_wide_text_dragged_far_to_the_right_stays_inside_the_screen() {
        for (float scale : new float[] {1f, 3f}) {
            Plain m = new Plain();
            m.show(repeat('M', 120));
            WideGlyphCanvas canvas = draw(m, "plain", 320, scale, Anchor.TOP_LEFT, 200, 100);
            String where = "scale " + scale;
            assertEquals(1, canvas.texts, where);
            assertTrue(canvas.minLeft >= 0, where + " left " + canvas.minLeft);
            assertTrue(canvas.maxRight <= 320, where + " right " + canvas.maxRight);
            assertTrue(canvas.maxFillRight <= 320.5, where + " background " + canvas.maxFillRight);
        }
    }

    @Test
    void any_text_module_is_covered() {
        for (int width : WIDTHS) {
            for (float scale : SCALES) {
                Plain m = new Plain();
                m.show(repeat('M', 120));
                WideGlyphCanvas canvas = draw(m, "plain", width, scale, Anchor.TOP_RIGHT);
                assertTrue(canvas.minLeft >= 0 && canvas.maxRight <= width, "width " + width + " scale " + scale);
                assertTrue(canvas.lastText.contains(".."), canvas.lastText);
            }
        }
    }

    @Test
    void the_cut_keeps_start_and_end_and_a_text_that_fits_is_drawn_as_is() {
        Plain m = new Plain();
        m.show("short text");
        assertEquals("short text", draw(m, "plain", 320, 1f, Anchor.TOP_LEFT).lastText);
        assertSame(m.text(), draw(m, "plain", 320, 1f, Anchor.TOP_LEFT).lastText);
        String wide = "START" + repeat('M', 80) + "END";
        m.show(wide);
        String drawn = draw(m, "plain", 320, 3f, Anchor.TOP_LEFT).lastText;
        assertTrue(drawn.startsWith("START") && drawn.endsWith("END") && drawn.contains(".."), drawn);
        assertEquals(wide, m.text(), "the module's own text is not changed");
    }

    @Test
    void the_brief_sample_is_drawn_unchanged_at_320_wide_and_scale_one() {
        platform.serverAddress = "aaaaaaaaaa.bbbbbbbbbb.cccccccccc.dddddddddd.eeeee";
        WideGlyphCanvas canvas = draw(new ServerAddressModule(), "server_address", 320, 1f, Anchor.TOP_RIGHT);
        assertEquals("aaaaaaaaaa.bbbbbbbb..cc.dddddddddd.eeeee", canvas.lastText);
    }

    @Test
    void the_reported_width_is_the_fitted_width() {
        Plain m = new Plain();
        m.show(repeat('W', 100));
        WideGlyphCanvas canvas = new WideGlyphCanvas(320, 240);
        ModuleConfig cfg = ModuleConfig.defaults();
        cfg.setScale(1.5f);
        int w = m.width(canvas, cfg);
        assertTrue(w * 1.5 <= 320, "width " + w);
        assertTrue(w > 100, "the box is filled up to the screen: " + w);
        m.show("ab");
        assertEquals(canvas.textWidth("ab") + 4, m.width(canvas, cfg));
        WideGlyphCanvas wide = new WideGlyphCanvas(480, 240);
        m.show(repeat('W', 40));
        assertNotEquals(m.width(canvas, cfg), m.width(wide, cfg), "the fit follows the canvas width");
    }

    @Test
    void a_language_change_drops_the_fitted_text() {
        Plain m = new Plain();
        m.show(repeat('W', 100));
        WideGlyphCanvas canvas = new WideGlyphCanvas(320, 240);
        ModuleConfig cfg = ModuleConfig.defaults();
        int before = m.width(canvas, cfg);
        m.onLanguageChanged();
        assertEquals(before, m.width(canvas, cfg));
    }
}
