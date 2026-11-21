package io.github.clemensjl.hengst.mod.common.feature;

public final class OptionSpec {
    public enum Kind { TOGGLE, INT_RANGE, FLOAT_RANGE, CHOICE }

    private final String key;
    private final Kind kind;
    private final Object def;
    private final float min;
    private final float max;
    private final float step;
    // The decimal values of min and step (0.1f as 0.1), computed once: coerce runs on every option read.
    private final double decMin;
    private final double decStep;
    private final String[] values;

    private OptionSpec(String key, Kind kind, Object def, float min, float max, float step, String[] values) {
        this.key = key;
        this.kind = kind;
        this.def = def;
        this.min = min;
        this.max = max;
        this.step = step;
        this.decMin = Double.parseDouble(Float.toString(min));
        this.decStep = Double.parseDouble(Float.toString(step));
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

    private void requireKind(Kind expected) {
        if (kind != expected) {
            throw new IllegalArgumentException("option " + key + " is " + kind + ", not " + expected);
        }
    }

    /** Toggle: the raw value if it is a Boolean, else the default. */
    public boolean coerceBool(Object raw) {
        requireKind(Kind.TOGGLE);
        return raw instanceof Boolean ? ((Boolean) raw).booleanValue() : ((Boolean) def).booleanValue();
    }

    /** Int range: rounded and clamped into range; wrong types and NaN give the default. Does not box. */
    public int coerceInt(Object raw) {
        requireKind(Kind.INT_RANGE);
        if (!(raw instanceof Number)) {
            return ((Integer) def).intValue();
        }
        double n = ((Number) raw).doubleValue();
        if (Double.isNaN(n)) {
            return ((Integer) def).intValue();
        }
        long v = Math.round(n);
        return (int) Math.max((long) min, Math.min((long) max, v));
    }

    /** Float range: rounded to the step grid and clamped; wrong types and NaN give the default. Does not box. */
    public float coerceFloat(Object raw) {
        requireKind(Kind.FLOAT_RANGE);
        if (!(raw instanceof Number)) {
            return ((Float) def).floatValue();
        }
        double d = ((Number) raw).doubleValue();
        if (Double.isNaN(d)) {
            return ((Float) def).floatValue();
        }
        if (step > 0f) {
            // The grid is computed on the decimal values (0.1f as 0.1, not 0.100000001490116...), so the
            // result is the float nearest the grid value: 1.8 is stored as 1.8f, not 1.8000001f.
            d = decMin + Math.round((d - decMin) / decStep) * decStep;
        }
        return (float) Math.max(min, Math.min(max, d));
    }

    /** Choice: index into {@link #value}; wrong types and unknown values give {@link #defaultIndex()}. */
    public int coerceChoice(Object raw) {
        requireKind(Kind.CHOICE);
        if (raw instanceof String) {
            for (int i = 0; i < values.length; i++) {
                if (values[i].equals(raw)) {
                    return i;
                }
            }
        }
        return defaultIndex();
    }

    /** The choice value at the index, without copying the array. */
    public String value(int index) {
        return values[index];
    }

    /** Index of the default of a choice option. */
    public int defaultIndex() {
        requireKind(Kind.CHOICE);
        for (int i = 0; i < values.length; i++) {
            if (values[i].equals(def)) {
                return i;
            }
        }
        return 0; // unreachable: the factory checks that the default is one of the values
    }

    /** Clamps numbers into range (floats rounded to the step); wrong types and unknown choices give the default. */
    public Object coerce(Object raw) {
        switch (kind) {
            case TOGGLE:
                return Boolean.valueOf(coerceBool(raw));
            case INT_RANGE:
                return Integer.valueOf(coerceInt(raw));
            case FLOAT_RANGE: {
                float f = coerceFloat(raw);
                // An input that is already on the grid is returned as it is: no new box per read.
                return raw instanceof Float && ((Float) raw).floatValue() == f ? raw : (Object) f;
            }
            default:
                return values[coerceChoice(raw)];
        }
    }
}
