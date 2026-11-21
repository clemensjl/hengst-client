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
 * never drop them silently: a failed save aborts the action and is reported through the warn hook and
 * {@link #failureKey()}.
 */
public final class SettingsModel {
    /** Language keys of the user-facing failure sentences; see {@link #failureKey()}. */
    public static final String KEY_SAVE_FAILED = "hengst.warn.save_failed";
    public static final String KEY_PROFILE_LOAD_FAILED = "hengst.warn.profile_load_failed";
    public static final String KEY_SETTINGS_LOAD_FAILED = "hengst.warn.settings_load_failed";
    public static final String KEY_SWITCH_FAILED = "hengst.warn.switch_failed";
    public static final String KEY_COPY_FAILED = "hengst.warn.copy_failed";
    public static final String KEY_DELETE_FAILED = "hengst.warn.delete_failed";
    public static final String KEY_EDITOR_FAILED = "hengst.warn.editor_failed";

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
    private String failureKey;
    // True after "default" was selected following a deleted profile but the switch hook failed: the active Profile
    // object still holds the deleted profile's data, so nothing may be saved until a switch succeeds.
    private boolean detached;

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
        try {
            changed.run();
        } catch (RuntimeException e) {
            log("The change hook failed: " + e);
        }
    }

    public boolean dirty() {
        return dirty;
    }

    /**
     * The language key of the most recent failure sentence for the user, or null. Set by a failed action, cleared
     * by the next successful profile action or save. The technical detail goes to the warn hook only.
     */
    public String failureKey() {
        return failureKey;
    }

    /** Records a failure: the user sentence key for the screen, the technical text for the warn hook. */
    public void fail(String key, String detail) {
        failureKey = key;
        warn.accept(detail);
    }

    /** Sends a technical message to the warn hook only. */
    public void log(String message) {
        warn.accept(message);
    }

    /** Saves if there are pending changes. A failed save is reported, keeps the state dirty and returns false. */
    public boolean saveIfDirty() {
        return !dirty || save();
    }

    /** A successful action clears its own warning; the two load-failure warnings stay until the screen closes. */
    private void clearFailure() {
        if (!KEY_PROFILE_LOAD_FAILED.equals(failureKey) && !KEY_SETTINGS_LOAD_FAILED.equals(failureKey)) {
            failureKey = null;
        }
    }

    private boolean save() {
        if (detached) {
            fail(KEY_SWITCH_FAILED, "Not saving: the switch to the default profile after a delete failed, "
                    + "the loaded data belongs to the deleted profile");
            return false;
        }
        try {
            saveNow.run();
            dirty = false;
            clearFailure();
            return true;
        } catch (RuntimeException e) {
            String name = activeProfileName();
            fail(profiles.loadFailed(name) ? KEY_PROFILE_LOAD_FAILED : KEY_SAVE_FAILED,
                    "Could not save profile " + name + ": " + e);
            return false;
        }
    }

    private boolean doSwitch(String name) {
        try {
            switchProfile.accept(name);
            return true;
        } catch (RuntimeException e) {
            fail(KEY_SWITCH_FAILED, "Could not switch to profile " + name + ": " + e);
            return false;
        }
    }

    /** Reports (the screen calls this once when it opens) if the active profile or settings.json failed to load. */
    public void warnLoadProblems() {
        String name = activeProfileName();
        if (profiles.loadFailed(name)) {
            fail(KEY_PROFILE_LOAD_FAILED,
                    "Profile " + name + " could not be read; the defaults are shown and the file will not be overwritten.");
        }
        if (settingsLoadFailed != null && settingsLoadFailed.getAsBoolean()) {
            fail(KEY_SETTINGS_LOAD_FAILED, "settings.json could not be read; the active profile choice will not be saved.");
        }
    }

    /**
     * Switches to another profile; pending changes are saved first and a failed save aborts the switch. If the
     * active profile could not be loaded its file is never overwritten: the switch then proceeds without saving.
     */
    public void switchTo(String name) {
        if (name == null || !detached && ProfileStore.sameName(name, activeProfileName())) {
            return;
        }
        boolean blocked = detached || profiles.loadFailed(activeProfileName());
        if (!blocked && !saveIfDirty()) {
            return;
        }
        boolean lost = blocked && dirty;
        String lostKey = detached ? KEY_SWITCH_FAILED : KEY_PROFILE_LOAD_FAILED;
        if (!doSwitch(name)) {
            return;
        }
        dirty = false;
        detached = false;
        if (lost) {
            fail(lostKey, "Dropped the unsaved changes of the unreadable profile when switching to " + name);
        } else {
            clearFailure();
        }
    }

    /** Copies the active profile (pending changes included) and switches to the copy. Returns the new name. */
    public String copyActive() {
        String current = activeProfileName();
        if (!save()) {
            return current;
        }
        String copy;
        try {
            copy = profiles.createCopy(current);
        } catch (IOException | RuntimeException e) {
            fail(KEY_COPY_FAILED, "Could not copy profile " + current + ": " + e.getMessage());
            return current;
        }
        if (!doSwitch(copy)) {
            return current;
        }
        clearFailure();
        return copy;
    }

    /**
     * Deletes the active profile and switches to "default" in the same call. The file goes first: if that fails
     * nothing changed and the pending edits stay. "default" and the last remaining profile are never deleted.
     * Pending edits of a deleted profile are dropped on purpose.
     */
    public void deleteActive() {
        String name = activeProfileName();
        if (ProfileStore.sameName(ProfileStore.DEFAULT, name) || profiles.names().size() <= 1) {
            return;
        }
        try {
            profiles.delete(name);
        } catch (IOException | RuntimeException e) {
            fail(KEY_DELETE_FAILED, "Could not delete profile " + name + ": " + e.getMessage());
            return;
        }
        // The file is gone: select "default" in memory and drop the deleted profile's edits before the hook runs, so
        // a failing hook cannot leave a state that writes the deleted data anywhere.
        settings.setActiveProfile(ProfileStore.DEFAULT);
        dirty = false;
        if (doSwitch(ProfileStore.DEFAULT)) {
            clearFailure();
        } else {
            detached = true;
        }
    }

    /** Saves pending changes (a failure is reported but does not block the editor), then opens the HUD editor. */
    public void openHudEditor() {
        saveIfDirty();
        openHudEditor.run();
    }
}
