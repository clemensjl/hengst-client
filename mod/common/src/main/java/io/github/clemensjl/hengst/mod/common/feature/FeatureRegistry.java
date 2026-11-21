package io.github.clemensjl.hengst.mod.common.feature;

import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

public final class FeatureRegistry {
    private final Map<String, Feature> byId = new HashMap<String, Feature>();
    private final Map<Integer, Feature> byNumber = new HashMap<Integer, Feature>();

    private boolean sealed;

    /** Features are registered while the core is built; afterwards the registry is sealed and this throws. */
    public void register(Feature f) {
        if (sealed) {
            throw new IllegalStateException("the registry is sealed: features are registered before start");
        }
        if (byId.containsKey(f.id())) {
            throw new IllegalArgumentException("duplicate feature id: " + f.id());
        }
        if (byNumber.containsKey(f.number())) {
            throw new IllegalArgumentException("duplicate feature number: " + f.number());
        }
        byId.put(f.id(), f);
        byNumber.put(f.number(), f);
    }

    /** After this, {@link #register} throws. The renderer's active set is built from the registry once. */
    public void seal() {
        sealed = true;
    }

    /** Test access only: {@link HengstCore#registerForTest} unseals around one registration and rebuilds the HUD. */
    public void unseal() {
        sealed = false;
    }

    public Feature byId(String id) {
        return byId.get(id);
    }

    public List<Feature> supportedOn(GameVersion v) {
        List<Feature> out = new ArrayList<Feature>();
        for (Feature f : byId.values()) {
            if (f.supports(v)) {
                out.add(f);
            }
        }
        Collections.sort(out, new Comparator<Feature>() {
            @Override
            public int compare(Feature a, Feature b) {
                return Integer.compare(a.number(), b.number());
            }
        });
        return Collections.unmodifiableList(out);
    }
}
