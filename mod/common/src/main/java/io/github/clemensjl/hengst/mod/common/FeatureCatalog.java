package io.github.clemensjl.hengst.mod.common;

import io.github.clemensjl.hengst.mod.common.feature.DefaultKey;
import io.github.clemensjl.hengst.mod.common.feature.Feature;
import io.github.clemensjl.hengst.mod.common.feature.KeySpec;
import io.github.clemensjl.hengst.mod.common.hud.modules.ClockModule;
import io.github.clemensjl.hengst.mod.common.hud.modules.CoordinatesModule;
import io.github.clemensjl.hengst.mod.common.hud.modules.CpsModule;
import io.github.clemensjl.hengst.mod.common.hud.modules.DirectionModule;
import io.github.clemensjl.hengst.mod.common.hud.modules.FpsModule;
import io.github.clemensjl.hengst.mod.common.hud.modules.KeystrokesModule;
import io.github.clemensjl.hengst.mod.common.hud.modules.MemoryModule;
import io.github.clemensjl.hengst.mod.common.hud.modules.PingModule;
import io.github.clemensjl.hengst.mod.common.hud.modules.ServerAddressModule;
import io.github.clemensjl.hengst.mod.common.hud.modules.SpeedModule;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/** Every feature of the mod, in number order. The one place a new feature is listed. Cold path. */
public final class FeatureCatalog {
    private FeatureCatalog() {}

    /** New instances of all features in number order. */
    public static List<Feature> create() {
        List<Feature> all = new ArrayList<Feature>();
        all.add(new FpsModule());
        all.add(new CpsModule());
        all.add(new KeystrokesModule());
        all.add(new CoordinatesModule());
        all.add(new DirectionModule());
        all.add(new PingModule());
        all.add(new ClockModule());
        all.add(new ServerAddressModule());
        all.add(new MemoryModule());
        all.add(new SpeedModule());
        return all;
    }

    /** The two core keys, index 0 and 1 of every key list; owned by no feature. */
    static List<KeySpec> coreKeys() {
        List<KeySpec> keys = new ArrayList<KeySpec>(2);
        keys.add(new KeySpec("settings", DefaultKey.RIGHT_SHIFT, KeySpec.Mode.PRESS));
        keys.add(new KeySpec("hud_editor", DefaultKey.NONE, KeySpec.Mode.PRESS));
        return keys;
    }

    /**
     * The key bindings for {@code version}: the core keys {@code settings} (index 0) and {@code hud_editor} (index 1),
     * then the keys of the features that version supports, in number order.
     */
    public static List<KeySpec> keys(GameVersion version) {
        List<KeySpec> keys = coreKeys();
        for (Feature f : create()) {
            if (f.supports(version)) {
                keys.addAll(f.keys());
            }
        }
        return Collections.unmodifiableList(keys);
    }
}
