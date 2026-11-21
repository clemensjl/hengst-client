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
import java.nio.file.Path;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

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
