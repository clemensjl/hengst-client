package io.github.clemensjl.hengst.mod.common.hud;

import io.github.clemensjl.hengst.mod.common.I18n;
import io.github.clemensjl.hengst.mod.common.platform.Platform;
import io.github.clemensjl.hengst.mod.common.platform.TimeSource;
import io.github.clemensjl.hengst.mod.common.platform.WallClock;

/** Shared, long-lived objects handed to every HUD module on tick and render. */
public final class HudContext {
    private final Platform platform;
    private final TimeSource time;
    private final WallClock wallClock;
    private final FrameCounter frames;
    private final CpsCounter cps;

    private I18n i18n;
    private int languageGeneration;

    /** The core's constructor: it read the language code itself (guarded) and passes the loaded texts. */
    public HudContext(Platform platform, TimeSource time, WallClock wallClock, FrameCounter frames, CpsCounter cps,
            I18n i18n) {
        this.platform = platform;
        this.time = time;
        this.wallClock = wallClock;
        this.i18n = i18n;
        this.frames = frames;
        this.cps = cps;
    }

    public Platform platform() {
        return platform;
    }

    public TimeSource time() {
        return time;
    }

    public WallClock wallClock() {
        return wallClock;
    }

    /** The texts of the game's current language, reloaded by the core (in the tick) when the language changes. */
    public I18n i18n() {
        return i18n;
    }

    /** Starts at 0, +1 for every {@link #setI18n}. */
    public int languageGeneration() {
        return languageGeneration;
    }

    /**
     * Only the core calls this, in the tick. Public because the core lives in another package and Java 8 has no
     * narrower way to share it.
     */
    public void setI18n(I18n i18n) {
        this.i18n = i18n;
        languageGeneration++;
    }

    public FrameCounter frames() {
        return frames;
    }

    public CpsCounter cps() {
        return cps;
    }
}
