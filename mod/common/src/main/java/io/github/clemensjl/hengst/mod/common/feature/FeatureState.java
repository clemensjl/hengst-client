package io.github.clemensjl.hengst.mod.common.feature;

import io.github.clemensjl.hengst.mod.common.config.Profile;
import java.util.List;

/**
 * Which features are on, indexed by feature number, for mixins: they read {@code FeatureState.on(n)} as their first
 * statement, so a disabled feature costs one volatile read and a bounds check. Readers may run on any thread (the
 * integrated server, netty, chunk meshing, sound). The only writer is the core on the client thread: it builds a new
 * array and publishes it with one assignment, so a reader sees the old or the new state, never a half-written one, and
 * a rewrite that throws leaves the published state as it was.
 */
public final class FeatureState {
    private static volatile boolean[] state = new boolean[64];

    private FeatureState() {}

    /** False for unknown and negative numbers. No allocation. */
    public static boolean on(int number) {
        boolean[] s = state;
        return number >= 0 && number < s.length && s[number];
    }

    /** The array readers currently see; never mutated after publishing. For tests of this package. */
    static boolean[] published() {
        return state;
    }

    /**
     * Only the core calls this, on the client thread: builds the state of the active features and publishes it.
     * Cold path; allocates one array per call. An exception (a throwing restriction policy) leaves the previous state.
     */
    public static void rewrite(List<Feature> features, Profile profile, FeatureGate gate) {
        int max = 0;
        for (Feature f : features) {
            max = Math.max(max, f.number());
        }
        boolean[] next = new boolean[max + 1];
        for (Feature f : features) {
            // supports() first: an unsupported feature gets no profile entry just because the state was rewritten
            if (f.number() >= 0 && f.supports(gate.version()) && gate.active(f, profile.module(f.id()))) {
                next[f.number()] = true;
            }
        }
        state = next;
    }
}
