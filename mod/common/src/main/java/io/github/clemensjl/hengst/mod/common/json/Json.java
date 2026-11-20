package io.github.clemensjl.hengst.mod.common.json;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Minimal JSON reader and writer. Used at load and save time only, never per frame.
 *
 * <p>Parsed types: {@code LinkedHashMap<String,Object>}, {@code ArrayList<Object>}, {@code String},
 * {@code Long} (integers that fit a long), {@code Double} (all other numbers), {@code Boolean}, {@code null}.
 * Bad input only ever raises {@link JsonException}. Duplicate object keys: the last value wins and the key keeps
 * the position of its first occurrence.
 */
public final class Json {
    private static final int MAX_DEPTH = 64;

    private Json() {}

    public static Object parse(String text) throws JsonException {
        return new Parser(text.toCharArray()).parseDocument();
    }

    public static String write(Object value) {
        StringBuilder out = new StringBuilder();
        writeValue(out, value, 0, 0);
        return out.toString();
    }

    // ---- reader ----

    private static final class Parser {
        private final char[] s;
        private int pos;

        Parser(char[] s) {
            this.s = s;
        }

        Object parseDocument() throws JsonException {
            if (s.length > 0 && s[0] == '﻿') {
                pos = 1;
            }
            skipWs();
            Object v = parseValue(0);
            skipWs();
            if (pos < s.length) {
                throw new JsonException("unexpected trailing data", pos);
            }
            return v;
        }

        private void skipWs() {
            while (pos < s.length) {
                char c = s[pos];
                if (c == ' ' || c == '\t' || c == '\n' || c == '\r') {
                    pos++;
                } else {
                    break;
                }
            }
        }

        private Object parseValue(int depth) throws JsonException {
            if (pos >= s.length) {
                throw new JsonException("unexpected end of input", pos);
            }
            char c = s[pos];
            switch (c) {
                case '{':
                    return parseObject(depth + 1);
                case '[':
                    return parseArray(depth + 1);
                case '"':
                    return parseString();
                case 't':
                    return literal("true", Boolean.TRUE);
                case 'f':
                    return literal("false", Boolean.FALSE);
                case 'n':
                    return literal("null", null);
                default:
                    if (c == '-' || isDigit(c)) {
                        return parseNumber();
                    }
                    throw new JsonException("unexpected character", pos);
            }
        }

        private Object literal(String word, Object value) throws JsonException {
            int n = word.length();
            if (pos + n > s.length) {
                throw new JsonException("invalid literal", pos);
            }
            for (int i = 0; i < n; i++) {
                if (s[pos + i] != word.charAt(i)) {
                    throw new JsonException("invalid literal", pos);
                }
            }
            pos += n;
            return value;
        }

        private Object parseObject(int depth) throws JsonException {
            if (depth > MAX_DEPTH) {
                throw new JsonException("too deep", pos);
            }
            pos++; // opening brace
            Map<String, Object> map = new LinkedHashMap<String, Object>();
            skipWs();
            if (pos < s.length && s[pos] == '}') {
                pos++;
                return map;
            }
            while (true) {
                skipWs();
                if (pos >= s.length) {
                    throw new JsonException("unexpected end of input", pos);
                }
                if (s[pos] != '"') {
                    throw new JsonException("expected string key", pos);
                }
                String key = parseString();
                skipWs();
                if (pos >= s.length) {
                    throw new JsonException("unexpected end of input", pos);
                }
                if (s[pos] != ':') {
                    throw new JsonException("expected colon", pos);
                }
                pos++;
                skipWs();
                map.put(key, parseValue(depth));
                skipWs();
                if (pos >= s.length) {
                    throw new JsonException("unexpected end of input", pos);
                }
                char c = s[pos];
                if (c == ',') {
                    pos++;
                } else if (c == '}') {
                    pos++;
                    return map;
                } else {
                    throw new JsonException("expected comma or closing brace", pos);
                }
            }
        }

        private Object parseArray(int depth) throws JsonException {
            if (depth > MAX_DEPTH) {
                throw new JsonException("too deep", pos);
            }
            pos++; // opening bracket
            List<Object> list = new ArrayList<Object>();
            skipWs();
            if (pos < s.length && s[pos] == ']') {
                pos++;
                return list;
            }
            while (true) {
                skipWs();
                list.add(parseValue(depth));
                skipWs();
                if (pos >= s.length) {
                    throw new JsonException("unexpected end of input", pos);
                }
                char c = s[pos];
                if (c == ',') {
                    pos++;
                } else if (c == ']') {
                    pos++;
                    return list;
                } else {
                    throw new JsonException("expected comma or closing bracket", pos);
                }
            }
        }

        private String parseString() throws JsonException {
            pos++; // opening quote
            StringBuilder b = new StringBuilder();
            while (true) {
                if (pos >= s.length) {
                    throw new JsonException("unterminated string", pos);
                }
                char c = s[pos];
                if (c == '"') {
                    pos++;
                    return b.toString();
                }
                if (c < 0x20) {
                    throw new JsonException("control character in string", pos);
                }
                if (c != '\\') {
                    b.append(c);
                    pos++;
                    continue;
                }
                int escPos = pos;
                pos++;
                if (pos >= s.length) {
                    throw new JsonException("unterminated string", pos);
                }
                char e = s[pos++];
                switch (e) {
                    case '"': b.append('"'); break;
                    case '\\': b.append('\\'); break;
                    case '/': b.append('/'); break;
                    case 'b': b.append('\b'); break;
                    case 'f': b.append('\f'); break;
                    case 'n': b.append('\n'); break;
                    case 'r': b.append('\r'); break;
                    case 't': b.append('\t'); break;
                    case 'u': {
                        char u = hex4(escPos);
                        if (Character.isHighSurrogate(u)) {
                            if (pos + 1 < s.length && s[pos] == '\\' && s[pos + 1] == 'u') {
                                int lowPos = pos;
                                pos += 2;
                                char low = hex4(lowPos);
                                if (!Character.isLowSurrogate(low)) {
                                    throw new JsonException("invalid surrogate pair", lowPos);
                                }
                                b.append(u).append(low);
                            } else {
                                throw new JsonException("lone high surrogate", escPos);
                            }
                        } else if (Character.isLowSurrogate(u)) {
                            throw new JsonException("lone low surrogate", escPos);
                        } else {
                            b.append(u);
                        }
                        break;
                    }
                    default:
                        throw new JsonException("invalid escape", escPos);
                }
            }
        }

        /** Reads four hex digits at pos; errors are reported at errPos (the escape start). */
        private char hex4(int errPos) throws JsonException {
            if (pos + 4 > s.length) {
                throw new JsonException("invalid unicode escape", errPos);
            }
            int v = 0;
            for (int i = 0; i < 4; i++) {
                char h = s[pos + i];
                int d = h < 128 ? Character.digit(h, 16) : -1;
                if (d < 0) {
                    throw new JsonException("invalid unicode escape", errPos);
                }
                v = (v << 4) | d;
            }
            pos += 4;
            return (char) v;
        }

        private Object parseNumber() throws JsonException {
            int start = pos;
            if (s[pos] == '-') {
                pos++;
            }
            if (pos >= s.length || !isDigit(s[pos])) {
                throw new JsonException("invalid number", pos);
            }
            if (s[pos] == '0') {
                pos++;
                if (pos < s.length && isDigit(s[pos])) {
                    throw new JsonException("leading zero in number", pos);
                }
            } else {
                while (pos < s.length && isDigit(s[pos])) {
                    pos++;
                }
            }
            boolean integral = true;
            if (pos < s.length && s[pos] == '.') {
                integral = false;
                pos++;
                digits();
            }
            if (pos < s.length && (s[pos] == 'e' || s[pos] == 'E')) {
                integral = false;
                pos++;
                if (pos < s.length && (s[pos] == '+' || s[pos] == '-')) {
                    pos++;
                }
                digits();
            }
            String num = new String(s, start, pos - start);
            if (integral) {
                try {
                    return Long.valueOf(Long.parseLong(num));
                } catch (NumberFormatException overflow) {
                    // beyond the long range: fall through to double
                }
            }
            double d = Double.parseDouble(num);
            if (Double.isInfinite(d)) {
                throw new JsonException("number out of range", start);
            }
            return Double.valueOf(d);
        }

        private void digits() throws JsonException {
            if (pos >= s.length || !isDigit(s[pos])) {
                throw new JsonException("invalid number", pos);
            }
            while (pos < s.length && isDigit(s[pos])) {
                pos++;
            }
        }

        private static boolean isDigit(char c) {
            return c >= '0' && c <= '9';
        }
    }

    // ---- writer ----

    private static void writeValue(StringBuilder out, Object v, int indent, int depth) {
        if (depth > MAX_DEPTH) {
            throw new IllegalArgumentException("value nested deeper than " + MAX_DEPTH);
        }
        if (v == null) {
            out.append("null");
        } else if (v instanceof String) {
            writeString(out, (String) v);
        } else if (v instanceof Boolean || v instanceof Long || v instanceof Integer) {
            out.append(v.toString());
        } else if (v instanceof Double) {
            double d = (Double) v;
            if (Double.isNaN(d) || Double.isInfinite(d)) {
                throw new IllegalArgumentException("NaN and Infinity are not representable in JSON");
            }
            out.append(v.toString());
        } else if (v instanceof Map) {
            Map<?, ?> m = (Map<?, ?>) v;
            if (m.isEmpty()) {
                out.append("{}");
                return;
            }
            out.append("{\n");
            boolean first = true;
            for (Map.Entry<?, ?> e : m.entrySet()) {
                if (!(e.getKey() instanceof String)) {
                    throw new IllegalArgumentException("object keys must be strings");
                }
                if (!first) {
                    out.append(",\n");
                }
                first = false;
                pad(out, indent + 1);
                writeString(out, (String) e.getKey());
                out.append(": ");
                writeValue(out, e.getValue(), indent + 1, depth + 1);
            }
            out.append('\n');
            pad(out, indent);
            out.append('}');
        } else if (v instanceof List) {
            List<?> l = (List<?>) v;
            if (l.isEmpty()) {
                out.append("[]");
                return;
            }
            out.append("[\n");
            for (int i = 0; i < l.size(); i++) {
                if (i > 0) {
                    out.append(",\n");
                }
                pad(out, indent + 1);
                writeValue(out, l.get(i), indent + 1, depth + 1);
            }
            out.append('\n');
            pad(out, indent);
            out.append(']');
        } else {
            throw new IllegalArgumentException("unsupported type " + v.getClass().getName());
        }
    }

    private static void pad(StringBuilder out, int level) {
        for (int i = 0; i < level; i++) {
            out.append("  ");
        }
    }

    private static final char[] HEX = "0123456789abcdef".toCharArray();

    private static void writeString(StringBuilder out, String str) {
        out.append('"');
        for (int i = 0; i < str.length(); i++) {
            char c = str.charAt(i);
            if (c == '"') {
                out.append("\\\"");
            } else if (c == '\\') {
                out.append("\\\\");
            } else if (c < 0x20) {
                out.append("\\u00").append(HEX[c >> 4]).append(HEX[c & 15]);
            } else {
                out.append(c);
            }
        }
        out.append('"');
    }
}
