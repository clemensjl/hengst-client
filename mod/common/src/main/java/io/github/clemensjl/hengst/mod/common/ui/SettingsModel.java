package io.github.clemensjl.hengst.mod.common.ui;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.config.Profile;
import io.github.clemensjl.hengst.mod.common.config.ProfileStore;
import io.github.clemensjl.hengst.mod.common.config.Settings;
import io.github.clemensjl.hengst.mod.common.feature.Feature;
import io.github.clemensjl.hengst.mod.common.feature.FeatureRegistry;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import java.io.IOException;
import java.util.List;
import java.util.function.BooleanSupplier;
import java.util.function.Consumer;
import java.util.function.Supplier;

/**
 * State and actions of the settings screen. Pending changes are tracked in {@link #markChanged}; profile actions
 * never drop them silently: a failed save aborts the action and is reported through the warn hook.
 */
public final class SettingsModel {
    private final ProfileStore profiles;
    private final Settings settings;
    private final Supplier<Profile> activeProfile;
    private final Consumer<String> switchProfile;
    private final Runnable changed;
    private final Runnable openHudEditor;
    private final Runnable saveNow;
    private final Consumer<String> warn;
    private final List<Feature> features;
    private Feature selected;
    private boolean dirty;
    private BooleanSupplier settingsLoadFailed;

    public SettingsModel(
            FeatureRegistry registry,
            GameVersion version,
            ProfileStore profiles,
            Settings settings,
            Supplier<Profile> activeProfile,
            Consumer<String> switchProfile,
            Runnable changed,
            Runnable openHudEditor,
            Runnable saveNow,
            Consumer<String> warn) {
        this.profiles = profiles;
        this.settings = settings;
        this.activeProfile = activeProfile;
        this.switchProfile = switchProfile;
        this.changed = changed;
        this.openHudEditor = openHudEditor;
        this.saveNow = saveNow;
        this.warn = warn;
        this.features = registry.supportedOn(version);
        this.selected = features.isEmpty() ? null : features.get(0);
    }

    /** Optional: lets the screen report a failed load of settings.json (the store lives in the core). */
    public void setSettingsLoadFailedCheck(BooleanSupplier check) {
        this.settingsLoadFailed = check;
    }

    public List<Feature> features() {
        return features;
    }

    public Feature selected() {
        return selected;
    }

    public void select(String id) {
        for (int i = 0; i < features.size(); i++) {
            if (features.get(i).id().equals(id)) {
                selected = features.get(i);
                return;
            }
        }
    }

    public ModuleConfig config(String id) {
        return activeProfile.get().module(id);
    }

    public List<String> profiles() {
        return profiles.names();
    }

    public String activeProfileName() {
        return settings.activeProfile();
    }

    /** Called by the UI after every change of a value: marks the profile dirty and calls the changed hook. */
    public void markChanged() {
        dirty = true;
        changed.run();
    }

    public boolean dirty() {
        return dirty;
    }

    /** Saves if there are pending changes. A failed save is reported, keeps the state dirty and returns false. */
    public boolean saveIfDirty() {
        return !dirty || save();
    }

    private boolean save() {
        try {
            saveNow.run();
            dirty = false;
            return true;
        } catch (RuntimeException e) {
            warn.accept("Could not save profile " + activeProfileName() + ": " + e);
            return false;
        }
    }

    /** Warns once (the screen calls this when it opens) if the active profile or settings.json failed to load. */
    public void warnLoadProblems() {
        String name = activeProfileName();
        if (profiles.loadFailed(name)) {
            warn.accept("Profile " + name + " could not be read; the defaults are shown and the file will not be overwritten.");
        }
        if (settingsLoadFailed != null && settingsLoadFailed.getAsBoolean()) {
            warn.accept("settings.json could not be read; the active profile choice will not be saved.");
        }
    }

    /** Switches to another profile; pending changes are saved first and a failed save aborts the switch. */
    public void switchTo(String name) {
        if (name == null || ProfileStore.sameName(name, activeProfileName())) {
            return;
        }
        if (!saveIfDirty()) {
            return;
        }
        switchProfile.accept(name);
        dirty = false;
    }

    /** Copies the active profile (pending changes included) and switches to the copy. Returns the new name. */
    public String copyActive() {
        String current = activeProfileName();
        if (!save()) {
            return current;
        }
        try {
            String copy = profiles.createCopy(current);
            switchProfile.accept(copy);
            return copy;
        } catch (IOException e) {
            warn.accept("Could not copy profile " + current + ": " + e.getMessage());
            return current;
        }
    }

    /**
     * Deletes the active profile: first switches to "default", then deletes the file. Pending changes of the deleted
     * profile are dropped on purpose. "default" and the last remaining profile are never deleted. If the delete
     * fails the previous profile is made active again (as stored on disk).
     */
    public void deleteActive() {
        String name = activeProfileName();
        if (ProfileStore.sameName(ProfileStore.DEFAULT, name) || profiles.names().size() <= 1) {
            return;
        }
        switchProfile.accept(ProfileStore.DEFAULT);
        dirty = false;
        try {
            profiles.delete(name);
        } catch (IOException e) {
            warn.accept("Could not delete profile " + name + ": " + e.getMessage());
            switchProfile.accept(name);
        }
    }

    /** Saves pending changes (a failure is reported but does not block the editor), then opens the HUD editor. */
    public void openHudEditor() {
        saveIfDirty();
        openHudEditor.run();
    }
}
