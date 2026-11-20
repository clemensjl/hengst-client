package io.github.clemensjl.hengst.mod.common.layout;

public enum Anchor {
    TOP_LEFT(0f, 0f),
    TOP_CENTER(.5f, 0f),
    TOP_RIGHT(1f, 0f),
    MIDDLE_LEFT(0f, .5f),
    CENTER(.5f, .5f),
    MIDDLE_RIGHT(1f, .5f),
    BOTTOM_LEFT(0f, 1f),
    BOTTOM_CENTER(.5f, 1f),
    BOTTOM_RIGHT(1f, 1f);

    private final float fx;
    private final float fy;

    Anchor(float fx, float fy) {
        this.fx = fx;
        this.fy = fy;
    }

    public float fx() {
        return fx;
    }

    public float fy() {
        return fy;
    }

    /** Exact constant name, otherwise the fallback (also for null). */
    public static Anchor parse(String name, Anchor fallback) {
        if (name != null) {
            for (Anchor a : values()) {
                if (a.name().equals(name)) {
                    return a;
                }
            }
        }
        return fallback;
    }
}
