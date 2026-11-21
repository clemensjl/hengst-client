package io.github.clemensjl.hengst.mod.common.hud;

import io.github.clemensjl.hengst.mod.common.platform.Platform;
import io.github.clemensjl.hengst.mod.common.platform.TimeSource;

/** Shared, long-lived objects handed to every HUD module on tick and render. */
public final class HudContext {
    private final Platform platform;
    private final TimeSource time;
    private final FrameCounter frames;
    private final CpsCounter cps;

    public HudContext(Platform platform, TimeSource time, FrameCounter frames, CpsCounter cps) {
        this.platform = platform;
        this.time = time;
        this.frames = frames;
        this.cps = cps;
    }

    public Platform platform() {
        return platform;
    }

    public TimeSource time() {
        return time;
    }

    public FrameCounter frames() {
        return frames;
    }

    public CpsCounter cps() {
        return cps;
    }
}
