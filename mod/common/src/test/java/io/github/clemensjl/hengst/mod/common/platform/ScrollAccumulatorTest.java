package io.github.clemensjl.hengst.mod.common.platform;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

import org.junit.jupiter.api.Test;

class ScrollAccumulatorTest {
    @Test void whole_notches_with_sign_and_remainder() {
        ScrollAccumulator legacy = new ScrollAccumulator(120.0);
        assertEquals(1, legacy.add(120)); assertEquals(0, legacy.add(60)); assertEquals(1, legacy.add(60));
        assertEquals(-1, legacy.add(-130)); assertEquals(0, legacy.add(-100)); assertEquals(-1, legacy.add(-10));
        ScrollAccumulator modern = new ScrollAccumulator(1.0);
        int sum = 0; for (int k = 0; k < 10; k++) { sum += modern.add(0.1); }
        assertEquals(1, sum);
    }

    @Test void nan_and_infinity_are_dropped_and_large_values_capped_at_100_notches() {
        ScrollAccumulator a = new ScrollAccumulator(1.0);
        assertEquals(0, a.add(Double.NaN)); assertEquals(0, a.add(Double.POSITIVE_INFINITY)); assertEquals(0, a.add(Double.NEGATIVE_INFINITY));
        assertEquals(100, a.add(500.25)); assertEquals(0, a.add(0.0));          // the remainder .25 is dropped with the excess
        assertEquals(-100, a.add(-1e5)); assertEquals(0, a.add(0.0));
        assertEquals(100, a.add(2e6));                                          // finite and absurd: capped, not dropped
        assertEquals(1, a.add(1.0));
        a.reset(); assertEquals(0, a.add(0.6)); assertEquals(1, a.add(0.4));
    }

    @Test void legacy_int_deltas_at_the_extremes_are_capped_like_today() {
        ScrollAccumulator legacy = new ScrollAccumulator(120.0);
        assertEquals(100, legacy.add(Integer.MAX_VALUE));
        assertEquals(-100, legacy.add(Integer.MIN_VALUE));
        assertEquals(1, legacy.add(120));                                       // excess and remainder were dropped
        assertEquals(0, legacy.add(100)); legacy.reset(); assertEquals(0, legacy.add(100)); assertEquals(1, legacy.add(20));
    }

    @Test void infinite_input_yields_zero_and_keeps_the_partial_notch() {
        ScrollAccumulator a = new ScrollAccumulator(1.0);
        assertEquals(0, a.add(0.75));
        assertEquals(0, a.add(Double.POSITIVE_INFINITY));
        assertEquals(0, a.add(Double.NEGATIVE_INFINITY));
        assertEquals(0, a.add(Double.NaN));
        assertEquals(1, a.add(0.25));                                           // the 0.75 survived
    }

    @Test void the_largest_finite_values_are_capped_with_their_sign() {
        ScrollAccumulator modern = new ScrollAccumulator(1.0);
        assertEquals(100, modern.add(Double.MAX_VALUE));
        assertEquals(-100, modern.add(-Double.MAX_VALUE));
        assertEquals(1, modern.add(1.0));
        ScrollAccumulator small = new ScrollAccumulator(1e-300);                // finite amount, infinite once scaled
        assertEquals(100, small.add(Double.MAX_VALUE));
        assertEquals(-100, small.add(-Double.MAX_VALUE));
        assertEquals(0, small.add(0.0));
    }

    @Test void absurd_finite_values_from_1e6_are_capped_not_dropped() {
        ScrollAccumulator modern = new ScrollAccumulator(1.0);
        assertEquals(100, modern.add(1e6));                                     // was 0 on the modern versions before
        assertEquals(-100, modern.add(-1e6));
        assertEquals(100, modern.add(1e12));
        assertEquals(-100, modern.add(-1e12));
        assertEquals(0, modern.add(0.0));
    }

    @Test void exactly_100_notches_pass_and_101_are_capped() {
        ScrollAccumulator a = new ScrollAccumulator(1.0);
        assertEquals(100, a.add(100.0));
        assertEquals(100, a.add(101.0));
        assertEquals(-100, a.add(-100.0));
        assertEquals(0, a.add(0.4));
        assertEquals(1, a.add(0.6));
    }

    @Test void a_sign_change_mid_accumulation_cancels_the_partial_notch() {
        ScrollAccumulator a = new ScrollAccumulator(1.0);
        assertEquals(0, a.add(0.6));
        assertEquals(0, a.add(-0.4));                                           // 0.2 left
        assertEquals(0, a.add(-0.5));                                           // -0.3 left
        assertEquals(-1, a.add(-0.7));                                          // -1.0 completes, nothing left
        assertEquals(0, a.add(0.5));
        assertEquals(2, a.add(1.5));                                            // 0.5 + 1.5
    }

    @Test void many_small_fractional_steps_add_up_to_one_notch() {
        for (double step : new double[] {0.1, 0.05, 0.01, 0.001, 1.0 / 3.0}) {
            ScrollAccumulator a = new ScrollAccumulator(1.0);
            int steps = (int) Math.round(1.0 / step);
            int total = 0;
            for (int k = 0; k < steps; k++) {
                int notches = a.add(step);
                if (k < steps - 1) { assertEquals(0, notches, "step " + step + " at " + k); }
                total += notches;
            }
            assertEquals(1, total, "step " + step);
        }
        ScrollAccumulator legacy = new ScrollAccumulator(120.0);
        int total = 0;
        for (int k = 0; k < 120; k++) { total += legacy.add(1); }
        assertEquals(1, total);
        ScrollAccumulator negative = new ScrollAccumulator(1.0);
        total = 0;
        for (int k = 0; k < 1000; k++) { total += negative.add(-0.001); }
        assertEquals(-1, total);
    }

    @Test void the_unit_must_be_positive_and_finite() {
        assertThrows(IllegalArgumentException.class, () -> new ScrollAccumulator(0.0));
        assertThrows(IllegalArgumentException.class, () -> new ScrollAccumulator(-1.0));
        assertThrows(IllegalArgumentException.class, () -> new ScrollAccumulator(Double.NaN));
        assertThrows(IllegalArgumentException.class, () -> new ScrollAccumulator(Double.POSITIVE_INFINITY));
    }

    // The moved adapter tests, same values.

    @Test void one_wheel_notch_at_default_sensitivity_is_one_notch() {
        ScrollAccumulator s = new ScrollAccumulator(1.0);
        assertEquals(1, s.add(1.0)); assertEquals(-1, s.add(-1.0)); assertEquals(0, s.add(0.0));
    }

    @Test void ten_touchpad_events_of_one_tenth_are_one_notch() {
        ScrollAccumulator s = new ScrollAccumulator(1.0);
        int total = 0;
        for (int i = 0; i < 9; i++) { total += s.add(0.1); }
        assertEquals(0, total); assertEquals(1, s.add(0.1)); assertEquals(0, s.add(0.05));
    }

    @Test void lowest_sensitivity_still_reaches_a_notch() {
        ScrollAccumulator s = new ScrollAccumulator(1.0);
        int total = 0; int events = 0;
        while (total == 0 && events < 1000) { total += s.add(-0.01); events++; }
        assertEquals(-1, total); assertEquals(100, events, 1.0);
    }

    @Test void large_amounts_give_several_notches_and_keep_the_remainder() {
        ScrollAccumulator s = new ScrollAccumulator(1.0);
        assertEquals(3, s.add(3.5)); assertEquals(1, s.add(0.5)); assertEquals(0, s.add(-0.5)); assertEquals(-1, s.add(-0.5));
    }

    @Test void one_event_emits_at_most_one_hundred_notches() {
        ScrollAccumulator s = new ScrollAccumulator(1.0);
        assertEquals(100, s.add(500.25)); assertEquals(0, s.add(0.0));
        assertEquals(-100, s.add(-1e5)); assertEquals(0, s.add(0.0)); assertEquals(1, s.add(1.0));
    }

    @Test void reset_drops_the_remainder_and_bad_values_are_ignored() {
        ScrollAccumulator s = new ScrollAccumulator(1.0);
        assertEquals(0, s.add(0.9)); s.reset(); assertEquals(0, s.add(0.9));
        assertEquals(0, s.add(Double.NaN)); assertEquals(0, s.add(Double.POSITIVE_INFINITY)); assertEquals(1, s.add(0.1));
    }

    @Test void legacy_one_wheel_notch_is_120() {
        ScrollAccumulator s = new ScrollAccumulator(120.0);
        assertEquals(1, s.add(120)); assertEquals(-1, s.add(-120)); assertEquals(3, s.add(360)); assertEquals(0, s.add(0));
    }

    @Test void legacy_small_deltas_add_up_to_whole_notches() {
        ScrollAccumulator s = new ScrollAccumulator(120.0);
        assertEquals(0, s.add(30)); assertEquals(0, s.add(30)); assertEquals(0, s.add(30)); assertEquals(1, s.add(30));
        assertEquals(0, s.add(-60)); assertEquals(-1, s.add(-60));
    }

    @Test void legacy_a_partial_notch_is_kept() {
        ScrollAccumulator s = new ScrollAccumulator(120.0);
        assertEquals(1, s.add(180)); assertEquals(1, s.add(60)); assertEquals(0, s.add(-60)); assertEquals(-1, s.add(-60));
    }

    @Test void legacy_reset_forgets_the_partial_notch() {
        ScrollAccumulator s = new ScrollAccumulator(120.0);
        assertEquals(0, s.add(100)); s.reset(); assertEquals(0, s.add(100)); assertEquals(1, s.add(20));
    }
}
