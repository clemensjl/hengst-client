package io.github.clemensjl.hengst.mod.common.config;

import io.github.clemensjl.hengst.mod.common.json.Json;
import io.github.clemensjl.hengst.mod.common.json.JsonObjects;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Clock;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.function.Consumer;

/** Reads and writes {@code settings.json}. Unknown keys are not preserved. */
public final class SettingsStore {
    private final Path file;
    private final Clock clock;
    private final Consumer<String> warn;

    public SettingsStore(Path file, Clock clock, Consumer<String> warn) {
        this.file = file;
        this.clock = clock;
        this.warn = warn;
    }

    /** Never throws: a missing or damaged file gives defaults (a damaged one is moved aside with a warning). */
    public Settings load() {
        Settings s = new Settings();
        Map<String, Object> json = AtomicFiles.readObjectOrBackup(file, clock, warn);
        if (json != null) {
            String name = JsonObjects.string(json, "activeProfile", ProfileStore.DEFAULT);
            if (ProfileStore.isValidName(name)) {
                s.setActiveProfile(name);
            }
        }
        return s;
    }

    public void save(Settings s) throws IOException {
        Map<String, Object> json = new LinkedHashMap<String, Object>();
        json.put("schema", 1);
        json.put("activeProfile", s.activeProfile());
        Path dir = file.toAbsolutePath().getParent();
        if (dir != null) {
            Files.createDirectories(dir);
        }
        AtomicFiles.writeUtf8(file, Json.write(json));
    }
}
