package io.github.clemensjl.hengst.mod.common.config;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;

import io.github.clemensjl.hengst.mod.common.feature.OptionSpec;
import org.junit.jupiter.api.Test;

class TypedOptionTest {
    @Test
    void typed_reads_equal_the_boxed_reads_for_every_raw_value() {
        OptionSpec t = OptionSpec.toggle("t", true);
        OptionSpec i = OptionSpec.intRange("i", 1, 0, 2);
        OptionSpec f = OptionSpec.floatRange("f", 1f, 0.5f, 3f, 0.1f);
        OptionSpec c = OptionSpec.choice("c", "b", "a", "b", "c");
        Object[] raws = {null, Boolean.FALSE, "x", "c", 2L, 9L, -4L, 1.26, 2.6, Double.NaN, -1e300, 1.3f, Float.NaN, 500L};
        for (Object raw : raws) {
            ModuleConfig m = ModuleConfig.defaults();
            m.setOption("t", raw);
            m.setOption("i", raw);
            m.setOption("f", raw);
            m.setOption("c", raw);
            assertEquals(m.option(t), m.bool(t), "toggle " + raw);
            assertEquals(m.option(i), m.integer(i), "int " + raw);
            assertEquals((Float) m.option(f), m.decimal(f), 0f, "float " + raw);
            assertEquals(m.option(c), c.value(m.choice(c)), "choice " + raw);
        }
    }

    @Test
    void typed_read_of_the_wrong_kind_throws() {
        ModuleConfig m = ModuleConfig.defaults();
        assertThrows(IllegalArgumentException.class, () -> m.integer(OptionSpec.toggle("t", true)));
        assertThrows(IllegalArgumentException.class, () -> m.bool(OptionSpec.intRange("i", 0, 0, 2)));
        assertThrows(IllegalArgumentException.class, () -> m.decimal(OptionSpec.choice("c", "a", "a")));
        assertThrows(IllegalArgumentException.class, () -> m.choice(OptionSpec.floatRange("f", 1f, 0f, 2f, 0.5f)));
    }

    @Test
    void choice_index_value_and_default_index() {
        OptionSpec c = OptionSpec.choice("c", "b", "a", "b", "c");
        assertEquals(1, c.defaultIndex());
        ModuleConfig m = ModuleConfig.defaults();
        assertEquals(1, m.choice(c));
        m.setOption("c", "c");
        assertEquals(2, m.choice(c));
        assertSame(c.value(2), c.value(2));
    }

    @Test
    void coerce_keeps_the_identity_shortcut_for_a_float_on_the_grid() {
        OptionSpec f = OptionSpec.floatRange("f", 1f, 0.5f, 3f, 0.1f);
        Object onGrid = f.coerce(1.79); // 1.8f, boxed once
        assertSame(onGrid, f.coerce(onGrid));
        assertEquals(1.8f, (Float) onGrid, 0f);
    }
}
