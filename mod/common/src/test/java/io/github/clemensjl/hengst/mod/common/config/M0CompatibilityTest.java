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
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/**
 * m0RewriteProfile and m0RewriteSettings are a frozen copy of what the shipped M0 build does when it loads and saves
 * (test code only, never in main; source: commit 4feca31): Profile.fromJson keeps only "modules"; the entry of one of
 * the four M0 modules is rebuilt by ModuleConfig (only the known entry keys, all keys inside "options"); entries of
 * other modules stay raw; SettingsStore.save writes only "schema" and "activeProfile".
 */
class M0CompatibilityTest {
    @TempDir Path dir;
    Clock clock = Clock.fixed(Instant.parse("2026-11-20T10:15:30Z"), ZoneOffset.UTC);

    private static final Set<String> M0_MODULES = new HashSet<>(Arrays.asList("fps", "cps", "keystrokes", "coordinates"));
    private static final Set<String> M0_ENTRY_KEYS = new HashSet<>(Arrays.asList(
            "enabled", "anchor", "x", "y", "scale", "background", "shadow", "textColor", "options"));

    static Map<String, Object> m0RewriteProfile(Map<String, Object> root) {
        Map<String, Object> modules = new LinkedHashMap<>();
        for (Map.Entry<String, Object> e : JsonObjects.object(root.get("modules")).entrySet()) {
            if (!M0_MODULES.contains(e.getKey()) || !(e.getValue() instanceof Map)) { modules.put(e.getKey(), e.getValue()); continue; }
            Map<String, Object> kept = new LinkedHashMap<>();
            for (Map.Entry<String, Object> k : JsonObjects.object(e.getValue()).entrySet()) {
                if (M0_ENTRY_KEYS.contains(k.getKey())) { kept.put(k.getKey(), k.getValue()); }
            }
            modules.put(e.getKey(), kept);
        }
        Map<String, Object> out = new LinkedHashMap<>();
        out.put("schema", 1L); out.put("modules", modules);
        return out;
    }

    static Map<String, Object> m0RewriteSettings(Map<String, Object> root) {
        Map<String, Object> out = new LinkedHashMap<>();
        out.put("schema", 1L); out.put("activeProfile", root.get("activeProfile"));
        return out;
    }

    private String resource(String name) throws IOException {
        try (InputStream in = M0CompatibilityTest.class.getResourceAsStream(name)) { return new String(in.readAllBytes(), UTF_8); }
    }

    @Test void an_m0_round_trip_keeps_everything_m1a_wrote() throws Exception {
        Map<String, Object> original = JsonObjects.object(Json.parse(resource("/config/m1a-profile.json")));
        Map<String, Object> afterM0 = m0RewriteProfile(original);
        assertEquals(original, afterM0);
        Files.write(dir.resolve("default.json"), Json.write(afterM0).getBytes(UTF_8));
        Profile p = new ProfileStore(dir, clock, w -> { }).load("default");
        OptionSpec style = OptionSpec.choice("style", "cardinal_degrees", "cardinal", "cardinal_degrees", "degrees");
        OptionSpec format = OptionSpec.choice("format", "24h", "24h", "12h");
        assertTrue(p.module("direction").enabled());
        assertEquals("degrees", p.module("direction").option(style));
        assertFalse(p.module("clock").enabled());
        assertEquals(Anchor.TOP_RIGHT, p.module("clock").anchor());
        assertEquals("12h", p.module("clock").option(format));
        assertEquals(Boolean.TRUE, p.module("clock").option(OptionSpec.toggle("seconds", false)));
        assertEquals(1.5f, p.module("clock").scale(), 0f);
        assertEquals(Boolean.FALSE, p.module("stopwatch").option(OptionSpec.toggle("tenths", true)));
        assertFalse(p.module("stopwatch").shadow());
        Map<String, Object> settings = JsonObjects.object(Json.parse(resource("/config/m1a-settings.json")));
        assertEquals(settings, m0RewriteSettings(settings));
        Files.write(dir.resolve("settings.json"), Json.write(m0RewriteSettings(settings)).getBytes(UTF_8));
        assertEquals("Profil 2", new SettingsStore(dir.resolve("settings.json"), clock, w -> { }).load().activeProfile());
    }

    @Test void an_m0_round_trip_loses_future_root_and_entry_keys_and_the_m1a_reader_sees_that() throws Exception {
        // Not a promise but the recorded limit: what a later version adds outside the M0-safe places is gone after an M0 run.
        String future = "{\"schema\":1,\"layoutVersion\":3,\"modules\":{\"fps\":{\"enabled\":true,\"zIndex\":7,"
                + "\"options\":{\"future_opt\":[1,2]}},\"waypoints\":{\"enabled\":true,\"zIndex\":9,\"options\":{\"n\":1}}},"
                + "\"servers\":{\"a.net\":{\"deny\":[\"x\"]}}}";
        Map<String, Object> afterM0 = m0RewriteProfile(JsonObjects.object(Json.parse(future)));
        assertEquals(Arrays.asList("schema", "modules"), new ArrayList<>(afterM0.keySet()));
        Map<String, Object> modules = JsonObjects.object(afterM0.get("modules"));
        Map<String, Object> fps = JsonObjects.object(modules.get("fps"));
        assertFalse(fps.containsKey("zIndex"));
        assertEquals(Arrays.asList(1L, 2L), JsonObjects.object(fps.get("options")).get("future_opt"));
        assertEquals(9L, JsonObjects.object(modules.get("waypoints")).get("zIndex"));
        Files.write(dir.resolve("default.json"), Json.write(afterM0).getBytes(UTF_8));
        ProfileStore store = new ProfileStore(dir, clock, w -> { });
        store.save("default", store.load("default"));
        Map<String, Object> back = JsonObjects.object(Json.parse(new String(Files.readAllBytes(dir.resolve("default.json")), UTF_8)));
        assertEquals(new HashSet<>(Arrays.asList("schema", "modules")), back.keySet());
        assertEquals(9L, JsonObjects.object(JsonObjects.object(back.get("modules")).get("waypoints")).get("zIndex"));
        Map<String, Object> futureSettings = JsonObjects.object(Json.parse("{\"schema\":1,\"activeProfile\":\"default\",\"servers\":{\"a\":1}}"));
        assertEquals(new HashSet<>(Arrays.asList("schema", "activeProfile")), m0RewriteSettings(futureSettings).keySet());
    }
}
