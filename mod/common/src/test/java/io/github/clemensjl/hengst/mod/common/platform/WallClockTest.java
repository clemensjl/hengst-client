package io.github.clemensjl.hengst.mod.common.platform;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.TimeZone;
import org.junit.jupiter.api.Test;

class WallClockTest {
    @Test void the_system_clock_reads_the_system_time_and_the_default_zone() {
        long before = System.currentTimeMillis();
        long read = WallClock.SYSTEM.currentTimeMillis();
        assertTrue(read >= before && read <= System.currentTimeMillis());
        long winter = 1_704_067_200_000L;   // 2024-01-01T00:00:00Z
        long summer = 1_719_792_000_000L;   // 2024-07-01T00:00:00Z
        assertEquals(TimeZone.getDefault().getOffset(winter), WallClock.SYSTEM.offsetMillis(winter));
        assertEquals(TimeZone.getDefault().getOffset(summer), WallClock.SYSTEM.offsetMillis(summer));
    }
}
