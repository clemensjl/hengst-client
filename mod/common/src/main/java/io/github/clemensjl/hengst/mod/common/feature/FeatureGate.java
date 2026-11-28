package io.github.clemensjl.hengst.mod.common.feature;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;

/** The single question "does this feature run now": switched on, supported on this version and not restricted here. */
public final class FeatureGate {
    private final GameVersion version;
    private final RestrictionPolicy policy;
    private String server;

    public FeatureGate(GameVersion version, RestrictionPolicy policy) {
        this.version = version;
        this.policy = policy;
    }

    public boolean active(Feature f, ModuleConfig cfg) {
        return cfg.enabled() && f.supports(version) && !policy.restricted(f, server);
    }

    public void setServer(String server) {
        this.server = server;
    }

    public String server() {
        return server;
    }

    public GameVersion version() {
        return version;
    }
}
