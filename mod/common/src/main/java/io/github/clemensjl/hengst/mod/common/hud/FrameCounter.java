package io.github.clemensjl.hengst.mod.common.hud;

import io.github.clemensjl.hengst.mod.common.platform.TimeSource;

/** Counts rendered frames per second over one-second windows. */
public final class FrameCounter {
    private static final long WINDOW_NANOS = 1_000_000_000L;
    private static final long STALE_NANOS = 2_000_000_000L;

    private final TimeSource time;
    private long windowStart;
    private int frames;
    private int fps;

    /** The first window begins now. */
    public FrameCounter(TimeSource time) {
        this.time = time;
        this.windowStart = time.nanoTime();
    }

    /** Call once per rendered frame. Closes the window first if a second has passed, then counts this frame. */
    public void onFrame() {
        long now = time.nanoTime();
        long elapsed = now - windowStart;
        if (elapsed >= WINDOW_NANOS) {
            // after a gap (HUD not drawn for a while) the old frames say nothing: start fresh
            fps = elapsed > STALE_NANOS ? 0 : (int) Math.round(frames * 1.0e9 / elapsed);
            frames = 0;
            windowStart = now;
        }
        frames++;
    }

    /** Frames per second of the last full window; 0 before the first one is complete and after more than 2 s without a closed window. */
    public int fps() {
        return time.nanoTime() - windowStart > STALE_NANOS ? 0 : fps;
    }
}
