package io.github.clemensjl.hengst.mod.common;

import static org.junit.jupiter.api.Assertions.assertEquals;

import io.github.clemensjl.hengst.mod.common.platform.ScrollAccumulator;
import java.lang.management.ManagementFactory;
import org.junit.jupiter.api.Tag;
import org.junit.jupiter.api.Test;

/** Runs with -Xint (task interpretedTest), where the thread's allocation count is exact. */
@Tag("interpreted")
class AdapterGuardAllocationTest {
    private static final com.sun.management.ThreadMXBean MX =
            (com.sun.management.ThreadMXBean) ManagementFactory.getThreadMXBean();

    private static long allocated() {
        return MX.getThreadAllocatedBytes(Thread.currentThread().getId());
    }

    /** A fault repeated every frame, after it was logged, must cost no allocation and no log line. */
    @Test void a_repeated_known_fault_allocates_nothing_and_logs_nothing() {
        int[] logged = new int[1];
        AdapterGuard guard = new AdapterGuard(new String[] {"HUD", "frame"}, new AdapterGuard.Log() {
            public void info(String m) { logged[0]++; }
            public void error(String m, Throwable t) { logged[0]++; }
        });
        IllegalStateException failure = new IllegalStateException("every frame");
        IllegalArgumentException other = new IllegalArgumentException("every frame");
        guard.hookFailed(1, failure);
        guard.hookFailed(1, other);
        for (int i = 0; i < 3000; i++) { guard.hookFailed(1, failure); guard.hookFailed(1, other); }
        int calls = 20000;
        long before = allocated();
        for (int i = 0; i < calls; i++) { guard.hookFailed(1, failure); guard.hookFailed(1, other); }
        long bytes = allocated() - before;
        System.out.println("AdapterGuard.hookFailed allocation over " + 2 * calls + " repeated faults: " + bytes + " bytes");
        assertEquals(0, bytes, "hookFailed allocated " + bytes + " bytes for known faults");
        assertEquals(2, logged[0]);
    }

    @Test void a_repeated_linkage_error_allocates_nothing_after_the_first_log() {
        AdapterGuard guard = new AdapterGuard(new String[] {"HUD"}, new AdapterGuard.Log() {
            public void info(String m) { }
            public void error(String m, Throwable t) { }
        });
        NoSuchMethodError failure = new NoSuchMethodError("m");
        guard.hookFailed(0, failure);
        for (int i = 0; i < 3000; i++) { guard.hookFailed(0, failure); }
        int calls = 20000;
        long before = allocated();
        for (int i = 0; i < calls; i++) { guard.hookFailed(0, failure); }
        long bytes = allocated() - before;
        assertEquals(0, bytes, "hookFailed allocated " + bytes + " bytes for a known linkage error");
    }

    /** The accumulator runs per wheel event: no allocation, whatever the input. */
    @Test void the_scroll_accumulator_allocates_nothing() {
        ScrollAccumulator modern = new ScrollAccumulator(1.0);
        ScrollAccumulator legacy = new ScrollAccumulator(120.0);
        int sink = 0;
        for (int i = 0; i < 3000; i++) { sink += modern.add(0.1) + legacy.add(30); }
        int calls = 20000;
        long before = allocated();
        for (int i = 0; i < calls; i++) {
            sink += modern.add(0.1) + legacy.add(-30) + modern.add(Double.NaN) + legacy.add(1e300) + modern.add(-1e6);
        }
        long bytes = allocated() - before;
        System.out.println("ScrollAccumulator allocation over " + 5 * calls + " calls: " + bytes + " bytes (sink " + sink + ")");
        assertEquals(0, bytes, "ScrollAccumulator allocated " + bytes + " bytes");
    }
}
