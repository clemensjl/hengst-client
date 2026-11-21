package io.github.clemensjl.hengst.mod.common;

import io.github.clemensjl.hengst.mod.common.json.Json;
import io.github.clemensjl.hengst.mod.common.json.JsonException;
import io.github.clemensjl.hengst.mod.common.json.JsonObjects;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.Collections;
import java.util.Locale;
import java.util.Map;

public final class I18n {
    private static final String FALLBACK = "en_us";

    private final Map<String, Object> primary;
    private final Map<String, Object> fallback;

    private I18n(Map<String, Object> primary, Map<String, Object> fallback) {
        this.primary = primary;
        this.fallback = fallback;
    }

    /** For tests: texts given directly instead of read from the language files. */
    public static I18n of(Map<String, Object> primary, Map<String, Object> fallback) {
        return new I18n(primary, fallback);
    }

    public static I18n load(String languageCode) {
        String code = languageCode == null ? FALLBACK : languageCode.toLowerCase(Locale.ROOT);
        Map<String, Object> fallback = read(FALLBACK);
        Map<String, Object> primary = code.equals(FALLBACK) ? fallback : read(code);
        return new I18n(primary, fallback);
    }

    public String get(String key) {
        Object v = primary.get(key);
        if (!(v instanceof String)) {
            v = fallback.get(key);
        }
        return v instanceof String ? (String) v : key;
    }

    /** Whether the key has a text in the active or the fallback language. */
    public boolean has(String key) {
        return primary.get(key) instanceof String || fallback.get(key) instanceof String;
    }

    /** The text of the first key that has one; the last key's text (or the key itself) if none has. */
    public String first(String... keys) {
        for (String key : keys) {
            if (has(key)) {
                return get(key);
            }
        }
        return get(keys[keys.length - 1]);
    }

    public String get(String key, Object arg) {
        return get(key).replace("%s", String.valueOf(arg));
    }

    private static Map<String, Object> read(String code) {
        try (InputStream in = I18n.class.getResourceAsStream("/hengst/lang/" + code + ".json")) {
            if (in == null) {
                return Collections.emptyMap();
            }
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            byte[] buf = new byte[4096];
            int n;
            while ((n = in.read(buf)) > 0) {
                out.write(buf, 0, n);
            }
            return JsonObjects.object(Json.parse(new String(out.toByteArray(), StandardCharsets.UTF_8)));
        } catch (IOException | JsonException e) {
            return Collections.emptyMap();
        }
    }
}
