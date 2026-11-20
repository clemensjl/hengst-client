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

/** Reads and writes {@code settings.json}. Unknown keys are not preserved. Not thread-safe: client thread only (the batched save runs there too). */
public final class SettingsStore {
    private final Path file;
    private final Clock clock;
    private final Consumer<String> warn;
    private final AtomicFiles.Guard guard = new AtomicFiles.Guard();

    public SettingsStore(Path file, Clock clock, Consumer<String> warn) {
        this.file = file;
        this.clock = clock;
        this.warn = warn;
    }

    /** Never throws: a missing or damaged file gives defaults (a damaged one is moved aside with a warning). */
    public Settings load() {
        Settings s = new Settings();
        AtomicFiles.Loaded l = guard.read(file, clock, warn);
        if (l.state == AtomicFiles.State.OK) {
            String name = JsonObjects.string(l.json, "activeProfile", ProfileStore.DEFAULT);
            if (ProfileStore.isValidName(name)) {
                s.setActiveProfile(name);
            }
        }
        return s;
    }

    /** True if the last load failed without the file being replaced by defaults (unreadable, damaged and not movable, or newer schema). */
    public boolean settingsLoadFailed() {
        return guard.isBlocked(file);
    }

    /** Refuses (IOException) to overwrite a file whose load failed, unless it could be backed up first. */
    public void save(Settings s) throws IOException {
        guard.beforeWrite(file, clock, warn);
        Map<String, Object> json = new LinkedHashMap<String, Object>();
        json.put("schema", AtomicFiles.SCHEMA);
        json.put("activeProfile", s.activeProfile());
        Path dir = file.toAbsolutePath().getParent();
        if (dir != null) {
            Files.createDirectories(dir);
        }
        AtomicFiles.writeUtf8(file, Json.write(json));
    }
}
