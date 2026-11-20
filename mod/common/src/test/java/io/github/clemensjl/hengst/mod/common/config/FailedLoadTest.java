package io.github.clemensjl.hengst.mod.common.config;

import static java.nio.charset.StandardCharsets.UTF_8;
import static org.junit.jupiter.api.Assertions.*;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Clock;
import java.time.Instant;
import java.time.ZoneOffset;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/** A failed load must never be followed by a silent overwrite; schema guard; failed saves; name helpers. */
class FailedLoadTest {
    @TempDir Path dir;
    Clock clock = Clock.fixed(Instant.parse("2026-11-20T10:15:30Z"), ZoneOffset.UTC);
    List<String> warnings = new ArrayList<>();
    AtomicFiles.Mover real = AtomicFiles.mover;

    @AfterEach void restoreMover() { AtomicFiles.mover = real; }

    ProfileStore store() { return new ProfileStore(dir, clock, warnings::add); }
    SettingsStore settings() { return new SettingsStore(dir.resolve("settings.json"), clock, warnings::add); }

    private static AtomicFiles.Mover failing(final boolean failCopy) {
        return new AtomicFiles.Mover() {
            public Path move(Path from, Path to) throws IOException { throw new IOException("move blocked"); }
            public Path copy(Path from, Path to) throws IOException {
                if (failCopy) throw new IOException("copy blocked");
                return Files.copy(from, to);
            }
        };
    }

    private boolean hasFileWithBytes(byte[] bytes) throws Exception {
        try (java.util.stream.Stream<Path> s = Files.list(dir)) {
            for (Path p : (Iterable<Path>) s::iterator) {
                if (Files.isRegularFile(p) && Arrays.equals(Files.readAllBytes(p), bytes)) return true;
            }
        }
        return false;
    }

    // ---- profiles ----

    @Test void directory_at_profile_path_is_never_overwritten() throws Exception {
        Path d = dir.resolve("default.json");
        Files.createDirectory(d);
        Files.write(d.resolve("keep.txt"), "x".getBytes(UTF_8));
        ProfileStore s = store();
        assertFalse(s.load("default").module("fps").enabled());
        assertTrue(s.loadFailed("default"));
        assertFalse(s.loadedFromDefaults());
        assertEquals(1, warnings.size());
        assertThrows(IOException.class, () -> s.save("default", new Profile()));
        assertTrue(Files.isDirectory(d));
        assertTrue(Files.exists(d.resolve("keep.txt")));
        assertEquals(Arrays.asList("default"), s.names());
    }

    @Test void damaged_file_that_cannot_be_moved_or_copied_is_not_overwritten() throws Exception {
        byte[] bad = new byte[] {'{', (byte) 0xC3, '}'};
        Files.write(dir.resolve("default.json"), bad);
        AtomicFiles.mover = failing(true);
        ProfileStore s = store();
        assertFalse(s.load("default").module("fps").enabled());
        assertTrue(s.loadFailed("default"));
        assertFalse(s.loadedFromDefaults());
        assertThrows(IOException.class, () -> s.save("default", new Profile()));
        assertArrayEquals(bad, Files.readAllBytes(dir.resolve("default.json")));
    }

    @Test void damaged_file_is_backed_up_by_copy_when_the_move_keeps_failing() throws Exception {
        byte[] bad = new byte[] {'{', (byte) 0xC3, '}'};
        Files.write(dir.resolve("default.json"), bad);
        ProfileStore s = store();
        AtomicFiles.mover = failing(false);
        s.load("default");
        assertTrue(s.loadFailed("default"));
        Profile p = new Profile(); p.module("fps").setEnabled(true);
        s.save("default", p);
        AtomicFiles.mover = real;
        assertFalse(s.loadFailed("default"));
        assertTrue(hasFileWithBytes(bad));
        assertTrue(store().load("default").module("fps").enabled());
    }

    @Test void unreadable_file_that_becomes_valid_gets_a_backup_before_it_is_replaced() throws Exception {
        Path f = dir.resolve("default.json");
        Files.createDirectory(f);
        ProfileStore s = store();
        s.load("default");
        Files.delete(f);
        byte[] good = "{\"schema\":1,\"modules\":{\"fps\":{\"enabled\":true}}}".getBytes(UTF_8);
        Files.write(f, good);
        s.save("default", new Profile());
        assertTrue(hasFileWithBytes(good));
        assertFalse(store().load("default").module("fps").enabled());
    }

    @Test void create_copy_refuses_a_source_that_failed_to_load() throws Exception {
        Files.createDirectory(dir.resolve("default.json"));
        assertThrows(IOException.class, () -> store().createCopy("default"));
        assertFalse(Files.exists(dir.resolve("Profil 2.json")));
    }

    @Test void normal_broken_file_still_backs_up_and_allows_save() throws Exception {
        Files.write(dir.resolve("default.json"), "{".getBytes(UTF_8));
        ProfileStore s = store();
        s.load("default");
        assertFalse(s.loadFailed("default"));
        assertTrue(s.loadedFromDefaults());
        s.save("default", new Profile());
        assertTrue(Files.exists(dir.resolve("default.json.broken-20261120-101530")));
        assertFalse(store().load("default").module("fps").enabled());
    }

    @Test void newer_schema_file_is_never_overwritten() throws Exception {
        byte[] newer = "{\"schema\":2,\"modules\":{\"fps\":{\"enabled\":true}}}".getBytes(UTF_8);
        Files.write(dir.resolve("default.json"), newer);
        ProfileStore s = store();
        assertFalse(s.load("default").module("fps").enabled());
        assertTrue(s.loadFailed("default"));
        assertFalse(s.loadedFromDefaults());
        assertEquals(1, warnings.size());
        IOException e = assertThrows(IOException.class, () -> s.save("default", new Profile()));
        assertTrue(e.getMessage().contains("newer version"));
        assertArrayEquals(newer, Files.readAllBytes(dir.resolve("default.json")));
        assertFalse(Files.exists(dir.resolve("default.json.tmp")));
    }

    // ---- failed saves ----

    @Test void save_into_a_non_empty_directory_fails_cleanly() throws Exception {
        Path d = dir.resolve("default.json");
        Files.createDirectory(d);
        Files.write(d.resolve("keep.txt"), "x".getBytes(UTF_8));
        assertThrows(IOException.class, () -> store().save("default", new Profile()));
        assertTrue(Files.exists(d.resolve("keep.txt")));
        assertFalse(Files.exists(dir.resolve("default.json.tmp")));
    }

    @Test void failed_save_keeps_the_existing_valid_file() throws Exception {
        Profile p = new Profile(); p.module("fps").setEnabled(true);
        store().save("default", p);
        byte[] before = Files.readAllBytes(dir.resolve("default.json"));
        // I/O failure: a non-empty directory occupies the temp file name
        Files.createDirectory(dir.resolve("default.json.tmp"));
        Files.write(dir.resolve("default.json.tmp").resolve("k"), new byte[0]);
        assertThrows(IOException.class, () -> store().save("default", new Profile()));
        assertArrayEquals(before, Files.readAllBytes(dir.resolve("default.json")));
        // serialization failure
        Profile bad = new Profile(); bad.module("fps").setOption("o", new Object());
        assertThrows(IllegalArgumentException.class, () -> store().save("default", bad));
        assertArrayEquals(before, Files.readAllBytes(dir.resolve("default.json")));
    }

    @Test void names_lists_regular_files_only() throws Exception {
        Files.createDirectory(dir.resolve("x.json"));
        Files.write(dir.resolve("PvP.json"), new byte[0]);
        assertEquals(Arrays.asList("PvP", "default"), store().names());
    }

    @Test void same_name_is_case_insensitive() {
        assertTrue(ProfileStore.sameName("Default", "default"));
        assertTrue(ProfileStore.sameName("PvP", "pvp"));
        assertFalse(ProfileStore.sameName("PvP", "PvP 2"));
        assertFalse(ProfileStore.sameName(null, "a"));
        assertFalse(ProfileStore.sameName(null, null));
    }

    // ---- settings ----

    @Test void directory_at_settings_path_is_never_overwritten() throws Exception {
        Path d = dir.resolve("settings.json");
        Files.createDirectory(d);
        SettingsStore s = settings();
        assertEquals("default", s.load().activeProfile());
        assertTrue(s.settingsLoadFailed());
        assertThrows(IOException.class, () -> s.save(new Settings()));
        assertTrue(Files.isDirectory(d));
    }

    @Test void damaged_settings_that_cannot_be_backed_up_are_not_overwritten() throws Exception {
        byte[] bad = new byte[] {'{', (byte) 0xC3};
        Files.write(dir.resolve("settings.json"), bad);
        AtomicFiles.mover = failing(true);
        SettingsStore s = settings();
        s.load();
        assertTrue(s.settingsLoadFailed());
        assertThrows(IOException.class, () -> s.save(new Settings()));
        assertArrayEquals(bad, Files.readAllBytes(dir.resolve("settings.json")));
    }

    @Test void newer_schema_settings_are_never_overwritten() throws Exception {
        byte[] newer = "{\"schema\":5,\"activeProfile\":\"PvP\"}".getBytes(UTF_8);
        Files.write(dir.resolve("settings.json"), newer);
        SettingsStore s = settings();
        assertEquals("default", s.load().activeProfile());
        assertTrue(s.settingsLoadFailed());
        assertThrows(IOException.class, () -> s.save(new Settings()));
        assertArrayEquals(newer, Files.readAllBytes(dir.resolve("settings.json")));
    }

    @Test void normal_broken_settings_allow_save() throws Exception {
        Files.write(dir.resolve("settings.json"), "{".getBytes(UTF_8));
        SettingsStore s = settings();
        s.load();
        assertFalse(s.settingsLoadFailed());
        s.save(new Settings());
        assertEquals("default", settings().load().activeProfile());
    }
}
