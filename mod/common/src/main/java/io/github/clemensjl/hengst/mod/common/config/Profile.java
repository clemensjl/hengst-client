package io.github.clemensjl.hengst.mod.common.config;

import io.github.clemensjl.hengst.mod.common.json.JsonObjects;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.Map;
import java.util.Set;

/**
 * All module settings of one named profile. Entries read from a file stay raw until a module is asked for by id,
 * so modules this version does not know (or entries it could not read) are written back unchanged.
 * Not thread-safe: client thread only.
 */
public final class Profile {
    // id -> ModuleConfig, or the raw parsed JSON value of an entry that was not accessed yet
    private final Map<String, Object> entries = new LinkedHashMap<String, Object>();
    // root keys other than "schema" and "modules", kept so that a newer version's data is written back
    final Map<String, Object> extras = new LinkedHashMap<String, Object>();

    /** The module's config; creates the defaults if there is none (or the stored entry is not an object). */
    public ModuleConfig module(String id) {
        Object e = entries.get(id);
        if (e instanceof ModuleConfig) {
            return (ModuleConfig) e;
        }
        ModuleConfig c = e instanceof Map ? ModuleConfig.fromJson(JsonObjects.object(e)) : ModuleConfig.defaults();
        entries.put(id, c);
        return c;
    }

    public Set<String> moduleIds() {
        return new LinkedHashSet<String>(entries.keySet());
    }

    public Profile copy() {
        Profile p = new Profile();
        for (Map.Entry<String, Object> e : entries.entrySet()) {
            Object v = e.getValue();
            p.entries.put(e.getKey(), v instanceof ModuleConfig ? ((ModuleConfig) v).copy() : ModuleConfig.deepCopy(v));
        }
        for (Map.Entry<String, Object> e : extras.entrySet()) {
            p.extras.put(e.getKey(), ModuleConfig.deepCopy(e.getValue()));
        }
        return p;
    }

    /** Never throws for wrong shapes; unreadable parts fall back to defaults. Top-level keys other than "schema" and "modules" are kept as read. */
    public static Profile fromJson(Map<String, Object> json) {
        Profile p = new Profile();
        for (Map.Entry<String, Object> e : JsonObjects.object(json.get("modules")).entrySet()) {
            p.entries.put(e.getKey(), ModuleConfig.deepCopy(e.getValue()));
        }
        for (Map.Entry<String, Object> e : json.entrySet()) {
            if (!"schema".equals(e.getKey()) && !"modules".equals(e.getKey())) {
                p.extras.put(e.getKey(), ModuleConfig.deepCopy(e.getValue()));
            }
        }
        return p;
    }

    public Map<String, Object> toJson() {
        Map<String, Object> modules = new LinkedHashMap<String, Object>();
        for (Map.Entry<String, Object> e : entries.entrySet()) {
            Object v = e.getValue();
            modules.put(e.getKey(), v instanceof ModuleConfig ? ((ModuleConfig) v).toJson() : v);
        }
        Map<String, Object> root = new LinkedHashMap<String, Object>();
        root.put("schema", AtomicFiles.SCHEMA);
        root.put("modules", modules);
        root.putAll(extras);
        return root;
    }
}
