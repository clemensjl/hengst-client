package io.github.clemensjl.hengst.mod.common.config;

import io.github.clemensjl.hengst.mod.common.json.Json;
import io.github.clemensjl.hengst.mod.common.json.JsonException;
import io.github.clemensjl.hengst.mod.common.json.JsonObjects;
import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.channels.FileChannel;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.StandardCharsets;
import java.nio.file.AtomicMoveNotSupportedException;
import java.nio.file.FileAlreadyExistsException;
import java.nio.file.Files;
import java.nio.file.NoSuchFileException;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.nio.file.StandardOpenOption;
import java.time.Clock;
import java.time.format.DateTimeFormatter;
import java.util.HashMap;
import java.util.Map;
import java.util.function.Consumer;

/**
 * File helpers for the config stores. Used at load and save time only, never per frame or tick.
 * Not thread-safe: use from the client thread only (the batched save runs on that same thread).
 */
public final class AtomicFiles {
    /** The schema version this build reads and writes. Files with a higher one are never overwritten. */
    static final int SCHEMA = 1;

    private static final DateTimeFormatter STAMP = DateTimeFormatter.ofPattern("yyyyMMdd-HHmmss");

    /** Seam so tests can make backups fail; production always uses {@link Files}. */
    interface Mover {
        Path move(Path from, Path to) throws IOException;

        Path copy(Path from, Path to) throws IOException;
    }

    static volatile Mover mover = new Mover() {
        @Override
        public Path move(Path from, Path to) throws IOException {
            return Files.move(from, to);
        }

        @Override
        public Path copy(Path from, Path to) throws IOException {
            return Files.copy(from, to);
        }
    };

    private AtomicFiles() {}

    /** Writes to {@code <target>.tmp} in the same directory, then moves it over the target. */
    public static void writeUtf8(Path target, String content) throws IOException {
        Path tmp = target.resolveSibling(target.getFileName() + ".tmp");
        try {
            ByteBuffer buf = ByteBuffer.wrap(content.getBytes(StandardCharsets.UTF_8));
            try (FileChannel ch = FileChannel.open(tmp, StandardOpenOption.CREATE, StandardOpenOption.WRITE,
                    StandardOpenOption.TRUNCATE_EXISTING)) {
                while (buf.hasRemaining()) {
                    ch.write(buf);
                }
                ch.force(true);
            }
            try {
                Files.move(tmp, target, StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING);
            } catch (AtomicMoveNotSupportedException e) {
                Files.move(tmp, target, StandardCopyOption.REPLACE_EXISTING);
            }
        } catch (IOException | RuntimeException e) {
            try {
                Files.deleteIfExists(tmp);
            } catch (IOException ignored) {
                // the original failure is the one that matters
            }
            throw e;
        }
    }

    /** Renames the file to {@code <name>.broken-yyyyMMdd-HHmmss} (with {@code -2}, {@code -3}, ... if taken). */
    public static Path backupBroken(Path file, Clock clock) throws IOException {
        while (true) {
            try {
                return mover.move(file, freeBackupName(file, clock));
            } catch (FileAlreadyExistsException raced) {
                // somebody took the name between the check and the move: pick the next one
            }
        }
    }

    /** Like {@link #backupBroken} but copies the file if it cannot be moved; the original stays in that case. */
    static Path backupOrCopy(Path file, Clock clock) throws IOException {
        try {
            return backupBroken(file, clock);
        } catch (IOException moveFailed) {
            while (true) {
                try {
                    return mover.copy(file, freeBackupName(file, clock));
                } catch (FileAlreadyExistsException raced) {
                    // next name
                } catch (IOException copyFailed) {
                    copyFailed.addSuppressed(moveFailed);
                    throw copyFailed;
                }
            }
        }
    }

    private static Path freeBackupName(Path file, Clock clock) {
        String base = file.getFileName() + ".broken-" + STAMP.withZone(clock.getZone()).format(clock.instant());
        Path target = file.resolveSibling(base);
        for (int n = 2; Files.exists(target); n++) {
            target = file.resolveSibling(base + "-" + n);
        }
        return target;
    }

    enum State {
        /** Valid object with a known schema. */
        OK,
        /** No such file: a normal first start. */
        MISSING,
        /** Damaged file, moved aside; defaults apply. */
        MOVED_ASIDE,
        /** Could not be read, or is damaged and could not be moved aside; the file is still in place. */
        UNSAFE,
        /** Written by a newer version of the mod; never overwritten. */
        NEWER
    }

    static final class Loaded {
        final State state;
        final Map<String, Object> json; // only for OK
        final String message;

        Loaded(State state, Map<String, Object> json, String message) {
            this.state = state;
            this.json = json;
            this.message = message;
        }
    }

    /** Reads a JSON object; every problem is reported once through warn and described by the state. */
    static Loaded read(Path file, Clock clock, Consumer<String> warn) {
        String text;
        try {
            byte[] bytes = Files.readAllBytes(file);
            text = StandardCharsets.UTF_8.newDecoder().decode(ByteBuffer.wrap(bytes)).toString();
        } catch (NoSuchFileException e) {
            return new Loaded(State.MISSING, null, null);
        } catch (CharacterCodingException e) {
            return moveAside(file, clock, warn, "not valid UTF-8");
        } catch (IOException e) {
            String msg = "Cannot read " + file + " (" + e + "), using defaults; the file is left untouched";
            warn.accept(msg);
            return new Loaded(State.UNSAFE, null, msg);
        }
        Object parsed;
        try {
            parsed = Json.parse(text);
        } catch (JsonException e) {
            return moveAside(file, clock, warn, e.getMessage());
        }
        if (!(parsed instanceof Map)) {
            return moveAside(file, clock, warn, "not a JSON object");
        }
        Map<String, Object> json = JsonObjects.object(parsed);
        long schema = JsonObjects.longValue(json, "schema", SCHEMA);
        if (schema > SCHEMA) {
            String msg = file + " was written by a newer version of the mod (schema " + schema
                    + "), using defaults in memory and not overwriting it";
            warn.accept(msg);
            return new Loaded(State.NEWER, null, msg);
        }
        return new Loaded(State.OK, json, null);
    }

    private static Loaded moveAside(Path file, Clock clock, Consumer<String> warn, String reason) {
        try {
            Path moved = backupBroken(file, clock);
            warn.accept("Damaged file " + file + " (" + reason + ") moved to " + moved.getFileName() + ", using defaults");
            return new Loaded(State.MOVED_ASIDE, null, null);
        } catch (IOException e) {
            String msg = "Damaged file " + file + " (" + reason + ") could not be moved aside (" + e
                    + "), using defaults; the file is left untouched";
            warn.accept(msg);
            return new Loaded(State.UNSAFE, null, msg);
        }
    }

    /**
     * Remembers files whose load failed in a way that makes overwriting them unsafe, and enforces that before a write.
     * One instance per store; not thread-safe.
     */
    static final class Guard {
        private final Map<Path, Loaded> blocked = new HashMap<Path, Loaded>();

        Loaded read(Path file, Clock clock, Consumer<String> warn) {
            Loaded l = AtomicFiles.read(file, clock, warn);
            if (l.state == State.UNSAFE || l.state == State.NEWER) {
                blocked.put(file, l);
            } else {
                blocked.remove(file);
            }
            return l;
        }

        boolean isBlocked(Path file) {
            return blocked.containsKey(file);
        }

        /**
         * Call before writing the file. Newer-schema files are refused. Unreadable or damaged files get their backup
         * retried (move, then copy); if no backup can be made, or the path is not a regular file, the write is refused.
         */
        void beforeWrite(Path file, Clock clock, Consumer<String> warn) throws IOException {
            Loaded l = blocked.get(file);
            if (l == null) {
                return;
            }
            if (l.state == State.NEWER) {
                throw new IOException(l.message);
            }
            if (!Files.exists(file)) {
                blocked.remove(file);
                return;
            }
            if (!Files.isRegularFile(file)) {
                throw new IOException(file + " is not a regular file, not overwriting it");
            }
            Path backup;
            try {
                backup = backupOrCopy(file, clock);
            } catch (IOException e) {
                throw new IOException("Cannot back up " + file + " before overwriting it; leaving it untouched", e);
            }
            blocked.remove(file);
            warn.accept("Backed up " + file + " to " + backup.getFileName() + " before overwriting it");
        }
    }
}
