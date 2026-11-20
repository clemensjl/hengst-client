package io.github.clemensjl.hengst.mod.common.json;

import static org.junit.jupiter.api.Assertions.*;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.junit.jupiter.api.Test;

@SuppressWarnings("unchecked")
class JsonTest {

    @Test void roundtrip_keeps_order_and_types() throws Exception {
        String src = "{\"b\":1,\"a\":[true,null,2.5,\"x\"],\"c\":{}}";
        Map<String, Object> m = (Map<String, Object>) Json.parse(src);
        assertEquals(Arrays.asList("b", "a", "c"), new ArrayList<>(m.keySet()));
        assertEquals(1L, m.get("b"));
        assertEquals(Arrays.asList(true, null, 2.5, "x"), m.get("a"));
        assertEquals(m, Json.parse(Json.write(m)));
    }

    @Test void escapes_roundtrip() throws Exception {
        String s = "q\"b\\s/\n\t\u0001\u00e4\ud83d\ude00";
        assertEquals(s, Json.parse(Json.write(s)));
        assertEquals("\u00e4", Json.parse("\"\\u00e4\""));
    }

    @Test void errors_report_offset() {
        JsonException e = assertThrows(JsonException.class, () -> Json.parse("{\"a\":1,}"));
        assertEquals(7, e.offset());
    }

    @Test void rejects_trailing_garbage_and_empty() {
        assertThrows(JsonException.class, () -> Json.parse("{} x"));
        assertThrows(JsonException.class, () -> Json.parse(""));
        assertThrows(JsonException.class, () -> Json.parse("{\"a\":"));
    }

    @Test void tolerates_bom_and_whitespace() throws Exception {
        assertEquals(Collections.emptyMap(), Json.parse("\ufeff  {}\r\n"));
    }

    @Test void nesting_deeper_than_64_is_rejected() throws Exception {
        assertThrows(JsonException.class, () -> Json.parse(nested(65)));
        assertNotNull(Json.parse(nested(64)));
    }

    private static String nested(int depth) {
        StringBuilder b = new StringBuilder();
        for (int i = 0; i < depth; i++) b.append('[');
        for (int i = 0; i < depth; i++) b.append(']');
        return b.toString();
    }

    @Test void big_integers_become_double() throws Exception {
        assertEquals(1.0E20, Json.parse("100000000000000000000"));
        assertEquals(-3L, Json.parse("-3"));
    }

    @Test void accessors_fall_back_on_wrong_type() throws Exception {
        Map<String, Object> m = JsonObjects.object(Json.parse("{\"i\":\"x\",\"f\":2,\"b\":1}"));
        assertEquals(7, JsonObjects.integer(m, "i", 7));
        assertEquals(2.0f, JsonObjects.decimal(m, "f", 0f));
        assertFalse(JsonObjects.bool(m, "b", false));
        assertTrue(JsonObjects.object("nope").isEmpty());
    }

    @Test void long_accessor_and_int_range() throws Exception {
        Map<String, Object> m = JsonObjects.object(Json.parse("{\"c\":4294967295,\"d\":2.5,\"s\":\"x\"}"));
        assertEquals(4294967295L, JsonObjects.longValue(m, "c", 0L));
        assertEquals(9L, JsonObjects.longValue(m, "s", 9L));
        assertEquals(9L, JsonObjects.longValue(m, "d", 9L));
        assertEquals(7, JsonObjects.integer(m, "c", 7));
    }

    // ---- hostile input beyond the brief ----

    private static void bad(String text) {
        assertThrows(JsonException.class, () -> Json.parse(text), text);
    }

    @Test void rejects_unterminated_and_invalid_strings() {
        bad("\"abc");
        bad("\"abc\\");
        bad("\"a\\x\"");
        bad("\"a\\u12\"");
        bad("\"a\\u12G4\"");
        bad("\"a\nb\"");
    }

    @Test void rejects_lone_surrogate_escapes() {
        bad("\"\\ud83d\"");
        bad("\"\\ude00\"");
        bad("\"\\ud83dx\"");
        bad("\"\\ud83d\\u0041\"");
    }

    @Test void rejects_bad_numbers() {
        for (String s : new String[] {"-", "1.", "1e", "1e+", "01", "-01", "+1", ".5", "--1", "1.e5", "0x10"}) {
            bad(s);
        }
    }

    @Test void accepts_good_numbers() throws Exception {
        assertEquals(0L, Json.parse("0"));
        assertEquals(0L, Json.parse("-0"));
        assertEquals(1500.0, Json.parse("1.5e3"));
        assertEquals(0.01, Json.parse("1E-2"));
        assertEquals(Long.MAX_VALUE, Json.parse("9223372036854775807"));
        assertEquals(Long.MIN_VALUE, Json.parse("-9223372036854775808"));
        assertEquals(9.223372036854775808E18, Json.parse("9223372036854775808"));
    }

    @Test void number_overflowing_to_infinity_is_rejected() {
        bad("1e999999");
        bad("-1e999999");
    }

    @Test void deep_nesting_does_not_overflow_the_stack() {
        StringBuilder a = new StringBuilder();
        for (int i = 0; i < 10000; i++) a.append('[');
        JsonException e = assertThrows(JsonException.class, () -> Json.parse(a.toString()));
        assertTrue(e.offset() <= 65);
        StringBuilder o = new StringBuilder();
        for (int i = 0; i < 10000; i++) o.append("{\"a\":");
        assertThrows(JsonException.class, () -> Json.parse(o.toString()));
    }

    @Test void rejects_trailing_garbage_variants() {
        bad("1 2");
        bad("[] []");
        bad("true false");
        bad("{}}");
        bad("\"a\"b");
    }

    @Test void rejects_structural_errors() {
        bad("[1,]");
        bad("[1 2]");
        bad("[");
        bad("{\"a\" 1}");
        bad("{a:1}");
        bad("{\"a\":1 \"b\":2}");
        bad("{1:2}");
        bad("nul");
        bad("tru");
        bad("True");
        bad("'a'");
        bad("\u0000");
    }

    @Test void duplicate_keys_last_wins_and_keeps_first_position() throws Exception {
        Map<String, Object> m = (Map<String, Object>) Json.parse("{\"a\":1,\"b\":2,\"a\":3}");
        assertEquals(3L, m.get("a"));
        assertEquals(Arrays.asList("a", "b"), new ArrayList<>(m.keySet()));
    }

    @Test void bom_only_and_whitespace_only_are_empty() {
        bad("\ufeff");
        bad("  \n\t ");
    }

    @Test void scalar_roots_and_null() throws Exception {
        assertNull(Json.parse("null"));
        assertEquals(Boolean.TRUE, Json.parse(" true "));
        assertEquals("x", Json.parse("\"x\""));
        assertEquals(new ArrayList<>(), Json.parse("[ ]"));
    }

    @Test void error_offset_points_at_problem() {
        assertEquals(1, assertThrows(JsonException.class, () -> Json.parse("[x]")).offset());
        assertEquals(3, assertThrows(JsonException.class, () -> Json.parse("{} x")).offset());
    }

    @Test void write_format() {
        Map<String, Object> m = new LinkedHashMap<>();
        m.put("a", 1);
        m.put("l", Arrays.asList(1L, 2.0, "x"));
        m.put("e", new ArrayList<>());
        m.put("o", new LinkedHashMap<String, Object>());
        m.put("n", null);
        assertEquals("{\n  \"a\": 1,\n  \"l\": [\n    1,\n    2.0,\n    \"x\"\n  ],\n  \"e\": [],\n  \"o\": {},\n  \"n\": null\n}",
                Json.write(m));
    }

    @Test void write_escapes_controls_and_keeps_non_ascii() {
        assertEquals("\"\\u0001\\u001f\u00e4\\u000a\\\"\\\\\"", Json.write("\u0001\u001f\u00e4\n\"\\"));
        assertEquals("\"\u007f\"", Json.write("\u007f"));
    }

    @Test void write_rejects_unsupported_types() {
        assertThrows(IllegalArgumentException.class, () -> Json.write(new Object()));
        assertThrows(IllegalArgumentException.class, () -> Json.write(Arrays.asList(new Object())));
        Map<Object, Object> badKey = new HashMap<>();
        badKey.put(1, 2);
        assertThrows(IllegalArgumentException.class, () -> Json.write(badKey));
        assertThrows(IllegalArgumentException.class, () -> Json.write(Double.NaN));
        assertThrows(IllegalArgumentException.class, () -> Json.write(Double.POSITIVE_INFINITY));
    }

    @Test void write_handles_number_types() {
        assertEquals("5", Json.write(5));
        assertEquals("5", Json.write(5L));
        assertEquals("1.0", Json.write(1.0));
        assertEquals("true", Json.write(true));
        assertEquals("null", Json.write(null));
    }

    @Test void write_rejects_self_containing_structures() {
        List<Object> l = new ArrayList<>();
        l.add(l);
        assertThrows(IllegalArgumentException.class, () -> Json.write(l));
    }

    @Test void accessor_defaults() throws Exception {
        Map<String, Object> m = JsonObjects.object(Json.parse("{\"d\":2.5,\"s\":\"x\",\"b\":true,\"n\":null}"));
        assertEquals(2.5f, JsonObjects.decimal(m, "d", 0f));
        assertEquals(1.5f, JsonObjects.decimal(m, "s", 1.5f));
        assertEquals("x", JsonObjects.string(m, "s", "f"));
        assertEquals("f", JsonObjects.string(m, "d", "f"));
        assertEquals("f", JsonObjects.string(m, "n", "f"));
        assertEquals("f", JsonObjects.string(m, "missing", "f"));
        assertTrue(JsonObjects.bool(m, "b", false));
        assertEquals(3, JsonObjects.integer(m, "missing", 3));
        assertEquals(-2, JsonObjects.integer(Collections.<String, Object>singletonMap("k", -2L), "k", 0));
        assertEquals(5, JsonObjects.integer(Collections.<String, Object>singletonMap("k", -2147483649L), "k", 5));
        assertTrue(JsonObjects.object(null).isEmpty());
        assertTrue(JsonObjects.object(Json.parse("[]")).isEmpty());
    }

    private static int offsetOf(String text) {
        return assertThrows(JsonException.class, () -> Json.parse(text), text).offset();
    }

    @Test void truncated_input_points_at_what_was_left_open() {
        assertEquals(0, offsetOf("\"abc"));
        assertEquals(5, offsetOf("{\"a\":\"abc"));
        assertEquals(5, offsetOf("{\"a\":\"abc\\"));
        assertEquals(0, offsetOf("{\"a\":"));
        assertEquals(0, offsetOf("{\"a\":1"));
        assertEquals(0, offsetOf("{\"a\""));
        assertEquals(0, offsetOf("{"));
        assertEquals(0, offsetOf("{\"a\":1,"));
        assertEquals(5, offsetOf("{\"a\":[1,2"));
        assertEquals(5, offsetOf("{\"a\":["));
        assertEquals(1, offsetOf("[[1,"));
        assertEquals(0, offsetOf("["));
        assertEquals(0, offsetOf(""));
        assertEquals(3, offsetOf("  \n"));
    }

    @Test void invalid_escape_offset_points_at_backslash() {
        assertEquals(2, offsetOf("\"a\\x\""));
        assertEquals(1, offsetOf("\"\\u12G4\""));
    }

    @Test void depth_cap_is_the_same_for_parser_and_writer() throws Exception {
        Object ok = Json.parse(nested(64));
        assertEquals(ok, Json.parse(Json.write(ok)));
        assertThrows(JsonException.class, () -> Json.parse(nested(65)));
        assertNotNull(Json.write(nestedValue(64)));
        assertThrows(IllegalArgumentException.class, () -> Json.write(nestedValue(65)));
        Object mapsOk = Json.parse(nestedObjects(64));
        assertEquals(mapsOk, Json.parse(Json.write(mapsOk)));
        assertThrows(JsonException.class, () -> Json.parse(nestedObjects(65)));
    }

    private static Object nestedValue(int depth) {
        Object v = "leaf";
        for (int i = 0; i < depth; i++) {
            List<Object> l = new ArrayList<>();
            l.add(v);
            v = l;
        }
        return v;
    }

    private static String nestedObjects(int depth) {
        StringBuilder b = new StringBuilder();
        for (int i = 0; i < depth; i++) b.append("{\"a\":");
        b.append('1');
        for (int i = 0; i < depth; i++) b.append('}');
        return b.toString();
    }

    @Test void float_is_written_with_float_precision() throws Exception {
        assertEquals("0.1", Json.write(0.1f));
        assertEquals("1.0", Json.write(1.0f));
        assertThrows(IllegalArgumentException.class, () -> Json.write(Float.NaN));
        assertThrows(IllegalArgumentException.class, () -> Json.write(Float.NEGATIVE_INFINITY));
        Map<String, Object> m = new LinkedHashMap<>();
        m.put("f", 0.1f);
        Map<String, Object> back = JsonObjects.object(Json.parse(Json.write(m)));
        assertEquals(0.1f, JsonObjects.decimal(back, "f", 0f));
    }
}
