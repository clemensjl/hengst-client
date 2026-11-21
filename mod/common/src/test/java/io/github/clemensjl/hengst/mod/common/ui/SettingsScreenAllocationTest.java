package io.github.clemensjl.hengst.mod.common.ui;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.I18n;
import io.github.clemensjl.hengst.mod.common.config.Profile;
import io.github.clemensjl.hengst.mod.common.config.ProfileStore;
import io.github.clemensjl.hengst.mod.common.config.Settings;
import io.github.clemensjl.hengst.mod.common.feature.FeatureGroup;
import io.github.clemensjl.hengst.mod.common.feature.FeatureRegistry;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.testutil.CountingCanvas;
import io.github.clemensjl.hengst.mod.common.testutil.GroupFeature;
import java.lang.management.ManagementFactory;
import java.nio.file.Path;
import java.time.Clock;
import java.time.Instant;
import java.time.ZoneOffset;
import org.junit.jupiter.api.Tag;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/** Runs with -Xint (task interpretedTest), where the thread's allocation count is exact. */
@Tag("interpreted")
class SettingsScreenAllocationTest {
    private static final com.sun.management.ThreadMXBean MX =
            (com.sun.management.ThreadMXBean) ManagementFactory.getThreadMXBean();

    @TempDir Path dir;

    /** Headers of all three groups and an overflowing list with the scroll indicator: steady frames allocate nothing. */
    @Test void frames_with_headers_and_the_scroll_indicator_allocate_nothing_after_the_first() {
        FeatureRegistry reg = new FeatureRegistry();
        FeatureGroup[] groups = FeatureGroup.values();
        for (int i = 0; i < 12; i++) {
            reg.register(new GroupFeature("f" + i, i + 1, groups[i % groups.length]));
        }
        Settings settings = new Settings();
        ProfileStore store = new ProfileStore(dir, Clock.fixed(Instant.parse("2026-11-20T10:15:30Z"), ZoneOffset.UTC), s -> {});
        Profile[] active = { new Profile() };
        SettingsModel model = new SettingsModel(reg, GameVersion.V1_8_9, store, settings, () -> active[0],
                name -> {}, () -> {}, () -> {}, () -> {}, s -> {});
        SettingsScreen screen = new SettingsScreen(model, I18n.load("en_us"), () -> {});
        CountingCanvas canvas = new CountingCanvas();
        screen.render(canvas, 50, 80, 0f);                 // first frame: layout and text fitting
        assertTrue(screen.featureList().overflows());
        screen.mouseScrolled(40, 100, -3);                 // a scrolled position: the thumb is not at the top
        screen.render(canvas, 50, 80, 0f);
        screen.render(canvas, 50, 80, 0f);
        int frames = 5000;
        long before = MX.getThreadAllocatedBytes(Thread.currentThread().getId());
        for (int i = 0; i < frames; i++) { screen.render(canvas, 50, 80, 0f); }
        long bytes = MX.getThreadAllocatedBytes(Thread.currentThread().getId()) - before;
        System.out.println("SettingsScreen allocation over " + frames + " steady frames: " + bytes + " bytes");
        assertEquals(0, bytes, "the screen allocated " + bytes + " bytes in " + frames + " steady frames");
    }
}
