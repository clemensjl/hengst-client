package io.github.clemensjl.hengst.mod.legacy;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.FeatureCatalog;
import io.github.clemensjl.hengst.mod.common.feature.DefaultKey;
import io.github.clemensjl.hengst.mod.common.feature.KeySpec;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import java.io.BufferedReader;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import org.junit.jupiter.api.Test;
import org.lwjgl.input.Keyboard;

class LegacyKeysTest {
    @Test
    void every_default_key_has_a_code() {
        for (DefaultKey k : DefaultKey.values()) {
            LegacyKeys.code(k);
        }
        assertEquals(Keyboard.KEY_RSHIFT, LegacyKeys.code(DefaultKey.RIGHT_SHIFT));
        assertEquals(Keyboard.KEY_NONE, LegacyKeys.code(DefaultKey.NONE));
    }

    /** Registration computes the code of every default key first and fails (the mod stays inert) on an unmapped one. */
    @Test
    void every_default_key_of_the_real_catalog_has_a_code() {
        for (KeySpec k : FeatureCatalog.keys(GameVersion.V1_8_9)) {
            LegacyKeys.code(k.defaultKey());
        }
        List<String> ids = new ArrayList<>();
        for (KeySpec k : FeatureCatalog.keys(GameVersion.V1_8_9)) {
            ids.add(k.id());
        }
        assertEquals(Arrays.asList("settings", "hud_editor", "stopwatch.toggle", "stopwatch.reset"), ids);
    }

    @Test
    void every_key_is_named_in_both_language_files() throws Exception {
        Set<String> en = langKeys("en_US");
        Set<String> de = langKeys("de_DE");
        assertEquals(en, de);
        for (KeySpec k : FeatureCatalog.keys(GameVersion.V1_8_9)) {
            assertTrue(en.contains(k.translationKey()), k.translationKey());
        }
        assertTrue(en.contains(LegacyKeys.CATEGORY));
    }

    /** The keys of a 1.8.9 .lang file: one {@code key=value} per line. */
    private static Set<String> langKeys(String language) throws Exception {
        Set<String> keys = new HashSet<>();
        try (InputStream in = LegacyKeysTest.class.getResourceAsStream("/assets/hengst/lang/" + language + ".lang")) {
            assertNotNull(in, language);
            BufferedReader reader = new BufferedReader(new InputStreamReader(in, StandardCharsets.UTF_8));
            String line;
            while ((line = reader.readLine()) != null) {
                int eq = line.indexOf('=');
                if (eq > 0) {
                    keys.add(line.substring(0, eq));
                }
            }
        }
        return keys;
    }
}
