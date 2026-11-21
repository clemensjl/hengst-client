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
    @Test void notice_is_drawn_wrapped_unscaled_and_fits_the_width() {
        screen.render(canvas, 0, 0, 0f);
        String notice = I18n.load("en_us").get("hengst.notice");
        List<Drawn> lines = textCalls(canvas, "ffaaaaaa");
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
        assertEquals(20, list.offset());
        for (int i = 0; i < 50; i++) screen.mouseScrolled(50, 100, -1);
        // whole rows only: the last row ends inside the list, no partial row at the top
        assertEquals((12 - list.height() / 20) * 20, list.offset());
        for (int i = 0; i < 50; i++) screen.mouseScrolled(50, 100, 1);
        assertEquals(0, list.offset());
        assertFalse(screen.mouseScrolled(5, 230, 1));
        // one notch is one whole row: row 0 is above the list, row 1 starts at the list top (y 36)
        screen.mouseScrolled(50, 100, -1);
        screen.render(canvas, 0, 0, 0f);
        assertEquals(0, changed);
        assertTrue(screen.mouseClicked(12, 36 + 5, 0));
        assertTrue(model.config("f1").enabled());
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
            screen.mouseClicked(60, 36 + row * 20 + 5, 0);
            canvas.calls.clear();
            screen.render(canvas, 0, 0, 0f);
            List<Drawn> texts = textCalls(canvas, null);
            assertFalse(texts.isEmpty());
            checkFits(texts);
            // hint-like free texts: notice and warning are wrapped, not cut
            assertEquals(I18n.load("de_de").get("hengst.notice"), joined(textCalls(canvas, "ffaaaaaa")));
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
        String cut = ChoiceWidget.middleCut(c, name, 76);
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
