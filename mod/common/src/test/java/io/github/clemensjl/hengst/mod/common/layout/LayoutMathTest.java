package io.github.clemensjl.hengst.mod.common.layout;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import java.util.EnumSet;
import org.junit.jupiter.api.Test;

class LayoutMathTest {
    private static ModuleConfig cfg(Anchor a, int ox, int oy) {
        ModuleConfig c = ModuleConfig.defaults();
        c.setAnchor(a);
        c.setOffsetX(ox);
        c.setOffsetY(oy);
        return c;
    }

    @Test
    void top_right_is_flush_right() {
        MutableRect r = new MutableRect();
        LayoutMath.resolve(cfg(Anchor.TOP_RIGHT, -2, 2), 40, 10, 320, 240, r);
        assertEquals(278, r.x);
        assertEquals(2, r.y);
        assertEquals(40, r.w);
        assertEquals(10, r.h);
    }

    @Test
    void center_is_centered() {
        MutableRect r = new MutableRect();
        LayoutMath.resolve(cfg(Anchor.CENTER, 0, 0), 40, 10, 320, 240, r);
        assertEquals(140, r.x);
        assertEquals(115, r.y);
    }

    @Test
    void resolve_clamps_after_screen_shrinks() {
        MutableRect r = new MutableRect();
        LayoutMath.resolve(cfg(Anchor.TOP_LEFT, 600, 400), 40, 10, 320, 240, r);
        assertEquals(280, r.x);
        assertEquals(230, r.y);
    }

    @Test
    void resolve_wider_than_screen_starts_at_zero() {
        MutableRect r = new MutableRect();
        LayoutMath.resolve(cfg(Anchor.TOP_RIGHT, 0, 0), 400, 10, 320, 240, r);
        assertEquals(0, r.x);
        LayoutMath.resolve(cfg(Anchor.CENTER, 50, 50), 400, 300, 320, 240, r);
        assertEquals(0, r.x);
        assertEquals(0, r.y);
        LayoutMath.resolve(cfg(Anchor.BOTTOM_RIGHT, -50, -50), 400, 300, 320, 240, r);
        assertEquals(0, r.x);
        assertEquals(0, r.y);
    }

    @Test
    void anchor_follows_thirds() {
        assertEquals(Anchor.BOTTOM_RIGHT, LayoutMath.anchorFor(300, 230, 320, 240));
        assertEquals(Anchor.CENTER, LayoutMath.anchorFor(160, 120, 320, 240));
        assertEquals(Anchor.TOP_LEFT, LayoutMath.anchorFor(0, 0, 320, 240));
        assertEquals(Anchor.TOP_CENTER, LayoutMath.anchorFor(160, 10, 320, 240));
        assertEquals(Anchor.MIDDLE_RIGHT, LayoutMath.anchorFor(300, 120, 320, 240));
        assertEquals(Anchor.BOTTOM_LEFT, LayoutMath.anchorFor(10, 230, 320, 240));
    }

    @Test
    void reanchor_roundtrips_for_every_position_and_screen() {
        MutableRect r = new MutableRect();
        EnumSet<Anchor> seen = EnumSet.noneOf(Anchor.class);
        for (int x = 0; x <= 280; x += 7) {
            for (int y = 0; y <= 230; y += 5) {
                ModuleConfig c = ModuleConfig.defaults();
                LayoutMath.reanchor(c, x, y, 40, 10, 320, 240);
                seen.add(c.anchor());
                LayoutMath.resolve(c, 40, 10, 320, 240, r);
                assertEquals(x, r.x);
                assertEquals(y, r.y);
            }
        }
        assertEquals(EnumSet.allOf(Anchor.class), seen);
    }

    @Test
    void roundtrip_holds_for_odd_sizes_and_every_stored_anchor() {
        MutableRect a = new MutableRect();
        MutableRect b = new MutableRect();
        for (Anchor anchor : Anchor.values()) {
            for (int sw = 50; sw <= 53; sw++) {
                for (int w = 1; w <= 9; w++) {
                    ModuleConfig c = cfg(anchor, 3, -2);
                    LayoutMath.resolve(c, w, 7, sw, 41, a);
                    ModuleConfig d = ModuleConfig.defaults();
                    LayoutMath.reanchor(d, a.x, a.y, w, 7, sw, 41);
                    LayoutMath.resolve(d, w, 7, sw, 41, b);
                    assertEquals(a.x, b.x);
                    assertEquals(a.y, b.y);
                }
            }
        }
    }

    @Test
    void corner_stays_at_corner_on_bigger_screen() {
        ModuleConfig c = ModuleConfig.defaults();
        MutableRect r = new MutableRect();
        LayoutMath.reanchor(c, 278, 228, 40, 10, 320, 240);
        LayoutMath.resolve(c, 40, 10, 640, 480, r);
        assertEquals(598, r.x);
        assertEquals(468, r.y);
    }

    @Test
    void module_stays_fully_visible_after_gui_scale_change() {
        // Dragged to the bottom right on a large screen, then the GUI scale rises (smaller scaled screen).
        ModuleConfig c = ModuleConfig.defaults();
        MutableRect r = new MutableRect();
        LayoutMath.reanchor(c, 600, 440, 40, 10, 640, 450);
        int[][] screens = {{427, 240}, {320, 180}, {160, 90}, {45, 12}};
        for (int[] s : screens) {
            LayoutMath.resolve(c, 40, 10, s[0], s[1], r);
            assertTrue(r.x >= 0 && r.x + 40 <= Math.max(40, s[0]), "x fits on " + s[0]);
            assertTrue(r.y >= 0 && r.y + 10 <= Math.max(10, s[1]), "y fits on " + s[1]);
        }
        LayoutMath.resolve(c, 40, 10, 320, 180, r);
        assertEquals(280, r.x);
        assertEquals(170, r.y);
    }

    @Test
    void zero_and_one_pixel_screens() {
        MutableRect r = new MutableRect();
        for (Anchor a : Anchor.values()) {
            LayoutMath.resolve(cfg(a, 5, -5), 40, 10, 0, 0, r);
            assertEquals(0, r.x);
            assertEquals(0, r.y);
            LayoutMath.resolve(cfg(a, 5, -5), 40, 10, 1, 1, r);
            assertEquals(0, r.x);
            assertEquals(0, r.y);
            LayoutMath.resolve(cfg(a, 5, -5), 1, 1, 1, 1, r);
            assertEquals(0, r.x);
            assertEquals(0, r.y);
            LayoutMath.resolve(cfg(a, 0, 0), 0, 0, 0, 0, r);
            assertEquals(0, r.x);
            assertEquals(0, r.y);
        }
        ModuleConfig c = ModuleConfig.defaults();
        LayoutMath.reanchor(c, 0, 0, 40, 10, 0, 0);
        LayoutMath.resolve(c, 40, 10, 0, 0, r);
        assertEquals(0, r.x);
        assertEquals(0, r.y);
        LayoutMath.anchorFor(0, 0, 0, 0);
        LayoutMath.anchorFor(0, 0, 1, 1);
    }

    @Test
    void negative_offsets_clamp_to_zero_and_keep_valid_ones() {
        MutableRect r = new MutableRect();
        LayoutMath.resolve(cfg(Anchor.TOP_LEFT, -30, -30), 40, 10, 320, 240, r);
        assertEquals(0, r.x);
        assertEquals(0, r.y);
        LayoutMath.resolve(cfg(Anchor.BOTTOM_RIGHT, -30, -30), 40, 10, 320, 240, r);
        assertEquals(250, r.x);
        assertEquals(200, r.y);
        LayoutMath.resolve(cfg(Anchor.CENTER, -1000, -1000), 40, 10, 320, 240, r);
        assertEquals(0, r.x);
        assertEquals(0, r.y);
    }

    @Test
    void extreme_offsets_do_not_overflow() {
        MutableRect r = new MutableRect();
        LayoutMath.resolve(cfg(Anchor.BOTTOM_RIGHT, Integer.MAX_VALUE, Integer.MAX_VALUE), 40, 10, 320, 240, r);
        assertEquals(280, r.x);
        assertEquals(230, r.y);
        LayoutMath.resolve(cfg(Anchor.BOTTOM_RIGHT, Integer.MIN_VALUE, Integer.MIN_VALUE), 40, 10, 320, 240, r);
        assertEquals(0, r.x);
        assertEquals(0, r.y);
    }

    @Test
    void scale_limits_and_non_finite_scale_give_sane_rects() {
        float[] scales = {0.5f, 3.0f, Float.NaN, Float.POSITIVE_INFINITY, Float.NEGATIVE_INFINITY, 0f, -4f};
        float[] expected = {0.5f, 3.0f, 1.0f, 3.0f, 0.5f, 0.5f, 0.5f};
        MutableRect r = new MutableRect();
        for (int i = 0; i < scales.length; i++) {
            ModuleConfig c = cfg(Anchor.BOTTOM_RIGHT, 0, 0);
            c.setScale(scales[i]);
            assertEquals(expected[i], c.scale(), 0f);
            int w = Math.round(80 * c.scale());
            int h = Math.round(20 * c.scale());
            LayoutMath.resolve(c, w, h, 320, 240, r);
            assertEquals(320 - w, r.x);
            assertEquals(240 - h, r.y);
        }
    }

    @Test
    void contains_is_half_open() {
        MutableRect r = new MutableRect();
        r.x = 10;
        r.y = 20;
        r.w = 5;
        r.h = 4;
        assertTrue(r.contains(10, 20));
        assertTrue(r.contains(14, 23));
        assertFalse(r.contains(15, 20));
        assertFalse(r.contains(10, 24));
        assertFalse(r.contains(9, 20));
        assertFalse(r.contains(10, 19));
    }
}
