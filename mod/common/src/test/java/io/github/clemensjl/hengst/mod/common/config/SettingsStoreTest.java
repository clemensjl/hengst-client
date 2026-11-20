package io.github.clemensjl.hengst.mod.common.config;

import static java.nio.charset.StandardCharsets.UTF_8;
import static org.junit.jupiter.api.Assertions.*;

import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Clock;
import java.time.Instant;
import java.time.ZoneOffset;
import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class SettingsStoreTest {
    @TempDir Path dir;
    Clock clock = Clock.fixed(Instant.parse("2026-11-20T10:15:30Z"), ZoneOffset.UTC);
    List<String> warnings = new ArrayList<>();

    SettingsStore store() { return new SettingsStore(dir.resolve("settings.json"), clock, warnings::add); }

    @Test void settings_roundtrip() throws Exception {
        Settings s = new Settings(); s.setActiveProfile("Profil 2"); store().save(s);
        assertEquals("Profil 2", store().load().activeProfile());
    }

    @Test void missing_settings_file_gives_default_without_warning() {
        assertEquals("default", store().load().activeProfile()); assertTrue(warnings.isEmpty());
    }

    @Test void broken_settings_file_backs_up_and_gives_default() throws Exception {
        Files.write(dir.resolve("settings.json"), "{".getBytes(UTF_8));
        assertEquals("default", store().load().activeProfile());
        assertTrue(Files.exists(dir.resolve("settings.json.broken-20261120-101530")));
        assertEquals(1, warnings.size());
    }

    @Test void invalid_active_profile_name_gives_default() throws Exception {
        Files.write(dir.resolve("settings.json"), "{\"schema\":1,\"activeProfile\":\"../x\"}".getBytes(UTF_8));
        assertEquals("default", store().load().activeProfile());
    }

    @Test void wrong_typed_active_profile_gives_default() throws Exception {
        Files.write(dir.resolve("settings.json"), "{\"schema\":1,\"activeProfile\":5}".getBytes(UTF_8));
        assertEquals("default", store().load().activeProfile());
    }

    @Test void save_after_broken_load_writes_valid_file() throws Exception {
        Files.write(dir.resolve("settings.json"), "".getBytes(UTF_8));
        Settings s = store().load(); s.setActiveProfile("PvP"); store().save(s);
        assertEquals("PvP", store().load().activeProfile());
        assertEquals(1, warnings.size());
    }

    @Test void invalid_name_is_rejected_by_the_setter() {
        assertThrows(IllegalArgumentException.class, () -> new Settings().setActiveProfile("../x"));
    }
}
