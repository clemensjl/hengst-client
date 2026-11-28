package io.github.clemensjl.hengst.mod.common.testutil;

import io.github.clemensjl.hengst.mod.common.I18n;
import io.github.clemensjl.hengst.mod.common.feature.FeatureGate;
import io.github.clemensjl.hengst.mod.common.feature.FeatureRegistry;
import io.github.clemensjl.hengst.mod.common.feature.RestrictionPolicy;
import io.github.clemensjl.hengst.mod.common.hud.CpsCounter;
import io.github.clemensjl.hengst.mod.common.hud.FrameCounter;
import io.github.clemensjl.hengst.mod.common.hud.HudContext;
import io.github.clemensjl.hengst.mod.common.hud.HudRenderer;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.platform.Platform;
import io.github.clemensjl.hengst.mod.common.platform.TimeSource;
import io.github.clemensjl.hengst.mod.common.platform.WallClock;

/** Test-only shortcuts for the main constructors (the shipped code has only the core's forms). */
public final class TestContexts {
    private TestContexts() {
    }

    /** System wall clock, the platform's language loaded. */
    public static HudContext context(Platform platform, TimeSource time, FrameCounter frames, CpsCounter cps) {
        return context(platform, time, WallClock.SYSTEM, frames, cps);
    }

    /** The platform's language loaded. */
    public static HudContext context(Platform platform, TimeSource time, WallClock wallClock, FrameCounter frames,
            CpsCounter cps) {
        return new HudContext(platform, time, wallClock, frames, cps, I18n.load(platform.languageCode()));
    }

    /** Without restrictions. */
    public static HudRenderer renderer(FeatureRegistry registry, GameVersion version) {
        return new HudRenderer(registry, new FeatureGate(version, RestrictionPolicy.NONE));
    }
}
