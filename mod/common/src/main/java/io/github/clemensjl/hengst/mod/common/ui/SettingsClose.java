package io.github.clemensjl.hengst.mod.common.ui;

import io.github.clemensjl.hengst.mod.common.config.Profile;
import io.github.clemensjl.hengst.mod.common.hud.HudRenderer;
import io.github.clemensjl.hengst.mod.common.platform.Platform;
import java.util.function.Supplier;

/**
 * The close callback of the settings screen: saves pending changes, rebuilds the renderer and closes the screen, in
 * that order. Failures (also unchecked ones) are logged; a failed save leaves the model dirty, and the screen
 * always closes.
 */
public final class SettingsClose implements Runnable {
    private final SettingsModel model;
    private final HudRenderer renderer;
    private final Supplier<Profile> activeProfile;
    private final Platform platform;

    public SettingsClose(SettingsModel model, HudRenderer renderer, Supplier<Profile> activeProfile, Platform platform) {
        this.model = model;
        this.renderer = renderer;
        this.activeProfile = activeProfile;
        this.platform = platform;
    }

    @Override
    public void run() {
        try {
            try {
                model.saveIfDirty();
            } catch (RuntimeException e) {
                platform.logWarning("Could not save the settings: " + e);
            }
            try {
                renderer.rebuild(activeProfile.get());
            } catch (RuntimeException e) {
                platform.logWarning("Could not rebuild the HUD after closing the settings: " + e);
            }
        } finally {
            platform.closeScreen();
        }
    }
}
