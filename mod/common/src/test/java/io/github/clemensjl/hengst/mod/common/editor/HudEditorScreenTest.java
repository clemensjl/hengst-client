package io.github.clemensjl.hengst.mod.common.editor;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.I18n;
import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.config.Profile;
import io.github.clemensjl.hengst.mod.common.config.ProfileStore;
import io.github.clemensjl.hengst.mod.common.feature.FeatureRegistry;
import io.github.clemensjl.hengst.mod.common.hud.CpsCounter;
import io.github.clemensjl.hengst.mod.common.hud.FrameCounter;
import io.github.clemensjl.hengst.mod.common.hud.HudContext;
import io.github.clemensjl.hengst.mod.common.hud.HudRenderer;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.platform.UiKey;
import io.github.clemensjl.hengst.mod.common.testutil.FakeCanvas;
import io.github.clemensjl.hengst.mod.common.testutil.FakePlatform;
import io.github.clemensjl.hengst.mod.common.testutil.FakeTime;
import io.github.clemensjl.hengst.mod.common.testutil.FixedSizeModule;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Clock;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class HudEditorScreenTest {
    @TempDir Path dir;
    FakeCanvas canvas = new FakeCanvas();
    FakePlatform platform;
    HudContext ctx;
    HudRenderer renderer;
    Profile profile;
    ProfileStore store;
    HudEditorModel model;
    ModuleConfig[] configs;
    int closeCount;

    @BeforeEach
    void setUp() {
        FakeTime t = new FakeTime();
        platform = new FakePlatform(dir);
        ctx = new HudContext(platform, t, new FrameCounter(t), new CpsCounter(t));
        FeatureRegistry reg = new FeatureRegistry();
        reg.register(new FixedSizeModule("a", 1, 40, 10));
        reg.register(new FixedSizeModule("b", 2, 40, 10));
        profile = new Profile();
        profile.module("a").setEnabled(true);
        profile.module("b").setEnabled(true);
        profile.module("b").setOffsetX(250);
        profile.module("b").setOffsetY(100);
        renderer = new HudRenderer(reg, GameVersion.V1_8_9);
        renderer.rebuild(profile);
        configs = renderer.activeConfigs();
        model = new HudEditorModel(renderer.activeModules(), configs);
        store = new ProfileStore(dir.resolve("profiles"), Clock.systemUTC(), platform.warnings::add);
    }

    private Path defaultFile() {
        return dir.resolve("profiles").resolve("default.json");
    }

    private HudEditorScreen screen(Runnable onClose) {
        return new HudEditorScreen(model, ctx, I18n.load("en_us"), onClose);
    }

    private HudEditorScreen closingScreen() {
        return screen(new HudEditorClose(model, store, ProfileStore.DEFAULT, profile, renderer, platform));
    }

    @Test
    void escape_during_drag_ends_drag_and_closes() {
        HudEditorScreen screen = screen(() -> closeCount++);
        screen.render(canvas, 0, 0, 0f);
        screen.mouseClicked(5, 5, 0);
        screen.mouseDragged(50, 50, 0);
        assertTrue(model.dragging());
        assertTrue(screen.keyPressed(UiKey.ESCAPE, false, false));
        assertFalse(model.dragging());
        assertEquals(1, closeCount);
        screen.closed();
        assertEquals(1, closeCount);
    }

    @Test
    void closed_without_escape_runs_on_close_once() {
        HudEditorScreen screen = screen(() -> closeCount++);
        screen.closed();
        screen.closed();
        assertEquals(1, closeCount);
    }

    @Test
    void drag_outside_window_then_escape_clamps_and_saves_exactly_once() throws IOException {
        HudEditorScreen screen = closingScreen();
        screen.render(canvas, 0, 0, 0f);
        screen.mouseClicked(5, 5, 0);
        screen.mouseDragged(-500, 9000, 0);
        screen.mouseReleased(-500, 9000, 0);
        assertFalse(Files.exists(defaultFile()), "nothing is saved while the editor is open");
        model.layout(canvas);
        assertEquals(0, model.rect(0).x);
        assertEquals(230, model.rect(0).y);
        assertTrue(screen.keyPressed(UiKey.ESCAPE, false, false));
        assertTrue(Files.exists(defaultFile()));
        assertEquals(1, platform.closeCount);
        Files.delete(defaultFile());
        screen.closed();
        assertFalse(Files.exists(defaultFile()), "a second save happened");
        assertEquals(1, platform.closeCount);
    }

    @Test
    void escape_in_the_middle_of_a_drag_saves_exactly_once() throws IOException {
        HudEditorScreen screen = closingScreen();
        screen.render(canvas, 0, 0, 0f);
        screen.mouseClicked(5, 5, 0);
        screen.mouseDragged(100, 100, 0);
        screen.keyPressed(UiKey.ESCAPE, false, false);
        assertFalse(model.dragging());
        assertTrue(Files.exists(defaultFile()));
        assertEquals(1, platform.closeCount);
        Files.delete(defaultFile());
        screen.mouseReleased(100, 100, 0);
        screen.closed();
        assertFalse(Files.exists(defaultFile()));
        assertEquals(1, platform.closeCount);
    }

    @Test
    void failed_save_keeps_layout_dirty_and_still_closes() throws IOException {
        Files.write(dir.resolve("profiles"), new byte[0]); // a file where the directory should be
        HudEditorScreen screen = closingScreen();
        screen.render(canvas, 0, 0, 0f);
        screen.mouseClicked(5, 5, 0);
        screen.mouseDragged(100, 50, 0);
        screen.mouseReleased(100, 50, 0);
        int offsetX = configs[0].offsetX();
        screen.keyPressed(UiKey.ESCAPE, false, false);
        assertEquals(1, platform.closeCount);
        assertEquals(1, platform.warnings.size());
        assertTrue(model.dirty());
        assertEquals(offsetX, configs[0].offsetX());
        assertEquals(offsetX, profile.module("a").offsetX());
        // the next attempt, with the directory fixed, saves the kept layout
        Files.delete(dir.resolve("profiles"));
        new HudEditorClose(model, store, ProfileStore.DEFAULT, profile, renderer, platform).run();
        assertTrue(Files.exists(defaultFile()));
        assertFalse(model.dirty());
    }

    @Test
    void close_rebuilds_renderer_after_save() {
        HudEditorScreen screen = closingScreen();
        screen.render(canvas, 0, 0, 0f);
        screen.mouseClicked(5, 5, 0);
        screen.keyPressed(UiKey.DELETE, false, false);
        assertEquals(2, renderer.activeCount());
        screen.keyPressed(UiKey.ESCAPE, false, false);
        assertEquals(1, renderer.activeCount());
        assertTrue(Files.exists(defaultFile()));
    }

    @Test
    void arrow_keys_nudge_and_shift_moves_ten() {
        HudEditorScreen screen = screen(() -> {});
        screen.render(canvas, 0, 0, 0f);
        screen.mouseClicked(5, 5, 0);
        screen.mouseReleased(5, 5, 0);
        assertTrue(screen.keyPressed(UiKey.RIGHT, false, false));
        assertTrue(screen.keyPressed(UiKey.DOWN, true, false));
        model.layout(canvas);
        assertEquals(1, model.rect(0).x);
        assertEquals(10, model.rect(0).y);
        screen.keyPressed(UiKey.LEFT, false, false);
        screen.keyPressed(UiKey.UP, true, false);
        model.layout(canvas);
        assertEquals(0, model.rect(0).x);
        assertEquals(0, model.rect(0).y);
        assertFalse(screen.keyPressed(UiKey.OTHER, false, false));
    }

    @Test
    void scroll_scales_selected_module() {
        HudEditorScreen screen = screen(() -> {});
        screen.render(canvas, 0, 0, 0f);
        screen.mouseClicked(5, 5, 0);
        screen.mouseReleased(5, 5, 0);
        screen.mouseScrolled(5, 5, 1.0);
        assertEquals(1.1f, configs[0].scale(), 1e-6f);
    }

    @Test
    void render_draws_dim_background_modules_frames_hint_and_selection() {
        HudEditorScreen screen = screen(() -> {});
        screen.render(canvas, 0, 0, 0f);
        screen.mouseClicked(5, 5, 0);
        screen.mouseReleased(5, 5, 0);
        canvas.calls.clear();
        screen.render(canvas, 0, 0, 0f);
        assertEquals("fill 0 0 320 240 80000000", canvas.calls.get(0));
        assertTrue(canvas.calls.stream().anyMatch(s -> s.startsWith("fill") && s.endsWith("ffffaa00")));
        assertTrue(canvas.calls.stream().anyMatch(s -> s.startsWith("fill") && s.endsWith("80ffffff")));
        assertEquals(I18n.load("en_us").get("hengst.editor.hint"), String.join(" ", textLines(canvas)));
        assertEquals(2, canvas.calls.stream().filter(s -> s.startsWith("push")).count());
    }

    @Test
    void placeholder_module_shows_its_name() {
        FeatureRegistry reg = new FeatureRegistry();
        reg.register(new FixedSizeModule("fps", 1, 0, 0));
        Profile p = new Profile();
        p.module("fps").setEnabled(true);
        HudRenderer r = new HudRenderer(reg, GameVersion.V1_8_9);
        r.rebuild(p);
        HudEditorModel m = new HudEditorModel(r.activeModules(), r.activeConfigs());
        HudEditorScreen screen = new HudEditorScreen(m, ctx, I18n.load("en_us"), () -> {});
        screen.render(canvas, 0, 0, 0f);
        assertTrue(canvas.calls.stream().anyMatch(s -> s.startsWith("text FPS ")));
    }

    private static FakeCanvas canvasOfWidth(int w) {
        return new FakeCanvas() {
            @Override
            public int width() {
                return w;
            }
        };
    }

    private static java.util.List<String> textLines(FakeCanvas c) {
        java.util.List<String> out = new java.util.ArrayList<String>();
        for (String call : c.calls) {
            if (call.startsWith("text ")) {
                String[] t = call.split(" ");
                out.add(String.join(" ", java.util.Arrays.copyOfRange(t, 1, t.length - 4)));
            }
        }
        return out;
    }

    @Test
    void hint_is_wrapped_to_the_screen_width_in_both_languages() {
        for (String lang : new String[] {"en_us", "de_de"}) {
            I18n i18n = I18n.load(lang);
            for (int width : new int[] {320, 480}) {
                FakeCanvas c = canvasOfWidth(width);
                new HudEditorScreen(model, ctx, i18n, () -> {}).render(c, 0, 0, 0f);
                java.util.List<String> lines = textLines(c);
                assertTrue(lines.size() > 1);
                for (String line : lines) {
                    assertTrue(6 * line.length() <= width, lang + " " + width + ": " + line);
                }
                assertEquals(i18n.get("hengst.editor.hint"), String.join(" ", lines));
            }
        }
    }

    @Test
    void render_draws_exact_frames_and_centred_hint_on_a_backing_box() {
        HudEditorScreen screen = screen(() -> {});
        screen.render(canvas, 0, 0, 0f);
        screen.mouseClicked(5, 5, 0);
        screen.mouseReleased(5, 5, 0);
        canvas.calls.clear();
        screen.render(canvas, 0, 0, 0f);
        java.util.List<String> c = canvas.calls;
        assertEquals("fill 0 0 320 240 80000000", c.get(0));
        // module a (selected, 0,0 40x10)
        assertEquals("push 0.0 0.0 1.0", c.get(1));
        assertEquals("fill 0 0 40 10 ffffffff", c.get(2));
        assertEquals("pop", c.get(3));
        assertEquals("fill 0 0 40 1 ffffaa00", c.get(4));
        assertEquals("fill 0 9 40 10 ffffaa00", c.get(5));
        assertEquals("fill 0 1 1 9 ffffaa00", c.get(6));
        assertEquals("fill 39 1 40 9 ffffaa00", c.get(7));
        // module b (250,100 40x10)
        assertEquals("push 250.0 100.0 1.0", c.get(8));
        assertEquals("fill 250 100 290 101 80ffffff", c.get(11));
        assertEquals("fill 250 109 290 110 80ffffff", c.get(12));
        assertEquals("fill 250 101 251 109 80ffffff", c.get(13));
        assertEquals("fill 289 101 290 109 80ffffff", c.get(14));
        java.util.List<String> lines = textLines(canvas);
        int n = lines.size();
        int maxW = 0;
        for (String line : lines) {
            maxW = Math.max(maxW, 6 * line.length());
        }
        int boxW = maxW + 8;
        int boxH = (n - 1) * 10 + 9 + 8;
        int bx = (320 - boxW) / 2;
        int by = (240 - boxH) / 2;
        assertEquals("fill " + bx + " " + by + " " + (bx + boxW) + " " + (by + boxH) + " c0000000", c.get(15));
        for (int k = 0; k < n; k++) {
            String line = lines.get(k);
            int y = by + 4 + k * 10;
            int x = (320 - 6 * line.length()) / 2;
            assertEquals("text " + line + " " + x + " " + y + " ffffffff true", c.get(16 + k));
        }
        assertEquals(16 + n, c.size());
    }

    private static FakeCanvas canvasOfSize(int w, int h) {
        return new FakeCanvas() {
            @Override
            public int width() {
                return w;
            }

            @Override
            public int height() {
                return h;
            }
        };
    }

    @Test
    void dim_is_the_first_call_and_covers_the_whole_canvas() {
        for (int[] size : new int[][] {{320, 240}, {854, 480}}) {
            FakeCanvas c = canvasOfSize(size[0], size[1]);
            screen(() -> {}).render(c, 0, 0, 0f);
            assertEquals("fill 0 0 " + size[0] + " " + size[1] + " 80000000", c.calls.get(0));
        }
    }

    /** The backing box of the hint: the fill drawn right after the module frames, parsed as x1 y1 x2 y2. */
    private static int[] hintBox(FakeCanvas c) {
        for (String call : c.calls) {
            if (call.endsWith(" c0000000")) {
                String[] t = call.split(" ");
                return new int[] {
                    Integer.parseInt(t[1]), Integer.parseInt(t[2]), Integer.parseInt(t[3]), Integer.parseInt(t[4])
                };
            }
        }
        throw new AssertionError("no hint box drawn");
    }

    @Test
    void a_module_under_the_hint_box_can_still_be_grabbed_and_empty_space_under_it_is_not() {
        configs[0].setOffsetX(150);
        configs[0].setOffsetY(110);
        HudEditorScreen screen = screen(() -> {});
        screen.render(canvas, 0, 0, 0f);
        int[] box = hintBox(canvas);
        // module a (150,110, 40x10) lies inside the box
        assertTrue(box[0] < 150 && box[2] > 190 && box[1] < 110 && box[3] > 120);
        screen.mouseClicked(160, 115, 0);
        assertEquals(0, model.selected());
        assertTrue(model.dragging());
        screen.mouseReleased(160, 115, 0);
        // empty space inside the box, away from the module: nothing is grabbed and the selection is cleared
        screen.mouseClicked(box[0] + 2, box[1] + 2, 0);
        assertEquals(-1, model.selected());
        assertFalse(model.dragging());
    }

    @Test
    void the_dragged_module_is_drawn_after_the_hint() {
        configs[0].setOffsetX(150);
        configs[0].setOffsetY(110);
        HudEditorScreen screen = screen(() -> {});
        screen.render(canvas, 0, 0, 0f);
        screen.mouseClicked(160, 115, 0);
        assertTrue(model.dragging());
        canvas.calls.clear();
        screen.render(canvas, 160, 115, 0f);
        java.util.List<String> c = canvas.calls;
        int boxIndex = -1;
        int lastText = -1;
        int pushA = -1;
        for (int i = 0; i < c.size(); i++) {
            if (c.get(i).endsWith(" c0000000")) {
                boxIndex = i;
            }
            if (c.get(i).startsWith("text ")) {
                lastText = i;
            }
            if (c.get(i).startsWith("push 150.0 110.0")) {
                pushA = i;
            }
        }
        assertTrue(boxIndex > 0 && lastText > boxIndex);
        assertTrue(pushA > lastText, "dragged module must follow the hint");
        // and its frame follows too: the last four calls are the selected frame
        assertEquals("fill 189 111 190 119 ffffaa00", c.get(c.size() - 1));
        // module b is still drawn once, before the hint
        assertEquals(1, c.stream().filter(s -> s.startsWith("push 250.0 100.0")).count());
        assertTrue(c.indexOf("push 250.0 100.0 1.0") < boxIndex);
    }

    @Test
    void non_left_buttons_are_ignored() {
        HudEditorScreen screen = screen(() -> {});
        screen.render(canvas, 0, 0, 0f);
        assertFalse(screen.mouseClicked(5, 5, 1));
        assertEquals(-1, model.selected());
        screen.mouseClicked(5, 5, 0);
        assertFalse(screen.mouseDragged(100, 100, 1));
        assertFalse(screen.mouseReleased(100, 100, 1));
        assertTrue(model.dragging());
        model.layout(canvas);
        assertEquals(0, model.rect(0).x);
    }

    @Test
    void mouse_up_without_mouse_down_changes_nothing() {
        HudEditorScreen screen = screen(() -> {});
        screen.render(canvas, 0, 0, 0f);
        screen.mouseReleased(5, 5, 0);
        screen.mouseDragged(100, 100, 0);
        assertFalse(model.dragging());
        assertFalse(model.dirty());
        assertEquals(-1, model.selected());
    }

    @Test
    void close_still_closes_when_the_save_throws_unchecked() {
        screen(() -> {}).render(canvas, 0, 0, 0f);
        model.press(5, 5);
        model.drag(100, 100);
        model.release();
        new HudEditorClose(model, store, "bad/name", profile, renderer, platform).run();
        assertEquals(1, platform.closeCount);
        assertEquals(1, platform.warnings.size());
        assertTrue(model.dirty());
    }

    @Test
    void close_still_closes_when_rebuild_throws() {
        new HudEditorClose(model, store, ProfileStore.DEFAULT, null, renderer, platform).run();
        assertEquals(1, platform.closeCount);
        assertEquals(1, platform.warnings.size());
    }
}
