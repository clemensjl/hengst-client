package io.github.clemensjl.hengst.mod.common.hud;

import static org.junit.jupiter.api.Assertions.assertEquals;

import io.github.clemensjl.hengst.mod.common.testutil.FakeTime;
import org.junit.jupiter.api.Test;

class CountersTest {
    @Test
    void fps_counts_frames_per_second() {
        FakeTime t = new FakeTime();
        FrameCounter f = new FrameCounter(t);
        for (int i = 0; i < 120; i++) {
            f.onFrame();
            t.advanceMillis(5);
        }
        assertEquals(0, f.fps());
        for (int i = 0; i < 80; i++) {
            f.onFrame();
            t.advanceMillis(5);
        }
        f.onFrame();
        assertEquals(200, f.fps());
    }

    @Test
    void fps_window_starts_at_construction() {
        FakeTime t = new FakeTime();
        t.advanceMillis(5000);
        FrameCounter f = new FrameCounter(t);
        for (int i = 0; i < 60; i++) {
            f.onFrame();
            t.advanceMillis(16);
        }
        assertEquals(0, f.fps());
        t.advanceMillis(40);
        f.onFrame();
        assertEquals(60, f.fps());
    }

    @Test
    void fps_is_scaled_by_the_real_window_length() {
        FakeTime t = new FakeTime();
        FrameCounter f = new FrameCounter(t);
        for (int i = 0; i < 30; i++) {
            f.onFrame();
        }
        t.advanceMillis(1500);
        f.onFrame();
        assertEquals(20, f.fps());
    }

    @Test
    void fps_keeps_the_last_value_until_the_next_window_closes() {
        FakeTime t = new FakeTime();
        FrameCounter f = new FrameCounter(t);
        for (int i = 0; i < 100; i++) {
            f.onFrame();
        }
        t.advanceMillis(1000);
        f.onFrame();
        assertEquals(100, f.fps());
        t.advanceMillis(999);
        f.onFrame();
        assertEquals(100, f.fps());
    }

    @Test
    void fps_is_zero_after_a_gap_and_the_next_window_starts_fresh() {
        FakeTime t = new FakeTime();
        FrameCounter f = new FrameCounter(t);
        for (int i = 0; i < 100; i++) {
            f.onFrame();
        }
        t.advanceMillis(1000);
        f.onFrame();
        assertEquals(100, f.fps());
        t.advanceMillis(2000);
        assertEquals(100, f.fps());
        t.advanceMillis(1);
        assertEquals(0, f.fps());
        f.onFrame();
        assertEquals(0, f.fps());
        for (int i = 0; i < 49; i++) {
            f.onFrame();
        }
        t.advanceMillis(1000);
        f.onFrame();
        assertEquals(50, f.fps());
    }

    @Test
    void fps_at_a_very_low_frame_rate_is_not_absurd() {
        FakeTime t = new FakeTime();
        FrameCounter f = new FrameCounter(t);
        f.onFrame();
        t.advanceMillis(1500);
        f.onFrame();
        assertEquals(1, f.fps());
        t.advanceMillis(5000);
        f.onFrame();
        assertEquals(0, f.fps());
    }

    @Test
    void cps_counts_presses_in_window() {
        FakeTime t = new FakeTime();
        CpsCounter c = new CpsCounter(t);
        for (int i = 0; i < 10; i++) {
            c.onPress(0);
            t.advanceMillis(50);
        }
        assertEquals(10, c.cps(0));
        assertEquals(0, c.cps(1));
    }

    @Test
    void cps_counts_left_and_right_separately() {
        FakeTime t = new FakeTime();
        CpsCounter c = new CpsCounter(t);
        for (int i = 0; i < 5; i++) {
            c.onPress(0);
        }
        for (int i = 0; i < 2; i++) {
            c.onPress(1);
        }
        assertEquals(5, c.cps(0));
        assertEquals(2, c.cps(1));
    }

    @Test
    void cps_drops_to_zero_after_window() {
        FakeTime t = new FakeTime();
        CpsCounter c = new CpsCounter(t);
        c.onPress(1);
        t.advanceMillis(1001);
        assertEquals(0, c.cps(1));
    }

    @Test
    void cps_click_exactly_one_second_old_no_longer_counts() {
        FakeTime t = new FakeTime();
        CpsCounter c = new CpsCounter(t);
        c.onPress(0);
        t.advanceMillis(999);
        assertEquals(1, c.cps(0));
        t.advanceMillis(1);
        assertEquals(0, c.cps(0));
    }

    @Test
    void cps_window_slides_one_click_at_a_time() {
        FakeTime t = new FakeTime();
        CpsCounter c = new CpsCounter(t);
        c.onPress(0);
        t.advanceMillis(400);
        c.onPress(0);
        t.advanceMillis(400);
        c.onPress(0);
        assertEquals(3, c.cps(0));
        t.advanceMillis(200);
        assertEquals(2, c.cps(0));
        t.advanceMillis(400);
        assertEquals(1, c.cps(0));
        t.advanceMillis(400);
        assertEquals(0, c.cps(0));
    }

    @Test
    void cps_caps_at_buffer_size() {
        FakeTime t = new FakeTime();
        CpsCounter c = new CpsCounter(t);
        for (int i = 0; i < 100; i++) {
            c.onPress(0);
            t.advanceMillis(5);
        }
        assertEquals(CpsCounter.CAPACITY, c.cps(0));
        assertEquals(64, c.cps(0));
    }

    @Test
    void cps_never_shows_a_wrapped_small_number_after_the_buffer_overflows() {
        FakeTime t = new FakeTime();
        CpsCounter c = new CpsCounter(t);
        for (int i = 0; i < CpsCounter.CAPACITY + 1; i++) {
            c.onPress(0);
        }
        assertEquals(CpsCounter.CAPACITY, c.cps(0));
        for (int i = 0; i < 2 * CpsCounter.CAPACITY + 1; i++) {
            c.onPress(0);
        }
        assertEquals(CpsCounter.CAPACITY, c.cps(0));
    }

    @Test
    void cps_expires_the_oldest_clicks_correctly_after_the_buffer_wrapped() {
        FakeTime t = new FakeTime();
        CpsCounter c = new CpsCounter(t);
        for (int i = 0; i < 100; i++) {
            c.onPress(0);
            t.advanceMillis(5);
        }
        // now = 500 ms; clicks at 0, 5, ..., 495 ms. 700 ms later only clicks stamped after 200 ms are younger than 1 s.
        t.advanceMillis(700);
        assertEquals(59, c.cps(0));
    }

    @Test
    void cps_ignores_other_buttons() {
        CpsCounter c = new CpsCounter(new FakeTime());
        c.onPress(2);
        c.onPress(-1);
        assertEquals(0, c.cps(0));
        assertEquals(0, c.cps(1));
        assertEquals(0, c.cps(2));
        assertEquals(0, c.cps(-1));
    }
}
