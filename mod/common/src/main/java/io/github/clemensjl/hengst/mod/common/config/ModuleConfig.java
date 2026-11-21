package io.github.clemensjl.hengst.mod.common.config;

import io.github.clemensjl.hengst.mod.common.feature.OptionSpec;
import io.github.clemensjl.hengst.mod.common.json.JsonObjects;
import io.github.clemensjl.hengst.mod.common.layout.Anchor;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** Stored settings of one module: switch, HUD placement, style and option values. Not thread-safe: client thread only. */
public final class ModuleConfig {
    private static final float MIN_SCALE = 0.5f;
    private static final float MAX_SCALE = 3.0f;

    private boolean enabled;
    private Anchor anchor = Anchor.TOP_LEFT;
    private int offsetX;
    private int offsetY;
    private float scale = 1.0f;
    private boolean background = true;
    private boolean shadow = true;
    private int textColor = 0xFFFFFFFF;
    // Raw values as read from the file or set by the UI; the option spec coerces them on every read.
    private final Map<String, Object> options = new LinkedHashMap<String, Object>();

    private ModuleConfig() {}

    public static ModuleConfig defaults() {
        return new ModuleConfig();
    }

    public boolean enabled() {
        return enabled;
    }

    public void setEnabled(boolean enabled) {
        this.enabled = enabled;
    }

    public Anchor anchor() {
        return anchor;
    }

    public void setAnchor(Anchor anchor) {
        this.anchor = anchor == null ? Anchor.TOP_LEFT : anchor;
    }

    public int offsetX() {
        return offsetX;
    }

    public void setOffsetX(int offsetX) {
        this.offsetX = offsetX;
    }

    public int offsetY() {
        return offsetY;
    }

    public void setOffsetY(int offsetY) {
        this.offsetY = offsetY;
    }

    public float scale() {
        return scale;
    }

    /** Clamps to 0.5 .. 3.0; NaN gives 1.0. */
    public void setScale(float scale) {
        this.scale = Float.isNaN(scale) ? 1.0f : Math.max(MIN_SCALE, Math.min(MAX_SCALE, scale));
    }

    public boolean background() {
        return background;
    }

    public void setBackground(boolean background) {
        this.background = background;
    }

    public boolean shadow() {
        return shadow;
    }

    public void setShadow(boolean shadow) {
        this.shadow = shadow;
    }

    public int textColor() {
        return textColor;
    }

    public void setTextColor(int textColor) {
        this.textColor = textColor;
    }

    /** The stored value coerced through the spec: in range, never non-finite, default if missing or wrong type. */
    public Object option(OptionSpec spec) {
        return spec.coerce(options.get(spec.key()));
    }

    /** Stores the value as given (null removes it); it is coerced when read through {@link #option}. */
    public void setOption(String key, Object value) {
        if (value == null) {
            options.remove(key);
        } else {
            options.put(key, value);
        }
    }

    public ModuleConfig copy() {
        ModuleConfig c = new ModuleConfig();
        c.enabled = enabled;
        c.anchor = anchor;
        c.offsetX = offsetX;
        c.offsetY = offsetY;
        c.scale = scale;
        c.background = background;
        c.shadow = shadow;
        c.textColor = textColor;
        for (Map.Entry<String, Object> e : options.entrySet()) {
            c.options.put(e.getKey(), deepCopy(e.getValue()));
        }
        return c;
    }

    static ModuleConfig fromJson(Map<String, Object> json) {
        ModuleConfig c = new ModuleConfig();
        c.enabled = JsonObjects.bool(json, "enabled", false);
        c.anchor = Anchor.parse(JsonObjects.string(json, "anchor", null), Anchor.TOP_LEFT);
        c.offsetX = JsonObjects.integer(json, "x", 0);
        c.offsetY = JsonObjects.integer(json, "y", 0);
        c.setScale(JsonObjects.decimal(json, "scale", 1.0f));
        c.background = JsonObjects.bool(json, "background", true);
        c.shadow = JsonObjects.bool(json, "shadow", true);
        c.textColor = (int) JsonObjects.longValue(json, "textColor", 0xFFFFFFFFL);
        for (Map.Entry<String, Object> e : JsonObjects.object(json.get("options")).entrySet()) {
            if (e.getValue() != null) {
                c.options.put(e.getKey(), deepCopy(e.getValue()));
            }
        }
        return c;
    }

    Map<String, Object> toJson() {
        Map<String, Object> m = new LinkedHashMap<String, Object>();
        m.put("enabled", enabled);
        m.put("anchor", anchor.name());
        m.put("x", offsetX);
        m.put("y", offsetY);
        m.put("scale", shortDecimal(scale));
        m.put("background", background);
        m.put("shadow", shadow);
        m.put("textColor", Long.valueOf(textColor & 0xFFFFFFFFL));
        Map<String, Object> opts = new LinkedHashMap<String, Object>();
        for (Map.Entry<String, Object> e : options.entrySet()) {
            Object v = e.getValue();
            if (v instanceof Float) {
                v = Float.isNaN((Float) v) || Float.isInfinite((Float) v) ? null : shortDecimal((Float) v);
            } else if (v instanceof Double && (Double.isNaN((Double) v) || Double.isInfinite((Double) v))) {
                v = null;
            }
            if (v != null) {
                opts.put(e.getKey(), v);
            }
        }
        m.put("options", opts);
        return m;
    }

    /** 1.3f becomes 1.3 instead of 1.2999999523162842. */
    private static Double shortDecimal(float f) {
        return Double.valueOf(Float.toString(f));
    }

    @SuppressWarnings("unchecked")
    static Object deepCopy(Object v) {
        if (v instanceof Map) {
            Map<String, Object> copy = new LinkedHashMap<String, Object>();
            for (Map.Entry<String, Object> e : ((Map<String, Object>) v).entrySet()) {
                copy.put(e.getKey(), deepCopy(e.getValue()));
            }
            return copy;
        }
        if (v instanceof List) {
            List<Object> copy = new ArrayList<Object>();
            for (Object o : (List<Object>) v) {
                copy.add(deepCopy(o));
            }
            return copy;
        }
        return v;
    }
}
