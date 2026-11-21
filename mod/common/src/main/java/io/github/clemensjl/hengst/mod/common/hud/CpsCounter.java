package io.github.clemensjl.hengst.mod.common.hud;

import io.github.clemensjl.hengst.mod.common.platform.TimeSource;

/**
 * Counts mouse clicks per second for the left (0) and right (1) button in a ring buffer each.
 * The caller reports press edges only; held buttons, key repeat and clicks while a screen is open
 * are filtered before {@link #onPress}.
 */
public final class CpsCounter {
    public static final int CAPACITY = 64;
    private static final long WINDOW_NANOS = 1_000_000_000L;

    private final TimeSource time;
    private final long[][] stamps = new long[2][CAPACITY];
    private final int[] next = new int[2];
    private final int[] size = new int[2];

    public CpsCounter(TimeSource time) {
        this.time = time;
    }

    /** Records one click now. Buttons other than 0 and 1 are ignored. */
    public void onPress(int button) {
        if (button < 0 || button > 1) {
            return;
        }
        int i = next[button];
        stamps[button][i] = time.nanoTime();
        next[button] = i + 1 == CAPACITY ? 0 : i + 1;
        if (size[button] < CAPACITY) {
            size[button]++;
        }
    }

    /** Clicks younger than one second, at most {@link #CAPACITY}; 0 for other buttons. A click exactly one second old no longer counts. */
    public int cps(int button) {
        if (button < 0 || button > 1) {
            return 0;
        }
        long now = time.nanoTime();
        long[] s = stamps[button];
        int idx = next[button];
        int n = size[button];
        int count = 0;
        while (count < n) {
            idx = idx == 0 ? CAPACITY - 1 : idx - 1;
            if (now - s[idx] >= WINDOW_NANOS) {
                break;
            }
            count++;
        }
        return count;
    }
}
