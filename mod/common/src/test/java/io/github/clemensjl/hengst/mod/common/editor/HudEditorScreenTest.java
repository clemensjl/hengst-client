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
        assertEquals("fill 0 0 320 240 50000000", canvas.calls.get(0));
        assertTrue(canvas.calls.stream().anyMatch(s -> s.startsWith("fill") && s.endsWith("ffffaa00")));
        assertTrue(canvas.calls.stream().anyMatch(s -> s.startsWith("fill") && s.endsWith("80ffffff")));
        String hint = I18n.load("en_us").get("hengst.editor.hint");
        assertTrue(canvas.calls.stream().anyMatch(s -> s.startsWith("text " + hint + " ")));
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
}
