package io.github.clemensjl.hengst.mod.common.feature;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import java.lang.management.ManagementFactory;
import org.junit.jupiter.api.Tag;
import org.junit.jupiter.api.Test;

/** Runs with -Xint (task interpretedTest) for exact counts. ModuleConfig.option is the documented way to read options: a float read must not cost more than its box. */
@Tag("interpreted")
class OptionReadAllocationTest {
    private static final com.sun.management.ThreadMXBean MX =
            (com.sun.management.ThreadMXBean) ManagementFactory.getThreadMXBean();
    private static final int READS = 20000;
    /** A boxed Float is 16 bytes; a little room for the measuring itself. */
    private static final long MAX_BYTES_PER_READ_FROM_FILE_VALUE = 16;

    private static long allocated() {
        return MX.getThreadAllocatedBytes(Thread.currentThread().getId());
    }

    private static long measure(ModuleConfig cfg, OptionSpec spec) {
        Object sink = null;
        for (int i = 0; i < READS; i++) { sink = cfg.option(spec); }
        long before = allocated();
        for (int i = 0; i < READS; i++) { sink = cfg.option(spec); }
        long used = allocated() - before;
        assertTrue(sink != null);
        return used;
    }

    @Test
    void a_float_option_stored_on_the_grid_is_read_without_allocation() {
        OptionSpec spec = OptionSpec.floatRange("f", 1.0f, 0.5f, 3.0f, 0.1f);
        ModuleConfig cfg = ModuleConfig.defaults();
        cfg.setOption("f", spec.coerce(1.79));                    // what the settings screen stores
        long used = measure(cfg, spec);
        System.out.println("float option read from a coerced value: " + used + " bytes over " + READS + " reads");
        assertEquals(0, used);
    }

    @Test
    void a_float_option_read_from_a_file_value_costs_at_most_its_box() {
        OptionSpec spec = OptionSpec.floatRange("f", 1.0f, 0.5f, 3.0f, 0.1f);
        ModuleConfig cfg = ModuleConfig.defaults();
        cfg.setOption("f", Double.valueOf(1.8));                   // a Double, as the JSON parser delivers it
        long used = measure(cfg, spec);
        System.out.println("float option read from a file value: " + used + " bytes over " + READS + " reads");
        assertTrue(used <= READS * MAX_BYTES_PER_READ_FROM_FILE_VALUE, "allocated " + used);
    }

    @Test
    void typed_reads_allocate_nothing() {
        OptionSpec t = OptionSpec.toggle("t", true), i = OptionSpec.intRange("i", 0, 0, 1000),
                f = OptionSpec.floatRange("f", 1f, 0.5f, 3f, 0.1f), c = OptionSpec.choice("c", "a", "a", "b");
        ModuleConfig m = ModuleConfig.defaults();
        m.setOption("t", Boolean.TRUE);
        m.setOption("i", 500L);
        m.setOption("f", 1.26);
        m.setOption("c", "b");
        long sink = 0;
        for (int k = 0; k < 1000; k++) { sink += (m.bool(t) ? 1 : 0) + m.integer(i) + (int) m.decimal(f) + m.choice(c); }
        long before = allocated();
        for (int k = 0; k < 10000; k++) { sink += (m.bool(t) ? 1 : 0) + m.integer(i) + (int) m.decimal(f) + m.choice(c); }
        assertEquals(0, allocated() - before);
        assertTrue(sink > 0);
    }

    @Test
    void a_missing_float_option_is_read_through_option_without_allocation() {
        OptionSpec spec = OptionSpec.floatRange("f", 1.0f, 0.5f, 3.0f, 0.1f);
        ModuleConfig cfg = ModuleConfig.defaults();
        assertSame(spec.defaultValue(), cfg.option(spec));
        assertEquals(0, measure(cfg, spec));
    }

    @Test
    void typed_reads_of_missing_keys_allocate_nothing() {
        OptionSpec t = OptionSpec.toggle("t", true), i = OptionSpec.intRange("i", 7, 0, 1000),
                f = OptionSpec.floatRange("f", 1f, 0.5f, 3f, 0.1f), c = OptionSpec.choice("c", "b", "a", "b");
        ModuleConfig m = ModuleConfig.defaults();
        long sink = 0;
        for (int k = 0; k < 1000; k++) { sink += (m.bool(t) ? 1 : 0) + m.integer(i) + (int) m.decimal(f) + m.choice(c); }
        long before = allocated();
        for (int k = 0; k < 10000; k++) { sink += (m.bool(t) ? 1 : 0) + m.integer(i) + (int) m.decimal(f) + m.choice(c); }
        assertEquals(0, allocated() - before);
        assertTrue(sink > 0);
    }
}
