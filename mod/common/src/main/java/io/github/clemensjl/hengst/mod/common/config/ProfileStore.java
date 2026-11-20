package io.github.clemensjl.hengst.mod.common.config;

import io.github.clemensjl.hengst.mod.common.json.Json;
import java.io.IOException;
import java.nio.file.DirectoryStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Clock;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashSet;
import java.util.List;
import java.util.Locale;
import java.util.Set;
import java.util.function.Consumer;

/** Profiles as {@code <dir>/<name>.json}. Not thread-safe: client thread only; the batched save runs on that same thread. */
public final class ProfileStore {
    public static final String DEFAULT = "default";

    private static final int MAX_NAME_LENGTH = 32;
    private static final String SUFFIX = ".json";
    private static final Set<String> RESERVED = new HashSet<String>();

    static {
        RESERVED.add("con");
        RESERVED.add("prn");
        RESERVED.add("aux");
        RESERVED.add("nul");
        for (int i = 1; i <= 9; i++) {
            RESERVED.add("com" + i);
            RESERVED.add("lpt" + i);
        }
    }

    private final Path dir;
    private final Clock clock;
    private final Consumer<String> warn;
    private final AtomicFiles.Guard guard = new AtomicFiles.Guard();
    private boolean loadedFromDefaults;

    public ProfileStore(Path dir, Clock clock, Consumer<String> warn) {
        this.dir = dir;
        this.clock = clock;
        this.warn = warn;
    }

    /** Sorted names of all profiles on disk; always contains {@link #DEFAULT}. */
    public List<String> names() {
        Set<String> found = new HashSet<String>();
        found.add(DEFAULT);
        if (Files.isDirectory(dir)) {
            try (DirectoryStream<Path> ds = Files.newDirectoryStream(dir, "*" + SUFFIX)) {
                for (Path p : ds) {
                    if (!Files.isRegularFile(p)) {
                        continue;
                    }
                    String file = p.getFileName().toString();
                    String name = file.substring(0, file.length() - SUFFIX.length());
                    if (isValidName(name)) {
                        found.add(name);
                    }
                }
            } catch (IOException e) {
                warn.accept("Cannot list profiles in " + dir + ": " + e);
            }
        }
        List<String> names = new ArrayList<String>(found);
        Collections.sort(names);
        return names;
    }

    /** Never throws for file problems: a missing or damaged file gives defaults (damaged ones are moved aside). */
    public Profile load(String name) {
        Path file = fileOf(name);
        AtomicFiles.Loaded l = guard.read(file, clock, warn);
        loadedFromDefaults = l.state == AtomicFiles.State.MISSING || l.state == AtomicFiles.State.MOVED_ASIDE;
        return l.state == AtomicFiles.State.OK ? Profile.fromJson(l.json) : new Profile();
    }

    /**
     * True if the last {@link #load} found the file missing, empty or damaged (and moved aside). False if the file
     * could not be read or was written by a newer version: the defaults are then in memory only, see {@link #loadFailed}.
     */
    public boolean loadedFromDefaults() {
        return loadedFromDefaults;
    }

    /**
     * True if the last load of this profile failed without the file being replaced by defaults (unreadable, damaged
     * and not movable, or newer schema). {@link #save} will not overwrite such a file blindly; callers can show a warning.
     */
    public boolean loadFailed(String name) {
        return guard.isBlocked(fileOf(name));
    }

    /**
     * Writes the profile atomically. On IOException nothing was replaced, so the caller's dirty state stays intact
     * and it can retry on the next batch. Refuses (IOException) to overwrite a file whose load failed, unless it
     * could be backed up first.
     */
    public void save(String name, Profile p) throws IOException {
        Path file = fileOf(name);
        guard.beforeWrite(file, clock, warn);
        Files.createDirectories(dir);
        AtomicFiles.writeUtf8(file, Json.write(p.toJson()));
    }

    /** Copies the stored source profile to the first free "Profil N" (N from 2) and returns the new name. */
    public String createCopy(String sourceName) throws IOException {
        Path sourceFile = fileOf(sourceName);
        AtomicFiles.Loaded l = guard.read(sourceFile, clock, warn);
        if (guard.isBlocked(sourceFile)) {
            throw new IOException("Cannot copy " + sourceName + ": it could not be loaded");
        }
        Profile source = l.state == AtomicFiles.State.OK ? Profile.fromJson(l.json) : new Profile();
        for (int n = 2; ; n++) {
            String name = "Profil " + n;
            if (!Files.exists(fileOf(name))) {
                save(name, source);
                return name;
            }
        }
    }

    public void delete(String name) throws IOException {
        Path file = fileOf(name);
        // Windows file names are case-insensitive: "Default" would be the default file as well.
        if (sameName(DEFAULT, name)) {
            throw new IllegalArgumentException("the default profile cannot be deleted");
        }
        Files.deleteIfExists(file);
    }

    public static boolean isValidName(String name) {
        if (name == null || name.isEmpty() || name.length() > MAX_NAME_LENGTH) {
            return false;
        }
        for (int i = 0; i < name.length(); i++) {
            char c = name.charAt(i);
            boolean ok = c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z' || c >= '0' && c <= '9'
                    || c == ' ' || c == '_' || c == '-';
            if (!ok) {
                return false;
            }
        }
        if (name.charAt(0) == ' ' || name.charAt(name.length() - 1) == ' ') {
            return false;
        }
        return !RESERVED.contains(name.toLowerCase(Locale.ROOT));
    }

    /** Case-insensitive name comparison: on Windows "Default" and "default" are the same file. */
    public static boolean sameName(String a, String b) {
        return a != null && b != null && a.toLowerCase(Locale.ROOT).equals(b.toLowerCase(Locale.ROOT));
    }

    private Path fileOf(String name) {
        if (!isValidName(name)) {
            throw new IllegalArgumentException("invalid profile name: " + name);
        }
        return dir.resolve(name + SUFFIX);
    }
}
