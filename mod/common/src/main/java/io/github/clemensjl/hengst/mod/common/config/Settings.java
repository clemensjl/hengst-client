package io.github.clemensjl.hengst.mod.common.config;

import java.util.LinkedHashMap;
import java.util.Map;

/** Global settings: which profile is active. */
public final class Settings {
    private String activeProfile = ProfileStore.DEFAULT;
    // keys other than "schema" and "activeProfile", kept so that a newer version's data is written back
    final Map<String, Object> extras = new LinkedHashMap<String, Object>();

    public String activeProfile() {
        return activeProfile;
    }

    /** @throws IllegalArgumentException if the name is not a valid profile name */
    public void setActiveProfile(String name) {
        if (!ProfileStore.isValidName(name)) {
            throw new IllegalArgumentException("invalid profile name: " + name);
        }
        this.activeProfile = name;
    }
}
