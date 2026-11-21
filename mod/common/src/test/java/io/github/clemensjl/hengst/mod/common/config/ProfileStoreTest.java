package io.github.clemensjl.hengst.mod.common.config;

import static java.nio.charset.StandardCharsets.UTF_8;
import static org.junit.jupiter.api.Assertions.*;

import io.github.clemensjl.hengst.mod.common.feature.OptionSpec;
import io.github.clemensjl.hengst.mod.common.json.Json;
import io.github.clemensjl.hengst.mod.common.json.JsonObjects;
import io.github.clemensjl.hengst.mod.common.layout.Anchor;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Clock;
import java.time.Instant;
import java.time.ZoneOffset;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
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

    @Test void unknown_keys_survive_at_all_three_levels() throws Exception {
        Files.write(dir.resolve("default.json"), ("{\"schema\":1,\"layoutVersion\":3,\"modules\":{\"fps\":{\"enabled\":true,"
                + "\"zIndex\":7,\"options\":{\"future_opt\":[1,2]}}},\"servers\":{\"a.net\":{\"deny\":[\"x\"]}}}").getBytes(UTF_8));
        Profile p = store().load("default");
        p.module("fps").setOffsetX(5);
        store().save("default", p);
        Map<String, Object> back = JsonObjects.object(Json.parse(read("default.json")));
        assertEquals(Arrays.asList("schema", "modules", "layoutVersion", "servers"), new ArrayList<>(back.keySet()));
        assertEquals(3L, back.get("layoutVersion"));
        assertEquals(Collections.singletonMap("a.net", Collections.singletonMap("deny", Arrays.asList("x"))), back.get("servers"));
        Map<String, Object> fps = JsonObjects.object(JsonObjects.object(back.get("modules")).get("fps"));
        assertEquals(7L, fps.get("zIndex"));
        assertEquals(5L, fps.get("x"));
        assertEquals(Arrays.asList(1L, 2L), JsonObjects.object(fps.get("options")).get("future_opt"));
    }

    @Test void a_profile_copy_keeps_unknown_keys() throws Exception {
        Files.write(dir.resolve("default.json"), "{\"schema\":1,\"extra\":true,\"modules\":{\"fps\":{\"zIndex\":7}}}".getBytes(UTF_8));
        String name = store().createCopy("default");
        String json = read(name + ".json");
        assertTrue(json.contains("\"extra\": true"));
        assertTrue(json.contains("\"zIndex\": 7"));
    }
    private static final String NESTED = "{\"m\":{\"k\":1},\"l\":[1,[2]],\"lm\":[{\"k\":1}]}";

    @SuppressWarnings("unchecked")
    private static void mutateNested(Object unknown) {
        Map<String, Object> m = (Map<String, Object>) unknown;
        ((Map<String, Object>) m.get("m")).put("k", 2L);
        ((List<Object>) m.get("l")).add("added");
        ((List<Object>) ((List<Object>) m.get("l")).get(1)).add(3L);
        ((Map<String, Object>) ((List<Object>) m.get("lm")).get(0)).put("k", 2L);
    }

    private Profile loadedWithNestedUnknowns() throws Exception {
        Files.write(dir.resolve("default.json"), ("{\"schema\":1,\"extra\":" + NESTED + ",\"modules\":{\"fps\":{\"zIndex\":" + NESTED + "}}}").getBytes(UTF_8));
        Profile p = store().load("default");
        p.module("fps");
        return p;
    }

    @Test void a_profile_copy_is_deep_for_unknown_data_at_root_and_entry_level_in_both_directions() throws Exception {
        Profile a = loadedWithNestedUnknowns();
        Profile b = a.copy();
        String before = Json.write(a.toJson());
        assertEquals(before, Json.write(b.toJson()));
        // the original is mutated: the copy stays as it was
        mutateNested(a.extras.get("extra"));
        mutateNested(a.module("fps").extras.get("zIndex"));
        assertNotEquals(before, Json.write(a.toJson()));
        assertEquals(before, Json.write(b.toJson()));
        // the copy is mutated: a fresh original stays as it was
        Profile c = loadedWithNestedUnknowns();
        Profile d = c.copy();
        mutateNested(d.extras.get("extra"));
        mutateNested(d.module("fps").extras.get("zIndex"));
        assertNotEquals(before, Json.write(d.toJson()));
        assertEquals(before, Json.write(c.toJson()));
    }

    private static Object field(Object target, String name) throws Exception {
        java.lang.reflect.Field f = target.getClass().getDeclaredField(name);
        f.setAccessible(true);
        return f.get(target);
    }

    @SuppressWarnings("unchecked")
    private static Map<String, Object> rawEntry(Profile p, String id) throws Exception {
        return (Map<String, Object>) ((Map<String, Object>) field(p, "entries")).get(id);
    }

    private Profile loadedWithRawEntries() throws Exception {
        Files.write(dir.resolve("default.json"), ("{\"schema\":1,\"modules\":{\"fps\":{\"zIndex\":" + NESTED + ",\"options\":{\"o\":" + NESTED + "}},"
                + "\"waypoints\":{\"zIndex\":" + NESTED + ",\"options\":{\"o\":" + NESTED + "}}}}").getBytes(UTF_8));
        return store().load("default");   // nothing accessed: the entries are raw
    }

    @SuppressWarnings("unchecked")
    private static void mutateRaw(Map<String, Object> entry) {
        mutateNested(entry.get("zIndex"));
        mutateNested(((Map<String, Object>) entry.get("options")).get("o"));
    }

    @Test void copying_entries_that_were_not_accessed_is_deep_including_unknown_modules() throws Exception {
        Profile a = loadedWithRawEntries();
        Profile b = a.copy();
        String before = Json.write(a.toJson());
        for (String id : new String[] {"fps", "waypoints"}) {
            mutateRaw(rawEntry(a, id));
        }
        assertNotEquals(before, Json.write(a.toJson()));
        assertEquals(before, Json.write(b.toJson()));
        Profile c = loadedWithRawEntries();
        Profile d = c.copy();
        for (String id : new String[] {"fps", "waypoints"}) {
            mutateRaw(rawEntry(d, id));
        }
        assertNotEquals(before, Json.write(d.toJson()));
        assertEquals(before, Json.write(c.toJson()));
    }

    @Test void copying_option_values_that_are_maps_or_lists_is_deep() throws Exception {
        Object value = Json.parse(NESTED);
        ModuleConfig a = new Profile().module("fps");
        a.setOption("o", value);
        ModuleConfig b = a.copy();
        String before = Json.write(a.toJson());
        mutateNested(value);                                   // the original's stored object
        assertNotEquals(before, Json.write(a.toJson()));
        assertEquals(before, Json.write(b.toJson()));
        ModuleConfig c = new Profile().module("fps");
        c.setOption("o", Json.parse(NESTED));
        ModuleConfig d = c.copy();
        mutateNested(JsonObjects.object(JsonObjects.object(d.toJson()).get("options")).get("o"));   // the copy's stored object
        assertEquals(before, Json.write(c.toJson()));
        assertNotEquals(before, Json.write(d.toJson()));
    }

    @Test void a_module_copy_is_deep_for_unknown_entry_keys() throws Exception {
        ModuleConfig a = loadedWithNestedUnknowns().module("fps");
        ModuleConfig b = a.copy();
        String before = Json.write(a.toJson());
        mutateNested(a.extras.get("zIndex"));
        assertEquals(before, Json.write(b.toJson()));
        String afterA = Json.write(a.toJson());
        mutateNested(b.extras.get("zIndex"));
        assertNotEquals(before, Json.write(b.toJson()));
        assertEquals(afterA, Json.write(a.toJson()));
    }

    @Test void the_writer_never_emits_null_or_non_finite_option_values() throws Exception {
        Profile p = new Profile();
        ModuleConfig fps = p.module("fps");
        fps.setOption("keep", 1L);
        fps.setOption("gone", "x");
        fps.setOption("gone", null);
        fps.setOption("nan_f", Float.NaN);
        fps.setOption("inf_f", Float.POSITIVE_INFINITY);
        fps.setOption("nan_d", Double.NaN);
        fps.setOption("ninf_d", Double.NEGATIVE_INFINITY);
        store().save("default", p);
        Map<String, Object> options = JsonObjects.object(JsonObjects.object(JsonObjects.object(
                JsonObjects.object(Json.parse(read("default.json"))).get("modules")).get("fps")).get("options"));
        assertEquals(Collections.singletonMap("keep", 1L), options);
    }

    @Test void a_null_option_value_is_dropped_on_read_and_not_written_back() throws Exception {
        Files.write(dir.resolve("default.json"), "{\"schema\":1,\"modules\":{\"fps\":{\"options\":{\"a\":null,\"b\":2}}}}".getBytes(UTF_8));
        Profile p = store().load("default");
        p.module("fps");
        store().save("default", p);
        assertFalse(read("default.json").contains("\"a\""));
        assertTrue(read("default.json").contains("\"b\": 2"));
    }

    @Test void number_forms_in_unknown_data_after_a_round_trip() throws Exception {
        Files.write(dir.resolve("default.json"), ("{\"schema\":1,\"modules\":{},\"max\":9223372036854775807,\"min\":-9223372036854775808,"
                + "\"big\":9223372036854775808,\"huge\":12345678901234567890123,\"int\":1,\"dec\":1.0,\"exp\":1e2}").getBytes(UTF_8));
        Profile p = store().load("default");
        store().save("default", p);
        Map<String, Object> back = JsonObjects.object(Json.parse(read("default.json")));
        assertEquals(Long.MAX_VALUE, back.get("max"));                       // exact: still a long
        assertEquals(Long.MIN_VALUE, back.get("min"));
        assertEquals(Long.valueOf(1L), back.get("int"));                      // 1 stays an integer
        assertEquals(Double.valueOf(1.0), back.get("dec"));                   // 1.0 stays a decimal, not 1
        assertEquals(Double.valueOf(100.0), back.get("exp"));                 // exponent form becomes 100.0
        assertEquals(Double.valueOf(9.223372036854775808E18), back.get("big")); // beyond long: a double (exact here, 2^63)
        assertEquals(Double.valueOf(1.2345678901234568E22), back.get("huge")); // beyond long: a double, digits beyond 17 are lost
        assertTrue(read("default.json").contains("\"dec\": 1.0"));
        assertTrue(read("default.json").contains("\"int\": 1,") || read("default.json").contains("\"int\": 1\n"));
    }
}
