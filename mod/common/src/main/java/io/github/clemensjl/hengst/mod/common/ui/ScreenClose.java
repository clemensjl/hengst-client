package io.github.clemensjl.hengst.mod.common.ui;

import io.github.clemensjl.hengst.mod.common.platform.Platform;
import java.io.IOException;

/**
 * The close callback of the settings screen and of the HUD editor: saves, rebuilds, then closes the screen, in that
 * order. A failure of either step is logged and does not stop the next one; the screen always closes.
 */
public final class ScreenClose implements Runnable {
    /** The save step. */
    public interface Save {
        void run() throws IOException;
    }

    private final Save save;
    private final Runnable rebuild;
    private final Platform platform;
    private final String what;

    /** {@code what} names the screen in the log texts, for example {@code "the settings"}. */
    public ScreenClose(Save save, Runnable rebuild, Platform platform, String what) {
        this.save = save;
        this.rebuild = rebuild;
        this.platform = platform;
        this.what = what;
    }

    @Override
    public void run() {
        try {
            try {
                save.run();
            } catch (IOException | RuntimeException e) {
                platform.logWarning("Could not save " + what + ": " + e);
            }
            try {
                rebuild.run();
            } catch (RuntimeException e) {
                platform.logWarning("Could not rebuild the HUD after closing " + what + ": " + e);
            }
        } finally {
            platform.closeScreen();
        }
    }
}
