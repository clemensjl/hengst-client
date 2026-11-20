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

    public void register(Feature f) {
        if (byId.containsKey(f.id())) {
            throw new IllegalArgumentException("duplicate feature id: " + f.id());
        }
        if (byNumber.containsKey(f.number())) {
            throw new IllegalArgumentException("duplicate feature number: " + f.number());
        }
        byId.put(f.id(), f);
        byNumber.put(f.number(), f);
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
