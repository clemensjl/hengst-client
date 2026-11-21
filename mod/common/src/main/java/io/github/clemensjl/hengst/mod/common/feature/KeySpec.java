package io.github.clemensjl.hengst.mod.common.feature;

import java.util.regex.Pattern;

/** One key binding the game lists: an id, its default key and whether the feature wants the release as well. */
public final class KeySpec {
    /** HOLD: the feature gets press and release. PRESS: only the press. */
    public enum Mode { HOLD, PRESS }

    private static final Pattern ID = Pattern.compile("[a-z0-9_]+(\\.[a-z0-9_]+)*");

    private final String id;
    private final DefaultKey defaultKey;
    private final Mode mode;
    private final String translationKey;

    public KeySpec(String id, DefaultKey defaultKey, Mode mode) {
        if (id == null || !ID.matcher(id).matches()) {
            throw new IllegalArgumentException("bad key id: " + id);
        }
        if (defaultKey == null || mode == null) {
            throw new IllegalArgumentException("default key and mode are required");
        }
        this.id = id;
        this.defaultKey = defaultKey;
        this.mode = mode;
        this.translationKey = "key.hengst." + id;
    }

    public String id() {
        return id;
    }

    public DefaultKey defaultKey() {
        return defaultKey;
    }

    public Mode mode() {
        return mode;
    }

    /** {@code "key.hengst." + id}; the name the game stores the binding under. */
    public String translationKey() {
        return translationKey;
    }
}
