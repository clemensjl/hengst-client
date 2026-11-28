package io.github.clemensjl.hengst.mod.common.config;

import static java.nio.charset.StandardCharsets.UTF_8;
import static org.junit.jupiter.api.Assertions.*;

import io.github.clemensjl.hengst.mod.common.feature.OptionSpec;
import io.github.clemensjl.hengst.mod.common.json.Json;
import io.github.clemensjl.hengst.mod.common.json.JsonObjects;
import io.github.clemensjl.hengst.mod.common.layout.Anchor;
import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Clock;
import java.time.Instant;
import java.time.ZoneOffset;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/**
 * m0RewriteProfile and m0RewriteSettings are a frozen copy of what the shipped M0 build does when it loads and saves
 * (test code only, never in main; source: commit 4feca31, Profile, ModuleConfig, SettingsStore, JsonObjects):
 * <ul>
 * <li>Profile.fromJson keeps only "modules"; every entry stays a raw parsed value until Profile.module(id) is called.
 * <li>module(id) on an object entry rebuilds it through ModuleConfig.fromJson (a missing or non-object entry gives
 *     defaults and is created). The rebuilt entry is written with all nine known keys from the model: wrong types
 *     fall back to the defaults, scale is clamped to 0.5 .. 3.0, an unknown anchor becomes TOP_LEFT, x and y must be
 *     integers within the int range, textColor must be an integer (cut to 32 bits), "options" that is not an object
 *     becomes {}. Option values that are null are dropped on read and on write, and so are non-finite numbers on
 *     write; every other option key is kept.
 * <li>Unknown entry keys of a rebuilt entry are dropped; entries nobody asked for are written back raw.
 * <li>SettingsStore.save writes only "schema" and "activeProfile".
 * </ul>
 */
class M0CompatibilityTest {
    @TempDir Path dir;
    Clock clock = Clock.fixed(Instant.parse("2026-11-20T10:15:30Z"), ZoneOffset.UTC);

    static final Set<String> M0_MODULES = new HashSet<>(Arrays.asList("fps", "cps", "keystrokes", "coordinates"));
    static final Set<String> M0_ENTRY_KEYS = new HashSet<>(Arrays.asList(
            "enabled", "anchor", "x", "y", "scale", "background", "shadow", "textColor", "options"));
    static final Set<String> M0_ROOT_KEYS = new HashSet<>(Arrays.asList("schema", "modules"));
    static final Set<String> M0_SETTINGS_KEYS = new HashSet<>(Arrays.asList("schema", "activeProfile"));

    /** accessed: the ids M0 asked Profile.module for; the others stay raw. */
    static Map<String, Object> m0RewriteProfile(Map<String, Object> root, Set<String> accessed) {
        Map<String, Object> modules = new LinkedHashMap<>();
        for (Map.Entry<String, Object> e : JsonObjects.object(root.get("modules")).entrySet()) {
            modules.put(e.getKey(), accessed.contains(e.getKey()) ? m0Rebuild(e.getValue()) : e.getValue());
        }
        for (String id : accessed) {
            if (!modules.containsKey(id)) { modules.put(id, m0Rebuild(null)); }
        }
        Map<String, Object> out = new LinkedHashMap<>();
        out.put("schema", 1L); out.put("modules", modules);
        return reparse(out);
    }

    private static Map<String, Object> m0Rebuild(Object entry) {
        Map<String, Object> in = JsonObjects.object(entry);
        Anchor anchor = Anchor.parse(JsonObjects.string(in, "anchor", null), Anchor.TOP_LEFT);
        float scale = Math.max(0.5f, Math.min(3.0f, JsonObjects.decimal(in, "scale", 1.0f)));
        Map<String, Object> options = new LinkedHashMap<>();
        for (Map.Entry<String, Object> o : JsonObjects.object(in.get("options")).entrySet()) {
            if (o.getValue() != null) { options.put(o.getKey(), o.getValue()); }
        }
        Map<String, Object> out = new LinkedHashMap<>();
        out.put("enabled", JsonObjects.bool(in, "enabled", false));
        out.put("anchor", anchor.name());
        out.put("x", JsonObjects.integer(in, "x", 0));
        out.put("y", JsonObjects.integer(in, "y", 0));
        out.put("scale", Double.valueOf(Float.toString(scale)));
        out.put("background", JsonObjects.bool(in, "background", true));
        out.put("shadow", JsonObjects.bool(in, "shadow", true));
        out.put("textColor", Long.valueOf(((int) JsonObjects.longValue(in, "textColor", 0xFFFFFFFFL)) & 0xFFFFFFFFL));
        out.put("options", options);
        return out;
    }

    static Map<String, Object> m0RewriteSettings(Map<String, Object> root) {
        Map<String, Object> out = new LinkedHashMap<>();
        out.put("schema", 1L); out.put("activeProfile", root.get("activeProfile"));
        return reparse(out);
    }

    /** What the next reader sees: the written text parsed again (Integer becomes Long and so on). */
    @SuppressWarnings("unchecked")
    private static Map<String, Object> reparse(Map<String, Object> m) {
        try { return (Map<String, Object>) Json.parse(Json.write(m)); } catch (Exception e) { throw new AssertionError(e); }
    }

    private Map<String, Object> readFile(String name) throws IOException {
        try { return JsonObjects.object(Json.parse(new String(Files.readAllBytes(dir.resolve(name)), UTF_8))); }
        catch (Exception e) { throw new AssertionError(e); }
    }

    private String resource(String name) throws IOException {
        try (InputStream in = M0CompatibilityTest.class.getResourceAsStream(name)) { return new String(in.readAllBytes(), UTF_8); }
    }

    private ProfileStore store() { return new ProfileStore(dir, clock, w -> { }); }

    private static final Set<String> ALL_FOUR = M0_MODULES;
    private static final OptionSpec STYLE = OptionSpec.choice("style", "cardinal_degrees", "cardinal", "cardinal_degrees", "degrees");
    private static final OptionSpec FORMAT = OptionSpec.choice("format", "24h", "24h", "12h");
    private static final OptionSpec SECONDS = OptionSpec.toggle("seconds", false);
    private static final OptionSpec TENTHS = OptionSpec.toggle("tenths", true);

    /** A profile as M1a writes it: M0 modules with changed values, new modules, options of every kind, an unknown future module. */
    private Map<String, Object> writeM1aProfile() throws Exception {
        Files.write(dir.resolve("default.json"), ("{\"schema\":1,\"modules\":{\"waypoints\":{\"enabled\":true,\"zIndex\":9,"
                + "\"options\":{\"n\":[1,2]}}}}").getBytes(UTF_8));
        Profile p = store().load("default");
        ModuleConfig fps = p.module("fps");
        fps.setEnabled(true); fps.setAnchor(Anchor.BOTTOM_RIGHT); fps.setOffsetX(-7); fps.setOffsetY(40); fps.setScale(1.3f);
        fps.setBackground(false); fps.setShadow(false); fps.setTextColor(0xFF00FF80);
        fps.setOption("future_opt", Arrays.asList(1L, 2L));
        p.module("keystrokes").setEnabled(true);
        ModuleConfig direction = p.module("direction");
        direction.setEnabled(true); direction.setOption("style", "degrees");
        p.module("cps"); p.module("coordinates");
        ModuleConfig clock = p.module("clock");
        clock.setAnchor(Anchor.TOP_RIGHT); clock.setScale(1.5f); clock.setTextColor(0xFFFFFF00);
        clock.setOption("format", "12h"); clock.setOption("seconds", true); clock.setOption("ratio", 0.25);
        ModuleConfig sw = p.module("stopwatch");
        sw.setEnabled(true); sw.setShadow(false); sw.setOption("tenths", false);
        store().save("default", p);
        return readFile("default.json");
    }

    private void assertM1aValuesRead(Profile p) {
        ModuleConfig fps = p.module("fps");
        assertTrue(fps.enabled());
        assertEquals(Anchor.BOTTOM_RIGHT, fps.anchor());
        assertEquals(-7, fps.offsetX());
        assertEquals(40, fps.offsetY());
        assertEquals(1.3f, fps.scale(), 0f);
        assertFalse(fps.background());
        assertFalse(fps.shadow());
        assertEquals(0xFF00FF80, fps.textColor());
        assertTrue(p.module("keystrokes").enabled());
        assertTrue(p.module("direction").enabled());
        assertEquals("degrees", p.module("direction").option(STYLE));
        assertFalse(p.module("clock").enabled());
        assertEquals(Anchor.TOP_RIGHT, p.module("clock").anchor());
        assertEquals(1.5f, p.module("clock").scale(), 0f);
        assertEquals(0xFFFFFF00, p.module("clock").textColor());
        assertEquals("12h", p.module("clock").option(FORMAT));
        assertEquals(Boolean.TRUE, p.module("clock").option(SECONDS));
        assertEquals(Boolean.FALSE, p.module("stopwatch").option(TENTHS));
        assertFalse(p.module("stopwatch").shadow());
    }

    @Test void a_profile_written_by_m1a_survives_an_m0_run_unchanged_whether_m0_rebuilds_entries_or_not() throws Exception {
        Map<String, Object> written = writeM1aProfile();
        assertEquals(M0_ROOT_KEYS, written.keySet());
        // M0 touched its four modules (rebuilt) and, in the other run, nothing (raw entries)
        for (Set<String> accessed : Arrays.asList(ALL_FOUR, Collections.<String>emptySet())) {
            Map<String, Object> afterM0 = m0RewriteProfile(written, accessed);
            // rebuilt entries get all nine keys from the model: M1a wrote exactly those, with M0-conform values, so nothing differs
            Map<String, Object> modules = JsonObjects.object(afterM0.get("modules"));
            for (String id : JsonObjects.object(written.get("modules")).keySet()) {
                assertEquals(JsonObjects.object(written.get("modules")).get(id), modules.get(id), id + " accessed=" + accessed);
            }
            assertEquals(written.get("modules"), afterM0.get("modules"));
            assertEquals(written, afterM0);
            Files.write(dir.resolve("default.json"), Json.write(afterM0).getBytes(UTF_8));
            Profile p = store().load("default");
            assertM1aValuesRead(p);
            assertEquals(Arrays.asList(1L, 2L), JsonObjects.object(JsonObjects.object(p.module("fps").toJson()).get("options")).get("future_opt"));
            store().save("default", p);
            assertEquals(JsonObjects.object(JsonObjects.object(written.get("modules")).get("waypoints")),
                    JsonObjects.object(JsonObjects.object(readFile("default.json").get("modules")).get("waypoints")));
        }
    }

    @Test void m1a_settings_survive_an_m0_run() throws Exception {
        Settings s = new Settings();
        s.setActiveProfile("Profil 2");
        new SettingsStore(dir.resolve("settings.json"), clock, w -> { }).save(s);
        Map<String, Object> written = readFile("settings.json");
        assertEquals(M0_SETTINGS_KEYS, written.keySet());
        assertEquals(written, m0RewriteSettings(written));
        Files.write(dir.resolve("settings.json"), Json.write(m0RewriteSettings(written)).getBytes(UTF_8));
        assertEquals("Profil 2", new SettingsStore(dir.resolve("settings.json"), clock, w -> { }).load().activeProfile());
    }

    @Test void m1a_writes_only_keys_an_m0_build_preserves_at_the_root_in_module_entries_and_in_settings() throws Exception {
        Map<String, Object> written = writeM1aProfile();
        assertEquals(M0_ROOT_KEYS, written.keySet());
        for (Map.Entry<String, Object> entry : JsonObjects.object(written.get("modules")).entrySet()) {
            Set<String> keys = JsonObjects.object(entry.getValue()).keySet();
            if (!"waypoints".equals(entry.getKey())) {   // the unknown future module is kept as read, not authored
                assertEquals(M0_ENTRY_KEYS, keys, entry.getKey());
            }
        }
        // a fresh profile and settings, nothing read from a file
        Profile fresh = new Profile();
        fresh.module("stopwatch").setEnabled(true);
        fresh.module("fps").setOption("new_option", true);
        store().save("fresh", fresh);
        Map<String, Object> root = readFile("fresh.json");
        assertEquals(M0_ROOT_KEYS, root.keySet());
        for (Object entry : JsonObjects.object(root.get("modules")).values()) {
            assertEquals(M0_ENTRY_KEYS, JsonObjects.object(entry).keySet());
        }
        new SettingsStore(dir.resolve("settings.json"), clock, w -> { }).save(new Settings());
        assertEquals(M0_SETTINGS_KEYS, readFile("settings.json").keySet());
    }

    @Test void the_fixture_files_m1a_writes_read_back_through_an_m0_run() throws Exception {
        // kept as fixtures: a snapshot of the format, independent of the current writer
        Map<String, Object> original = JsonObjects.object(Json.parse(resource("/config/m1a-profile.json")));
        assertEquals(original, m0RewriteProfile(original, JsonObjects.object(original.get("modules")).keySet()));
        assertEquals(original, m0RewriteProfile(original, Collections.<String>emptySet()));
        Map<String, Object> settings = JsonObjects.object(Json.parse(resource("/config/m1a-settings.json")));
        assertEquals(settings, m0RewriteSettings(settings));
    }

    @Test void m0_normalizes_a_malformed_entry_it_rebuilds_and_keeps_it_raw_when_it_is_not_accessed() throws Exception {
        // Hand-written: the M1a writer cannot produce malformed values (it normalizes on every save), so only a hand edit
        // or a later version could leave them in a file.
        String text = "{\"schema\":1,\"modules\":{"
                + "\"fps\":{\"enabled\":\"yes\",\"anchor\":\"MIDDLE_NOWHERE\",\"x\":2.5,\"y\":99999999999,\"scale\":9,\"background\":1,"
                + "\"textColor\":\"red\",\"zIndex\":3,\"options\":5},"
                + "\"cps\":{\"enabled\":true,\"scale\":0.1,\"x\":-4,\"textColor\":4294967040,"
                + "\"options\":{\"a\":null,\"b\":1.5,\"c\":[null,1],\"d\":{\"e\":null}}},"
                + "\"keystrokes\":7,"
                + "\"waypoints\":{\"zIndex\":9,\"options\":{\"a\":null}}}}";
        Map<String, Object> original = JsonObjects.object(Json.parse(text));
        // not accessed: all of it stays as it is
        assertEquals(original, m0RewriteProfile(original, Collections.<String>emptySet()));
        // accessed
        Map<String, Object> afterM0 = m0RewriteProfile(original, new HashSet<>(Arrays.asList("fps", "cps", "keystrokes", "coordinates")));
        Map<String, Object> modules = JsonObjects.object(afterM0.get("modules"));
        Map<String, Object> defaults = reparse(m0Rebuild(null));
        Map<String, Object> fps = JsonObjects.object(modules.get("fps"));
        assertEquals(M0_ENTRY_KEYS, fps.keySet());                      // zIndex gone, all nine keys present
        assertEquals(3.0, fps.get("scale"));                            // 9 clamped
        assertEquals("TOP_LEFT", fps.get("anchor"));                    // unknown anchor reset
        assertEquals(0L, fps.get("x"));                                 // 2.5 is not an integer
        assertEquals(0L, fps.get("y"));                                 // beyond int
        assertEquals(Boolean.FALSE, fps.get("enabled"));                // "yes" is not a boolean
        assertEquals(Boolean.TRUE, fps.get("background"));              // 1 is not a boolean: default true
        assertEquals(4294967295L, fps.get("textColor"));                // "red": default
        assertEquals(Collections.emptyMap(), fps.get("options"));       // options that is no object
        Map<String, Object> cps = JsonObjects.object(modules.get("cps"));
        assertEquals(0.5, cps.get("scale"));                            // 0.1 clamped
        assertEquals(-4L, cps.get("x"));
        assertEquals(4294967040L, cps.get("textColor"));
        Map<String, Object> cpsOptions = JsonObjects.object(cps.get("options"));
        assertEquals(new ArrayList<>(Arrays.asList("b", "c", "d")), new ArrayList<>(cpsOptions.keySet()));   // null option value dropped
        assertEquals(Arrays.asList(null, 1L), cpsOptions.get("c"));     // null inside a list is not an option value: kept
        assertEquals(Collections.singletonMap("e", null), cpsOptions.get("d"));
        assertEquals(defaults, modules.get("keystrokes"));              // not an object: defaults
        assertEquals(defaults, modules.get("coordinates"));             // absent: created with defaults
        assertEquals(JsonObjects.object(original.get("modules")).get("waypoints"), modules.get("waypoints"));                              // unknown module: kept whole, even its null option
        // and the M1a reader on that
        Files.write(dir.resolve("default.json"), Json.write(afterM0).getBytes(UTF_8));
        Profile p = store().load("default");
        assertEquals(3.0f, p.module("fps").scale(), 0f);
        assertEquals(Anchor.TOP_LEFT, p.module("fps").anchor());
        assertTrue(p.module("cps").enabled());
        assertEquals(0.5f, p.module("cps").scale(), 0f);
    }

    @Test void m1a_never_persists_null_or_non_finite_option_values_which_m0_would_drop() throws Exception {
        Profile p = new Profile();
        ModuleConfig fps = p.module("fps");
        fps.setOption("keep", true);
        fps.setOption("null_opt", "x");
        fps.setOption("null_opt", null);
        fps.setOption("nan", Double.NaN);
        fps.setOption("inf", Float.POSITIVE_INFINITY);
        store().save("default", p);
        Map<String, Object> written = readFile("default.json");
        assertEquals(written, m0RewriteProfile(written, Collections.singleton("fps")));
        assertEquals(Collections.singletonMap("keep", true), JsonObjects.object(JsonObjects.object(JsonObjects.object(written.get("modules")).get("fps")).get("options")));
    }

    @Test void documents_the_m0_limit_future_root_and_entry_keys_are_lost_after_an_m0_run() throws Exception {
        // Not a promise but the recorded limit: what a later version adds outside the M0-safe places is gone after an M0 run.
        String future = "{\"schema\":1,\"layoutVersion\":3,\"modules\":{\"fps\":{\"enabled\":true,\"zIndex\":7,"
                + "\"options\":{\"future_opt\":[1,2]}},\"waypoints\":{\"enabled\":true,\"zIndex\":9,\"options\":{\"n\":1}}},"
                + "\"servers\":{\"a.net\":{\"deny\":[\"x\"]}}}";
        Files.write(dir.resolve("default.json"), future.getBytes(UTF_8));
        store().save("default", store().load("default"));
        Map<String, Object> m1aOut = readFile("default.json");
        assertEquals(Arrays.asList("schema", "modules", "layoutVersion", "servers"), new ArrayList<>(m1aOut.keySet()));   // M1a keeps all
        // M0 run with fps accessed (rebuilt), waypoints not
        Map<String, Object> afterM0 = m0RewriteProfile(m1aOut, Collections.singleton("fps"));
        assertEquals(Arrays.asList("schema", "modules"), new ArrayList<>(afterM0.keySet()));           // layoutVersion, servers: gone
        Map<String, Object> modules = JsonObjects.object(afterM0.get("modules"));
        Map<String, Object> fps = JsonObjects.object(modules.get("fps"));
        assertFalse(fps.containsKey("zIndex"));                                                         // entry key of a rebuilt M0 module: gone
        assertEquals(Arrays.asList(1L, 2L), JsonObjects.object(fps.get("options")).get("future_opt"));  // option key: kept
        assertEquals(9L, JsonObjects.object(modules.get("waypoints")).get("zIndex"));                   // unknown module: kept whole
        // the same entry, but M0 never asked for it: kept raw
        assertEquals(7L, JsonObjects.object(JsonObjects.object(m0RewriteProfile(m1aOut, Collections.<String>emptySet()).get("modules")).get("fps")).get("zIndex"));
        Files.write(dir.resolve("default.json"), Json.write(afterM0).getBytes(UTF_8));
        store().save("default", store().load("default"));
        Map<String, Object> back = readFile("default.json");
        assertEquals(M0_ROOT_KEYS, back.keySet());
        assertEquals(9L, JsonObjects.object(JsonObjects.object(back.get("modules")).get("waypoints")).get("zIndex"));
        Map<String, Object> futureSettings = JsonObjects.object(Json.parse("{\"schema\":1,\"activeProfile\":\"default\",\"servers\":{\"a\":1}}"));
        assertEquals(M0_SETTINGS_KEYS, m0RewriteSettings(futureSettings).keySet());
    }
}
