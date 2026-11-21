package io.github.clemensjl.hengst.mod.common.platform;

import java.util.TimeZone;

/**
 * The system clock and the default time zone, fetched once when the class loads. A change of the operating system's
 * time zone while the game runs is not seen (the JVM keeps its default zone as well).
 */
final class SystemWallClock implements WallClock {
    static final SystemWallClock INSTANCE = new SystemWallClock();

    private final TimeZone zone = TimeZone.getDefault();

    private SystemWallClock() {}

    @Override
    public long currentTimeMillis() {
        return System.currentTimeMillis();
    }

    @Override
    public int offsetMillis(long utcMillis) {
        return zone.getOffset(utcMillis);
    }
}
