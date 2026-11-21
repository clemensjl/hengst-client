package io.github.clemensjl.hengst.mod.modern;

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
import java.nio.file.Path;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

class HengstModernModTest {
    @AfterEach
    void reset() {
        HengstModernMod.ENABLED = false;
    }

    @Test
    void aRuntimeExceptionInStartGivesNoCore() {
        assertNull(HengstModernMod.startCore(() -> failingPlatform(new IllegalStateException("broken"))));
    }

    @Test
    void aLinkageErrorInStartGivesNoCore() {
        assertNull(HengstModernMod.startCore(() -> failingPlatform(new NoClassDefFoundError("common class"))));
        assertNull(HengstModernMod.startCore(() -> failingPlatform(new ExceptionInInitializerError("static init"))));
    }

    @Test
    void aFailingPlatformConstructionGivesNoCore() {
        assertNull(HengstModernMod.startCore(() -> {
            throw new NoClassDefFoundError("io/github/clemensjl/hengst/mod/common/platform/Platform");
        }));
    }

    @Test
    void virtualMachineErrorsAreNotSwallowed() {
        OutOfMemoryError oom = new OutOfMemoryError("test");
        assertSame(oom, assertThrows(OutOfMemoryError.class,
                () -> HengstModernMod.startCore(() -> failingPlatform(oom))));
    }

    @Test
    void aRuntimeExceptionInAHookKeepsTheModRunning() {
        HengstModernMod.ENABLED = true;
        HengstModernMod.hookFailed(HengstModernMod.SITE_FRAME, new IllegalStateException("one frame"));
        HengstModernMod.hookFailed(HengstModernMod.SITE_FRAME, new IllegalStateException("again"));
        assertTrue(HengstModernMod.ENABLED);
    }

    @Test
    void aLinkageErrorInAHookDisablesTheMod() {
        HengstModernMod.ENABLED = true;
        HengstModernMod.hookFailed(HengstModernMod.SITE_HUD, new NoSuchMethodError("GuiGraphics.fill"));
        assertFalse(HengstModernMod.ENABLED);
    }

    @Test
    void hooksNeverThrowIntoTheGame() {
        // No core (start failed or not started): the hook bodies catch what the missing core causes.
        assertDoesNotThrow(HengstModernMod::onFrame);
        assertDoesNotThrow(() -> HengstModernMod.onMouseButton(0, true));
    }

    /** HengstCore.start asks the platform for its config directory first; this one throws there. */
    private static Platform failingPlatform(Throwable failure) {
        return new Platform() {
            @Override
            public GameVersion version() {
                return GameVersion.V26_3;
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
                return "en_us";
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
