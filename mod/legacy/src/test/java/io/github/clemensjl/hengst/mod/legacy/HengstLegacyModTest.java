package io.github.clemensjl.hengst.mod.legacy;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.platform.GameView;
import io.github.clemensjl.hengst.mod.common.platform.InputView;
import io.github.clemensjl.hengst.mod.common.platform.Platform;
import io.github.clemensjl.hengst.mod.common.platform.UiScreen;
import io.github.clemensjl.hengst.mod.common.HengstCore;
import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import java.nio.file.Files;
import java.nio.file.Path;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class HengstLegacyModTest {
    @AfterEach
    void reset() {
        HengstLegacyMod.ENABLED = false;
    }

    @Test
    void aRuntimeExceptionInStartGivesNoCore() {
        assertNull(HengstLegacyMod.startCore(() -> failingPlatform(new IllegalStateException("broken"))));
    }

    @Test
    void aLinkageErrorInStartGivesNoCore() {
        assertNull(HengstLegacyMod.startCore(() -> failingPlatform(new NoClassDefFoundError("common class"))));
        assertNull(HengstLegacyMod.startCore(() -> failingPlatform(new ExceptionInInitializerError("static init"))));
    }

    @Test
    void aFailingPlatformConstructionGivesNoCore() {
        assertNull(HengstLegacyMod.startCore(() -> {
            throw new NoClassDefFoundError("io/github/clemensjl/hengst/mod/common/platform/Platform");
        }));
    }

    @Test
    void virtualMachineErrorsAreNotSwallowed() {
        OutOfMemoryError oom = new OutOfMemoryError("test");
        assertSame(oom, assertThrows(OutOfMemoryError.class,
                () -> HengstLegacyMod.startCore(() -> failingPlatform(oom))));
    }

    @Test
    void aRuntimeExceptionInAHookKeepsTheModRunning() {
        HengstLegacyMod.ENABLED = true;
        HengstLegacyMod.hookFailed(HengstLegacyMod.SITE_FRAME, new IllegalStateException("one frame"));
        HengstLegacyMod.hookFailed(HengstLegacyMod.SITE_FRAME, new IllegalStateException("again"));
        assertTrue(HengstLegacyMod.ENABLED);
    }

    @Test
    void aLinkageErrorInAHookDisablesTheMod() {
        HengstLegacyMod.ENABLED = true;
        HengstLegacyMod.hookFailed(HengstLegacyMod.SITE_HUD, new NoSuchMethodError("DrawableHelper.fill"));
        assertFalse(HengstLegacyMod.ENABLED);
    }

    @Test
    void hooksNeverThrowIntoTheGame() {
        // No core (start failed or not started): the hook bodies catch what the missing core causes.
        assertDoesNotThrow(HengstLegacyMod::onFrame);
        assertDoesNotThrow(() -> HengstLegacyMod.onMouseButton(0, true));
    }

    @Test
    void stopWritesPendingChangesEvenAfterAHookFailureSwitchedTheModOff(@TempDir Path dir) throws Exception {
        UiScreen[] opened = new UiScreen[1];
        HengstCore core = HengstLegacyMod.startCore(() -> workingPlatform(dir, opened));
        core.onOpenSettingsKey();
        opened[0].render(CANVAS, 0, 0, 0f);
        opened[0].mouseClicked(12, 46, 0);                        // the FPS switch: a change waiting to be saved
        Path profile = dir.resolve("hengst/profiles/default.json");
        assertFalse(Files.exists(profile));
        HengstLegacyMod.ENABLED = true;
        HengstLegacyMod.hookFailed(HengstLegacyMod.SITE_HUD, new NoSuchMethodError("DrawableHelper.fill"));
        assertFalse(HengstLegacyMod.ENABLED);                                 // what a linkage error does to the flag
        HengstLegacyMod.stopCore(core);
        assertTrue(Files.exists(profile));
    }

    @Test
    void stopWithoutACoreDoesNothing() {
        assertDoesNotThrow(() -> HengstLegacyMod.stopCore(null));
    }

    private static final Canvas CANVAS = new Canvas() {
        @Override public int width() { return 320; }
        @Override public int height() { return 240; }
        @Override public int fontHeight() { return 9; }
        @Override public int textWidth(String text) { return 6 * text.length(); }
        @Override public void fill(int x1, int y1, int x2, int y2, int argb) { }
        @Override public void text(String text, int x, int y, int argb, boolean shadow) { }
        @Override public void pushTransform(float translateX, float translateY, float scale) { }
        @Override public void popTransform() { }
    };

    /** A platform that works: a world, an open screen is kept in {@code opened[0]}, a temporary config directory. */
    private static Platform workingPlatform(Path dir, UiScreen[] opened) {
        GameView view = new GameView() {
            @Override public boolean inWorld() { return true; }
            @Override public double playerX() { return 0; }
            @Override public double playerY() { return 0; }
            @Override public double playerZ() { return 0; }
            @Override public float playerYaw() { return 0; }
            @Override public boolean hudHidden() { return false; }
            @Override public boolean debugOverlayOpen() { return false; }
            @Override public boolean screenOpen() { return opened[0] != null; }
            @Override public String serverAddress() { return null; }
        };
        return new Platform() {
            @Override public GameVersion version() { return GameVersion.V1_8_9; }
            @Override public GameView game() { return view; }
            @Override public InputView input() { return null; }
            @Override public Path configDir() { return dir; }
            @Override public String languageCode() { return "en_us"; }
            @Override public void logWarning(String message) { }
            @Override public void openScreen(UiScreen screen) { opened[0] = screen; }
            @Override public void closeScreen() { opened[0] = null; }
        };
    }

    /** HengstCore.start asks the platform for its config directory first; this one throws there. */
    private static Platform failingPlatform(final Throwable failure) {
        return new Platform() {
            @Override
            public GameVersion version() {
                return GameVersion.V1_8_9;
            }

            @Override
            public GameView game() {
                return null;
            }

            @Override
            public InputView input() {
                return null;
            }

            @Override
            public Path configDir() {
                if (failure instanceof RuntimeException) {
                    throw (RuntimeException) failure;
                }
                throw (Error) failure;
            }

            @Override
            public String languageCode() {
                return "en_US";
            }

            @Override
            public void logWarning(String message) {
            }

            @Override
            public void openScreen(UiScreen screen) {
            }

            @Override
            public void closeScreen() {
            }
        };
    }
}
