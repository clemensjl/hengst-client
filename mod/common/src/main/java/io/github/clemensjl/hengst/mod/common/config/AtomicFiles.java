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
import java.nio.file.Files;
import java.nio.file.NoSuchFileException;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.nio.file.StandardOpenOption;
import java.time.Clock;
import java.time.format.DateTimeFormatter;
import java.util.Map;
import java.util.function.Consumer;

/** File helpers for the config stores. Used at load and save time only, never per frame or tick. */
public final class AtomicFiles {
    private static final DateTimeFormatter STAMP = DateTimeFormatter.ofPattern("yyyyMMdd-HHmmss");

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
        String base = file.getFileName() + ".broken-" + STAMP.withZone(clock.getZone()).format(clock.instant());
        Path target = file.resolveSibling(base);
        for (int n = 2; Files.exists(target); n++) {
            target = file.resolveSibling(base + "-" + n);
        }
        return Files.move(file, target);
    }

    /**
     * Reads a JSON object from the file. Returns null (and warns once) if the file is empty, unreadable, not UTF-8,
     * not JSON or not an object; a damaged file is moved aside first. Returns null without a warning if the file
     * does not exist.
     */
    static Map<String, Object> readObjectOrBackup(Path file, Clock clock, Consumer<String> warn) {
        String text;
        try {
            byte[] bytes = Files.readAllBytes(file);
            text = StandardCharsets.UTF_8.newDecoder().decode(ByteBuffer.wrap(bytes)).toString();
        } catch (NoSuchFileException e) {
            return null;
        } catch (CharacterCodingException e) {
            moveAside(file, clock, warn, "not valid UTF-8");
            return null;
        } catch (IOException e) {
            warn.accept("Cannot read " + file + ", using defaults: " + e);
            return null;
        }
        Object parsed;
        try {
            parsed = Json.parse(text);
        } catch (JsonException e) {
            moveAside(file, clock, warn, e.getMessage());
            return null;
        }
        if (!(parsed instanceof Map)) {
            moveAside(file, clock, warn, "not a JSON object");
            return null;
        }
        return JsonObjects.object(parsed);
    }

    private static void moveAside(Path file, Clock clock, Consumer<String> warn, String reason) {
        try {
            Path moved = backupBroken(file, clock);
            warn.accept("Damaged file " + file + " (" + reason + ") moved to " + moved.getFileName() + ", using defaults");
        } catch (IOException e) {
            warn.accept("Damaged file " + file + " (" + reason + ") could not be moved aside (" + e + "), using defaults");
        }
    }
}
