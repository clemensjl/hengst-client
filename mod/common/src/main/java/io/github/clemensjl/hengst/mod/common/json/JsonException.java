package io.github.clemensjl.hengst.mod.common.json;

/** Thrown for malformed JSON text; carries the character offset of the problem. */
public final class JsonException extends Exception {
    private static final long serialVersionUID = 1L;

    private final int offset;

    public JsonException(String message, int offset) {
        super(message + " at offset " + offset);
        this.offset = offset;
    }

    /** Zero-based character offset into the parsed text where the problem was detected. */
    public int offset() {
        return offset;
    }
}
