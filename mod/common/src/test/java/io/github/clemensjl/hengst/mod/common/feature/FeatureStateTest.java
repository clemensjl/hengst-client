package io.github.clemensjl.hengst.mod.common.feature;

import static org.junit.jupiter.api.Assertions.assertFalse;
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
        assertTrue("mc.example.net".equals(seen[0]) && "mc.example.net".equals(restricted.server()));
    }
}
