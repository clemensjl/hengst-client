package io.github.clemensjl.hengst.mod.common.editor;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.hud.HudModule;
import io.github.clemensjl.hengst.mod.common.layout.Anchor;
import io.github.clemensjl.hengst.mod.common.testutil.FakeCanvas;
import io.github.clemensjl.hengst.mod.common.testutil.FixedSizeModule;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

class HudEditorModelTest {
    FakeCanvas canvas = new FakeCanvas();
    HudModule[] modules;
    ModuleConfig[] configs;
    HudEditorModel model;

    @BeforeEach
    void setUp() {
        modules = new HudModule[] {new FixedSizeModule("a", 1, 40, 10), new FixedSizeModule("b", 2, 40, 10)};
        configs = new ModuleConfig[] {ModuleConfig.defaults(), ModuleConfig.defaults()};
        for (ModuleConfig c : configs) {
            c.setEnabled(true);
        }
        configs[1].setOffsetX(250);
        configs[1].setOffsetY(100); // a: x 0..40, y 0..10; b: x 250..290, y 100..110
        model = new HudEditorModel(modules, configs);
        model.layout(canvas);
    }

    @Test
    void hit_prefers_topmost() {
        configs[1].setOffsetX(20);
        configs[1].setOffsetY(0);
        model.layout(canvas); // a: 0..40, b: 20..60, overlapping
        assertEquals(1, model.hit(30, 5));
        assertEquals(0, model.hit(5, 5));
        assertEquals(-1, model.hit(300, 200));
    }

    @Test
    void drag_moves_and_reanchors() {
        model.press(5, 5);
        model.drag(305, 235);
        model.release();
        assertEquals(Anchor.BOTTOM_RIGHT, configs[0].anchor());
        model.layout(canvas);
        assertEquals(280, model.rect(0).x);
        assertEquals(230, model.rect(0).y);
        assertTrue(model.dirty());
    }

    @Test
    void drag_keeps_grab_offset() {
        model.press(20, 5); // middle of a
        model.drag(100, 50);
        model.release();
        model.layout(canvas);
        assertEquals(80, model.rect(0).x);
        assertEquals(45, model.rect(0).y);
    }

    @Test
    void drag_snaps_to_screen_center() {
        model.press(5, 5);
        model.drag(148, 5); // raw x = 143, centre 163, 3 px beside 160
        model.release();
        model.layout(canvas);
        assertEquals(140, model.rect(0).x);
        assertEquals(0, model.rect(0).y);
    }

    @Test
    void drag_snaps_to_other_module_edge() {
        configs[1].setOffsetX(60);
        configs[1].setOffsetY(100);
        model.layout(canvas); // b: x 60..100, y 100..110
        model.press(5, 5);
        model.drag(107, 105); // raw x = 102 (2 px right of b), y = 100
        model.release();
        model.layout(canvas);
        assertEquals(100, model.rect(0).x);
        assertEquals(100, model.rect(0).y);
    }

    @Test
    void drag_outside_screen_is_clamped() {
        model.press(5, 5);
        model.drag(-500, 9000);
        model.release();
        model.layout(canvas);
        assertEquals(0, model.rect(0).x);
        assertEquals(230, model.rect(0).y);
        assertFalse(model.dragging());
    }

    @Test
    void works_on_another_screen_size() {
        FakeCanvas big = new FakeCanvas() {
            @Override
            public int width() {
                return 640;
            }

            @Override
            public int height() {
                return 480;
            }
        };
        model.layout(big);
        model.press(5, 5);
        model.drag(5000, 5000);
        model.release();
        model.layout(big);
        assertEquals(Anchor.BOTTOM_RIGHT, configs[0].anchor());
        assertEquals(600, model.rect(0).x);
        assertEquals(470, model.rect(0).y);
    }

    @Test
    void scroll_scales_within_limits() {
        model.press(5, 5);
        model.release();
        model.scroll(1);
        assertEquals(1.1f, configs[0].scale(), 1e-6f);
        for (int i = 0; i < 30; i++) {
            model.scroll(1);
        }
        assertEquals(3.0f, configs[0].scale());
        for (int i = 0; i < 40; i++) {
            model.scroll(-1);
        }
        assertEquals(0.5f, configs[0].scale());
    }

    @Test
    void scroll_without_selection_does_nothing() {
        model.scroll(1);
        assertEquals(1.0f, configs[0].scale());
        assertEquals(1.0f, configs[1].scale());
        assertFalse(model.dirty());
    }

    @Test
    void nudge_and_hide() {
        model.press(5, 5);
        model.release();
        model.nudge(10, 0);
        model.layout(canvas);
        assertEquals(10, model.rect(0).x);
        assertEquals(0, model.rect(0).y);
        assertTrue(model.dirty());
        model.hideSelected();
        assertFalse(configs[0].enabled());
        assertTrue(configs[1].enabled());
    }

    @Test
    void hidden_module_is_not_hit() {
        model.press(5, 5);
        model.release();
        model.hideSelected();
        assertEquals(-1, model.hit(5, 5));
        assertEquals(-1, model.selected());
    }

    @Test
    void press_on_empty_area_clears_selection() {
        model.press(5, 5);
        model.release();
        assertEquals(0, model.selected());
        model.press(200, 200);
        assertFalse(model.dragging());
        assertEquals(-1, model.selected());
    }

    @Test
    void click_without_movement_is_not_a_change() {
        model.press(5, 5);
        model.release();
        assertFalse(model.dirty());
    }

    @Test
    void zero_size_module_is_grabbable_with_minimum_size() {
        modules = new HudModule[] {new FixedSizeModule("z", 1, 0, 0)};
        configs = new ModuleConfig[] {ModuleConfig.defaults()};
        configs[0].setEnabled(true);
        model = new HudEditorModel(modules, configs);
        model.layout(canvas);
        assertEquals(20, model.rect(0).w);
        assertEquals(10, model.rect(0).h);
        assertTrue(model.placeholder(0));
        assertEquals(0, model.hit(19, 9));
        model.press(10, 5);
        assertTrue(model.dragging());
    }
}
