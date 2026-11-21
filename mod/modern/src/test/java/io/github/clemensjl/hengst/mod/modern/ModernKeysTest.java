package io.github.clemensjl.hengst.mod.modern;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.google.gson.JsonParser;
import com.mojang.blaze3d.platform.InputConstants;
import io.github.clemensjl.hengst.mod.common.FeatureCatalog;
import io.github.clemensjl.hengst.mod.common.feature.DefaultKey;
import io.github.clemensjl.hengst.mod.common.feature.KeySpec;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.HashSet;
import java.util.Set;
import org.junit.jupiter.api.Test;

class ModernKeysTest {
    @Test
    void every_default_key_has_a_code() {
        for (DefaultKey k : DefaultKey.values()) {
            ModernKeys.code(k);
        }
        assertEquals(InputConstants.KEY_RSHIFT, ModernKeys.code(DefaultKey.RIGHT_SHIFT));
        // NONE is the game's "unknown" key, looked up in the key table without running InputConstants' static
        // initializer (it loads GLFW natives on 1.21.11); the in-game test compares with InputConstants.UNKNOWN itself.
        //? if >=26.1 {
        InputConstants.Type keyboard = InputConstants.Type.KEYBOARD;
        //?} else {
        /*InputConstants.Type keyboard = InputConstants.Type.KEYSYM;
        *///?}
        assertEquals("key.keyboard.unknown", keyboard.getOrCreate(ModernKeys.code(DefaultKey.NONE)).getName());
    }

    @Test
    void every_key_of_every_version_is_named_in_both_language_files() throws Exception {
        Set<String> en = langKeys("en_us");
        Set<String> de = langKeys("de_de");
        assertEquals(en, de);
        for (GameVersion v : new GameVersion[] {GameVersion.V1_21_11, GameVersion.V26_3}) {
            for (KeySpec k : FeatureCatalog.keys(v)) {
                assertTrue(en.contains(k.translationKey()), k.translationKey());
            }
        }
        assertTrue(en.contains("key.category.hengst.main"));
    }

    private static Set<String> langKeys(String language) throws Exception {
        try (InputStream in = ModernKeysTest.class.getResourceAsStream("/assets/hengst/lang/" + language + ".json")) {
            assertNotNull(in, language);
            return new HashSet<>(JsonParser.parseReader(new InputStreamReader(in, StandardCharsets.UTF_8))
                    .getAsJsonObject().keySet());
        }
    }
}
