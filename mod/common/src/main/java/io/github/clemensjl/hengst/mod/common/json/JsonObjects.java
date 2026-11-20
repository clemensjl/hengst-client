package io.github.clemensjl.hengst.mod.common.json;

import java.util.Collections;
import java.util.Map;

/** Typed, forgiving accessors for parsed JSON objects: a missing key or wrong type yields the fallback. */
public final class JsonObjects {
    private JsonObjects() {}

    // Returns the live parsed map (or a shared empty map): callers must not mutate it.
    @SuppressWarnings("unchecked")
    public static Map<String, Object> object(Object value) {
        if (value instanceof Map) {
            return (Map<String, Object>) value;
        }
        return Collections.emptyMap();
    }

    public static String string(Map<String, Object> m, String key, String fallback) {
        Object v = m.get(key);
        return v instanceof String ? (String) v : fallback;
    }

    public static int integer(Map<String, Object> m, String key, int fallback) {
        Object v = m.get(key);
        if (v instanceof Long) {
            long l = (Long) v;
            if (l >= Integer.MIN_VALUE && l <= Integer.MAX_VALUE) {
                return (int) l;
            }
        }
        return fallback;
    }

    public static float decimal(Map<String, Object> m, String key, float fallback) {
        Object v = m.get(key);
        if (v instanceof Long || v instanceof Double) {
            return ((Number) v).floatValue();
        }
        return fallback;
    }

    public static boolean bool(Map<String, Object> m, String key, boolean fallback) {
        Object v = m.get(key);
        return v instanceof Boolean ? (Boolean) v : fallback;
    }

    public static long longValue(Map<String, Object> m, String key, long fallback) {
        Object v = m.get(key);
        return v instanceof Long ? (Long) v : fallback;
    }
}
