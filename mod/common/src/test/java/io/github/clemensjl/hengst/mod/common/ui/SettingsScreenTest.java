package io.github.clemensjl.hengst.mod.common.ui;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.I18n;
import io.github.clemensjl.hengst.mod.common.config.Profile;
import io.github.clemensjl.hengst.mod.common.config.ProfileStore;
import io.github.clemensjl.hengst.mod.common.config.Settings;
import io.github.clemensjl.hengst.mod.common.feature.FeatureRegistry;
import io.github.clemensjl.hengst.mod.common.hud.HudRenderer;
import io.github.clemensjl.hengst.mod.common.hud.modules.CoordinatesModule;
import io.github.clemensjl.hengst.mod.common.hud.modules.CpsModule;
import io.github.clemensjl.hengst.mod.common.hud.modules.FpsModule;
import io.github.clemensjl.hengst.mod.common.hud.modules.KeystrokesModule;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.platform.UiKey;
import io.github.clemensjl.hengst.mod.common.testutil.CountingCanvas;
import io.github.clemensjl.hengst.mod.common.testutil.FakeCanvas;
import io.github.clemensjl.hengst.mod.common.testutil.FakePlatform;
import io.github.clemensjl.hengst.mod.common.testutil.TestFeature;
import java.io.IOException;
import java.io.UncheckedIOException;
import java.lang.management.ManagementFactory;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Clock;
import java.time.Instant;
import java.time.ZoneOffset;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.EnumSet;
import java.util.List;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class SettingsScreenTest {
    @TempDir Path dir;
    Clock clock = Clock.fixed(Instant.parse("2026-11-20T10:15:30Z"), ZoneOffset.UTC);
    List<String> warnings = new ArrayList<>();
    ProfileStore store; Settings settings; Profile[] active = { new Profile() };
    int changed, editorOpened, closeCount;
    boolean failSave;
    FakeCanvas canvas = new FakeCanvas();
    SettingsModel model; SettingsScreen screen;

    FeatureRegistry reg() {
        FeatureRegistry reg = new FeatureRegistry();
        reg.register(new FpsModule()); reg.register(new TestFeature("modernonly", 20, EnumSet.of(GameVersion.V26_3)));
        return reg;
    }

    FeatureRegistry allModules() {
        FeatureRegistry reg = new FeatureRegistry();
        reg.register(new FpsModule()); reg.register(new CpsModule());
        reg.register(new KeystrokesModule()); reg.register(new CoordinatesModule());
        return reg;
    }

    void open(FeatureRegistry reg, String lang) {
        model = new SettingsModel(reg, GameVersion.V1_8_9, store, settings, () -> active[0],
            name -> { settings.setActiveProfile(name); active[0] = store.load(name); },
            () -> changed++, () -> editorOpened++,
            () -> {
                if (failSave) throw new UncheckedIOException(new IOException("disk full"));
                try { store.save(settings.activeProfile(), active[0]); } catch (IOException e) { throw new UncheckedIOException(e); }
            },
            warnings::add);
        screen = new SettingsScreen(model, I18n.load(lang), () -> closeCount++);
    }

    @BeforeEach void setUp() {
        store = new ProfileStore(dir, clock, warnings::add); settings = new Settings();
        open(reg(), "en_us");
    }

    // ---- brief tests ----

    @Test void features_list_shows_only_supported() {
        assertEquals(Arrays.asList("fps"), TestFeature.ids(model.features()));
    }
    @Test void toggling_feature_marks_changed() {
        screen.render(canvas, 0, 0, 0f);
        assertFalse(model.config("fps").enabled());
        assertTrue(screen.mouseClicked(12, 46, 0));
        assertTrue(model.config("fps").enabled()); assertEquals(1, changed);
    }
    @Test void copy_and_delete_profile() {
        assertEquals("Profil 2", model.copyActive());
        assertEquals("Profil 2", model.activeProfileName()); assertTrue(model.profiles().contains("Profil 2"));
        model.deleteActive();
        assertEquals("default", model.activeProfileName()); assertFalse(model.profiles().contains("Profil 2"));
        model.deleteActive();
        assertEquals("default", model.activeProfileName()); assertTrue(model.profiles().contains("default"));
    }
    @Test void copy_includes_unsaved_changes() throws Exception {
        model.config("fps").setEnabled(true);
        String copy = model.copyActive();
        assertTrue(store.load(copy).module("fps").enabled());
    }
    @Test void copy_io_failure_goes_to_warn_hook() throws Exception {
        Path blocked = dir.resolve("blocked"); Files.write(blocked, new byte[0]);
        ProfileStore broken = new ProfileStore(blocked, clock, s -> {});
        SettingsModel m = new SettingsModel(reg(), GameVersion.V1_8_9, broken, settings, () -> active[0],
            n -> {}, () -> {}, () -> {}, () -> {}, warnings::add);
        assertEquals("default", m.copyActive());
        assertEquals(1, warnings.size());
    }
    @Test void escape_closes_once() {
        assertTrue(screen.keyPressed(UiKey.ESCAPE, false, false));
        screen.closed();
        assertEquals(1, closeCount);
    }
    @Test void notice_is_drawn() {
        screen.render(canvas, 0, 0, 0f);
        String notice = I18n.load("en_us").get("hengst.notice");
        assertTrue(canvas.calls.stream().anyMatch(c -> c.startsWith("text " + notice + " ")));
    }

    // ---- close ----

    @Test void done_button_closes_once() {
        screen.render(canvas, 0, 0, 0f);
        assertTrue(screen.mouseClicked(250, 220, 0));
        screen.keyPressed(UiKey.ESCAPE, false, false);
        screen.closed();
        assertEquals(1, closeCount);
    }

    @Test void close_saves_rebuilds_then_closes_in_that_order() throws Exception {
        FakePlatform platform = new FakePlatform(dir);
        HudRenderer renderer = new HudRenderer(reg(), GameVersion.V1_8_9);
        renderer.rebuild(active[0]);
        SettingsClose close = new SettingsClose(model, renderer, () -> active[0], platform);
        screen = new SettingsScreen(model, I18n.load("en_us"), close);
        screen.render(canvas, 0, 0, 0f);
        screen.mouseClicked(12, 46, 0);
        assertEquals(0, renderer.activeCount());
        screen.keyPressed(UiKey.ESCAPE, false, false);
        assertTrue(store.load("default").module("fps").enabled());
        assertEquals(1, renderer.activeCount());
        assertEquals(1, platform.closeCount);
        assertFalse(model.dirty());
    }

    @Test void failed_save_on_close_keeps_state_dirty_and_still_closes() {
        FakePlatform platform = new FakePlatform(dir);
        HudRenderer renderer = new HudRenderer(reg(), GameVersion.V1_8_9);
        model.markChanged();
        failSave = true;
        new SettingsClose(model, renderer, () -> active[0], platform).run();
        assertTrue(model.dirty());
        assertEquals(1, warnings.size());
        assertEquals(1, platform.closeCount);
    }

    @Test void unchecked_failure_while_closing_is_logged_and_the_screen_still_closes() {
        FakePlatform platform = new FakePlatform(dir);
        HudRenderer renderer = new HudRenderer(reg(), GameVersion.V1_8_9);
        new SettingsClose(model, renderer, () -> { throw new IllegalStateException("boom"); }, platform).run();
        assertEquals(1, platform.warnings.size());
        assertEquals(1, platform.closeCount);
    }

    // ---- profiles ----

    @Test void switching_saves_pending_changes_first() {
        String copy = model.copyActive();
        assertEquals("Profil 2", copy);
        model.config("fps").setEnabled(true);
        model.markChanged();
        model.switchTo("default");
        assertEquals("default", model.activeProfileName());
        assertTrue(store.load("Profil 2").module("fps").enabled());
        assertFalse(model.dirty());
    }

    @Test void failed_save_aborts_the_switch_and_warns() {
        model.copyActive();
        model.config("fps").setEnabled(true);
        model.markChanged();
        failSave = true;
        model.switchTo("default");
        assertEquals("Profil 2", model.activeProfileName());
        assertTrue(model.config("fps").enabled());
        assertTrue(model.dirty());
        assertEquals(1, warnings.size());
    }

    @Test void failed_save_aborts_the_copy() {
        model.markChanged();
        failSave = true;
        assertEquals("default", model.copyActive());
        assertEquals(Arrays.asList("default"), model.profiles());
        assertEquals(1, warnings.size());
    }

    @Test void delete_is_case_insensitive_about_default_and_keeps_the_last_profile() {
        settings.setActiveProfile("Default");
        model.deleteActive();
        assertTrue(model.profiles().contains("default"));
        assertEquals("Default", model.activeProfileName());
    }

    @Test void profile_choice_and_buttons_work_through_the_screen() {
        screen.render(canvas, 0, 0, 0f);
        // header: choice x 8..104, buttons start at 108
        assertTrue(screen.mouseClicked(130, 15, 0));   // new profile
        assertEquals("Profil 2", model.activeProfileName());
        screen.render(canvas, 0, 0, 0f);
        assertTrue(screen.mouseClicked(20, 15, 0));    // choice: next profile after "Profil 2" wraps to "default"
        assertEquals("default", model.activeProfileName());
        screen.mouseClicked(20, 15, 1);                // back: wraps to "Profil 2"
        assertEquals("Profil 2", model.activeProfileName());
        assertTrue(screen.mouseClicked(195, 15, 0));   // delete profile
        assertEquals("default", model.activeProfileName());
        assertEquals(Arrays.asList("default"), model.profiles());
    }

    @Test void edit_hud_button_opens_the_editor_and_does_not_close_this_screen_afterwards() {
        screen.render(canvas, 0, 0, 0f);
        assertTrue(screen.mouseClicked(270, 15, 0));
        assertEquals(1, editorOpened);
        screen.closed();
        assertEquals(0, closeCount);
    }

    @Test void edit_hud_saves_pending_changes_first() {
        screen.render(canvas, 0, 0, 0f);
        screen.mouseClicked(12, 46, 0);
        screen.mouseClicked(270, 15, 0);
        assertTrue(store.load("default").module("fps").enabled());
    }

    @Test void failed_profile_load_is_warned_once_on_open() throws Exception {
        Files.createDirectory(dir.resolve("default.json"));
        store.load("default");
        warnings.clear();
        open(reg(), "en_us");
        assertEquals(1, warnings.size());
        assertTrue(warnings.get(0).contains("default"));
        screen.render(canvas, 0, 0, 0f);
        assertEquals(1, warnings.size());
    }

    @Test void failed_settings_load_is_warned_on_open() {
        model.setSettingsLoadFailedCheck(() -> true);
        warnings.clear();
        new SettingsScreen(model, I18n.load("en_us"), () -> {});
        assertEquals(1, warnings.size());
        assertTrue(warnings.get(0).contains("settings"));
    }

    // ---- options ----

    @Test void slider_option_is_coerced_and_marks_changed() {
        open(allModules(), "en_us");
        screen.render(canvas, 0, 0, 0f);
        assertTrue(screen.mouseClicked(60, 36 + 3 * 20 + 5, 0));    // select coordinates (4th row)
        assertEquals("coordinates", model.selected().id());
        screen.render(canvas, 0, 0, 0f);
        int before = changed;
        // option rows: background, shadow, colour, decimals (0..2, step 1); track x 144..312
        assertTrue(screen.mouseClicked(144 + 84, 36 + 3 * 20 + 5, 0));
        assertEquals(1, ((Number) model.config("coordinates").option(model.selected().options().get(0))).intValue());
        screen.mouseDragged(5000, 0, 0);
        assertEquals(2, ((Number) model.config("coordinates").option(model.selected().options().get(0))).intValue());
        screen.mouseDragged(-5000, 0, 0);
        assertEquals(0, ((Number) model.config("coordinates").option(model.selected().options().get(0))).intValue());
        screen.mouseReleased(0, 0, 0);
        assertFalse(screen.mouseDragged(200, 0, 0));
        assertTrue(changed >= before + 3);
    }

    @Test void toggle_option_and_text_colour_choice() {
        open(allModules(), "en_us");
        screen.render(canvas, 0, 0, 0f);
        screen.mouseClicked(60, 36 + 2 * 20 + 5, 0);                 // keystrokes
        screen.render(canvas, 0, 0, 0f);
        assertTrue(model.config("keystrokes").background());
        assertTrue(screen.mouseClicked(200, 36 + 5, 0));             // background
        assertFalse(model.config("keystrokes").background());
        assertTrue(model.config("keystrokes").shadow());
        screen.mouseClicked(200, 36 + 20 + 5, 0);                    // shadow
        assertFalse(model.config("keystrokes").shadow());
        model.config("keystrokes").setTextColor(0xFF123456);         // outside the table: shows white, value kept
        screen.render(canvas, 0, 0, 0f);
        assertTrue(canvas.calls.stream().anyMatch(c -> c.startsWith("text Text color: White ")));
        assertEquals(0xFF123456, model.config("keystrokes").textColor());
        screen.mouseClicked(200, 36 + 40 + 5, 0);                    // colour: first entry -> next (yellow)
        assertEquals(0xFFFFFF55, model.config("keystrokes").textColor());
        screen.mouseClicked(200, 36 + 40 + 5, 1);
        assertEquals(0xFFFFFFFF, model.config("keystrokes").textColor());
        screen.mouseClicked(200, 36 + 40 + 5, 1);
        assertEquals(0xFF000000, model.config("keystrokes").textColor());
        assertTrue((Boolean) model.config("keystrokes").option(model.selected().options().get(1)));
        screen.mouseClicked(200, 36 + 80 + 5, 0);                    // show_space (fifth row)
        assertFalse((Boolean) model.config("keystrokes").option(model.selected().options().get(1)));
    }

    // ---- scrolling ----

    @Test void feature_list_scrolls_with_the_wheel_and_clamps() {
        FeatureRegistry r = new FeatureRegistry();
        for (int i = 0; i < 12; i++) r.register(new TestFeature("f" + i, i + 1, EnumSet.allOf(GameVersion.class)));
        open(r, "en_us");
        screen.render(canvas, 0, 0, 0f);
        ScrollList list = screen.featureList();
        assertTrue(screen.mouseScrolled(50, 100, -1));
        assertEquals(12, list.offset());
        for (int i = 0; i < 50; i++) screen.mouseScrolled(50, 100, -1);
        assertEquals(12 * 20 - 168, list.offset());
        for (int i = 0; i < 50; i++) screen.mouseScrolled(50, 100, 1);
        assertEquals(0, list.offset());
        assertFalse(screen.mouseScrolled(5, 230, 1));
        // offset 12: row 0 (y 24..42) is cut by the list top and must not react
        screen.mouseScrolled(50, 100, -1);
        screen.render(canvas, 0, 0, 0f);
        assertFalse(screen.mouseClicked(12, 36 + 3, 0));
        assertEquals(0, changed);
        assertTrue(screen.mouseClicked(12, 36 + 20 - 12 + 5, 0));   // row 1 is now at y 44
        assertTrue(model.config("f1").enabled());
    }

    // ---- fitting ----

    private static final class Drawn {
        String text; int x; int y;
    }

    private static List<Drawn> textCalls(FakeCanvas c, List<float[]> scaled) {
        List<Drawn> out = new ArrayList<>();
        int depth = 0;
        float[] top = null;
        for (String call : c.calls) {
            if (call.startsWith("push ")) {
                String[] p = call.split(" ");
                top = new float[] {Float.parseFloat(p[1]), Float.parseFloat(p[2]), Float.parseFloat(p[3])};
                depth++;
            } else if (call.equals("pop")) {
                depth--;
            } else if (call.startsWith("text ")) {
                int end = call.length();
                for (int i = 0; i < 4; i++) end = call.lastIndexOf(' ', end - 1);   // drop x, y, colour, shadow
                String rest = call.substring(5, end);
                String[] tail = call.split(" ");
                Drawn d = new Drawn();
                d.text = rest;
                d.x = Integer.parseInt(tail[tail.length - 4]);
                d.y = Integer.parseInt(tail[tail.length - 3]);
                if (depth > 0) {
                    scaled.add(new float[] {top[0] + d.x * top[2], top[1], top[2], 6 * d.text.length() * top[2]});
                } else {
                    out.add(d);
                }
            }
        }
        return out;
    }

    @Test void german_texts_fit_the_screen_and_their_widgets_at_320_by_240() {
        open(allModules(), "de_de");
        for (int row = 0; row < 4; row++) {
            screen.render(canvas, 0, 0, 0f);
            screen.mouseClicked(60, 36 + row * 20 + 5, 0);
            canvas.calls.clear();
            screen.render(canvas, 0, 0, 0f);
            List<float[]> scaled = new ArrayList<>();
            List<Drawn> texts = textCalls(canvas, scaled);
            assertFalse(texts.isEmpty());
            List<Widget> all = new ArrayList<>(screen.headerWidgets());
            all.addAll(screen.featureList().widgets());
            all.addAll(screen.optionList().widgets());
            all.add(screen.doneButton());
            for (Drawn d : texts) {
                int right = d.x + 6 * d.text.length();
                assertTrue(d.x >= 0 && right <= 320 && d.y >= 0 && d.y + 9 <= 240, "inside the screen: " + d.text);
                Widget owner = null;
                for (Widget w : all) {
                    if (d.x >= w.x && d.x < w.x + w.w && d.y >= w.y && d.y < w.y + w.h) { owner = w; break; }
                }
                assertNotNull(owner, "belongs to a widget: " + d.text);
                assertTrue(right <= owner.x + owner.w, "inside its widget: " + d.text + " ends at " + right + ", widget ends at " + (owner.x + owner.w));
            }
            assertEquals(1, scaled.size());
            float[] n = scaled.get(0);
            assertTrue(n[0] >= 0 && n[0] + n[3] <= 320.001f, "notice inside the screen");
        }
    }

    @Test void notice_is_scaled_to_the_screen_width_and_stays_a_single_call() {
        screen.render(canvas, 0, 0, 0f);
        List<float[]> scaled = new ArrayList<>();
        textCalls(canvas, scaled);
        assertEquals(1, scaled.size());
        assertTrue(scaled.get(0)[2] < 1f);
    }

    // ---- allocation ----

    @Test void steady_state_rendering_allocates_nothing_noticeable() {
        open(allModules(), "de_de");
        CountingCanvas cc = new CountingCanvas();
        screen.render(cc, 0, 0, 0f);
        screen.mouseClicked(60, 36 + 3 * 20 + 5, 0);   // coordinates: has a slider
        for (int i = 0; i < 300; i++) screen.render(cc, 150 + i % 50, 60, 0f);
        com.sun.management.ThreadMXBean mx = (com.sun.management.ThreadMXBean) ManagementFactory.getThreadMXBean();
        long id = Thread.currentThread().getId();
        long a = mx.getThreadAllocatedBytes(id);
        for (int i = 0; i < 1000; i++) screen.render(cc, 150 + i % 50, 60, 0f);
        long used = mx.getThreadAllocatedBytes(id) - a;
        assertTrue(used < 2048, "allocated " + used + " bytes in 1000 frames");
    }
}
