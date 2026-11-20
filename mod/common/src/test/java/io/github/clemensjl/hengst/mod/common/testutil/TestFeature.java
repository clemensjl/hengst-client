package io.github.clemensjl.hengst.mod.common.testutil;

import io.github.clemensjl.hengst.mod.common.feature.Feature;
import io.github.clemensjl.hengst.mod.common.feature.FeatureGroup;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import java.util.ArrayList;
import java.util.EnumSet;
import java.util.List;

public class TestFeature extends Feature {
    public TestFeature(String id, int number, EnumSet<GameVersion> versions) {
        super(id, number, FeatureGroup.HUD, versions);
    }

    public static List<String> ids(List<? extends Feature> features) {
        List<String> ids = new ArrayList<String>();
        for (Feature f : features) {
            ids.add(f.id());
        }
        return ids;
    }
}
