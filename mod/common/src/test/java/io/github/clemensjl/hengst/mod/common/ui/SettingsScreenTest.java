package io.github.clemensjl.hengst.mod.common.ui;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.I18n;
import io.github.clemensjl.hengst.mod.common.config.Profile;
import io.github.clemensjl.hengst.mod.common.config.ProfileStore;
import io.github.clemensjl.hengst.mod.common.config.Settings;
import io.github.clemensjl.hengst.mod.common.feature.Feature;
import io.github.clemensjl.hengst.mod.common.feature.FeatureGroup;
import io.github.clemensjl.hengst.mod.common.feature.FeatureRegistry;
import io.github.clemensjl.hengst.mod.common.feature.OptionSpec;
import io.github.clemensjl.hengst.mod.common.json.Json;
import io.github.clemensjl.hengst.mod.common.json.JsonObjects;
import io.github.clemensjl.hengst.mod.common.hud.HudRenderer;
import io.github.clemensjl.hengst.mod.common.hud.modules.CoordinatesModule;
import io.github.clemensjl.hengst.mod.common.hud.modules.CpsModule;
import io.github.clemensjl.hengst.mod.common.hud.modules.FpsModule;
import io.github.clemensjl.hengst.mod.common.hud.modules.KeystrokesModule;
import io.github.clemensjl.hengst.mod.common.hud.modules.StopwatchModule;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.platform.UiKey;
import io.github.clemensjl.hengst.mod.common.testutil.CountingCanvas;
import io.github.clemensjl.hengst.mod.common.testutil.FakeCanvas;
import io.github.clemensjl.hengst.mod.common.testutil.FakePlatform;
import io.github.clemensjl.hengst.mod.common.testutil.GroupFeature;
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
    boolean editorFail;
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
            () -> changed++, () -> { if (editorFail) throw new IllegalStateException("no editor"); editorOpened++; },
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
        assertTrue(screen.mouseClicked(12, 66, 0));
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
    @Test void notice_is_drawn_wrapped_unscaled_and_fits_the_width() {
        screen.render(canvas, 0, 0, 0f);
        String notice = I18n.load("en_us").get("hengst.notice");
        List<Drawn> lines = textCalls(canvas, "ffaaaaaa").stream().filter(d -> d.y >= screen.featureList().bottom()).collect(java.util.stream.Collectors.toList());   // below the feature list: the group headers share the colour
        assertTrue(lines.size() >= 2);
        assertEquals(notice, String.join(" ", lines.stream().map(d -> d.text).toArray(String[]::new)));
        for (Drawn d : lines) assertTrue(d.x >= 0 && d.x + 6 * d.text.length() <= 320, d.text);
        assertFalse(canvas.calls.stream().anyMatch(c -> c.startsWith("push ")));
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
        ScreenClose close = new ScreenClose(model::saveIfDirty, () -> renderer.rebuild(active[0]), platform, "the settings");
        screen = new SettingsScreen(model, I18n.load("en_us"), close);
        screen.render(canvas, 0, 0, 0f);
        screen.mouseClicked(12, 66, 0);
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
        new ScreenClose(model::saveIfDirty, () -> renderer.rebuild(active[0]), platform, "the settings").run();
        assertTrue(model.dirty());
        assertEquals(1, warnings.size());
        assertEquals(1, platform.closeCount);
    }

    @Test void unchecked_failure_while_closing_is_logged_and_the_screen_still_closes() {
        FakePlatform platform = new FakePlatform(dir);
        new ScreenClose(model::saveIfDirty, () -> { throw new IllegalStateException("boom"); }, platform, "the settings").run();
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
        // header (320 px): choice x 8..88, new 92..162, delete 166..236, edit HUD 240..310; all 20 px high
        assertTrue(screen.mouseClicked(120, 15, 0));   // new profile
        assertEquals("Profil 2", model.activeProfileName());
        screen.render(canvas, 0, 0, 0f);
        assertTrue(screen.mouseClicked(20, 15, 0));    // choice: next profile after "Profil 2" wraps to "default"
        assertEquals("default", model.activeProfileName());
        screen.mouseClicked(20, 15, 1);                // back: wraps to "Profil 2"
        assertEquals("Profil 2", model.activeProfileName());
        assertTrue(screen.mouseClicked(200, 15, 0));   // delete profile: asks first
        assertEquals("Profil 2", model.activeProfileName());
        assertTrue(screen.mouseClicked(200, 15, 0));   // second click deletes
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
        screen.mouseClicked(12, 66, 0);
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
        assertTrue(screen.mouseClicked(60, 36 + 4 * 20 + 5, 0));    // select coordinates (row 4, the header is row 0)
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
        screen.mouseClicked(60, 36 + 3 * 20 + 5, 0);                 // keystrokes
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
        assertEquals(20, list.offset());
        for (int i = 0; i < 50; i++) screen.mouseScrolled(50, 100, -1);
        // whole rows only: the last row ends inside the list, no partial row at the top
        assertEquals((13 - list.height() / 20) * 20, list.offset());
        for (int i = 0; i < 50; i++) screen.mouseScrolled(50, 100, 1);
        assertEquals(0, list.offset());
        assertFalse(screen.mouseScrolled(5, 230, 1));
        // one notch is one whole row: row 0 (the header) is above the list, row 1 (f0) starts at the list top (y 36)
        screen.mouseScrolled(50, 100, -1);
        screen.render(canvas, 0, 0, 0f);
        assertEquals(0, changed);
        assertTrue(screen.mouseClicked(12, 36 + 5, 0));
        assertTrue(model.config("f0").enabled());
    }

    // ---- fitting ----

    private static final class Drawn {
        String text; int x; int y; String color;
    }

    private static List<Drawn> textCalls(FakeCanvas c, String colorOrNull) {
        List<Drawn> out = new ArrayList<>();
        for (String call : c.calls) {
            if (!call.startsWith("text ")) continue;
            int end = call.length();
            for (int i = 0; i < 4; i++) end = call.lastIndexOf(' ', end - 1);   // drop x, y, colour, shadow
            String[] tail = call.split(" ");
            Drawn d = new Drawn();
            d.text = call.substring(5, end);
            d.x = Integer.parseInt(tail[tail.length - 4]);
            d.y = Integer.parseInt(tail[tail.length - 3]);
            d.color = tail[tail.length - 2];
            if (colorOrNull == null || colorOrNull.equals(d.color)) out.add(d);
        }
        return out;
    }

    private String joined(List<Drawn> lines) {
        StringBuilder sb = new StringBuilder();
        for (Drawn d : lines) { if (sb.length() > 0) sb.append(' '); sb.append(d.text); }
        return sb.toString();
    }

    private List<Widget> allWidgets() {
        List<Widget> all = new ArrayList<>(screen.headerWidgets());
        all.addAll(screen.featureList().widgets());
        all.addAll(screen.optionList().widgets());
        all.add(screen.doneButton());
        return all;
    }

    private void checkFits(List<Drawn> texts) {
        List<Widget> all = allWidgets();
        for (Drawn d : texts) {
            int right = d.x + 6 * d.text.length();
            assertTrue(d.x >= 0 && right <= 320 && d.y >= 0 && d.y + 9 <= 240, "inside the screen: " + d.text);
            if (d.color.equals("ffaaaaaa") || d.color.equals("ffff5555")) {
                assertTrue(right <= 312, "free text keeps the margin: " + d.text);
                continue;
            }
            Widget owner = null;
            for (Widget w : all) {
                if (d.x >= w.x && d.x < w.x + w.w && d.y >= w.y && d.y < w.y + w.h) { owner = w; break; }
            }
            assertNotNull(owner, "belongs to a widget: " + d.text);
            assertTrue(right <= owner.x + owner.w, "inside its widget: " + d.text + " ends at " + right + ", widget ends at " + (owner.x + owner.w));
        }
    }

    @Test void german_texts_fit_the_screen_and_their_widgets_at_320_by_240() {
        open(allModules(), "de_de");
        screen.render(canvas, 0, 0, 0f);
        screen.mouseClicked(120, 15, 0);                // new profile: Profil 2, so delete can ask for confirmation
        model.fail(SettingsModel.KEY_PROFILE_LOAD_FAILED, "x");   // longest warning sentence on screen
        for (int row = 0; row < 4; row++) {
            screen.render(canvas, 0, 0, 0f);
            screen.mouseClicked(60, 36 + (row + 1) * 20 + 5, 0);
            canvas.calls.clear();
            screen.render(canvas, 0, 0, 0f);
            List<Drawn> texts = textCalls(canvas, null);
            assertFalse(texts.isEmpty());
            checkFits(texts);
            // hint-like free texts: notice and warning are wrapped, not cut
            assertEquals(I18n.load("de_de").get("hengst.notice"), joined(textCalls(canvas, "ffaaaaaa").stream().filter(d -> d.y >= screen.featureList().bottom()).collect(java.util.stream.Collectors.toList())));
            assertEquals(I18n.load("de_de").get(SettingsModel.KEY_PROFILE_LOAD_FAILED), joined(textCalls(canvas, "ffff5555")));
        }
        // buttons are shown in full, also in the confirm state
        for (String label : new String[] {"Löschen", "Neu (Kopie)", "HUD ändern", "Fertig"}) {
            String l = label;
            assertTrue(canvas.calls.stream().anyMatch(c -> c.startsWith("text " + l + " ")), label);
        }
        model.fail(null, "x");
        screen.mouseClicked(200, 15, 0);
        canvas.calls.clear();
        screen.render(canvas, 0, 0, 0f);
        assertTrue(canvas.calls.stream().anyMatch(c -> c.startsWith("text Sicher? ")));
        checkFits(textCalls(canvas, null));
    }

    @Test void header_is_20_px_high() {
        screen.render(canvas, 0, 0, 0f);
        for (Widget w : screen.headerWidgets()) { assertEquals(8, w.y); assertEquals(20, w.h); }
    }

    @Test void profiles_with_similar_names_look_different_at_320_px_in_german() {
        open(allModules(), "de_de");
        screen.render(canvas, 0, 0, 0f);
        screen.mouseClicked(120, 15, 0);
        screen.mouseClicked(120, 15, 0);
        assertEquals("Profil 3", model.activeProfileName());
        canvas.calls.clear();
        screen.render(canvas, 0, 0, 0f);
        String three = joined(textCalls(canvas, "ffffffff").stream().filter(d -> d.y < 28 && d.x < 88).collect(java.util.stream.Collectors.toList()));
        screen.mouseClicked(20, 15, 1);
        assertEquals("Profil 2", model.activeProfileName());
        canvas.calls.clear();
        screen.render(canvas, 0, 0, 0f);
        String two = joined(textCalls(canvas, "ffffffff").stream().filter(d -> d.y < 28 && d.x < 88).collect(java.util.stream.Collectors.toList()));
        assertNotEquals(two, three);
        assertTrue(three.endsWith("3") && two.endsWith("2"), two + " / " + three);
    }

    @Test void long_profile_name_is_shortened_in_the_middle_keeping_start_and_end() {
        FakeCanvas c = new FakeCanvas();
        String name = "Competitive Season Setup 12";
        String cut = TextFit.middle(c, name, 76);
        assertTrue(c.textWidth(cut) <= 76);
        assertTrue(cut.contains("..") && cut.startsWith("Compe") && cut.endsWith("12"), cut);
    }

    // ---- stored values ----

    private static final class OptFeature extends Feature {
        OptFeature() { super("opt", 5, FeatureGroup.HUD, EnumSet.allOf(GameVersion.class)); }
        @Override public List<OptionSpec> options() {
            return Arrays.asList(OptionSpec.toggle("t", false), OptionSpec.intRange("i", 2, 0, 5),
                OptionSpec.floatRange("f", 1.0f, 0.5f, 3.0f, 0.1f), OptionSpec.choice("c", "a", "a", "b", "c"));
        }
    }

    private java.util.Map<String, Object> savedOptions() throws Exception {
        assertTrue(model.saveIfDirty());
        String text = new String(Files.readAllBytes(dir.resolve("default.json")), java.nio.charset.StandardCharsets.UTF_8);
        java.util.Map<String, Object> modules = JsonObjects.object(JsonObjects.object(Json.parse(text)).get("modules"));
        return JsonObjects.object(JsonObjects.object(modules.get("opt")).get("options"));
    }

    @Test void widgets_store_coerced_values_with_the_right_json_types() throws Exception {
        FeatureRegistry r = new FeatureRegistry();
        r.register(new OptFeature());
        open(r, "en_us");
        screen.render(canvas, 0, 0, 0f);
        // option rows: background, shadow, colour, t (y 101), i (121), f (141), c (161); tracks x 144..312
        assertTrue(screen.mouseClicked(200, 101, 0));
        screen.mouseClicked(261, 121, 0); screen.mouseReleased(0, 0, 0);          // int slider: 3.48 -> 3
        screen.mouseClicked(231, 141, 0);                                           // float slider: about 1.79 -> 1.8
        java.util.Map<String, Object> o = savedOptions();
        assertEquals(Boolean.TRUE, o.get("t"));
        assertEquals(Long.valueOf(3), o.get("i"));                                  // a Long in the file, not 3.0
        // On the grid (the click was at about 1.79); an uncoerced widget value would be 1.7946 here.
        assertEquals(1.8, (Double) o.get("f"));
        screen.mouseDragged(5000, 0, 0);
        assertEquals(3.0, (Double) savedOptions().get("f"));
        screen.mouseDragged(-5000, 0, 0);
        assertEquals(0.5, (Double) savedOptions().get("f"));
        screen.mouseReleased(0, 0, 0);
        screen.mouseClicked(200, 161, 0);
        assertEquals("b", savedOptions().get("c"));
        screen.mouseClicked(200, 121, 1);                                           // right click on a slider does nothing
        assertEquals(Long.valueOf(3), savedOptions().get("i"));
    }

    // ---- failures, confirm, warnings ----

    @Test void single_delete_click_deletes_nothing_and_two_clicks_do() {
        screen.render(canvas, 0, 0, 0f);
        screen.mouseClicked(120, 15, 0);
        assertEquals("Profil 2", model.activeProfileName());
        screen.mouseClicked(200, 15, 0);
        assertTrue(model.profiles().contains("Profil 2"));
        canvas.calls.clear();
        screen.render(canvas, 0, 0, 0f);
        assertTrue(canvas.calls.stream().anyMatch(c -> c.startsWith("text Sure? ")));
        screen.mouseClicked(200, 15, 0);
        assertFalse(model.profiles().contains("Profil 2"));
        assertEquals("default", model.activeProfileName());
    }

    @Test void another_click_or_a_key_cancels_the_delete_confirmation() {
        screen.render(canvas, 0, 0, 0f);
        screen.mouseClicked(120, 15, 0);
        screen.mouseClicked(200, 15, 0);
        screen.mouseClicked(300, 100, 0);                 // elsewhere
        screen.mouseClicked(200, 15, 0);                  // asks again instead of deleting
        assertTrue(model.profiles().contains("Profil 2"));
        screen.keyPressed(UiKey.LEFT, false, false);      // a key cancels
        screen.mouseClicked(200, 15, 0);
        assertTrue(model.profiles().contains("Profil 2"));
        canvas.calls.clear();
        screen.render(canvas, 0, 0, 0f);
        assertTrue(canvas.calls.stream().anyMatch(c -> c.startsWith("text Sure? ")));
    }

    @Test void default_profile_cannot_enter_the_confirm_state() {
        screen.render(canvas, 0, 0, 0f);
        screen.mouseClicked(200, 15, 0);
        canvas.calls.clear();
        screen.render(canvas, 0, 0, 0f);
        assertFalse(canvas.calls.stream().anyMatch(c -> c.startsWith("text Sure? ")));
    }

    @Test void failed_switch_shows_the_warning_and_a_later_success_clears_it() {
        screen.render(canvas, 0, 0, 0f);
        screen.mouseClicked(120, 15, 0);                  // Profil 2
        model.config("fps").setEnabled(true);
        model.markChanged();
        failSave = true;
        screen.render(canvas, 0, 0, 0f);
        screen.mouseClicked(20, 15, 0);                   // switch to default: save fails
        assertEquals("Profil 2", model.activeProfileName());
        canvas.calls.clear();
        screen.render(canvas, 0, 0, 0f);
        List<Drawn> w = textCalls(canvas, "ffff5555");
        assertEquals(I18n.load("en_us").get(SettingsModel.KEY_SAVE_FAILED), joined(w));
        for (Drawn d : w) assertTrue(d.x + 6 * d.text.length() <= 312);
        assertEquals(1, warnings.size());                 // still sent to the warn hook (technical detail)
        failSave = false;
        screen.mouseClicked(20, 15, 0);
        assertEquals("default", model.activeProfileName());
        canvas.calls.clear();
        screen.render(canvas, 0, 0, 0f);
        assertTrue(textCalls(canvas, "ffff5555").isEmpty());
    }

    @Test void load_failure_warning_is_drawn_on_open() throws Exception {
        Files.createDirectory(dir.resolve("default.json"));
        store.load("default");
        open(reg(), "en_us");
        screen.render(canvas, 0, 0, 0f);
        assertEquals(I18n.load("en_us").get(SettingsModel.KEY_PROFILE_LOAD_FAILED), joined(textCalls(canvas, "ffff5555")));
    }

    @Test void a_throwing_editor_hook_is_reported_and_done_and_escape_still_work() {
        editorFail = true;
        screen.render(canvas, 0, 0, 0f);
        assertTrue(screen.mouseClicked(270, 15, 0));
        assertEquals(SettingsModel.KEY_EDITOR_FAILED, model.failureKey());
        assertEquals(1, warnings.size());
        screen.keyPressed(UiKey.ESCAPE, false, false);
        assertEquals(1, closeCount);
    }

    @Test void switching_away_from_an_unreadable_profile_is_possible_without_saving_it() throws Exception {
        Files.createDirectory(dir.resolve("default.json"));
        store.load("default");
        store.save("Profil 2", new Profile());
        model.markChanged();
        model.switchTo("Profil 2");
        assertEquals("Profil 2", model.activeProfileName());
        assertEquals(SettingsModel.KEY_PROFILE_LOAD_FAILED, model.failureKey());
        assertTrue(Files.isDirectory(dir.resolve("default.json")));
        assertFalse(model.dirty());
    }

    @Test void failed_delete_keeps_the_pending_edits() throws Exception {
        store.save("Profil 3", new Profile());
        Files.createDirectories(dir.resolve("Profil 2.json"));
        Files.write(dir.resolve("Profil 2.json").resolve("x"), new byte[1]);   // non-empty folder: delete fails
        settings.setActiveProfile("Profil 2");
        Profile before = active[0];
        model.config("fps").setEnabled(true);
        model.markChanged();
        model.deleteActive();
        assertSame(before, active[0]);
        assertEquals("Profil 2", model.activeProfileName());
        assertTrue(model.config("fps").enabled());
        assertTrue(model.dirty());
        assertEquals(SettingsModel.KEY_DELETE_FAILED, model.failureKey());
        assertEquals(1, warnings.size());
    }

    @Test void exceptions_from_the_switch_hook_do_not_escape() {
        SettingsModel m = new SettingsModel(reg(), GameVersion.V1_8_9, store, settings, () -> active[0],
            n -> { throw new IllegalStateException("boom"); }, () -> {}, () -> {}, () -> {}, warnings::add);
        m.switchTo("Profil 2");
        assertEquals("default", m.activeProfileName());
        assertEquals(SettingsModel.KEY_SWITCH_FAILED, m.failureKey());
        assertEquals("default", m.copyActive());                 // the copy is made, the switch to it fails
        assertEquals("default", m.activeProfileName());
        assertEquals(SettingsModel.KEY_SWITCH_FAILED, m.failureKey());
        assertEquals(2, warnings.size());
    }

    // ---- allocation ----

    @Test void steady_state_rendering_allocates_nothing_noticeable() {
        open(allModules(), "de_de");
        CountingCanvas cc = new CountingCanvas();
        screen.render(cc, 0, 0, 0f);
        screen.mouseClicked(60, 36 + 4 * 20 + 5, 0);   // coordinates: has a slider
        for (int i = 0; i < 300; i++) screen.render(cc, 150 + i % 50, 60, 0f);
        com.sun.management.ThreadMXBean mx = (com.sun.management.ThreadMXBean) ManagementFactory.getThreadMXBean();
        long id = Thread.currentThread().getId();
        long a = mx.getThreadAllocatedBytes(id);
        for (int i = 0; i < 1000; i++) screen.render(cc, 150 + i % 50, 60, 0f);
        long used = mx.getThreadAllocatedBytes(id) - a;
        assertTrue(used < 2048, "allocated " + used + " bytes in 1000 frames");
    }

    // ---- rows by feature kind, labels, dim ----

    @Test void the_first_draw_call_is_a_dim_over_the_whole_screen() {
        screen.render(canvas, 0, 0, 0f);
        assertEquals("fill 0 0 320 240 c0101010", canvas.calls.get(0));
    }

    @Test void a_feature_that_is_not_on_the_hud_gets_no_hud_style_rows() {
        FeatureRegistry r = new FeatureRegistry();
        r.register(new Feature("zoom", 30, FeatureGroup.VISUAL, EnumSet.allOf(GameVersion.class)) {
            @Override public List<OptionSpec> options() { return Arrays.asList(OptionSpec.toggle("smooth", true)); }
        });
        r.register(new FpsModule());
        open(r, "en_us");
        screen.render(canvas, 0, 0, 0f);
        screen.mouseClicked(60, 36 + 1 * 20 + 5, 0);                // fps: row 1, the HUD header is row 0
        assertEquals("fps", model.selected().id());
        assertEquals(3, screen.optionList().widgets().size());       // background, shadow, colour, no options
        screen.mouseClicked(60, 36 + 3 * 20 + 5, 0);                 // zoom: row 3 (row 2 is the Visual header)
        assertEquals("zoom", model.selected().id());
        assertEquals(1, screen.optionList().widgets().size());       // only its own option
    }

    @Test void a_hud_module_in_another_group_gets_the_hud_style_rows() {
        FeatureRegistry r = new FeatureRegistry();
        r.register(new StopwatchModule());                       // group COMFORT: header row 0, stopwatch row 1
        open(r, "en_us");
        screen.render(canvas, 0, 0, 0f);
        screen.mouseClicked(60, 36 + 20 + 5, 0);
        assertEquals("stopwatch", model.selected().id());
        assertEquals(4, screen.optionList().widgets().size());   // background, shadow, colour, tenths
    }

    /** The hint lines of the last drawn frame: muted text in the option panel (x 144) above the option rows. */
    private List<Drawn> hintLines() {
        return textCalls(canvas, "ffaaaaaa").stream().filter(d -> d.x == 144 && d.y < screen.featureList().bottom()).collect(java.util.stream.Collectors.toList());
    }

    @Test void the_hint_is_drawn_for_the_stopwatch_and_not_for_a_feature_without_one() {
        FeatureRegistry r = new FeatureRegistry();
        r.register(new FpsModule());
        r.register(new StopwatchModule());                       // rows: HUD header, fps, Comfort header, stopwatch
        open(r, "en_us");
        screen.render(canvas, 0, 0, 0f);
        screen.mouseClicked(60, 36 + 20 + 5, 0);                 // fps
        canvas.calls.clear(); screen.render(canvas, 0, 0, 0f);
        assertTrue(hintLines().isEmpty());
        assertEquals(36, screen.optionList().widgets().get(0).y);
        screen.mouseClicked(60, 36 + 3 * 20 + 5, 0);             // stopwatch
        assertEquals("stopwatch", model.selected().id());
        canvas.calls.clear(); screen.render(canvas, 0, 0, 0f);
        List<Drawn> lines = hintLines();
        assertFalse(lines.isEmpty());
        assertEquals("Bind the keys under Options > Controls", joined(lines));
        screen.mouseClicked(60, 36 + 20 + 5, 0);                 // back to fps: the hint is gone
        canvas.calls.clear(); screen.render(canvas, 0, 0, 0f);
        assertTrue(hintLines().isEmpty());
    }

    @Test void the_german_hint_wraps_at_the_panel_width_and_pushes_the_option_rows_down() {
        FeatureRegistry r = new FeatureRegistry();
        r.register(new StopwatchModule());
        open(r, "de_de");
        screen.render(canvas, 0, 0, 0f);
        screen.mouseClicked(60, 36 + 20 + 5, 0);
        canvas.calls.clear(); screen.render(canvas, 0, 0, 0f);
        List<Drawn> lines = hintLines();
        assertTrue(lines.size() >= 2, "the 41 characters (246 px) do not fit the 168 px panel");
        assertEquals("Tasten unter Optionen > Steuerung belegen", joined(lines));
        for (int k = 0; k < lines.size(); k++) {
            assertTrue(lines.get(k).x + 6 * lines.get(k).text.length() <= 144 + 168, lines.get(k).text);
            assertEquals(36 + k * 10, lines.get(k).y);
        }
        int hintHeight = lines.size() * 10 + 4;
        assertEquals(36 + hintHeight, screen.optionList().widgets().get(0).y);
        // a click on the old first row position (now covered by the hint) hits no option, one on the shifted row does
        boolean before = model.config("stopwatch").background();
        assertFalse(screen.mouseClicked(200, 36 + 5, 0));
        assertEquals(before, model.config("stopwatch").background());
        assertTrue(screen.mouseClicked(200, 36 + hintHeight + 5, 0));
        assertEquals(!before, model.config("stopwatch").background());
    }

    private Feature featureWithOptions() {
        return new Feature("zoom", 30, FeatureGroup.VISUAL, EnumSet.allOf(GameVersion.class)) {
            @Override public List<OptionSpec> options() {
                return Arrays.asList(OptionSpec.toggle("smooth", true), OptionSpec.choice("mode", "hold", "hold", "toggle"));
            }
        };
    }

    private void openZoom(java.util.Map<String, Object> primary, java.util.Map<String, Object> fallback) {
        FeatureRegistry r = new FeatureRegistry();
        r.register(featureWithOptions());
        open(r, "en_us");
        screen = new SettingsScreen(model, I18n.of(primary, fallback), () -> closeCount++);
        screen.render(canvas, 0, 0, 0f);
        screen.mouseClicked(60, 36 + 20 + 5, 0);                     // row 1, the Visual header is row 0
        canvas.calls.clear();
        screen.render(canvas, 0, 0, 0f);
    }

    private static java.util.Map<String, Object> texts(String... kv) {
        java.util.Map<String, Object> m = new java.util.HashMap<>();
        for (int i = 0; i < kv.length; i += 2) m.put(kv[i], kv[i + 1]);
        return m;
    }

    static final EnumSet<GameVersion> ALL = EnumSet.allOf(GameVersion.class);
    /** n features "f0" .. "f<n-1>" with the numbers 1 .. n, all in group HUD. */
    private static FeatureRegistry registryWith(int n) {
        FeatureRegistry r = new FeatureRegistry();
        for (int i = 0; i < n; i++) { r.register(new TestFeature("f" + i, i + 1, ALL)); }
        return r;
    }
    private FeatureRegistry defaultRegistry() { return allModules(); }   // fps, cps, keystrokes, coordinates
    /** Draws the screen again and returns the calls of that frame. */
    private List<String> drawn() { canvas.calls.clear(); screen.render(canvas, 0, 0, 0f); return canvas.calls; }
    /** Opens with fixed English texts so headers and the names of the test features are readable (the real language file does not know "comfy" and "vis"). */
    private void openNamed(FeatureRegistry reg) {
        open(reg, "en_us");
        screen = new SettingsScreen(model, I18n.of(texts("hengst.group.hud", "HUD", "hengst.group.visual", "Visual", "hengst.group.comfort", "Comfort",
                "hengst.feature.fps", "fps", "hengst.feature.cps", "cps", "hengst.feature.comfy", "comfy", "hengst.feature.vis", "vis"), texts()), () -> closeCount++);
    }
    /** All texts the last frame draws in the feature list (x 8 to 136, y 36 to 186), top to bottom: headers and feature names. */
    private List<String> listTexts() {
        drawn();
        List<String> out = new ArrayList<>();
        for (Drawn d : textCalls(canvas, null)) { if (d.x >= 8 && d.x < 136 && d.y >= 36 && d.y < 186) { out.add(d.text); } }
        return out;
    }

    @Test void features_are_grouped_under_headers_in_group_order() {
        FeatureRegistry reg = new FeatureRegistry();
        reg.register(new TestFeature("fps", 1, ALL)); reg.register(new TestFeature("cps", 2, ALL));
        reg.register(new GroupFeature("comfy", 21, FeatureGroup.COMFORT)); reg.register(new GroupFeature("vis", 25, FeatureGroup.VISUAL));
        openNamed(reg);
        assertEquals(Arrays.asList("HUD", "fps", "cps", "Visual", "vis", "Comfort", "comfy"), listTexts());   // 7 rows, all visible
    }
    @Test void a_group_without_supported_features_has_no_header() {
        FeatureRegistry reg = new FeatureRegistry();
        reg.register(new TestFeature("fps", 1, ALL));
        reg.register(new GroupFeature("vis", 25, FeatureGroup.VISUAL, EnumSet.of(GameVersion.V26_3)));
        openNamed(reg);   // version 1.8.9
        assertEquals(Arrays.asList("HUD", "fps"), listTexts());
    }
    @Test void a_header_is_drawn_in_grey_at_x_plus_4_without_a_fill() {
        openNamed(registryWith(1));
        List<String> calls = drawn();
        assertTrue(calls.contains("text HUD 12 40 ffaaaaaa true"), calls.toString());
        assertFalse(calls.stream().anyMatch(c -> c.startsWith("fill 8 36 ")), "no fill for the header row");
    }
    @Test void a_header_too_wide_for_its_row_is_cut_by_text_fit() {
        open(registryWith(1), "en_us");
        screen = new SettingsScreen(model, I18n.of(texts("hengst.group.hud", "A very long group name that cannot fit"), texts()), () -> closeCount++);
        drawn();
        List<Drawn> headers = textCalls(canvas, "ffaaaaaa").stream().filter(d -> d.y < 186).collect(java.util.stream.Collectors.toList());
        assertEquals(1, headers.size());
        assertEquals("A very long group na", headers.get(0).text);   // 20 characters of 6 px fit into 128 - 6
        assertTrue(headers.get(0).x + 6 * headers.get(0).text.length() <= 8 + 128);
    }
    @Test void clicking_a_header_selects_and_changes_nothing() {
        open(defaultRegistry(), "en_us");
        screen.render(canvas, 0, 0, 0f);
        Feature before = model.selected();
        assertFalse(screen.mouseClicked(60, 46, 0));         // row 0 (y 36 to 56) is the header HUD
        assertSame(before, model.selected());
        assertFalse(model.dirty());
    }
    @Test void the_scroll_indicator_appears_only_when_the_list_overflows() {
        open(registryWith(4), "en_us");
        assertFalse(drawn().contains("fill 134 36 136 186 40ffffff"));
        open(registryWith(11), "en_us");   // 11 HUD features + 1 header = 12 rows, 7 visible
        assertTrue(drawn().contains("fill 134 36 136 186 40ffffff"));
        assertTrue(drawn().contains("fill 134 36 136 123 c0ffffff"));   // th = max(8, 150 * 7 / 12) = 87
        screen.mouseScrolled(40, 100, -10);
        assertTrue(drawn().contains("fill 134 99 136 186 c0ffffff"));   // maxOffset 100, ty = 36 + 63
    }
    @Test void the_thumb_of_a_very_long_list_keeps_its_minimum_size_and_reaches_both_ends() {
        open(registryWith(200), "en_us");   // 201 rows: 150 * 7 / 201 = 5 px would be too small to see
        assertTrue(drawn().contains("fill 134 36 136 44 c0ffffff"));    // th = MIN_THUMB 8, at the top
        screen.mouseScrolled(40, 100, -100000);
        assertTrue(drawn().contains("fill 134 178 136 186 c0ffffff"));  // 8 px at the bottom of the track
    }
    @Test void the_indicator_is_hidden_for_a_list_that_just_fits_and_shown_for_one_row_more() {
        open(registryWith(6), "en_us");    // 7 rows = 140 px in a 150 px list
        screen.render(canvas, 0, 0, 0f);
        assertFalse(screen.featureList().overflows());
        assertFalse(drawn().stream().anyMatch(c -> c.startsWith("fill 134 ") && c.endsWith("ffffff")));
        open(registryWith(7), "en_us");    // 8 rows = 160 px: one more than fits
        screen.render(canvas, 0, 0, 0f);
        assertTrue(screen.featureList().overflows());
        assertTrue(drawn().contains("fill 134 36 136 186 40ffffff"));
        assertTrue(drawn().contains("fill 134 36 136 167 c0ffffff"));   // th = 150 * 7 / 8 = 131, offset 0: top
        screen.mouseScrolled(40, 100, -5);
        assertEquals(20, screen.featureList().offset());
        assertTrue(drawn().contains("fill 134 55 136 186 c0ffffff"));   // maxOffset 20, ty = 36 + 19: bottom
    }
    @Test void rows_are_4_px_narrower_with_the_indicator() {
        open(registryWith(4), "en_us");
        screen.render(canvas, 0, 0, 0f);
        assertEquals(128, screen.featureList().widgets().get(1).w);
        open(registryWith(11), "en_us");
        screen.render(canvas, 0, 0, 0f);
        assertEquals(124, screen.featureList().widgets().get(1).w);
    }
    @Test void a_list_that_shrinks_while_scrolled_to_the_end_clamps_and_the_indicator_follows() {
        open(registryWith(11), "en_us");
        screen.render(canvas, 0, 0, 0f);
        screen.mouseScrolled(40, 100, -50);
        screen.render(canvas, 0, 0, 0f);
        assertEquals(100, screen.featureList().offset());
        List<Widget> nine = new ArrayList<>(screen.featureList().widgets().subList(0, 9));
        screen.featureList().setWidgets(nine, 20);
        assertEquals(40, screen.featureList().offset());                // (9 - 7) rows
        assertTrue(drawn().contains("fill 134 70 136 186 c0ffffff"));   // th = 150 * 7 / 9 = 116, at the end: 36 + 34
        screen.featureList().setWidgets(new ArrayList<>(nine.subList(0, 5)), 20);
        assertEquals(0, screen.featureList().offset());
        assertFalse(drawn().stream().anyMatch(c -> c.startsWith("fill 134 ") && c.endsWith("ffffff")));
    }
    @Test void the_down_key_does_not_scroll_the_list_and_two_wheel_notches_scroll_two_rows() {
        open(registryWith(11), "en_us");
        screen.render(canvas, 0, 0, 0f);
        assertFalse(screen.keyPressed(UiKey.DOWN, false, false));
        assertEquals(0, screen.featureList().offset());
        assertTrue(screen.mouseScrolled(40, 100, -2));
        assertEquals(40, screen.featureList().offset());
    }

    @Test void option_labels_prefer_the_feature_specific_key_then_the_global_one_then_the_key() {
        openZoom(texts("hengst.option.zoom.smooth", "Smooth zoom", "hengst.option.smooth", "Smooth"), texts());
        assertTrue(canvas.calls.stream().anyMatch(c -> c.startsWith("text Smooth zoom ")), canvas.calls.toString());
        openZoom(texts("hengst.option.smooth", "Smooth"), texts());
        assertTrue(canvas.calls.stream().anyMatch(c -> c.startsWith("text Smooth ")), canvas.calls.toString());
        openZoom(texts(), texts("hengst.option.zoom.smooth", "Fallback zoom"));
        assertTrue(canvas.calls.stream().anyMatch(c -> c.startsWith("text Fallback zoom ")), canvas.calls.toString());
        openZoom(texts(), texts());
        assertTrue(canvas.calls.stream().anyMatch(c -> c.startsWith("text hengst.option.smooth ")), canvas.calls.toString());
    }

    @Test void choice_values_are_shown_through_the_feature_key_then_the_global_key_then_raw() {
        openZoom(texts("hengst.option.mode", "Mode", "hengst.choice.zoom.hold", "Hold key", "hengst.choice.hold", "Hold"), texts());
        assertTrue(canvas.calls.stream().anyMatch(c -> c.startsWith("text Mode: Hold key ")), canvas.calls.toString());
        openZoom(texts("hengst.option.mode", "Mode", "hengst.choice.hold", "Hold"), texts());
        assertTrue(canvas.calls.stream().anyMatch(c -> c.startsWith("text Mode: Hold ")), canvas.calls.toString());
        openZoom(texts("hengst.option.mode", "Mode"), texts());
        assertTrue(canvas.calls.stream().anyMatch(c -> c.startsWith("text Mode: hold ")), canvas.calls.toString());
        // Clicking stores the raw value, not the shown one.
        openZoom(texts("hengst.choice.zoom.toggle", "Press to toggle"), texts());
        int row = 36 + 20 + 5;                                       // second option row (no HUD rows for zoom)
        screen.mouseClicked(200, row, 0);
        assertEquals("toggle", model.config("zoom").option(model.selected().options().get(1)));
    }
}
