package io.github.clemensjl.hengst.mod.common;

import io.github.clemensjl.hengst.mod.common.json.Json;
import io.github.clemensjl.hengst.mod.common.json.JsonException;
import io.github.clemensjl.hengst.mod.common.json.JsonObjects;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.HashMap;
import java.util.Locale;
import java.util.Map;

/**
 * The texts of one language. A code resolves to a bundled file in this order: the exact code, then the first bundled
 * file of the same language (the part before the underscore, in the order of {@link #BUNDLED}), then {@code en_us}.
 * Inside the chosen language a key without a text falls back to the {@code en_us} text, and one without that too is
 * shown as the key itself. The code is lower-cased first and must then be 2 to 16 characters of {@code a-z},
 * {@code 0-9} and {@code _}; anything else counts as an unsupported language (English) and never reaches the class
 * loader. A resolution is remembered per code, an unsupported one too, and each bundled file is read at most once.
 */
public final class I18n {
    private static final String FALLBACK = "en_us";
    /**
     * Every language file under {@code /hengst/lang/}, in order of preference: the first entry of a language answers a
     * regional code of that language (de_at, de_ch -> de_de). A test checks the table against the resources.
     */
    static final String[] BUNDLED = {"en_us", "de_de"};
    private static final int MAX_CACHED_CODES = 64;
    private static final Map<String, I18n> RESOLVED = new HashMap<String, I18n>();
    private static final Map<String, Map<String, Object>> FILES = new HashMap<String, Map<String, Object>>();
    private static int fileReads;

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

    /**
     * Resolves the code (see the class comment). Throws {@link IllegalStateException} if a bundled file that the
     * resolution needs cannot be read; nothing is remembered then, so a later call tries again.
     */
    public static synchronized I18n load(String languageCode) {
        String code = languageCode == null ? FALLBACK : languageCode.toLowerCase(Locale.ROOT);
        // All invalid codes share one entry, so a hostile code cannot fill the table or stay in memory.
        String key = valid(code) ? code : "";
        I18n known = RESOLVED.get(key);
        if (known != null) {
            return known;
        }
        String file = resolve(code);
        Map<String, Object> fallbackTexts = file(FALLBACK);
        I18n result = new I18n(file.equals(FALLBACK) ? fallbackTexts : file(file), fallbackTexts);
        if (RESOLVED.size() >= MAX_CACHED_CODES) {
            RESOLVED.clear();
        }
        RESOLVED.put(key, result);
        return result;
    }

    /** The bundled file that answers the (lower-cased) code; never touches the class loader. */
    private static String resolve(String code) {
        if (!valid(code)) {
            return FALLBACK;
        }
        for (String bundled : BUNDLED) {
            if (bundled.equals(code)) {
                return bundled;
            }
        }
        String language = language(code);
        for (String bundled : BUNDLED) {
            if (language(bundled).equals(language)) {
                return bundled;
            }
        }
        return FALLBACK;
    }

    private static String language(String code) {
        int underscore = code.indexOf('_');
        return underscore < 0 ? code : code.substring(0, underscore);
    }

    private static boolean valid(String code) {
        int n = code.length();
        if (n < 2 || n > 16) {
            return false;
        }
        for (int i = 0; i < n; i++) {
            char c = code.charAt(i);
            if (!(c >= 'a' && c <= 'z') && !(c >= '0' && c <= '9') && c != '_') {
                return false;
            }
        }
        return true;
    }

    /** How many bundled files were read from the class path so far; for tests. */
    static synchronized int fileReads() {
        return fileReads;
    }

    private static Map<String, Object> file(String code) {
        Map<String, Object> cached = FILES.get(code);
        if (cached == null) {
            cached = read(code);
            FILES.put(code, cached);
        }
        return cached;
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
        fileReads++;
        try (InputStream in = I18n.class.getResourceAsStream("/hengst/lang/" + code + ".json")) {
            if (in == null) {
                throw new IllegalStateException("the language file " + code + " is missing");
            }
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            byte[] buf = new byte[4096];
            int n;
            while ((n = in.read(buf)) > 0) {
                out.write(buf, 0, n);
            }
            return JsonObjects.object(Json.parse(new String(out.toByteArray(), StandardCharsets.UTF_8)));
        } catch (IOException | JsonException e) {
            throw new IllegalStateException("the language file " + code + " cannot be read: " + e, e);
        }
    }
}
