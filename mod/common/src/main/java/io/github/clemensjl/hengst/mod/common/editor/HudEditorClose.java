package io.github.clemensjl.hengst.mod.common.editor;

import io.github.clemensjl.hengst.mod.common.config.Profile;
import io.github.clemensjl.hengst.mod.common.config.ProfileStore;
import io.github.clemensjl.hengst.mod.common.hud.HudRenderer;
import io.github.clemensjl.hengst.mod.common.platform.Platform;
import java.io.IOException;

/**
 * The close callback of the HUD editor: saves the profile (only if the layout changed), rebuilds the renderer and
 * closes the screen, in that order. A failed save is logged; the layout stays in memory and dirty, and the screen
 * still closes.
 */
public final class HudEditorClose implements Runnable {
    private final HudEditorModel model;
    private final ProfileStore store;
    private final String profileName;
    private final Profile profile;
    private final HudRenderer renderer;
    private final Platform platform;

    public HudEditorClose(
            HudEditorModel model,
            ProfileStore store,
            String profileName,
            Profile profile,
            HudRenderer renderer,
            Platform platform) {
        this.model = model;
        this.store = store;
        this.profileName = profileName;
        this.profile = profile;
        this.renderer = renderer;
        this.platform = platform;
    }

    @Override
    public void run() {
        try {
            if (model.dirty()) {
                try {
                    store.save(profileName, profile);
                    model.markSaved();
                } catch (IOException | RuntimeException e) {
                    platform.logWarning("Could not save the HUD layout of profile " + profileName + ": " + e);
                }
            }
            try {
                renderer.rebuild(profile);
            } catch (RuntimeException e) {
                platform.logWarning("Could not rebuild the HUD after editing: " + e);
            }
        } finally {
            platform.closeScreen();
        }
    }
}
