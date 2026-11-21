package io.github.clemensjl.hengst.mod.common.feature;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotSame;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.config.Profile;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.testutil.TestFeature;
import java.util.Arrays;
import java.util.EnumSet;
import java.util.List;
import org.junit.jupiter.api.Test;

class FeatureStateTest {
    private final EnumSet<GameVersion> all = EnumSet.allOf(GameVersion.class);

    @Test void rewrite_sets_exactly_the_active_features_and_a_second_rewrite_resets() {
        List<Feature> features = Arrays.<Feature>asList(new TestFeature("a", 1, all), new TestFeature("b", 7, all),
                new TestFeature("c", 900, all));
        Profile p = new Profile();
        p.module("a").setEnabled(true);
        p.module("c").setEnabled(true);
        FeatureGate gate = new FeatureGate(GameVersion.V1_8_9, RestrictionPolicy.NONE);
        FeatureState.rewrite(features, p, gate);
        assertTrue(FeatureState.on(1));
        assertFalse(FeatureState.on(7));
        assertTrue(FeatureState.on(900));
        assertFalse(FeatureState.on(899));
        assertFalse(FeatureState.on(901));
        assertFalse(FeatureState.on(-1));
        assertFalse(FeatureState.on(Integer.MAX_VALUE));

        p.module("a").setEnabled(false);
        FeatureState.rewrite(features, p, gate);
        assertFalse(FeatureState.on(1));
        assertTrue(FeatureState.on(900));
    }

    @Test void a_rewrite_with_other_features_clears_the_numbers_of_the_earlier_one() {
        Profile p = new Profile();
        p.module("c").setEnabled(true);
        FeatureGate gate = new FeatureGate(GameVersion.V1_8_9, RestrictionPolicy.NONE);
        FeatureState.rewrite(Arrays.<Feature>asList(new TestFeature("c", 900, all)), p, gate);
        assertTrue(FeatureState.on(900));
        FeatureState.rewrite(Arrays.<Feature>asList(new TestFeature("a", 1, all)), p, gate);
        assertFalse(FeatureState.on(900));
    }

    @Test void the_gate_needs_enabled_supported_and_not_restricted() {
        TestFeature f = new TestFeature("a", 1, EnumSet.of(GameVersion.V26_3));
        Profile p = new Profile();
        p.module("a").setEnabled(true);
        assertFalse(new FeatureGate(GameVersion.V1_8_9, RestrictionPolicy.NONE).active(f, p.module("a")));
        FeatureGate gate = new FeatureGate(GameVersion.V26_3, RestrictionPolicy.NONE);
        assertTrue(gate.active(f, p.module("a")));
        p.module("a").setEnabled(false);
        assertFalse(gate.active(f, p.module("a")));
        p.module("a").setEnabled(true);
        String[] seen = new String[1];
        FeatureGate restricted = new FeatureGate(GameVersion.V26_3, (feature, server) -> { seen[0] = server; return true; });
        restricted.setServer("mc.example.net");
        assertFalse(restricted.active(f, p.module("a")));
        assertEquals("mc.example.net", seen[0]);
        assertEquals("mc.example.net", restricted.server());
    }

    @Test void a_rewrite_publishes_a_new_array_and_never_mutates_the_old_one() {
        List<Feature> features = Arrays.<Feature>asList(new TestFeature("a", 1, all), new TestFeature("c", 900, all));
        Profile p = new Profile();
        p.module("a").setEnabled(true);
        FeatureGate gate = new FeatureGate(GameVersion.V1_8_9, RestrictionPolicy.NONE);
        FeatureState.rewrite(features, p, gate);
        boolean[] first = FeatureState.published();
        boolean[] copy = first.clone();
        p.module("a").setEnabled(false);
        p.module("c").setEnabled(true);
        FeatureState.rewrite(features, p, gate);
        assertNotSame(first, FeatureState.published());
        assertTrue(Arrays.equals(copy, first));
        assertTrue(first[1]);
        assertFalse(FeatureState.on(1));
    }

    @Test void withdraw_publishes_a_copy_without_the_given_features_and_never_mutates_the_old_array() {
        List<Feature> features = Arrays.<Feature>asList(new TestFeature("a", 1, all), new TestFeature("b", 2, all));
        Profile p = new Profile();
        p.module("a").setEnabled(true);
        p.module("b").setEnabled(true);
        FeatureState.rewrite(features, p, new FeatureGate(GameVersion.V1_8_9, RestrictionPolicy.NONE));
        boolean[] before = FeatureState.published();
        boolean[] copy = before.clone();
        FeatureState.withdraw(Arrays.<Feature>asList(features.get(0), new TestFeature("far", 5000, all)));
        assertNotSame(before, FeatureState.published());
        assertTrue(Arrays.equals(copy, before));
        assertFalse(FeatureState.on(1));
        assertTrue(FeatureState.on(2));
    }

    @Test void a_throw_in_the_middle_of_a_rewrite_leaves_the_published_state_intact() {
        List<Feature> features = Arrays.<Feature>asList(new TestFeature("a", 1, all), new TestFeature("b", 2, all));
        Profile p = new Profile();
        p.module("a").setEnabled(true);
        p.module("b").setEnabled(true);
        FeatureState.rewrite(features, p, new FeatureGate(GameVersion.V1_8_9, RestrictionPolicy.NONE));
        boolean[] before = FeatureState.published();
        p.module("a").setEnabled(false);
        FeatureGate throwing = new FeatureGate(GameVersion.V1_8_9, (f, server) -> {
            if (f.id().equals("b")) { throw new IllegalStateException("policy"); }
            return false;
        });
        assertThrows(IllegalStateException.class, () -> FeatureState.rewrite(features, p, throwing));
        assertSame(before, FeatureState.published());
        assertTrue(FeatureState.on(1));
        assertTrue(FeatureState.on(2));
    }
}
