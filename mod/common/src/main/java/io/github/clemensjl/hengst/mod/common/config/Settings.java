package io.github.clemensjl.hengst.mod.common.config;

/** Global settings: which profile is active. */
public final class Settings {
    private String activeProfile = ProfileStore.DEFAULT;

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
