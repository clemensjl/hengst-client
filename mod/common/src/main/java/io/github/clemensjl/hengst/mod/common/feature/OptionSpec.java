package io.github.clemensjl.hengst.mod.common.feature;

public final class OptionSpec {
    public enum Kind { TOGGLE, INT_RANGE, FLOAT_RANGE, CHOICE }

    private final String key;
    private final Kind kind;
    private final Object def;
    private final float min;
    private final float max;
    private final float step;
    private final String[] values;

    private OptionSpec(String key, Kind kind, Object def, float min, float max, float step, String[] values) {
        this.key = key;
        this.kind = kind;
        this.def = def;
        this.min = min;
        this.max = max;
        this.step = step;
        this.values = values;
    }

    public static OptionSpec toggle(String key, boolean def) {
        requireKey(key);
        return new OptionSpec(key, Kind.TOGGLE, def, 0f, 0f, 0f, new String[0]);
    }

    public static OptionSpec intRange(String key, int def, int min, int max) {
        requireKey(key);
        requireRange(key, def, min, max);
        return new OptionSpec(key, Kind.INT_RANGE, def, min, max, 1f, new String[0]);
    }

    public static OptionSpec floatRange(String key, float def, float min, float max, float step) {
        requireKey(key);
        requireRange(key, def, min, max);
        if (!(step > 0f)) {
            throw new IllegalArgumentException("option " + key + ": step must be greater than 0, got " + step);
        }
        return new OptionSpec(key, Kind.FLOAT_RANGE, def, min, max, step, new String[0]);
    }

    public static OptionSpec choice(String key, String def, String... values) {
        requireKey(key);
        if (values == null || values.length == 0) {
            throw new IllegalArgumentException("option " + key + ": choice needs at least one value");
        }
        boolean found = false;
        for (String v : values) {
            found |= v != null && v.equals(def);
        }
        if (!found) {
            throw new IllegalArgumentException("option " + key + ": default '" + def + "' is not one of the values");
        }
        return new OptionSpec(key, Kind.CHOICE, def, 0f, 0f, 0f, values.clone());
    }

    private static void requireKey(String key) {
        if (key == null || key.isEmpty()) {
            throw new IllegalArgumentException("option key must not be null or empty");
        }
    }

    private static void requireRange(String key, double def, double min, double max) {
        if (min > max) {
            throw new IllegalArgumentException("option " + key + ": min " + min + " is greater than max " + max);
        }
        if (!(def >= min && def <= max)) {
            throw new IllegalArgumentException("option " + key + ": default " + def + " is outside [" + min + ", " + max + "]");
        }
    }

    public String key() {
        return key;
    }

    public Kind kind() {
        return kind;
    }

    public Object defaultValue() {
        return def;
    }

    public float min() {
        return min;
    }

    public float max() {
        return max;
    }

    public float step() {
        return step;
    }

    public String[] values() {
        return values.clone();
    }

    /** Clamps numbers into range (floats rounded to the step); wrong types and unknown choices give the default. */
    public Object coerce(Object raw) {
        switch (kind) {
            case TOGGLE:
                return raw instanceof Boolean ? raw : def;
            case INT_RANGE: {
                if (!(raw instanceof Number)) {
                    return def;
                }
                double n = ((Number) raw).doubleValue();
                if (Double.isNaN(n)) {
                    return def;
                }
                long v = Math.round(n);
                return (int) Math.max((long) min, Math.min((long) max, v));
            }
            case FLOAT_RANGE: {
                if (!(raw instanceof Number)) {
                    return def;
                }
                double d = ((Number) raw).doubleValue();
                if (Double.isNaN(d)) {
                    return def;
                }
                if (step > 0f) {
                    d = min + Math.round((d - min) / step) * (double) step;
                }
                return (float) Math.max(min, Math.min(max, d));
            }
            default:
                if (raw instanceof String) {
                    for (String s : values) {
                        if (s.equals(raw)) {
                            return s;
                        }
                    }
                }
                return def;
        }
    }
}
