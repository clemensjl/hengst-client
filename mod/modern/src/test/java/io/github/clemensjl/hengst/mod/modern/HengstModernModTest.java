package io.github.clemensjl.hengst.mod.modern;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;

import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.platform.GameView;
import io.github.clemensjl.hengst.mod.common.platform.InputView;
import io.github.clemensjl.hengst.mod.common.platform.Platform;
import io.github.clemensjl.hengst.mod.common.platform.UiScreen;
import java.nio.file.Path;
import org.junit.jupiter.api.Test;

class HengstModernModTest {
    @Test
    void aRuntimeExceptionInStartLeavesTheModInert() {
        assertNull(HengstModernMod.startCore(failingPlatform(new IllegalStateException("broken"))));
        assertFalse(HengstModernMod.ENABLED);
    }

    @Test
    void aLinkageErrorInStartLeavesTheModInert() {
        assertNull(HengstModernMod.startCore(failingPlatform(new NoClassDefFoundError("common class"))));
        assertNull(HengstModernMod.startCore(failingPlatform(new ExceptionInInitializerError("static init"))));
        assertFalse(HengstModernMod.ENABLED);
    }

    @Test
    void virtualMachineErrorsAreNotSwallowed() {
        OutOfMemoryError oom = new OutOfMemoryError("test");
        assertSame(oom, assertThrows(OutOfMemoryError.class, () -> HengstModernMod.startCore(failingPlatform(oom))));
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
