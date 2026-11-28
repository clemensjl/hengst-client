package io.github.clemensjl.hengst.mod.common.platform;

/**
 * The wall clock behind clock-like HUD modules; injectable for tests. Both methods are for the tick and may allocate
 * (the system implementation asks the JDK's time zone rules): a caller reads them at most when its displayed value
 * can change, never per frame.
 */
public interface WallClock {
    WallClock SYSTEM = SystemWallClock.INSTANCE;

    /** Milliseconds since the epoch, UTC. */
    long currentTimeMillis();

    /** The offset of the local time zone from UTC at the given instant, daylight saving included. */
    int offsetMillis(long utcMillis);
}
