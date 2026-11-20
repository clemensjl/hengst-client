package io.github.clemensjl.hengst.mod.common.feature;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;

import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.testutil.TestFeature;
import java.util.Arrays;
import java.util.EnumSet;
import org.junit.jupiter.api.Test;

class FeatureRegistryTest {
    @Test
    void duplicate_id_or_number_is_rejected() {
        FeatureRegistry r = new FeatureRegistry();
        r.register(new TestFeature("a", 1, EnumSet.allOf(GameVersion.class)));
        assertThrows(IllegalArgumentException.class, () -> r.register(new TestFeature("a", 2, EnumSet.allOf(GameVersion.class))));
        assertThrows(IllegalArgumentException.class, () -> r.register(new TestFeature("b", 1, EnumSet.allOf(GameVersion.class))));
    }

    @Test
    void supported_on_filters_and_sorts() {
        FeatureRegistry r = new FeatureRegistry();
        r.register(new TestFeature("totems", 14, EnumSet.of(GameVersion.V1_21_11, GameVersion.V26_3)));
        r.register(new TestFeature("fps", 1, EnumSet.allOf(GameVersion.class)));
        assertEquals(Arrays.asList("fps"), TestFeature.ids(r.supportedOn(GameVersion.V1_8_9)));
        assertEquals(Arrays.asList("fps", "totems"), TestFeature.ids(r.supportedOn(GameVersion.V26_3)));
    }

    @Test
    void supported_on_is_unmodifiable_and_by_id_finds() {
        FeatureRegistry r = new FeatureRegistry();
        Feature fps = new TestFeature("fps", 1, EnumSet.allOf(GameVersion.class));
        r.register(fps);
        assertThrows(UnsupportedOperationException.class, () -> r.supportedOn(GameVersion.V1_8_9).clear());
        assertEquals(fps, r.byId("fps"));
        assertNull(r.byId("nope"));
    }

    @Test
    void option_coerce_clamps_and_falls_back() {
        assertEquals(2, OptionSpec.intRange("d", 0, 0, 2).coerce(9L));
        assertEquals(0.5f, OptionSpec.floatRange("s", 1f, 0.5f, 3f, 0.1f).coerce(0.1));
        assertEquals(1.3f, (Float) OptionSpec.floatRange("s", 1f, 0.5f, 3f, 0.1f).coerce(1.26), 1e-6f);
        assertEquals("a", OptionSpec.choice("c", "a", "a", "b").coerce("zzz"));
        assertEquals(true, OptionSpec.toggle("t", true).coerce("yes"));
    }

    @Test
    void option_accessors_expose_range_and_values() {
        OptionSpec f = OptionSpec.floatRange("s", 1f, 0.5f, 3f, 0.1f);
        assertEquals(0.5f, f.min());
        assertEquals(3f, f.max());
        assertEquals(0.1f, f.step());
        OptionSpec i = OptionSpec.intRange("d", 1, 1, 5);
        assertEquals(1f, i.min());
        assertEquals(5f, i.max());
        assertEquals(1f, i.step());
        assertArrayEquals(new String[] {"a", "b"}, OptionSpec.choice("c", "a", "a", "b").values());
        assertEquals(0, OptionSpec.toggle("t", true).values().length);
    }

    @Test
    void int_coerce_edge_cases() {
        OptionSpec i = OptionSpec.intRange("d", 1, 0, 5);
        assertEquals(1, i.coerce(Float.NaN));
        assertEquals(1, i.coerce(Double.NaN));
        assertEquals(5, i.coerce(Double.POSITIVE_INFINITY));
        assertEquals(0, i.coerce(Float.NEGATIVE_INFINITY));
        assertEquals(1, i.coerce("3"));
    }

    @Test
    void float_coerce_edge_cases() {
        OptionSpec f = OptionSpec.floatRange("s", 1f, 0.5f, 3f, 0.1f);
        assertEquals(1f, f.coerce(Float.NaN));
        assertEquals(1f, f.coerce(Double.NaN));
        assertEquals(3f, f.coerce(Double.POSITIVE_INFINITY));
        assertEquals(0.5f, f.coerce(Float.NEGATIVE_INFINITY));
        assertEquals(1f, f.coerce("2"));
        assertEquals(0.5f, f.coerce(-4f));
        assertEquals(3f, f.coerce(9f));
    }

    @Test
    void choice_and_toggle_coerce() {
        OptionSpec c = OptionSpec.choice("c", "a", "a", "b");
        assertEquals("b", c.coerce("b"));
        assertEquals("a", c.coerce("zzz"));
        assertEquals(false, OptionSpec.toggle("t", true).coerce(Boolean.FALSE));
    }

    @Test
    void factories_reject_bad_arguments() {
        assertThrows(IllegalArgumentException.class, () -> OptionSpec.intRange("k", 1, 5, 0));
        assertThrows(IllegalArgumentException.class, () -> OptionSpec.floatRange("k", 1f, 3f, 0.5f, 0.1f));
        assertThrows(IllegalArgumentException.class, () -> OptionSpec.intRange("k", 9, 0, 5));
        assertThrows(IllegalArgumentException.class, () -> OptionSpec.floatRange("k", 0f, 0.5f, 3f, 0.1f));
        assertThrows(IllegalArgumentException.class, () -> OptionSpec.floatRange("k", 1f, 0.5f, 3f, 0f));
        assertThrows(IllegalArgumentException.class, () -> OptionSpec.floatRange("k", 1f, 0.5f, 3f, -1f));
        assertThrows(IllegalArgumentException.class, () -> OptionSpec.choice("k", "z", "a", "b"));
        assertThrows(IllegalArgumentException.class, () -> OptionSpec.choice("k", "a"));
        assertThrows(IllegalArgumentException.class, () -> OptionSpec.toggle(null, true));
        assertThrows(IllegalArgumentException.class, () -> OptionSpec.toggle("", true));
        assertThrows(IllegalArgumentException.class, () -> OptionSpec.intRange("", 1, 0, 5));
    }
}
