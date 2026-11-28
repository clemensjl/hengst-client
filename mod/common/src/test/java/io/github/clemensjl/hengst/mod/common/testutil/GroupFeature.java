package io.github.clemensjl.hengst.mod.common.testutil;

import io.github.clemensjl.hengst.mod.common.feature.Feature;
import io.github.clemensjl.hengst.mod.common.feature.FeatureGroup;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import java.util.EnumSet;

/** A feature without behaviour in a chosen group. */
public final class GroupFeature extends Feature {
    public GroupFeature(String id, int number, FeatureGroup group) {
        this(id, number, group, EnumSet.allOf(GameVersion.class));
    }

    public GroupFeature(String id, int number, FeatureGroup group, EnumSet<GameVersion> versions) {
        super(id, number, group, versions);
    }
}
