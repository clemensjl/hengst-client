package io.github.clemensjl.hengst.mod.common.hud;

import io.github.clemensjl.hengst.mod.common.platform.TimeSource;

/** Counts mouse clicks per second; the logic arrives with the CPS module. */
public final class CpsCounter {
    private final TimeSource time;

    public CpsCounter(TimeSource time) {
        this.time = time;
    }

    public void onPress(int button) {}

    public int cps(int button) {
        return 0;
    }
}
