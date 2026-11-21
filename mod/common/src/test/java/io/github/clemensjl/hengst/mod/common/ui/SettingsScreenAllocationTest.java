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
        // Every text is short ("ok") except the three group names, which are far too long for their 128 px row: only the
        // headers need a cut, so only their fit cache is under test.
        java.util.Map<String, Object> texts = new java.util.HashMap<String, Object>() {
            @Override public Object get(Object key) {
                return ((String) key).startsWith("hengst.group.") ? "A very long group name that cannot fit into its row at all" : "ok";
            }
        };
        SettingsScreen screen = new SettingsScreen(model, I18n.of(texts, new java.util.HashMap<String, Object>()), () -> {});
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

    /** The hint of the stopwatch, wrapped, with the option rows below it: steady frames allocate nothing. */
    @Test void frames_with_a_visible_hint_allocate_nothing_after_the_first() {
        FeatureRegistry reg = new FeatureRegistry();
        reg.register(new io.github.clemensjl.hengst.mod.common.hud.modules.StopwatchModule());
        Settings settings = new Settings();
        ProfileStore store = new ProfileStore(dir, Clock.fixed(Instant.parse("2026-11-20T10:15:30Z"), ZoneOffset.UTC), s -> {});
        Profile[] active = { new Profile() };
        SettingsModel model = new SettingsModel(reg, GameVersion.V1_8_9, store, settings, () -> active[0],
                name -> {}, () -> {}, () -> {}, () -> {}, s -> {});
        SettingsScreen screen = new SettingsScreen(model, I18n.load("de_de"), () -> {});
        io.github.clemensjl.hengst.mod.common.testutil.FakeCanvas fake = new io.github.clemensjl.hengst.mod.common.testutil.FakeCanvas();
        screen.render(fake, 50, 80, 0f);
        screen.mouseClicked(60, 36 + 20 + 5, 0);           // the stopwatch (row 0 is the Comfort header)
        CountingCanvas canvas = new CountingCanvas();
        screen.render(canvas, 50, 80, 0f);                 // the hint is wrapped on this frame
        screen.render(canvas, 50, 80, 0f);
        int textsPerFrame = canvas.texts;
        int frames = 5000;
        long before = MX.getThreadAllocatedBytes(Thread.currentThread().getId());
        for (int i = 0; i < frames; i++) { screen.render(canvas, 50, 80, 0f); }
        long bytes = MX.getThreadAllocatedBytes(Thread.currentThread().getId()) - before;
        assertTrue(textsPerFrame > 0);
        assertEquals(0, bytes, "the screen allocated " + bytes + " bytes in " + frames + " steady frames with a hint");
    }
}
