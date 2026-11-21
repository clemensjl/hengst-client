package io.github.clemensjl.hengst.mod.common.ui;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.testutil.FakeCanvas;
import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.Test;

class WidgetsTest {
    @Test void toggle_flips_on_click_inside_only() {
        boolean[] v = {false};
        ToggleWidget t = new ToggleWidget("T", () -> v[0], b -> v[0] = b);
        t.x = 10; t.y = 10; t.w = 100; t.h = 20;
        assertFalse(t.mouseClicked(5, 15, 0)); assertFalse(v[0]);
        assertTrue(t.mouseClicked(50, 15, 0)); assertTrue(v[0]);
        assertTrue(t.mouseClicked(50, 15, 0)); assertFalse(v[0]);
        assertFalse(t.mouseClicked(50, 15, 1)); assertFalse(v[0]);
    }

    @Test void slider_maps_position_and_snaps() {
        double[] v = {1.0};
        SliderWidget s = new SliderWidget("S", 0.5f, 3.0f, 0.1f, () -> v[0], d -> v[0] = d);
        s.x = 10; s.y = 10; s.w = 100; s.h = 20;
        assertTrue(s.mouseClicked(62, 15, 0)); assertEquals(1.8, v[0], 1e-5);
        assertTrue(s.mouseDragged(500, 15)); assertEquals(3.0, v[0], 1e-9);
        s.mouseReleased();
        assertFalse(s.mouseDragged(20, 15)); assertEquals(3.0, v[0], 1e-9);
    }

    @Test void slider_stays_in_range_and_on_step_when_the_step_does_not_divide_the_range() {
        double[] v = {0.0};
        // range 0..1 with step 0.3: the allowed values are 0, 0.3, 0.6, 0.9; 1.0 is off the grid
        SliderWidget s = new SliderWidget("S", 0f, 1f, 0.3f, () -> v[0], d -> v[0] = d);
        s.x = 0; s.y = 0; s.w = 100; s.h = 20;
        assertTrue(s.mouseClicked(99, 5, 0));
        assertEquals(0.9, v[0], 1e-6);
        s.mouseDragged(10000, 5);
        assertEquals(0.9, v[0], 1e-6);
        s.mouseDragged(-10000, 5);
        assertEquals(0.0, v[0], 1e-9);
        for (int mx = -20; mx <= 130; mx++) {
            s.mouseDragged(mx, 5);
            double n = v[0] / 0.3;
            assertTrue(v[0] >= 0.0 && v[0] <= 1.0, "in range at " + mx);
            assertEquals(Math.rint(n), n, 1e-5, "on step at " + mx);
        }
    }

    @Test void choice_cycles_both_directions() {
        String[] v = {"c"};
        ChoiceWidget w = new ChoiceWidget("C", new String[] {"a", "b", "c"}, () -> v[0], s -> v[0] = s);
        w.x = 0; w.y = 0; w.w = 100; w.h = 20;
        assertTrue(w.mouseClicked(5, 5, 0)); assertEquals("a", v[0]);
        assertTrue(w.mouseClicked(5, 5, 1)); assertEquals("c", v[0]);
    }

    @Test void choice_with_unknown_value_starts_at_the_first_entry() {
        String[] v = {"zzz"};
        ChoiceWidget w = new ChoiceWidget("C", new String[] {"a", "b"}, () -> v[0], s -> v[0] = s);
        w.x = 0; w.y = 0; w.w = 100; w.h = 20;
        w.mouseClicked(5, 5, 0);
        assertEquals("b", v[0]);
    }

    @Test void scroll_list_clamps_offset() {
        ScrollList l = new ScrollList(0, 0, 200, 100);
        List<Widget> ws = new ArrayList<>();
        for (int i = 0; i < 20; i++) ws.add(new ButtonWidget("b" + i, () -> {}));
        l.setWidgets(ws, 20);
        l.scroll(-100); assertEquals(300, l.offset());
        l.scroll(100); assertEquals(0, l.offset());
        l.scroll(-1); assertEquals(20, l.offset());
    }

    @Test void scroll_list_scrolls_by_whole_rows_so_no_row_is_cut_at_the_top() {
        ScrollList l = new ScrollList(0, 0, 200, 110);   // 5 full rows and a 10 px rest
        List<Widget> ws = new ArrayList<>();
        for (int i = 0; i < 20; i++) ws.add(new ButtonWidget("b" + i, () -> {}));
        l.setWidgets(ws, 20);
        for (int step = 0; step < 30; step++) {
            l.scroll(-1);
            assertEquals(0, l.offset() % 20, "offset " + l.offset());
        }
        assertEquals(300, l.offset());   // rows 15..19 fill the list, not 290 (row 14 cut at the top)
        l.scroll(0.4);                   // less than half a notch: no move
        assertEquals(300, l.offset());
        l.scroll(3);
        assertEquals(240, l.offset());
    }

    @Test void scroll_list_draws_and_clicks_only_rows_fully_inside() {
        ScrollList l = new ScrollList(0, 0, 200, 110);
        int[] clicks = new int[20];
        List<Widget> ws = new ArrayList<>();
        for (int i = 0; i < 20; i++) { int n = i; ws.add(new ButtonWidget("b" + i, () -> clicks[n]++)); }
        l.setWidgets(ws, 20);
        l.scroll(-1); // offset 20: row 0 above the list, row 1 at the top, row 6 (y 100..120) cut at the bottom
        FakeCanvas c = new FakeCanvas();
        l.render(c, 0, 0);
        assertFalse(c.calls.stream().anyMatch(s -> s.startsWith("text b0 ")));
        assertTrue(c.calls.stream().anyMatch(s -> s.startsWith("text b1 ")));
        assertTrue(c.calls.stream().anyMatch(s -> s.startsWith("text b5 ")));
        assertFalse(c.calls.stream().anyMatch(s -> s.startsWith("text b6 ")));
        assertTrue(l.mouseClicked(10, 105, 0) == false);        // inside the cut row 6
        assertEquals(0, clicks[0] + clicks[6]);
        assertTrue(l.mouseClicked(10, 4, 0)); assertEquals(1, clicks[1]);
        assertFalse(l.mouseClicked(10, 150, 0));                // outside the list
    }

    @Test void scroll_list_reset_goes_back_to_the_top() {
        ScrollList l = new ScrollList(0, 0, 200, 100);
        List<Widget> ws = new ArrayList<>();
        for (int i = 0; i < 20; i++) ws.add(new ButtonWidget("b" + i, () -> {}));
        l.setWidgets(ws, 20);
        l.scroll(-5);
        assertEquals(100, l.offset());
        l.reset();
        assertEquals(0, l.offset());
    }
}
