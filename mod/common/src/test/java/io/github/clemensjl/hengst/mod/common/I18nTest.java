package io.github.clemensjl.hengst.mod.common;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.json.Json;
import io.github.clemensjl.hengst.mod.common.json.JsonObjects;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Set;
import org.junit.jupiter.api.Test;

class I18nTest {
    @Test
    void i18n_falls_back_to_english_then_key() {
        I18n de = I18n.load("de_de");
        assertNotEquals("hengst.title", de.get("hengst.title"));
        assertEquals(I18n.load("en_us").get("hengst.feature.fps"), I18n.load("xx_yy").get("hengst.feature.fps"));
        assertEquals("no.such.key", de.get("no.such.key"));
    }

    @Test
    void language_code_is_lowercased() {
        assertEquals(I18n.load("de_de").get("hengst.button.done"), I18n.load("de_DE").get("hengst.button.done"));
        assertNotEquals(I18n.load("en_us").get("hengst.button.done"), I18n.load("de_DE").get("hengst.button.done"));
    }

    @Test
    void get_with_arg_replaces_placeholder() {
        assertEquals("Profil: x", I18n.load("de_de").get("hengst.profile", "x"));
    }

    @Test
    void get_with_arg_but_no_placeholder_returns_text_unchanged() {
        I18n en = I18n.load("en_us");
        assertEquals(en.get("hengst.title"), en.get("hengst.title", "x"));
    }

    @Test
    void both_language_files_have_the_same_keys() throws Exception {
        Set<String> en = langKeys("en_us");
        Set<String> de = langKeys("de_de");
        assertEquals(en, de);
        assertTrue(en.containsAll(Arrays.asList("hengst.title", "hengst.notice", "hengst.button.done",
                "hengst.color.white", "hengst.color.black", "hengst.editor.hint")));
    }

    static Set<String> langKeys(String code) throws Exception {
        try (InputStream in = I18nTest.class.getResourceAsStream("/hengst/lang/" + code + ".json")) {
            assertNotNull(in);
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            byte[] buf = new byte[4096];
            int n;
            while ((n = in.read(buf)) > 0) {
                out.write(buf, 0, n);
            }
            return JsonObjects.object(Json.parse(new String(out.toByteArray(), StandardCharsets.UTF_8))).keySet();
        }
    }
}
