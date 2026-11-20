package io.github.clemensjl.hengst.mod.common.ui;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.config.Profile;
import io.github.clemensjl.hengst.mod.common.config.ProfileStore;
import io.github.clemensjl.hengst.mod.common.config.Settings;
import io.github.clemensjl.hengst.mod.common.feature.FeatureRegistry;
import io.github.clemensjl.hengst.mod.common.hud.modules.FpsModule;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Clock;
import java.time.Instant;
import java.time.ZoneOffset;
import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/** Hook failures and warning lifetimes of {@link SettingsModel}. */
class SettingsModelHooksTest {
    @TempDir Path dir;
    Clock clock = Clock.fixed(Instant.parse("2026-11-20T10:15:30Z"), ZoneOffset.UTC);
    List<String> warnings = new ArrayList<>();
    ProfileStore store;
    Settings settings = new Settings();
    Profile[] active = {new Profile()};
    boolean changedThrows;
    boolean switchThrows;
    int saves;
    SettingsModel model;

    @BeforeEach
    void setUp() {
        store = new ProfileStore(dir, clock, warnings::add);
        FeatureRegistry reg = new FeatureRegistry();
        reg.register(new FpsModule());
        model = new SettingsModel(reg, GameVersion.V1_8_9, store, settings, () -> active[0],
            name -> {
                if (switchThrows) throw new IllegalStateException("switch hook");
                settings.setActiveProfile(name);
                active[0] = store.load(name);
            },
            () -> { if (changedThrows) throw new IllegalStateException("changed hook"); },
            () -> { },
            () -> {
                saves++;
                try { store.save(settings.activeProfile(), active[0]); } catch (IOException e) { throw new UncheckedIOException(e); }
            },
            warnings::add);
    }

    @Test void a_throwing_changed_hook_does_not_escape_and_keeps_the_state() {
        changedThrows = true;
        model.config("fps").setEnabled(true);
        model.markChanged();
        assertTrue(model.dirty());
        assertTrue(model.config("fps").enabled());
        assertEquals(1, warnings.size());
        assertTrue(warnings.get(0).contains("changed hook"));
    }

    private String read(String name) throws IOException {
        return new String(Files.readAllBytes(dir.resolve(name)), StandardCharsets.UTF_8);
    }

    @Test void failing_switch_after_the_delete_never_writes_the_deleted_profile_anywhere() throws Exception {
        String copy = model.copyActive();
        assertEquals("Profil 2", copy);
        String defaultBefore = read("default.json");
        model.config("fps").setEnabled(true);
        model.markChanged();
        switchThrows = true;
        model.deleteActive();
        assertFalse(Files.exists(dir.resolve("Profil 2.json")));
        assertEquals("default", model.activeProfileName());
        assertEquals(SettingsModel.KEY_SWITCH_FAILED, model.failureKey());
        assertFalse(model.dirty());
        int before = saves;
        model.markChanged();
        assertFalse(model.saveIfDirty());                    // refuses while the data belongs to the deleted profile
        assertEquals(before, saves);
        assertEquals(defaultBefore, read("default.json"));
        assertFalse(Files.exists(dir.resolve("Profil 2.json")));
        switchThrows = false;
        model.switchTo("default");                           // retry works although the name is already default
        assertEquals(SettingsModel.KEY_SWITCH_FAILED, model.failureKey());   // the edit made while detached was dropped
        assertFalse(model.dirty());
        model.config("fps").setEnabled(true);
        model.markChanged();
        assertTrue(model.saveIfDirty());
        assertTrue(read("default.json").contains("fps"));
    }

    @Test void a_successful_switch_keeps_the_settings_load_failed_warning() throws Exception {
        Files.createDirectories(dir);
        store.save("Profil 2", new Profile());
        model.setSettingsLoadFailedCheck(() -> true);
        model.warnLoadProblems();
        assertEquals(SettingsModel.KEY_SETTINGS_LOAD_FAILED, model.failureKey());
        model.switchTo("Profil 2");
        assertEquals("Profil 2", model.activeProfileName());
        assertEquals(SettingsModel.KEY_SETTINGS_LOAD_FAILED, model.failureKey());
        model.markChanged();
        assertTrue(model.saveIfDirty());                     // a successful save does not clear it either
        assertEquals(SettingsModel.KEY_SETTINGS_LOAD_FAILED, model.failureKey());
    }

    @Test void a_successful_action_still_clears_its_own_warning() {
        model.fail(SettingsModel.KEY_SWITCH_FAILED, "x");
        model.switchTo("default");                           // same name: no-op, warning stays
        assertEquals(SettingsModel.KEY_SWITCH_FAILED, model.failureKey());
        model.markChanged();
        assertTrue(model.saveIfDirty());
        assertEquals(null, model.failureKey());
    }
}
