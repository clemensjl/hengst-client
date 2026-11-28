package io.github.clemensjl.hengst.mod.common;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.platform.GameView;
import io.github.clemensjl.hengst.mod.common.platform.InputView;
import io.github.clemensjl.hengst.mod.common.platform.Platform;
import io.github.clemensjl.hengst.mod.common.platform.TimeSource;
import io.github.clemensjl.hengst.mod.common.platform.UiScreen;
import io.github.clemensjl.hengst.mod.common.testutil.FakePlatform;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class AdapterGuardTest {
    final List<String> lines = new ArrayList<>();
    final List<Throwable> errors = new ArrayList<>();
    final AdapterGuard guard = new AdapterGuard(new String[] {"HUD", "client tick"}, new AdapterGuard.Log() {
        public void info(String m) { lines.add("I " + m); }
        public void error(String m, Throwable t) { lines.add("E " + m); errors.add(t); }
    });

    @Test void a_failing_platform_gives_null_and_one_error() {
        assertNull(guard.start(() -> { throw new IllegalStateException("x"); }, TimeSource.SYSTEM));
        assertEquals(Arrays.asList("E [hengst] core failed to start; the mod stays inactive and its key bindings do nothing"), lines);
        assertTrue(errors.get(0) instanceof IllegalStateException);
    }

    @Test void linkage_errors_while_starting_give_null() {
        assertNull(guard.start(() -> { throw new NoClassDefFoundError("common class"); }, TimeSource.SYSTEM));
        assertNull(guard.start(() -> { throw new ExceptionInInitializerError("static init"); }, TimeSource.SYSTEM));
        assertEquals(2, lines.size());
    }

    @Test void vm_errors_are_not_swallowed() {
        OutOfMemoryError oom = new OutOfMemoryError("test");
        assertSame(oom, assertThrows(OutOfMemoryError.class, () -> guard.start(() -> { throw oom; }, TimeSource.SYSTEM)));
        StackOverflowError so = new StackOverflowError();
        assertSame(so, assertThrows(StackOverflowError.class, () -> guard.start(() -> { throw so; }, TimeSource.SYSTEM)));
        assertTrue(lines.isEmpty());
    }

    @Test void a_failure_inside_the_core_start_gives_null() {
        // HengstCore.start asks the platform for its config directory first.
        assertNull(guard.start(FailingPlatform::new, TimeSource.SYSTEM));
        assertEquals(2, lines.size());
        assertEquals("I [hengst] loaded on " + GameVersion.V26_3.id(), lines.get(0));
        assertTrue(lines.get(1).startsWith("E [hengst] core failed to start"));
    }

    @Test void a_working_platform_gives_a_core_and_one_info_line(@TempDir Path dir) {
        HengstCore core = guard.start(() -> new FakePlatform(dir), TimeSource.SYSTEM);
        assertNotNull(core);
        assertEquals(1, lines.size());
        assertTrue(lines.get(0).startsWith("I [hengst] loaded on "));
    }

    @Test void a_linkage_error_disables_and_logs_once() {
        assertTrue(guard.hookFailed(0, new NoSuchMethodError("m")));
        assertTrue(guard.hookFailed(0, new NoSuchMethodError("m")));
        assertTrue(guard.hookFailed(1, new NoClassDefFoundError("c")));
        assertEquals(Arrays.asList("E [hengst] the HUD hook does not match this game; the mod is disabled"), lines);
    }

    @Test void other_failures_log_once_per_site_and_class_up_to_four_classes() {
        for (int k = 0; k < 3; k++) { assertFalse(guard.hookFailed(1, new IllegalStateException())); }
        guard.hookFailed(1, new IllegalArgumentException()); guard.hookFailed(1, new ArithmeticException());
        guard.hookFailed(1, new NullPointerException()); guard.hookFailed(1, new IndexOutOfBoundsException());
        assertEquals(4, lines.size());
        assertEquals("E [hengst] the client tick hook failed; further IllegalStateException there are not logged", lines.get(0));
        assertEquals("E [hengst] the client tick hook failed; further NullPointerException there are not logged", lines.get(3));
    }

    @Test void sites_count_their_classes_separately() {
        guard.hookFailed(0, new IllegalStateException());
        guard.hookFailed(1, new IllegalStateException());
        guard.hookFailed(0, new IllegalStateException());
        assertEquals(2, lines.size());
        assertTrue(lines.get(0).contains("the HUD hook"));
        assertTrue(lines.get(1).contains("the client tick hook"));
    }

    @Test void the_log_receives_the_original_throwable() {
        IllegalStateException e = new IllegalStateException("x");
        guard.hookFailed(0, e);
        assertSame(e, errors.get(0));
    }

    @Test void a_subclass_is_a_class_of_its_own() {
        guard.hookFailed(0, new RuntimeException());
        guard.hookFailed(0, new IllegalStateException());
        assertEquals(2, lines.size());
    }

    /** Throws from configDir(), the first thing the core asks for. */
    private static final class FailingPlatform implements Platform {
        @Override public GameVersion version() { return GameVersion.V26_3; }
        @Override public GameView game() { return null; }
        @Override public InputView input() { return null; }
        @Override public Path configDir() { throw new IllegalStateException("no config dir"); }
        @Override public String languageCode() { return "en_us"; }
        @Override public void logWarning(String message) { }
        @Override public void openScreen(UiScreen screen) { }
        @Override public void closeScreen() { }
    }
}
