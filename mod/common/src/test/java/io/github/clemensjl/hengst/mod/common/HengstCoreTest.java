package io.github.clemensjl.hengst.mod.common;

import static java.nio.charset.StandardCharsets.UTF_8;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNotSame;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.config.ProfileStore;
import io.github.clemensjl.hengst.mod.common.feature.FeatureState;
import io.github.clemensjl.hengst.mod.common.feature.OptionSpec;
import io.github.clemensjl.hengst.mod.common.feature.RestrictionPolicy;
import io.github.clemensjl.hengst.mod.common.hud.HudModule;
import io.github.clemensjl.hengst.mod.common.layout.Anchor;
import io.github.clemensjl.hengst.mod.common.platform.GameKey;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.platform.WallClock;
import io.github.clemensjl.hengst.mod.common.platform.UiKey;
import io.github.clemensjl.hengst.mod.common.platform.UiScreen;
import io.github.clemensjl.hengst.mod.common.testutil.FakeCanvas;
import io.github.clemensjl.hengst.mod.common.testutil.FakePlatform;
import io.github.clemensjl.hengst.mod.common.testutil.FakeTime;
import io.github.clemensjl.hengst.mod.common.testutil.FakeWallClock;
import io.github.clemensjl.hengst.mod.common.testutil.FixedSizeModule;
import io.github.clemensjl.hengst.mod.common.testutil.TestFeature;
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
    // Screen geometry on the 320 x 240 FakeCanvas: the FPS switch in the feature list and the Done button.
    private static final int FPS_SWITCH_X = 12;
    private static final int FPS_SWITCH_Y = 46;
    private static final int DONE_X = 260;
    private static final int DONE_Y = 222;
    private static final int HEADER_Y = 18;
    private static final int NEW_PROFILE_X = 120;
    private static final int EDIT_HUD_X = 270;
    private static final int DELETE_X = 200;

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
        core.activeProfile().module("future").setEnabled(true);
        core.registerForTest(new FixedSizeModule("future", 99, 10, 10, EnumSet.of(GameVersion.V26_3)));
        assertEquals(4, core.renderer().activeCount());
    }

    @Test void the_feature_state_follows_enabled_supported_and_restricted() {
        core = HengstCore.start(platform, time, clock, (f, server) -> f.id().equals("cps"));
        assertTrue(FeatureState.on(1)); assertFalse(FeatureState.on(2)); assertTrue(FeatureState.on(3)); assertTrue(FeatureState.on(6));
        assertFalse(FeatureState.on(7)); assertFalse(FeatureState.on(0)); assertFalse(FeatureState.on(-1)); assertFalse(FeatureState.on(999));
        assertEquals(3, core.renderer().activeCount());
    }

    @Test void the_feature_state_is_rewritten_when_the_settings_close() {
        core = start(); platform.inWorld = true;
        core.onOpenSettingsKey();
        platform.openedScreen.render(canvas, 0, 0, 0f);
        platform.openedScreen.mouseClicked(FPS_SWITCH_X, FPS_SWITCH_Y, 0);   // FPS off
        assertTrue(FeatureState.on(1));
        platform.openedScreen.keyPressed(UiKey.ESCAPE, false, false);
        assertFalse(FeatureState.on(1));
        assertEquals(3, core.renderer().activeCount());
    }

    @Test void the_feature_state_is_rewritten_when_the_editor_closes() {
        core = start(); platform.inWorld = true;
        core.activeProfile().module("fps").setEnabled(false);
        assertTrue(FeatureState.on(1));
        core.onOpenHudEditorKey();
        platform.openedScreen.keyPressed(UiKey.ESCAPE, false, false);
        assertFalse(FeatureState.on(1));
    }

    @Test void a_toggle_persists_through_the_batched_save() {
        core = start(); platform.inWorld = true;
        core.onOpenSettingsKey();
        platform.openedScreen.render(canvas, 0, 0, 0f);
        platform.openedScreen.mouseClicked(FPS_SWITCH_X, FPS_SWITCH_Y, 0);
        platform.openedScreen.keyPressed(UiKey.ESCAPE, false, false);
        core = start();
        assertFalse(core.activeProfile().module("fps").enabled());
        assertFalse(FeatureState.on(1));
    }

    @Test void an_unsupported_feature_is_off_in_the_state() {
        core = start();
        core.activeProfile().module("future").setEnabled(true);
        core.registerForTest(new FixedSizeModule("future", 49, 10, 10, EnumSet.of(GameVersion.V26_3)));
        assertFalse(FeatureState.on(49));
    }

    private static class RecordingFeature extends TestFeature {
        int calls;
        ModuleConfig last;

        RecordingFeature(String id, int number) { super(id, number, EnumSet.allOf(GameVersion.class)); }

        @Override public void onConfigChanged(ModuleConfig cfg) { calls++; last = cfg; }
    }

    @Test void on_config_changed_reaches_each_supported_feature_with_its_own_config() {
        core = start(); platform.inWorld = true;
        RecordingFeature rec = new RecordingFeature("rec", 48);
        core.registerForTest(rec);
        assertEquals(1, rec.calls);
        core.onOpenSettingsKey();
        platform.openedScreen.keyPressed(UiKey.ESCAPE, false, false);
        assertEquals(2, rec.calls);
        assertSame(core.activeProfile().module("rec"), rec.last);
    }

    @Test void an_exception_from_on_config_changed_is_logged_and_breaks_nothing() {
        core = start(); platform.inWorld = true;
        core.registerForTest(new RecordingFeature("bad", 47) {
            @Override public void onConfigChanged(ModuleConfig cfg) { throw new IllegalStateException("boom"); }
        });
        RecordingFeature rec = new RecordingFeature("rec", 48);
        core.registerForTest(rec);
        assertEquals(1, rec.calls);
        assertEquals(1, platform.warnings.size());
        assertTrue(platform.warnings.get(0).contains("boom"));
        assertEquals(4, core.renderer().activeCount());
    }

    @Test void a_feature_whose_on_config_changed_throws_is_off_in_the_state_and_the_others_are_applied() {
        core = start(); platform.inWorld = true;
        core.activeProfile().module("bad").setEnabled(true);
        core.activeProfile().module("rec").setEnabled(true);
        core.registerForTest(new RecordingFeature("bad", 47) {
            @Override public void onConfigChanged(ModuleConfig cfg) { throw new IllegalStateException("boom"); }
        });
        RecordingFeature rec = new RecordingFeature("rec", 48);
        core.registerForTest(rec);
        assertFalse(FeatureState.on(47));
        assertTrue(FeatureState.on(48));
        assertTrue(FeatureState.on(1));
    }

    @Test void a_linkage_error_from_one_feature_does_not_abort_the_others() {
        core = start();
        core.registerForTest(new RecordingFeature("bad", 47) {
            @Override public void onConfigChanged(ModuleConfig cfg) { throw new NoClassDefFoundError("gone"); }
        });
        RecordingFeature rec = new RecordingFeature("rec", 48);
        core.registerForTest(rec);
        assertEquals(1, rec.calls);
        assertEquals(1, platform.warnings.size());
        assertTrue(platform.warnings.get(0).contains("bad"));
        assertTrue(platform.warnings.get(0).contains("NoClassDefFoundError"));
    }

    @Test void two_different_failing_features_are_each_named_once() {
        core = start();
        core.registerForTest(new RecordingFeature("bad1", 46) {
            @Override public void onConfigChanged(ModuleConfig cfg) { throw new IllegalStateException("one"); }
        });
        core.registerForTest(new RecordingFeature("bad2", 47) {
            @Override public void onConfigChanged(ModuleConfig cfg) { throw new IllegalStateException("two"); }
        });
        core.applyConfigForTest();
        core.applyConfigForTest();
        assertEquals(2, platform.warnings.size());
        assertTrue(platform.warnings.get(0).contains("bad1"));
        assertTrue(platform.warnings.get(1).contains("bad2"));
    }

    /** Flips a flag during the notification, so a policy keyed on the flag fails exactly in the rewrite that follows. */
    private static final class Flipper extends RecordingFeature {
        final boolean[] armed;
        final boolean[] policyThrows;
        final boolean failItself;

        Flipper(String id, int number, boolean[] armed, boolean[] policyThrows, boolean failItself) {
            super(id, number);
            this.armed = armed; this.policyThrows = policyThrows; this.failItself = failItself;
        }

        @Override public void onConfigChanged(ModuleConfig cfg) {
            super.onConfigChanged(cfg);
            if (armed[0]) {
                policyThrows[0] = true;
                if (failItself) { throw new IllegalStateException("notify"); }
            }
        }
    }

    @Test void a_failing_rebuild_keeps_the_previous_hud_and_does_not_skip_the_rewrite_and_the_notification() {
        boolean[] armed = new boolean[1];
        boolean[] first = new boolean[1];
        core = HengstCore.start(platform, time, clock, (f, server) -> {
            if (armed[0] && !first[0]) { first[0] = true; throw new IllegalStateException("rebuild"); }
            return false;
        });
        RecordingFeature rec = new RecordingFeature("rec", 48);
        core.registerForTest(rec);
        int before = rec.calls;
        assertEquals(4, core.renderer().activeCount());
        core.activeProfile().module("fps").setEnabled(false);
        armed[0] = true;
        core.applyConfigForTest();                       // the first policy call is the renderer's
        assertEquals(4, core.renderer().activeCount());  // the previous HUD arrays stay: a stale HUD beats an empty one
        platform.inWorld = true;
        core.onClientTick();
        core.onHudRender(canvas, 0f);
        assertFalse(canvas.calls.isEmpty());
        assertFalse(FeatureState.on(1));
        assertEquals(before + 1, rec.calls);
        assertEquals(1, platform.warnings.size());
        assertTrue(platform.warnings.get(0).contains("rebuilding the HUD"));
        assertTrue(platform.warnings.get(0).contains("keeping the previous HUD"));
    }

    @Test void a_failing_rewrite_keeps_the_previous_state_and_still_notifies() {
        boolean[] armed = new boolean[1];
        boolean[] policyThrows = new boolean[1];
        core = HengstCore.start(platform, time, clock, (f, server) -> {
            if (policyThrows[0]) { throw new IllegalStateException("rewrite"); }
            return false;
        });
        Flipper flip = new Flipper("flip", 49, armed, policyThrows, false);
        core.registerForTest(flip);
        int before = flip.calls;
        assertTrue(FeatureState.on(1));
        core.activeProfile().module("fps").setEnabled(false);
        armed[0] = true;
        core.applyConfigForTest();                       // rebuild passes, the notification flips the flag, the rewrite throws
        assertTrue(FeatureState.on(1));                  // the previously published state stays
        assertEquals(before + 1, flip.calls);
        assertEquals(1, platform.warnings.size());
        assertTrue(platform.warnings.get(0).contains("feature state"));
    }

    @Test void a_failing_notification_and_a_failing_rewrite_switch_that_feature_off_and_keep_the_rest() {
        boolean[] armed = new boolean[1];
        boolean[] policyThrows = new boolean[1];
        core = HengstCore.start(platform, time, clock, (f, server) -> {
            if (policyThrows[0]) { throw new IllegalStateException("rewrite"); }
            return false;
        });
        core.activeProfile().module("x").setEnabled(true);
        core.activeProfile().module("rec").setEnabled(true);
        core.registerForTest(new Flipper("x", 47, armed, policyThrows, true));
        core.registerForTest(new RecordingFeature("rec", 48));
        assertTrue(FeatureState.on(47));
        assertTrue(FeatureState.on(48));
        core.activeProfile().module("rec").setEnabled(false);   // a successful rewrite would turn rec off
        armed[0] = true;
        core.applyConfigForTest();
        assertFalse(FeatureState.on(47));                // its fields are half updated: off
        assertTrue(FeatureState.on(48));                 // the previous value stays
        assertTrue(FeatureState.on(1));
    }

    @Test void a_failing_editor_save_goes_through_the_core_wiring_closes_and_stays_dirty() throws Exception {
        core = start(); platform.inWorld = true;
        core.onClientTick();
        Files.createDirectories(profiles().getParent());
        Files.write(profiles(), new byte[0]);            // a file where the directory should be
        core.onOpenHudEditorKey();
        platform.openedScreen.render(canvas, 0, 0, 0f);
        platform.openedScreen.mouseClicked(5, 5, 0);
        platform.openedScreen.mouseDragged(100, 100, 0);
        platform.openedScreen.mouseReleased(100, 100, 0);
        platform.openedScreen.keyPressed(UiKey.ESCAPE, false, false);
        assertEquals(1, platform.closeCount);
        assertEquals(1, platform.warnings.size());
        assertTrue(platform.warnings.get(0).startsWith("Could not save the HUD editor (profile default): "));
        Files.delete(profiles());                        // still dirty: the tick writes it once the directory works
        time.advanceMillis(5000);
        core.onClientTick();
        assertTrue(Files.exists(defaultFile()));
    }

    @Test void a_module_without_an_entry_gets_its_default_place_and_stays_off() throws Exception {
        Files.createDirectories(profiles());
        Files.write(defaultFile(), "{\"schema\":1,\"modules\":{\"fps\":{\"enabled\":true,\"anchor\":\"TOP_RIGHT\",\"x\":-2,\"y\":2}}}".getBytes(UTF_8));
        core = start();
        ModuleConfig cps = core.activeProfile().module("cps");
        assertFalse(cps.enabled()); assertEquals(Anchor.TOP_LEFT, cps.anchor()); assertEquals(2, cps.offsetX()); assertEquals(16, cps.offsetY());
        assertEquals(Anchor.TOP_RIGHT, core.activeProfile().module("fps").anchor());
        assertEquals(1, core.renderer().activeCount());
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

    @Test void on_frame_advances_the_fps_counter_while_the_hud_is_hidden() {
        core = start(); platform.inWorld = true; platform.hudHidden = true;
        for (int i = 0; i < 45; i++) { core.onFrame(); core.onHudRender(canvas, 0f); time.advanceMillis(33); }
        assertTrue(canvas.calls.isEmpty());
        assertTrue(core.context().frames().fps() > 20);
    }

    @Test void on_hud_render_alone_does_not_advance_the_fps_counter() {
        core = start(); platform.inWorld = true;
        for (int i = 0; i < 100; i++) { core.onHudRender(canvas, 0f); time.advanceMillis(33); }
        assertEquals(0, core.context().frames().fps());
    }

    @Test void coordinates_are_cleared_when_leaving_the_world() {
        core = start(); platform.inWorld = true;
        platform.playerX = 12.5; platform.playerY = 64; platform.playerZ = -30.5;
        core.onClientTick();
        core.onHudRender(canvas, 0f);
        assertTrue(canvas.calls.toString().contains("X 12"));
        platform.inWorld = false;
        core.onClientTick();                                // ticks outside a world too
        platform.inWorld = true;                            // rejoin: no tick yet
        platform.playerX = 99;
        canvas.calls.clear();
        core.onHudRender(canvas, 0f);
        assertFalse(canvas.calls.toString().contains("X "));
        core.onClientTick();
        canvas.calls.clear();
        core.onHudRender(canvas, 0f);
        assertTrue(canvas.calls.toString().contains("X 99"));
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

    @Test void settings_close_through_the_platform_removing_the_screen() throws Exception {
        core = start(); openSettingsWithChange();
        UiScreen s = platform.openedScreen;
        platform.removeScreenFromGame();                   // host marked removed first, then closed()
        assertEquals(0, platform.closeCount);              // no second screen change
        assertTrue(Files.exists(defaultFile()));
        assertFalse(reread().load("default").module("fps").enabled());
        Files.delete(defaultFile());
        s.closed();
        time.advanceMillis(5000); core.onClientTick(); core.shutdown();
        assertEquals(0, platform.closeCount);
        assertFalse(Files.exists(defaultFile()));
        canvas.calls.clear();
        core.onHudRender(canvas, 0f);
        assertFalse(canvas.calls.isEmpty());
    }

    @Test void a_screen_dropped_by_the_game_is_ended_by_the_tick() throws Exception {
        core = start(); openSettingsWithChange();
        platform.screenOpen = false;                       // gone without closed()
        core.onClientTick();
        assertEquals(0, platform.closeCount);
        assertTrue(Files.exists(defaultFile()));
        assertFalse(reread().load("default").module("fps").enabled());
        Files.delete(defaultFile());
        time.advanceMillis(5000); core.onClientTick(); core.shutdown();
        assertFalse(Files.exists(defaultFile()));          // saved once
        canvas.calls.clear();
        core.onHudRender(canvas, 0f);
        assertFalse(canvas.calls.isEmpty());               // unsuppressed
        core.onOpenSettingsKey();
        assertNotNull(platform.openedScreen);              // the keys work again
        platform.removeScreenFromGame();
        core.onOpenHudEditorKey();
        assertNotNull(platform.openedScreen);
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
        core.registerForTest(new TestFeature("bad", 0, EnumSet.allOf(GameVersion.class)) {
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
        core.registerForTest(new TestFeature("bad", 99, EnumSet.allOf(GameVersion.class)) {
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

    // ---- first start, copy, delete, failures at start ----

    @Test void copying_the_profile_at_first_start_keeps_the_four_modules() throws Exception {
        core = start(); platform.inWorld = true;
        core.onOpenSettingsKey();
        UiScreen settings = platform.openedScreen;
        settings.render(canvas, 0, 0, 0f);
        assertTrue(settings.mouseClicked(NEW_PROFILE_X, HEADER_Y, 0));     // copy and switch, nothing was changed before
        assertEquals(4, core.renderer().activeCount());
        String copy = new String(Files.readAllBytes(profiles().resolve("Profil 2.json")), UTF_8);
        for (String id : new String[] {"fps", "cps", "keystrokes", "coordinates"}) {
            assertTrue(copy.contains("\"" + id + "\""), id);
        }
        assertTrue(reread().load("Profil 2").module("fps").enabled());
    }

    @Test void copying_after_a_broken_file_was_moved_aside_keeps_the_four_modules() throws Exception {
        Files.createDirectories(profiles());
        Files.write(defaultFile(), "{\"schema\":1,\"modules\":{".getBytes(UTF_8));
        core = start(); platform.inWorld = true;
        core.onOpenSettingsKey();
        platform.openedScreen.render(canvas, 0, 0, 0f);
        platform.openedScreen.mouseClicked(NEW_PROFILE_X, HEADER_Y, 0);
        assertEquals(4, core.renderer().activeCount());
    }

    @Test void a_directory_in_place_of_the_profile_file_is_not_a_first_start() throws Exception {
        Files.createDirectories(defaultFile());
        core = start();
        assertEquals(0, core.renderer().activeCount());
        assertEquals(1, platform.warnings.size());
        platform.inWorld = true;
        core.onClientTick(); time.advanceMillis(5000); core.onClientTick(); core.shutdown();
        assertTrue(Files.isDirectory(defaultFile()));
        assertEquals(1, platform.warnings.size());
    }

    @Test void a_broken_settings_file_is_moved_aside_and_the_start_is_a_first_start() throws Exception {
        Files.createDirectories(dir.resolve("hengst"));
        Files.write(dir.resolve("hengst/settings.json"), "{ nope".getBytes(UTF_8));
        core = start();
        assertEquals(1, platform.warnings.size());
        assertEquals(4, core.renderer().activeCount());
        assertTrue(Files.exists(dir.resolve("hengst/settings.json.broken-20261120-101530")));
        assertFalse(Files.exists(dir.resolve("hengst/settings.json")));
    }

    @Test void an_unreadable_settings_file_is_left_alone_and_nothing_is_enabled() throws Exception {
        Files.createDirectories(dir.resolve("hengst/settings.json"));
        core = start();
        assertEquals(0, core.renderer().activeCount());
        assertEquals(1, platform.warnings.size());
        core.shutdown();
        assertTrue(Files.isDirectory(dir.resolve("hengst/settings.json")));
        assertFalse(Files.exists(defaultFile()));
    }

    private void copyProfileThenDeleteIt() {
        core = start(); platform.inWorld = true;
        core.onOpenSettingsKey();
        platform.openedScreen.render(canvas, 0, 0, 0f);
        platform.openedScreen.mouseClicked(NEW_PROFILE_X, HEADER_Y, 0);   // now on "Profil 2"
        platform.openedScreen.mouseClicked(DELETE_X, HEADER_Y, 0);        // asks
        platform.openedScreen.mouseClicked(DELETE_X, HEADER_Y, 0);        // deletes
    }

    private String settingsJson() throws Exception {
        return new String(Files.readAllBytes(dir.resolve("hengst/settings.json")), UTF_8);
    }

    @Test void delete_ok_and_switch_ok() throws Exception {
        copyProfileThenDeleteIt();
        assertFalse(Files.exists(profiles().resolve("Profil 2.json")));
        assertTrue(settingsJson().contains("\"default\""));
        assertEquals(4, core.renderer().activeCount());
        platform.openedScreen.mouseClicked(FPS_SWITCH_X, FPS_SWITCH_Y, 0);   // an edit now belongs to default
        core.shutdown();
        assertFalse(reread().load("default").module("fps").enabled());
        assertFalse(Files.exists(profiles().resolve("Profil 2.json")));
        assertTrue(platform.warnings.isEmpty());
    }

    @Test void delete_ok_but_the_settings_write_fails() throws Exception {
        core = start(); platform.inWorld = true;
        core.onOpenSettingsKey();
        platform.openedScreen.render(canvas, 0, 0, 0f);
        platform.openedScreen.mouseClicked(NEW_PROFILE_X, HEADER_Y, 0);
        Files.delete(dir.resolve("hengst/settings.json"));
        Files.createDirectories(dir.resolve("hengst/settings.json"));      // writing settings.json fails now
        platform.openedScreen.mouseClicked(DELETE_X, HEADER_Y, 0);
        platform.openedScreen.mouseClicked(DELETE_X, HEADER_Y, 0);
        assertFalse(Files.exists(profiles().resolve("Profil 2.json")));
        assertEquals(4, core.renderer().activeCount());                    // default is active in memory
        assertEquals(1, platform.warnings.size(), platform.warnings.toString());
        platform.openedScreen.mouseClicked(FPS_SWITCH_X, FPS_SWITCH_Y, 0);
        time.advanceMillis(2100); core.onClientTick();                     // the edit goes to default.json, not Profil 2
        assertFalse(reread().load("default").module("fps").enabled());
        assertFalse(Files.exists(profiles().resolve("Profil 2.json")));
        assertEquals(1, platform.warnings.size());                         // the retry failure is the same problem
        Files.delete(dir.resolve("hengst/settings.json"));
        time.advanceMillis(2100); core.onClientTick();
        assertTrue(settingsJson().contains("\"default\""));
        assertFalse(settingsJson().contains("Profil 2"));
    }

    @Test void delete_fails_and_nothing_changes() throws Exception {
        core = start(); platform.inWorld = true;
        core.onOpenSettingsKey();
        platform.openedScreen.render(canvas, 0, 0, 0f);
        platform.openedScreen.mouseClicked(NEW_PROFILE_X, HEADER_Y, 0);
        Files.createFile(profiles().resolve("Other.json"));                // keeps the profile count above one
        Files.delete(profiles().resolve("Profil 2.json"));                 // a non-empty directory cannot be deleted
        Files.createDirectories(profiles().resolve("Profil 2.json"));
        Files.createFile(profiles().resolve("Profil 2.json/x"));
        platform.openedScreen.mouseClicked(DELETE_X, HEADER_Y, 0);
        platform.openedScreen.mouseClicked(DELETE_X, HEADER_Y, 0);
        assertTrue(settingsJson().contains("Profil 2"));                   // still active in settings.json
        assertEquals(4, core.renderer().activeCount());
        core.shutdown();                                                   // nothing is dirty: no refused save loop
        assertTrue(Files.isDirectory(profiles().resolve("Profil 2.json")));
    }

    // ---- failure logging ----

    @Test void a_different_failure_at_the_same_site_is_logged_and_a_success_re_arms_the_site() {
        platform.inWorld = true;
        core = start();
        platform.screenOpenFailure = new IllegalStateException("one");
        core.onClientTick(); core.onClientTick();
        assertEquals(1, platform.warnings.size());                         // same class: once
        platform.screenOpenFailure = new UnsupportedOperationException("two");
        core.onClientTick();
        assertEquals(2, platform.warnings.size());                         // another class: logged
        platform.screenOpenFailure = null;
        core.onClientTick();                                               // success re-arms the site
        platform.screenOpenFailure = new IllegalStateException("three");
        core.onClientTick();
        assertEquals(3, platform.warnings.size());
    }

    @Test void a_fifth_distinct_exception_class_at_one_site_is_not_logged_on_every_occurrence() {
        platform.inWorld = true;
        core = start();
        RuntimeException[] kinds = {new IllegalStateException("1"), new UnsupportedOperationException("2"),
                new IllegalArgumentException("3"), new ArithmeticException("4"), new ClassCastException("5")};
        for (RuntimeException kind : kinds) {
            platform.screenOpenFailure = kind;
            core.onClientTick();
        }
        assertEquals(4, platform.warnings.size());                         // the table holds four classes
        platform.screenOpenFailure = kinds[4];
        core.onClientTick(); core.onClientTick();
        assertEquals(4, platform.warnings.size());                         // the fifth class stays silent
        platform.screenOpenFailure = null;
        core.onClientTick();                                               // a success re-arms the site
        platform.screenOpenFailure = kinds[4];
        core.onClientTick();
        assertEquals(5, platform.warnings.size());
    }

    // ---- language, wall clock and server change ----

    @Test void a_language_switch_is_picked_up_within_a_second() {
        core = start(); platform.inWorld = true;
        assertEquals("Done", core.context().i18n().get("hengst.button.done"));
        int gen = core.context().languageGeneration();
        platform.languageCode = "de_de";
        time.advanceMillis(1000); core.onClientTick();
        assertEquals("Fertig", core.context().i18n().get("hengst.button.done"));
        assertEquals(gen + 1, core.context().languageGeneration());
        for (int i = 0; i < 100; i++) { time.advanceMillis(50); core.onClientTick(); }
        assertEquals(gen + 1, core.context().languageGeneration());      // loaded once per switch, not once per second
    }

    @Test void a_frame_never_reloads_the_language() {
        core = start(); platform.inWorld = true;
        int gen = core.context().languageGeneration();
        platform.languageCode = "de_de";
        time.advanceMillis(5000); core.onHudRender(canvas, 0f);          // frames only, no tick
        assertEquals(gen, core.context().languageGeneration());
        core.onClientTick();
        assertEquals(gen + 1, core.context().languageGeneration());
    }

    @Test void the_language_is_read_at_most_once_a_second() {
        core = start(); platform.inWorld = true;
        core.onClientTick();                                             // the first check runs in the first tick
        platform.languageCode = "de_de";
        time.advanceMillis(900); core.onClientTick();
        assertEquals(0, core.context().languageGeneration());            // not yet a second
        time.advanceMillis(100); core.onClientTick();
        assertEquals(1, core.context().languageGeneration());
    }

    @Test void the_first_language_check_runs_in_the_first_tick() {
        core = start(); platform.inWorld = true;
        platform.languageCode = "de_de";
        core.onClientTick();
        assertEquals(1, core.context().languageGeneration());
        assertEquals("Fertig", core.context().i18n().get("hengst.button.done"));
    }

    private String doneAfterSwitchTo(String code) {
        core = start(); platform.inWorld = true;
        core.onClientTick();
        platform.languageCode = code;
        time.advanceMillis(1000); core.onClientTick();
        return core.context().i18n().get("hengst.button.done");
    }

    @Test void regional_codes_use_the_language_of_their_family_and_unknown_ones_english() {
        assertEquals("Fertig", doneAfterSwitchTo("de_at"));
        assertEquals("Fertig", doneAfterSwitchTo("de_ch"));
        assertEquals("Done", doneAfterSwitchTo("en_gb"));
        assertEquals("Done", doneAfterSwitchTo("fr_fr"));
        assertEquals("Fertig", doneAfterSwitchTo("de_de"));
        assertEquals("Done", doneAfterSwitchTo(""));
        assertEquals("Done", doneAfterSwitchTo(null));
        assertEquals("Done", doneAfterSwitchTo("../x"));
        assertTrue(platform.warnings.isEmpty(), platform.warnings.toString());
    }

    @Test void an_unsupported_language_is_read_once_across_a_thousand_ticks() {
        core = start(); platform.inWorld = true;
        core.onClientTick();
        platform.languageCode = "fr_fr";
        int reads = I18n.fileReads();
        for (int i = 0; i < 1000; i++) { time.advanceMillis(50); core.onClientTick(); }
        assertTrue(I18n.fileReads() - reads <= 1, "reads: " + (I18n.fileReads() - reads));
        assertEquals(1, core.context().languageGeneration());
        int after = I18n.fileReads();
        for (int i = 0; i < 1000; i++) { time.advanceMillis(50); core.onClientTick(); }
        assertEquals(after, I18n.fileReads());
        assertEquals(1, core.context().languageGeneration());
    }

    @Test void a_throwing_language_code_at_start_is_logged_once_and_corrected_by_the_first_check() {
        platform.languageCodeFailure = new IllegalStateException("not ready");
        core = start(); platform.inWorld = true;
        assertEquals(1, platform.warnings.size());
        assertTrue(platform.warnings.get(0).contains("language"), platform.warnings.get(0));
        assertEquals("Done", core.context().i18n().get("hengst.button.done"));
        platform.languageCodeFailure = null; platform.languageCode = "de_de";
        core.onClientTick();
        assertEquals("Fertig", core.context().i18n().get("hengst.button.done"));
        assertEquals(1, platform.warnings.size());
    }

    @Test void a_throwing_language_code_in_the_tick_does_not_skip_the_hud_tick() {
        TickProbe probe = new TickProbe();
        core = start(); platform.inWorld = true;
        core.registerForTest(probe);
        core.activeProfile().module("probe").setEnabled(true);
        core.applyConfigForTest();
        core.onClientTick();
        int before = probe.ticks;
        platform.languageCodeFailure = new IllegalStateException("boom");
        time.advanceMillis(1000); core.onClientTick(); core.onClientTick();
        assertEquals(before + 2, probe.ticks);
        assertEquals(1, platform.warnings.size());
    }

    /** A HUD module that counts ticks and can throw when the language changes. */
    private static final class TickProbe extends HudModule {
        int ticks;
        int languageChanges;
        boolean throwOnLanguage;
        boolean throwOnTick;

        TickProbe() { super("probe", 0, EnumSet.allOf(GameVersion.class)); }
        @Override public void tick(io.github.clemensjl.hengst.mod.common.hud.HudContext ctx, ModuleConfig cfg) {
            ticks++;
            if (throwOnTick) { throw new IllegalStateException("tick"); }
        }
        @Override public void onLanguageChanged() {
            languageChanges++;
            if (throwOnLanguage) { throw new IllegalStateException("probe"); }
        }
        @Override public int width(io.github.clemensjl.hengst.mod.common.platform.Canvas c, ModuleConfig cfg) { return 1; }
        @Override public int height(io.github.clemensjl.hengst.mod.common.platform.Canvas c, ModuleConfig cfg) { return 1; }
        @Override public void render(io.github.clemensjl.hengst.mod.common.platform.Canvas c, ModuleConfig cfg,
                io.github.clemensjl.hengst.mod.common.hud.HudContext ctx) { }
    }

    @Test void a_module_that_throws_on_a_language_change_does_not_skip_the_others() {
        platform.labels.put(GameKey.FORWARD, "W");
        TickProbe probe = new TickProbe();
        probe.throwOnLanguage = true;
        core = start(); platform.inWorld = true;
        core.registerForTest(probe);                                     // number 0: before every other module
        WidthCanvas c = new WidthCanvas();
        core.onClientTick(); core.onHudRender(c, 0f);
        assertTrue(c.calls.contains("text W 26 4 ffffffff true"), c.calls.toString());
        c.charWidth = 10;
        platform.languageCode = "de_de"; time.advanceMillis(1000); core.onClientTick();
        assertEquals(1, probe.languageChanges);
        assertEquals("Fertig", core.context().i18n().get("hengst.button.done"));
        c.calls.clear(); core.onHudRender(c, 0f);
        assertTrue(c.calls.contains("text W 24 4 ffffffff true"), c.calls.toString());   // keystrokes refitted
        platform.languageCode = "en_us"; time.advanceMillis(1000); core.onClientTick();
        assertEquals(2, probe.languageChanges);
        long logged = platform.warnings.stream().filter(w -> w.contains("the language change of probe")).count();
        assertEquals(1, logged, platform.warnings.toString());           // once per module and exception class
    }

    @Test void a_failed_language_load_keeps_the_old_texts_and_is_tried_three_times_then_given_up() {
        int[] calls = {0};
        core = start(); platform.inWorld = true;
        core.setLanguageLoaderForTest(code -> { calls[0]++; throw new IllegalStateException("unreadable"); });
        core.onClientTick();
        platform.languageCode = "de_de";
        for (int i = 0; i < 20; i++) { time.advanceMillis(1000); core.onClientTick(); }
        assertEquals(3, calls[0]);
        assertEquals("Done", core.context().i18n().get("hengst.button.done"));   // never an empty map with raw keys
        assertEquals(0, core.context().languageGeneration());
        assertEquals(1, platform.warnings.size(), platform.warnings.toString());
        platform.languageCode = "de_at";                                  // a new code is tried again
        time.advanceMillis(1000); core.onClientTick();
        assertEquals(4, calls[0]);
    }

    @Test void a_language_load_that_fails_twice_and_then_works_is_applied() {
        int[] calls = {0};
        core = start(); platform.inWorld = true;
        core.setLanguageLoaderForTest(code -> {
            if (++calls[0] < 3) { throw new IllegalStateException("busy"); }
            return I18n.load(code);
        });
        core.onClientTick();
        platform.languageCode = "de_de";
        for (int i = 0; i < 10; i++) { time.advanceMillis(1000); core.onClientTick(); }
        assertEquals(3, calls[0]);
        assertEquals("Fertig", core.context().i18n().get("hengst.button.done"));
        assertEquals(1, core.context().languageGeneration());
        assertTrue(platform.warnings.isEmpty(), platform.warnings.toString());
    }

    @Test void switching_back_to_the_loaded_language_clears_a_give_up_so_the_failed_one_is_retried() {
        int[] calls = {0};
        core = start(); platform.inWorld = true;
        core.setLanguageLoaderForTest(code -> { calls[0]++; throw new IllegalStateException("unreadable"); });
        core.onClientTick();
        platform.languageCode = "de_de";
        for (int i = 0; i < 10; i++) { time.advanceMillis(1000); core.onClientTick(); }
        assertEquals(3, calls[0]);                                       // given up while it stays selected
        platform.languageCode = "en_us";                                 // the loaded language: nothing to load
        time.advanceMillis(1000); core.onClientTick();
        assertEquals(3, calls[0]);
        platform.languageCode = "de_de";
        for (int i = 0; i < 10; i++) { time.advanceMillis(1000); core.onClientTick(); }
        assertEquals(6, calls[0]);                                       // retried, again three times
    }

    @Test void an_unreadable_language_file_at_start_does_not_make_start_throw() {
        int[] calls = {0};
        core = HengstCore.start(platform, time, clock, new FakeWallClock(), RestrictionPolicy.NONE,
                code -> { calls[0]++; throw new IllegalStateException("en_us is unreadable"); });
        assertEquals(1, platform.warnings.size());
        assertEquals("hengst.button.done", core.context().i18n().get("hengst.button.done"));   // the key, never null
        assertEquals(1, calls[0]);
        platform.inWorld = true;
        core.onClientTick();                                             // the check tries the real language (bounded)
        assertEquals(2, calls[0]);
        core.setLanguageLoaderForTest(I18n::load);
        time.advanceMillis(1000); core.onClientTick();
        assertEquals("Done", core.context().i18n().get("hengst.button.done"));
        assertEquals(1, core.context().languageGeneration());
    }

    @Test void a_throwing_hud_tick_does_not_skip_the_batched_save() throws Exception {
        TickProbe probe = new TickProbe();
        core = start(); platform.inWorld = true;
        core.registerForTest(probe);
        Files.createDirectories(profiles().getParent());
        Files.write(profiles(), new byte[0]);                            // the save at the editor's close fails
        core.onOpenHudEditorKey();
        platform.openedScreen.render(canvas, 0, 0, 0f);
        platform.openedScreen.mouseClicked(5, 5, 0);
        platform.openedScreen.mouseDragged(100, 100, 0);
        platform.openedScreen.mouseReleased(100, 100, 0);
        platform.openedScreen.keyPressed(UiKey.ESCAPE, false, false);
        Files.delete(profiles());                                        // the change is still pending
        core.activeProfile().module("probe").setEnabled(true);
        core.applyConfigForTest();
        probe.throwOnTick = true;
        // The module's failure is logged by the renderer; make that log itself throw once so renderer.tick fails.
        platform.warnings = new java.util.ArrayList<String>() {
            boolean armed = true;
            @Override public boolean add(String w) {
                if (armed) { armed = false; throw new IllegalStateException("log failed"); }
                return super.add(w);
            }
        };
        time.advanceMillis(5000); core.onClientTick();
        assertTrue(Files.exists(defaultFile()));
        assertTrue(platform.warnings.stream().anyMatch(w -> w.contains("the HUD tick")), platform.warnings.toString());
    }

    @Test void a_failing_language_load_does_not_skip_the_hud_tick() {
        TickProbe probe = new TickProbe();
        core = start(); platform.inWorld = true;
        core.registerForTest(probe);
        core.activeProfile().module("probe").setEnabled(true);
        core.applyConfigForTest();
        core.setLanguageLoaderForTest(code -> { throw new IllegalStateException("x"); });
        platform.languageCode = "de_de";
        int before = probe.ticks;
        core.onClientTick();
        assertEquals(before + 1, probe.ticks);
    }

    @Test void a_failing_server_change_is_recorded_logged_once_and_keeps_the_published_state() {
        int[] asked = {0};
        core = HengstCore.start(platform, time, clock, new FakeWallClock(), (f, s) -> {
            if ("bad.net".equals(s)) { asked[0]++; throw new IllegalStateException("policy"); }
            return false;
        });
        TickProbe probe = new TickProbe();
        core.registerForTest(probe);
        core.activeProfile().module("probe").setEnabled(true);
        core.applyConfigForTest();
        platform.inWorld = true; core.onClientTick();
        assertTrue(FeatureState.on(1));
        platform.serverAddress = "bad.net";
        int before = probe.ticks;
        core.onClientTick();
        assertEquals(before + 1, probe.ticks);                           // the HUD tick still ran
        assertTrue(FeatureState.on(1));                                  // the previous state stays published
        int askedAfter = asked[0];
        for (int i = 0; i < 20; i++) { core.onClientTick(); }
        assertEquals(askedAfter, asked[0]);                              // recorded: no retry every tick
        assertEquals(1, platform.warnings.stream().filter(w -> w.contains("publishing the feature state")).count(),
                platform.warnings.toString());
    }

    @Test void a_throwing_server_read_does_not_skip_the_hud_tick() {
        TickProbe probe = new TickProbe();
        core = start(); platform.inWorld = true;
        core.registerForTest(probe);
        core.activeProfile().module("probe").setEnabled(true);
        core.applyConfigForTest();
        platform.serverAddressFailure = new IllegalStateException("gone");
        int before = probe.ticks;
        core.onClientTick(); core.onClientTick();
        assertEquals(before + 2, probe.ticks);
        assertEquals(1, platform.warnings.size());
    }

    @Test void the_settings_opened_after_a_switch_are_german() {
        core = start(); platform.inWorld = true;
        platform.languageCode = "de_DE"; time.advanceMillis(1000); core.onClientTick();
        core.onOpenSettingsKey();
        FakeCanvas c = new FakeCanvas(); platform.openedScreen.render(c, 0, 0, 0f);
        assertTrue(c.calls.stream().anyMatch(s -> s.startsWith("text Fertig ")));
    }

    @Test void a_screen_opened_after_a_switch_is_a_new_object_with_new_widgets_and_text_fits() {
        core = start(); platform.inWorld = true;
        core.onOpenSettingsKey();
        UiScreen english = platform.openedScreen;
        english.render(new FakeCanvas(), 0, 0, 0f);
        platform.removeScreenFromGame();
        platform.languageCode = "de_de"; time.advanceMillis(1000); core.onClientTick();
        core.onOpenSettingsKey();
        assertNotSame(english, platform.openedScreen);
        FakeCanvas c = new FakeCanvas(); platform.openedScreen.render(c, 0, 0, 0f);
        assertTrue(c.calls.stream().anyMatch(s -> s.startsWith("text Fertig ")));
        assertFalse(c.calls.stream().anyMatch(s -> s.startsWith("text Done ")));
    }

    /** A canvas whose glyph width can change, as it does after a font or language reload. */
    private static final class WidthCanvas extends FakeCanvas {
        int charWidth = 6;

        @Override public int textWidth(String text) { return charWidth * text.length(); }
    }

    @Test void a_language_switch_drops_every_cached_text_fit_of_the_hud_modules() {
        platform.labels.put(GameKey.FORWARD, "W");
        core = start(); platform.inWorld = true;
        HudModule fps = (HudModule) core.registry().byId("fps");
        ModuleConfig fpsCfg = core.activeProfile().module("fps");
        WidthCanvas c = new WidthCanvas();
        core.onClientTick();
        core.onHudRender(c, 0f);
        assertTrue(c.calls.contains("text W 26 4 ffffffff true"), c.calls.toString());   // cell at x 20, centred: 20 + (18 - 6) / 2
        int fpsWidthBefore = fps.width(c, fpsCfg);
        c.charWidth = 10;                                                // the glyphs got wider; nothing reloaded yet
        assertEquals(fpsWidthBefore, fps.width(c, fpsCfg));              // the cached width is what a reload must drop
        platform.languageCode = "de_de"; time.advanceMillis(1000); core.onClientTick();
        c.calls.clear();
        core.onHudRender(c, 0f);
        assertTrue(c.calls.contains("text W 24 4 ffffffff true"), c.calls.toString());   // 20 + (18 - 10) / 2
        assertEquals((fpsWidthBefore - 4) / 6 * 10 + 4, fps.width(c, fpsCfg));
    }

    @Test void the_context_carries_the_injected_wall_clock_and_system_by_default() {
        FakeWallClock wall = new FakeWallClock();
        core = HengstCore.start(platform, time, clock, wall, RestrictionPolicy.NONE);
        assertSame(wall, core.context().wallClock());
        core.onClientTick(); core.onFrame(); core.onHudRender(canvas, 0f);
        assertEquals(0, wall.currentCalls);                              // the core itself never reads the wall clock
        assertEquals(0, wall.offsetCalls);
        assertSame(WallClock.SYSTEM, start().context().wallClock());
    }

    @Test void a_server_change_rewrites_the_state_with_the_restriction() {
        core = HengstCore.start(platform, time, clock, new FakeWallClock(), (f, s) -> "mc.example.net".equals(s) && f.id().equals("fps"));
        platform.inWorld = true; core.onClientTick();
        assertTrue(FeatureState.on(1)); assertEquals(4, core.renderer().activeCount());
        platform.serverAddress = "mc.example.net"; core.onClientTick();
        assertFalse(FeatureState.on(1)); assertEquals(3, core.renderer().activeCount());
        platform.serverAddress = null; core.onClientTick();
        assertTrue(FeatureState.on(1)); assertEquals(4, core.renderer().activeCount());
    }

    @Test void switching_directly_from_one_server_to_another_applies_the_new_restriction_once() {
        int[] asked = {0};
        core = HengstCore.start(platform, time, clock, new FakeWallClock(), (f, s) -> {
            if (f.id().equals("fps")) { asked[0]++; }
            return "b.net".equals(s) && f.id().equals("fps");
        });
        platform.inWorld = true;
        platform.serverAddress = "a.net"; core.onClientTick();
        assertTrue(FeatureState.on(1));
        platform.serverAddress = "b.net"; core.onClientTick();           // no null in between
        assertFalse(FeatureState.on(1));
        int after = asked[0];
        core.onClientTick(); core.onClientTick();
        assertEquals(after, asked[0]);                                   // exactly once per change
        platform.serverAddress = "a.net"; core.onClientTick();
        assertTrue(FeatureState.on(1));
    }

    @Test void a_server_already_set_at_the_first_tick_counts_as_joining() {
        core = HengstCore.start(platform, time, clock, new FakeWallClock(), (f, s) -> "mc.example.net".equals(s) && f.id().equals("fps"));
        platform.serverAddress = "mc.example.net"; platform.inWorld = true;
        core.onClientTick();
        assertFalse(FeatureState.on(1));
    }

    @Test void single_player_is_no_change() {
        int[] asked = {0};
        core = HengstCore.start(platform, time, clock, new FakeWallClock(), (f, s) -> { asked[0]++; return false; });
        platform.inWorld = true; platform.serverAddress = null;
        int afterStart = asked[0];
        for (int i = 0; i < 50; i++) { core.onClientTick(); }
        assertEquals(afterStart, asked[0]);
    }

    @Test void an_equal_server_string_is_not_a_change() {
        int[] asked = {0};
        core = HengstCore.start(platform, time, clock, new FakeWallClock(), (f, s) -> { asked[0]++; return false; });
        platform.inWorld = true; platform.serverAddress = "a.net"; core.onClientTick();
        int after = asked[0];
        for (int i = 0; i < 100; i++) { platform.serverAddress = new String("a.net"); core.onClientTick(); }
        assertEquals(after, asked[0]);
    }

    // ---- registry ----

    @Test void registering_on_the_registry_after_start_fails_fast() {
        core = start();
        assertThrows(IllegalStateException.class,
                () -> core.registry().register(new FixedSizeModule("late", 98, 10, 10, EnumSet.of(GameVersion.V1_8_9))));
        assertNull(core.registry().byId("late"));
    }
}
