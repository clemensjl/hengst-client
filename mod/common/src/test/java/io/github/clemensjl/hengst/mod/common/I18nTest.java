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

    private static String done(String code) {
        return I18n.load(code).get("hengst.button.done");
    }

    @Test
    void a_regional_code_uses_the_bundled_file_of_its_language() {
        assertEquals("Fertig", done("de_at"));
        assertEquals("Fertig", done("de_ch"));
        assertEquals("Fertig", done("de_de"));
        assertEquals("Fertig", done("de"));
        assertEquals("Done", done("en_gb"));
        assertEquals("Done", done("en_us"));
    }

    @Test
    void an_unsupported_empty_or_missing_code_is_english() {
        assertEquals("Done", done("fr_fr"));
        assertEquals("Done", done(""));
        assertEquals("Done", done(null));
    }

    @Test
    void a_key_missing_in_a_translation_falls_back_to_the_english_text_then_the_key() {
        I18n partial = I18n.of(java.util.Collections.<String, Object>singletonMap("a", "A-de"),
                java.util.Collections.<String, Object>singletonMap("b", "B-en"));
        assertEquals("A-de", partial.get("a"));
        assertEquals("B-en", partial.get("b"));
        assertEquals("c", partial.get("c"));
    }

    @Test
    void a_code_is_lowercased_then_validated_before_it_is_used() {
        assertEquals("Fertig", done("DE_AT"));                 // the legacy version reports "de_DE"-style codes
        assertEquals("Done", done("../x"));
        assertEquals("Done", done("de/../en_us"));
        assertEquals("Done", done("de_de\0"));
        assertEquals("Done", done("de\0"));
        assertEquals("Done", done("d"));                       // too short
        assertEquals("Done", done("de_de_de_de_de_de"));      // 17 characters
        assertEquals("Done", done("de-de"));
    }

    @Test
    void every_bundled_file_is_in_the_table_and_the_table_names_only_bundled_files() throws Exception {
        java.nio.file.Path dir = java.nio.file.Paths.get(I18n.class.getResource("/hengst/lang").toURI());
        java.util.Set<String> onDisk = new java.util.TreeSet<String>();
        try (java.util.stream.Stream<java.nio.file.Path> files = java.nio.file.Files.list(dir)) {
            files.forEach(f -> onDisk.add(f.getFileName().toString().replace(".json", "")));
        }
        assertEquals(onDisk, new java.util.TreeSet<String>(Arrays.asList(I18n.BUNDLED)));
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
        // hengst.notice is English in both files on purpose (legal wording); do not translate it.
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
