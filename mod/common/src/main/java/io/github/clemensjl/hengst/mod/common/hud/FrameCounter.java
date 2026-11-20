package io.github.clemensjl.hengst.mod.common.hud;

import io.github.clemensjl.hengst.mod.common.platform.TimeSource;

/** Counts rendered frames per second; the logic arrives with the FPS module. */
public final class FrameCounter {
    private final TimeSource time;

    public FrameCounter(TimeSource time) {
        this.time = time;
    }

    public void onFrame() {}

    public int fps() {
        return 0;
    }
}
