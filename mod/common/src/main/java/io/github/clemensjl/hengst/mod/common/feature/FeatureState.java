package io.github.clemensjl.hengst.mod.common.feature;

import io.github.clemensjl.hengst.mod.common.config.Profile;
import java.util.Arrays;
import java.util.List;

/**
 * Which features are on, indexed by feature number, for mixins: they read {@code FeatureState.on(n)} as their first
 * statement, so a disabled feature costs one array read. Client thread only; written only by the core.
 */
public final class FeatureState {
    private static boolean[] state = new boolean[64];

    private FeatureState() {}

    /** False for unknown and negative numbers. No allocation. */
    public static boolean on(int number) {
        boolean[] s = state;
        return number >= 0 && number < s.length && s[number];
    }

    /** Only the core calls this: sets every number to false, then the active ones to true. Cold path. */
    public static void rewrite(List<Feature> features, Profile profile, FeatureGate gate) {
        int max = 0;
        for (Feature f : features) {
            max = Math.max(max, f.number());
        }
        boolean[] s = state;
        if (max >= s.length) {
            s = new boolean[max + 1];
        } else {
            Arrays.fill(s, false);
        }
        for (Feature f : features) {
            // supports() first: an unsupported feature gets no profile entry just because the state was rewritten
            if (f.number() >= 0 && f.supports(gate.version()) && gate.active(f, profile.module(f.id()))) {
                s[f.number()] = true;
            }
        }
        state = s;
    }
}
