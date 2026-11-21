package io.github.clemensjl.hengst.mod.common.config;

import static java.nio.charset.StandardCharsets.UTF_8;
import static org.junit.jupiter.api.Assertions.*;

import io.github.clemensjl.hengst.mod.common.feature.OptionSpec;
import io.github.clemensjl.hengst.mod.common.layout.Anchor;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Clock;
import java.time.Instant;
import java.time.ZoneOffset;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class ProfileStoreTest {
    @TempDir Path dir;
    Clock clock = Clock.fixed(Instant.parse("2026-11-20T10:15:30Z"), ZoneOffset.UTC);
    List<String> warnings = new ArrayList<>();

    ProfileStore store() { return new ProfileStore(dir, clock, warnings::add); }

    private static String repeat(char c, int n) {
        StringBuilder b = new StringBuilder();
        for (int i = 0; i < n; i++) b.append(c);
        return b.toString();
    }

    private String read(String file) throws Exception {
        return new String(Files.readAllBytes(dir.resolve(file)), UTF_8);
    }

    @Test void missing_profile_loads_defaults() { assertFalse(store().load("default").module("fps").enabled()); }

    @Test void save_then_load_roundtrip() throws Exception {
        Profile p = new Profile(); ModuleConfig fps = p.module("fps");
        fps.setEnabled(true); fps.setAnchor(Anchor.TOP_RIGHT); fps.setOffsetX(-3); fps.setScale(1.5f);
        store().save("default", p);
        ModuleConfig back = store().load("default").module("fps");
        assertTrue(back.enabled()); assertEquals(Anchor.TOP_RIGHT, back.anchor()); assertEquals(-3, back.offsetX()); assertEquals(1.5f, back.scale());
    }

    @Test void load_broken_profile_backs_up_and_returns_defaults() throws Exception {
        Files.write(dir.resolve("default.json"), "{\"schema\":1,\"modules\":{".getBytes(UTF_8));
        assertFalse(store().load("default").module("fps").enabled());
        assertTrue(Files.exists(dir.resolve("default.json.broken-20261120-101530")));
        assertEquals(1, warnings.size());
    }

    @Test void load_empty_file_backs_up_and_returns_defaults() throws Exception {
        Files.write(dir.resolve("default.json"), new byte[0]);
        assertFalse(store().load("default").module("fps").enabled());
        assertTrue(Files.exists(dir.resolve("default.json.broken-20261120-101530")));
        assertEquals(1, warnings.size());
    }

    @Test void save_after_broken_load_writes_valid_file() throws Exception {
        Files.write(dir.resolve("default.json"), "{".getBytes(UTF_8));
        Profile p = store().load("default"); p.module("fps").setEnabled(true);
        store().save("default", p);
        assertTrue(store().load("default").module("fps").enabled());
        assertEquals(1, warnings.size());
    }

    @Test void loaded_from_defaults_flag() throws Exception {
        ProfileStore s = store();
        s.load("default"); assertTrue(s.loadedFromDefaults());
        s.save("default", new Profile());
        s.load("default"); assertFalse(s.loadedFromDefaults());
    }

    @Test void loaded_from_defaults_flag_after_broken_file_was_moved_aside() throws Exception {
        Files.write(dir.resolve("default.json"), "{".getBytes(UTF_8));
        ProfileStore s = store();
        s.load("default"); assertTrue(s.loadedFromDefaults());
        s.load("default"); assertTrue(s.loadedFromDefaults());
    }

    @Test void text_color_and_float_option_roundtrip() throws Exception {
        Profile p = new Profile(); ModuleConfig m = p.module("fps");
        m.setTextColor(0xFFFFAA00); m.setOption("opacity", 1.3f);
        store().save("default", p);
        String json = read("default.json");
        assertTrue(json.contains("4294945280")); assertTrue(json.contains("1.3"));
        assertFalse(json.contains("1.2999"));
        ModuleConfig back = store().load("default").module("fps");
        assertEquals(0xFFFFAA00, back.textColor());
        assertEquals(1.3f, (Float) back.option(OptionSpec.floatRange("opacity", 1f, 0.5f, 3f, 0.1f)), 1e-6f);
    }

    @Test void scale_is_written_as_short_decimal() throws Exception {
        Profile p = new Profile(); p.module("fps").setScale(1.3f);
        store().save("default", p);
        assertTrue(read("default.json").contains("\"scale\": 1.3"));
    }

    @Test void unknown_modules_survive_a_save() throws Exception {
        Files.write(dir.resolve("default.json"), "{\"schema\":1,\"modules\":{\"future\":{\"x\":[1,2]}}}".getBytes(UTF_8));
        Profile p = store().load("default"); p.module("fps").setEnabled(true); store().save("default", p);
        assertTrue(read("default.json").contains("\"future\""));
    }

    @Test void untouched_unknown_module_is_written_back_unchanged() throws Exception {
        Files.write(dir.resolve("default.json"), "{\"schema\":1,\"modules\":{\"future\":{\"x\":[1,2],\"y\":\"z\"}}}".getBytes(UTF_8));
        Profile p = store().load("default");
        assertTrue(p.moduleIds().contains("future"));
        store().save("default", p);
        Profile back = store().load("default");
        assertEquals(p.toJson(), back.toJson());
        String json = read("default.json");
        assertTrue(json.contains("\"y\": \"z\""));
        assertFalse(json.contains("enabled"));
    }

    @Test void options_of_unknown_name_survive_a_save() throws Exception {
        Files.write(dir.resolve("default.json"), "{\"schema\":1,\"modules\":{\"fps\":{\"enabled\":true,\"options\":{\"newer\":7}}}}".getBytes(UTF_8));
        Profile p = store().load("default"); p.module("fps").setEnabled(false); store().save("default", p);
        assertTrue(read("default.json").contains("\"newer\": 7"));
    }

    @Test void wrong_types_fall_back_per_field() throws Exception {
        Files.write(dir.resolve("default.json"), "{\"schema\":1,\"modules\":{\"fps\":{\"enabled\":\"yes\",\"scale\":99,\"anchor\":\"NOWHERE\"}}}".getBytes(UTF_8));
        ModuleConfig c = store().load("default").module("fps");
        assertFalse(c.enabled()); assertEquals(3.0f, c.scale()); assertEquals(Anchor.TOP_LEFT, c.anchor());
    }

    @Test void non_whole_offsets_and_wrong_shaped_sections_use_defaults() throws Exception {
        Files.write(dir.resolve("default.json"), "{\"schema\":1,\"modules\":{\"fps\":{\"x\":2.0,\"y\":\"7\",\"options\":[1],\"textColor\":\"red\"},\"bad\":5}}".getBytes(UTF_8));
        Profile p = store().load("default");
        ModuleConfig c = p.module("fps");
        assertEquals(0, c.offsetX()); assertEquals(0, c.offsetY()); assertEquals(0xFFFFFFFF, c.textColor());
        assertEquals(5, c.option(OptionSpec.intRange("n", 5, 0, 10)));
        assertFalse(p.module("bad").enabled());
    }

    @Test void non_object_root_counts_as_broken() throws Exception {
        Files.write(dir.resolve("default.json"), "[1,2]".getBytes(UTF_8));
        assertFalse(store().load("default").module("fps").enabled());
        assertTrue(Files.exists(dir.resolve("default.json.broken-20261120-101530")));
        assertEquals(1, warnings.size());
    }

    @Test void invalid_utf8_counts_as_broken() throws Exception {
        Files.write(dir.resolve("default.json"), new byte[] {'{', (byte) 0xC3, '}'});
        ProfileStore s = store();
        assertFalse(s.load("default").module("fps").enabled());
        assertTrue(s.loadedFromDefaults());
        assertTrue(Files.exists(dir.resolve("default.json.broken-20261120-101530")));
    }

    @Test void second_broken_file_in_the_same_second_does_not_overwrite_the_first() throws Exception {
        Files.write(dir.resolve("default.json"), "{".getBytes(UTF_8));
        store().load("default");
        Files.write(dir.resolve("default.json"), "[".getBytes(UTF_8));
        store().load("default");
        assertEquals("{", read("default.json.broken-20261120-101530"));
        assertEquals("[", read("default.json.broken-20261120-101530-2"));
    }

    @Test void huge_option_values_are_clamped_by_the_spec() throws Exception {
        Files.write(dir.resolve("default.json"), "{\"schema\":1,\"modules\":{\"fps\":{\"options\":{\"o\":1e300,\"i\":-9999999999}}}}".getBytes(UTF_8));
        ModuleConfig c = store().load("default").module("fps");
        assertEquals(3f, (Float) c.option(OptionSpec.floatRange("o", 1f, 0.5f, 3f, 0.1f)));
        assertEquals(0, c.option(OptionSpec.intRange("i", 5, 0, 10)));
    }

    @Test void non_finite_values_never_break_a_save() throws Exception {
        Profile p = new Profile(); ModuleConfig m = p.module("fps");
        m.setScale(Float.NaN); m.setOption("bad", Double.POSITIVE_INFINITY); m.setOption("badf", Float.NaN);
        store().save("default", p);
        assertEquals(1.0f, store().load("default").module("fps").scale());
    }

    @Test void save_overwrites_and_leaves_no_tmp() throws Exception {
        Files.write(dir.resolve("default.json.tmp"), "stale".getBytes(UTF_8));
        Profile p = new Profile(); p.module("fps").setEnabled(true);
        store().save("default", p);
        assertTrue(store().load("default").module("fps").enabled());
        store().save("default", new Profile());
        assertFalse(Files.exists(dir.resolve("default.json.tmp")));
        assertFalse(store().load("default").module("fps").enabled());
    }

    @Test void save_creates_missing_directory() throws Exception {
        ProfileStore s = new ProfileStore(dir.resolve("a").resolve("profiles"), clock, warnings::add);
        Profile p = new Profile(); p.module("fps").setEnabled(true);
        s.save("default", p);
        assertTrue(s.load("default").module("fps").enabled());
    }

    @Test void create_copy_names_and_delete_rules() throws Exception {
        assertEquals("Profil 2", store().createCopy("default"));
        assertEquals("Profil 3", store().createCopy("default"));
        assertEquals(Arrays.asList("Profil 2", "Profil 3", "default"), store().names());
        assertThrows(IllegalArgumentException.class, () -> store().delete("default"));
        assertThrows(IllegalArgumentException.class, () -> store().delete("DEFAULT"));
        assertThrows(IllegalArgumentException.class, () -> store().delete("../x"));
        store().delete("Profil 2");
        assertEquals(Arrays.asList("Profil 3", "default"), store().names());
    }

    @Test void create_copy_reads_source_from_disk_and_keeps_flag() throws Exception {
        Profile p = new Profile(); p.module("fps").setEnabled(true);
        ProfileStore s = store();
        s.save("default", p);
        s.load("nothing");
        assertTrue(s.loadedFromDefaults());
        String copy = s.createCopy("default");
        assertTrue(s.loadedFromDefaults());
        assertTrue(s.load(copy).module("fps").enabled());
    }

    @Test void create_copy_takes_first_free_number() throws Exception {
        store().save("Profil 3", new Profile());
        assertEquals("Profil 2", store().createCopy("default"));
        assertEquals("Profil 4", store().createCopy("default"));
    }

    @Test void names_ignores_foreign_files_and_always_lists_default() throws Exception {
        assertEquals(Arrays.asList("default"), store().names());
        Files.write(dir.resolve("x.json.broken-1"), new byte[0]);
        Files.write(dir.resolve("y.json.tmp"), new byte[0]);
        Files.write(dir.resolve("con.json"), new byte[0]);
        Files.write(dir.resolve("notes.txt"), new byte[0]);
        Files.write(dir.resolve("PvP.json"), new byte[0]);
        assertEquals(Arrays.asList("PvP", "default"), store().names());
    }

    @Test void invalid_names_are_rejected_by_store_operations() {
        assertThrows(IllegalArgumentException.class, () -> store().load("../x"));
        assertThrows(IllegalArgumentException.class, () -> store().save("con", new Profile()));
        assertThrows(IllegalArgumentException.class, () -> store().createCopy("a/b"));
    }

    @Test void name_validation() {
        assertTrue(ProfileStore.isValidName("PvP 1_a-b"));
        assertFalse(ProfileStore.isValidName("../x")); assertFalse(ProfileStore.isValidName(""));
        assertFalse(ProfileStore.isValidName("con.json"));
        assertTrue(ProfileStore.isValidName(repeat('a', 32))); assertFalse(ProfileStore.isValidName(repeat('a', 33)));
        for (String reserved : new String[] {"CON", "con", "NUL", "aux", "PRN", "com1", "COM9", "LPT1", "lpt9", "Com5"}) assertFalse(ProfileStore.isValidName(reserved), reserved);
        assertTrue(ProfileStore.isValidName("console")); assertTrue(ProfileStore.isValidName("com10")); assertTrue(ProfileStore.isValidName("com0"));
        assertFalse(ProfileStore.isValidName(" a")); assertFalse(ProfileStore.isValidName("a ")); assertTrue(ProfileStore.isValidName("a b"));
        assertFalse(ProfileStore.isValidName(null));
        for (String bad : new String[] {"a/b", "a\\b", "..", ".a", "a.", "a\tb", "a\nb", "a\u0000b", "a:b", "ä", "a*b", "a\u007fb"}) assertFalse(ProfileStore.isValidName(bad), bad);
    }
}
