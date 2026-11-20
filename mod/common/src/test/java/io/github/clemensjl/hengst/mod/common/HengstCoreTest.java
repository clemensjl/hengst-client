package io.github.clemensjl.hengst.mod.common;

import static java.nio.charset.StandardCharsets.UTF_8;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.config.ProfileStore;
import io.github.clemensjl.hengst.mod.common.feature.OptionSpec;
import io.github.clemensjl.hengst.mod.common.layout.Anchor;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.platform.UiKey;
import io.github.clemensjl.hengst.mod.common.platform.UiScreen;
import io.github.clemensjl.hengst.mod.common.testutil.CountingCanvas;
import io.github.clemensjl.hengst.mod.common.testutil.FakeCanvas;
import io.github.clemensjl.hengst.mod.common.testutil.FakePlatform;
import io.github.clemensjl.hengst.mod.common.testutil.FakeTime;
import io.github.clemensjl.hengst.mod.common.testutil.FixedSizeModule;
import io.github.clemensjl.hengst.mod.common.testutil.TestFeature;
import java.lang.management.ManagementFactory;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Clock;
import java.time.Instant;
import java.time.ZoneOffset;
import java.util.EnumSet;
import java.util.List;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class HengstCoreTest {
    private static final com.sun.management.ThreadMXBean MX =
            (com.sun.management.ThreadMXBean) ManagementFactory.getThreadMXBean();

    // Screen geometry on the 320 x 240 FakeCanvas: the FPS switch in the feature list and the Done button.
    private static final int FPS_SWITCH_X = 12;
    private static final int FPS_SWITCH_Y = 46;
    private static final int DONE_X = 260;
    private static final int DONE_Y = 222;
    private static final int HEADER_Y = 18;
    private static final int NEW_PROFILE_X = 120;
    private static final int EDIT_HUD_X = 270;

    @TempDir Path dir;
    FakePlatform platform;
    FakeTime time;
    FakeCanvas canvas;
    Clock clock;
    HengstCore core;

    Path profiles() { return dir.resolve("hengst/profiles"); }
    Path defaultFile() { return profiles().resolve("default.json"); }
    HengstCore start() { return HengstCore.start(platform, time, clock); }
    ProfileStore reread() { return new ProfileStore(profiles(), clock, platform.warnings::add); }

    @BeforeEach void setUp() {
        platform = new FakePlatform(dir); time = new FakeTime(); canvas = new FakeCanvas();
        clock = Clock.fixed(Instant.parse("2026-11-20T10:15:30Z"), ZoneOffset.UTC);
    }

    // ---- start ----

    @Test void first_start_enables_m0_modules_and_writes_nothing_yet() {
        core = start();
        assertEquals(4, core.renderer().activeCount());
        assertFalse(Files.exists(defaultFile()));
        assertFalse(Files.exists(dir.resolve("hengst/settings.json")));
        core.shutdown();
        assertFalse(Files.exists(defaultFile()));
    }

    @Test void first_start_uses_the_default_places() {
        core = start();
        assertEquals(Anchor.TOP_LEFT, core.activeProfile().module("fps").anchor());
        assertEquals(2, core.activeProfile().module("fps").offsetX());
        assertEquals(2, core.activeProfile().module("fps").offsetY());
        assertEquals(16, core.activeProfile().module("cps").offsetY());
        assertEquals(30, core.activeProfile().module("coordinates").offsetY());
        assertEquals(Anchor.BOTTOM_LEFT, core.activeProfile().module("keystrokes").anchor());
        assertEquals(-2, core.activeProfile().module("keystrokes").offsetY());
    }

    @Test void a_normal_start_enables_and_moves_nothing() throws Exception {
        core = start();
        core.activeProfile().module("fps").setEnabled(false);
        core.activeProfile().module("cps").setOffsetX(77);
        platform.inWorld = true;
        core.onOpenSettingsKey();
        platform.openedScreen.render(canvas, 0, 0, 0f);
        platform.openedScreen.mouseClicked(FPS_SWITCH_X, FPS_SWITCH_Y, 0);   // FPS on again
        platform.openedScreen.mouseClicked(FPS_SWITCH_X, FPS_SWITCH_Y, 0);   // and off: a change that is saved
        platform.openedScreen.keyPressed(UiKey.ESCAPE, false, false);
        assertTrue(Files.exists(defaultFile()));
        core = start();
        assertFalse(core.activeProfile().module("fps").enabled());
        assertEquals(77, core.activeProfile().module("cps").offsetX());
        assertEquals(3, core.renderer().activeCount());
    }

    @Test void broken_config_logs_one_warning_and_starts() throws Exception {
        Files.createDirectories(profiles());
        Files.write(defaultFile(), "{\"schema\":1,\"modules\":{".getBytes(UTF_8));
        core = start();
        assertEquals(1, platform.warnings.size());
        assertEquals(4, core.renderer().activeCount());   // the broken file was moved aside, the M0 defaults apply
        assertTrue(Files.exists(profiles().resolve("default.json.broken-20261120-101530")));
    }

    @Test void a_profile_from_a_newer_version_is_neither_replaced_nor_treated_as_first_start() throws Exception {
        Files.createDirectories(profiles());
        String newer = "{\"schema\":999,\"modules\":{\"fps\":{\"enabled\":true}}}";
        Files.write(defaultFile(), newer.getBytes(UTF_8));
        core = start();
        assertEquals(0, core.renderer().activeCount());   // no first start: nothing is enabled behind the user's back
        core.onClientTick();
        time.advanceMillis(5000);
        core.onClientTick();
        core.shutdown();
        assertEquals(newer, new String(Files.readAllBytes(defaultFile()), UTF_8));
        assertEquals(1, platform.warnings.size());
    }

    @Test void unsupported_module_never_renders() {
        core = start();   // FakePlatform.version is V1_8_9
        core.registry().register(new FixedSizeModule("future", 99, 10, 10, EnumSet.of(GameVersion.V26_3)));
        core.activeProfile().module("future").setEnabled(true);
        core.renderer().rebuild(core.activeProfile());
        assertEquals(4, core.renderer().activeCount());
    }

    // ---- rendering ----

    @Test void hud_not_rendered_when_hidden() {
        core = start(); platform.inWorld = true; platform.hudHidden = true;
        core.onHudRender(canvas, 0f);
        assertTrue(canvas.calls.isEmpty());
    }

    @Test void hud_not_rendered_outside_world() {
        core = start(); platform.inWorld = false; platform.hudHidden = false;
        core.onHudRender(canvas, 0f);
        assertTrue(canvas.calls.isEmpty());
    }

    @Test void hud_stays_visible_under_a_vanilla_screen() {
        core = start(); platform.inWorld = true; platform.screenOpen = true;
        core.onClientTick(); core.onHudRender(canvas, 0f);
        assertFalse(canvas.calls.isEmpty());
    }

    @Test void hud_not_rendered_while_own_screen_open() {
        core = start(); platform.inWorld = true;
        core.onClientTick(); core.onHudRender(canvas, 0f);
        assertFalse(canvas.calls.isEmpty());
        canvas.calls.clear();
        core.onOpenSettingsKey();
        assertNotNull(platform.openedScreen);
        core.onHudRender(canvas, 0f);
        assertTrue(canvas.calls.isEmpty());
        assertTrue(platform.openedScreen.keyPressed(UiKey.ESCAPE, false, false));
        assertEquals(1, platform.closeCount);
        core.onHudRender(canvas, 0f);
        assertFalse(canvas.calls.isEmpty());
    }

    @Test void frames_are_counted_also_when_nothing_is_drawn() {
        core = start(); platform.inWorld = true; platform.hudHidden = true;
        for (int i = 0; i < 45; i++) { core.onHudRender(canvas, 0f); time.advanceMillis(33); }
        assertTrue(canvas.calls.isEmpty());
        assertTrue(core.context().frames().fps() > 20);
    }

    // ---- input ----

    @Test void clicks_ignored_while_screen_open() {
        core = start();
        platform.inWorld = true; platform.screenOpen = true; core.onMouseButton(0, true);
        platform.screenOpen = false; core.onMouseButton(0, true); core.onMouseButton(0, false);
        assertEquals(1, core.context().cps().cps(0));
    }

    @Test void a_held_button_counts_once_until_released() {
        core = start(); platform.inWorld = true;
        core.onMouseButton(0, true); core.onMouseButton(0, true);
        assertEquals(1, core.context().cps().cps(0));
        core.onMouseButton(0, false); core.onMouseButton(0, true);
        assertEquals(2, core.context().cps().cps(0));
    }

    @Test void left_and_right_are_counted_separately_and_other_buttons_ignored() {
        core = start(); platform.inWorld = true;
        core.onMouseButton(0, true); core.onMouseButton(1, true); core.onMouseButton(1, false);
        core.onMouseButton(1, true); core.onMouseButton(2, true);
        assertEquals(1, core.context().cps().cps(0));
        assertEquals(2, core.context().cps().cps(1));
    }

    @Test void clicks_outside_a_world_are_ignored() {
        core = start(); platform.inWorld = false;
        core.onMouseButton(0, true); core.onMouseButton(0, false);
        assertEquals(0, core.context().cps().cps(0));
    }

    @Test void a_release_swallowed_by_a_screen_does_not_hide_the_next_click() {
        core = start(); platform.inWorld = true;
        core.onMouseButton(0, true);                       // held, then a screen opens and takes the release
        platform.screenOpen = true; core.onClientTick();
        platform.screenOpen = false;
        core.onMouseButton(0, true);
        assertEquals(2, core.context().cps().cps(0));
    }

    // ---- closing screens ----

    private void openSettingsWithChange() {
        platform.inWorld = true;
        core.onOpenSettingsKey();
        platform.openedScreen.render(canvas, 0, 0, 0f);
        platform.openedScreen.mouseClicked(FPS_SWITCH_X, FPS_SWITCH_Y, 0);   // FPS off: dirty
    }

    private void assertClosedOnceAndSavedOnce(UiScreen screen) throws Exception {
        assertEquals(1, platform.closeCount);
        assertNull(platform.openedScreen);
        assertTrue(Files.exists(defaultFile()));
        assertFalse(reread().load("default").module("fps").enabled());
        // Nothing is pending any more: a second close signal, the tick and shutdown write nothing.
        Files.delete(defaultFile());
        screen.closed();
        time.advanceMillis(5000);
        core.onClientTick();
        core.shutdown();
        assertEquals(1, platform.closeCount);
        assertFalse(Files.exists(defaultFile()));
        canvas.calls.clear();
        core.onHudRender(canvas, 0f);                      // unsuppressed: the three remaining modules draw
        assertFalse(canvas.calls.isEmpty());
    }

    @Test void settings_close_through_escape() throws Exception {
        core = start(); openSettingsWithChange();
        UiScreen s = platform.openedScreen;
        assertTrue(s.keyPressed(UiKey.ESCAPE, false, false));
        assertClosedOnceAndSavedOnce(s);
    }

    @Test void settings_close_through_the_done_button() throws Exception {
        core = start(); openSettingsWithChange();
        UiScreen s = platform.openedScreen;
        assertTrue(s.mouseClicked(DONE_X, DONE_Y, 0));
        assertClosedOnceAndSavedOnce(s);
    }

    @Test void settings_close_through_the_platform_calling_closed() throws Exception {
        core = start(); openSettingsWithChange();
        UiScreen s = platform.openedScreen;
        s.closed();
        assertClosedOnceAndSavedOnce(s);
    }

    @Test void the_editor_closes_once_through_escape_and_through_the_platform() throws Exception {
        core = start(); platform.inWorld = true; core.onClientTick();
        core.onOpenHudEditorKey();
        UiScreen editor = platform.openedScreen;
        editor.render(canvas, 0, 0, 0f);
        editor.mouseClicked(5, 5, 0); editor.mouseReleased(5, 5, 0);
        editor.keyPressed(UiKey.DELETE, false, false);     // hides FPS: a change
        editor.keyPressed(UiKey.ESCAPE, false, false);
        editor.closed();
        assertEquals(1, platform.closeCount);
        assertFalse(reread().load("default").module("fps").enabled());
        Files.delete(defaultFile());
        time.advanceMillis(5000); core.onClientTick(); core.shutdown();
        assertFalse(Files.exists(defaultFile()));
    }

    @Test void opening_a_second_screen_does_nothing() {
        core = start(); platform.inWorld = true;
        core.onOpenSettingsKey();
        UiScreen first = platform.openedScreen;
        core.onOpenHudEditorKey(); core.onOpenSettingsKey();
        assertEquals(first, platform.openedScreen);
    }

    @Test void the_editor_opened_from_the_settings_survives_the_old_screen_closing() throws Exception {
        core = start(); openSettingsWithChange();
        UiScreen settings = platform.openedScreen;
        assertTrue(settings.mouseClicked(EDIT_HUD_X, HEADER_Y, 0));
        UiScreen editor = platform.openedScreen;
        assertNotNull(editor);
        assertTrue(editor != settings);
        settings.closed();                                 // the platform tells the replaced screen it was closed
        assertEquals(0, platform.closeCount);
        assertEquals(editor, platform.openedScreen);
        canvas.calls.clear();
        core.onHudRender(canvas, 0f);
        assertTrue(canvas.calls.isEmpty());                // still suppressed under the editor
        assertTrue(Files.exists(defaultFile()));           // the settings change was saved before the editor opened
        editor.keyPressed(UiKey.ESCAPE, false, false);
        assertEquals(1, platform.closeCount);
        core.onHudRender(canvas, 0f);
        assertFalse(canvas.calls.isEmpty());
    }

    // ---- saving ----

    @Test void editor_change_is_saved_on_close() {
        core = start(); platform.inWorld = true; core.onClientTick();   // FPS text "0 FPS": 34 x 13 at (2, 2)
        core.onOpenHudEditorKey();
        UiScreen editor = platform.openedScreen;
        editor.render(canvas, 0, 0, 0f);
        editor.mouseClicked(5, 5, 0); editor.mouseDragged(105, 205, 0); editor.mouseReleased(105, 205, 0);
        assertTrue(editor.keyPressed(UiKey.ESCAPE, false, false));
        assertEquals(1, platform.closeCount);
        assertEquals(Anchor.BOTTOM_CENTER, reread().load("default").module("fps").anchor());
    }

    @Test void an_editor_change_is_saved_by_the_tick_while_the_editor_is_open() {
        core = start(); platform.inWorld = true; core.onClientTick();
        core.onOpenHudEditorKey();
        UiScreen editor = platform.openedScreen;
        editor.render(canvas, 0, 0, 0f);
        editor.mouseClicked(5, 5, 0); editor.mouseDragged(105, 205, 0); editor.mouseReleased(105, 205, 0);
        time.advanceMillis(2100); core.onClientTick();
        assertEquals(Anchor.BOTTOM_CENTER, reread().load("default").module("fps").anchor());
    }

    @Test void saves_are_debounced_to_two_seconds() {
        core = start(); platform.inWorld = true;
        core.onOpenSettingsKey();
        UiScreen settings = platform.openedScreen;
        settings.render(canvas, 0, 0, 0f);
        for (int i = 0; i < 5; i++) { settings.mouseClicked(FPS_SWITCH_X, FPS_SWITCH_Y, 0); time.advanceMillis(200); core.onClientTick(); }
        assertFalse(Files.exists(defaultFile()));
        time.advanceMillis(1100); core.onClientTick();
        assertTrue(Files.exists(defaultFile()));
        assertFalse(reread().load("default").module("fps").enabled());   // five clicks switch FPS off
    }

    @Test void shutdown_saves_dirty_state() {
        core = start(); platform.inWorld = true;
        core.onOpenSettingsKey();
        platform.openedScreen.render(canvas, 0, 0, 0f);
        platform.openedScreen.mouseClicked(FPS_SWITCH_X, FPS_SWITCH_Y, 0);
        assertFalse(Files.exists(defaultFile()));
        core.shutdown();
        assertFalse(reread().load("default").module("fps").enabled());
    }

    @Test void a_failed_save_keeps_the_state_dirty_logs_once_and_retries() throws Exception {
        core = start(); platform.inWorld = true;
        Files.createDirectories(profiles().getParent());
        Files.write(profiles(), "not a directory".getBytes(UTF_8));        // the profiles folder cannot be created
        core.onOpenSettingsKey();
        platform.openedScreen.render(canvas, 0, 0, 0f);
        platform.openedScreen.mouseClicked(FPS_SWITCH_X, FPS_SWITCH_Y, 0);
        time.advanceMillis(2100); core.onClientTick();
        assertEquals(1, platform.warnings.size());
        time.advanceMillis(2100); core.onClientTick();
        assertEquals(1, platform.warnings.size());                         // not again for the same problem
        Files.delete(profiles());
        time.advanceMillis(2100); core.onClientTick();
        assertTrue(Files.exists(defaultFile()));
        assertFalse(reread().load("default").module("fps").enabled());
    }

    @Test void a_failed_save_on_close_still_closes_and_the_tick_saves_later() throws Exception {
        core = start(); platform.inWorld = true;
        Files.createDirectories(profiles().getParent());
        Files.write(profiles(), "not a directory".getBytes(UTF_8));
        openSettingsWithChange();
        platform.openedScreen.keyPressed(UiKey.ESCAPE, false, false);
        assertEquals(1, platform.closeCount);
        assertFalse(platform.warnings.isEmpty());
        Files.delete(profiles());
        time.advanceMillis(2100); core.onClientTick();
        assertFalse(reread().load("default").module("fps").enabled());
    }

    @Test void switching_the_profile_writes_settings_json_at_once_and_loads_the_profile() throws Exception {
        core = start(); platform.inWorld = true;
        core.onOpenSettingsKey();
        UiScreen settings = platform.openedScreen;
        settings.render(canvas, 0, 0, 0f);
        assertTrue(settings.mouseClicked(NEW_PROFILE_X, HEADER_Y, 0));   // copies the active profile and switches to the copy
        String json = new String(Files.readAllBytes(dir.resolve("hengst/settings.json")), UTF_8);
        assertTrue(json.contains("Profil 2"));
        assertTrue(Files.exists(profiles().resolve("Profil 2.json")));
    }

    // ---- isolation ----

    @Test void a_failing_screen_open_leaves_a_clean_state() {
        core = start(); platform.inWorld = true;
        core.registry().register(new TestFeature("bad", 0, EnumSet.allOf(GameVersion.class)) {
            @Override public List<OptionSpec> options() { throw new IllegalStateException("boom"); }
        });
        core.onOpenSettingsKey();
        assertNull(platform.openedScreen);
        assertEquals(1, platform.warnings.size());
        core.onClientTick(); core.onHudRender(canvas, 0f);
        assertFalse(canvas.calls.isEmpty());               // the HUD is not left suppressed
        core.onOpenHudEditorKey();
        assertNotNull(platform.openedScreen);
    }

    @Test void an_exception_in_a_screen_input_does_not_escape_and_is_logged_once() {
        core = start(); platform.inWorld = true;
        final boolean[] armed = {false};
        core.registry().register(new TestFeature("bad", 99, EnumSet.allOf(GameVersion.class)) {
            @Override public List<OptionSpec> options() {
                if (armed[0]) throw new IllegalStateException("boom");
                return super.options();
            }
        });
        core.onOpenSettingsKey();
        UiScreen s = platform.openedScreen;
        s.render(canvas, 0, 0, 0f);
        armed[0] = true;
        for (int y = 46; y < 46 + 20 * 5; y += 20) {       // selecting the row of the bad feature rebuilds its options
            assertFalse(throwsOnClick(s, 60, y));
        }
        assertEquals(1, platform.warnings.size());
    }

    private static boolean throwsOnClick(UiScreen s, int x, int y) {
        try {
            s.mouseClicked(x, y, 0);
            return false;
        } catch (RuntimeException e) {
            return true;
        }
    }

    // ---- allocation ----

    @Test void render_and_tick_hooks_allocate_nothing_with_the_four_modules_active() {
        core = start(); platform.inWorld = true;
        platform.playerX = 10.25; platform.playerY = 64; platform.playerZ = -3.5;
        core.onMouseButton(0, true);
        assertEquals(4, core.renderer().activeCount());
        CountingCanvas counting = new CountingCanvas();
        for (int i = 0; i < 3000; i++) { core.onClientTick(); core.onHudRender(counting, 0f); }
        int calls = 20000;
        // Warm-up per phase: the first long run of each loop allocated 616 bytes once (JIT deoptimization, not a
        // per-call allocation: the following rounds allocate 0), so each loop runs once unmeasured first.
        for (int i = 0; i < calls; i++) { core.onHudRender(counting, 0f); }
        for (int i = 0; i < calls; i++) { core.onClientTick(); }
        long before = MX.getThreadAllocatedBytes(Thread.currentThread().getId());
        for (int i = 0; i < calls; i++) { core.onHudRender(counting, 0f); }
        long renderBytes = MX.getThreadAllocatedBytes(Thread.currentThread().getId()) - before;
        before = MX.getThreadAllocatedBytes(Thread.currentThread().getId());
        for (int i = 0; i < calls; i++) { core.onClientTick(); }
        long tickBytes = MX.getThreadAllocatedBytes(Thread.currentThread().getId()) - before;
        System.out.println("HengstCore allocation over " + calls + " calls: render " + renderBytes + " bytes, tick " + tickBytes + " bytes");
        assertEquals(0, renderBytes, "render hook allocated " + renderBytes + " bytes");
        assertEquals(0, tickBytes, "tick hook allocated " + tickBytes + " bytes");
        assertTrue(counting.texts > 0);
    }
}
