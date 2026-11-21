package io.github.clemensjl.hengst.mod.common.feature;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import java.util.Collections;
import java.util.EnumSet;
import java.util.List;

public abstract class Feature {
    private final String id;
    private final int number;
    private final FeatureGroup group;
    private final EnumSet<GameVersion> versions;

    protected Feature(String id, int number, FeatureGroup group, EnumSet<GameVersion> versions) {
        this.id = id;
        this.number = number;
        this.group = group;
        this.versions = EnumSet.copyOf(versions);
    }

    public final String id() {
        return id;
    }

    public final int number() {
        return number;
    }

    public final FeatureGroup group() {
        return group;
    }

    public final boolean supports(GameVersion v) {
        return versions.contains(v);
    }

    public List<OptionSpec> options() {
        return Collections.emptyList();
    }

    /**
     * Called by the core, once per pass and with this feature's own configuration, for every feature supported on
     * this version: at start, after the settings and the HUD editor close, on a profile switch and when the server
     * changes (join, leave, switch). Each pass also rebuilds the HUD and rewrites {@link FeatureState}. Features
     * without a HUD copy their options into primitive fields here; HUD modules keep reading in {@code tick}. An
     * exception or linkage error is logged once and stops nothing, but the feature is off in {@link FeatureState}
     * until a pass in which this method returns normally. During the call {@link FeatureState} still shows the values
     * from before the change: do not read it here.
     */
    public void onConfigChanged(ModuleConfig cfg) {}
}
