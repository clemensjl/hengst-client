package io.github.clemensjl.hengst.mod.common.hud;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.config.Profile;
import io.github.clemensjl.hengst.mod.common.feature.Feature;
import io.github.clemensjl.hengst.mod.common.feature.FeatureRegistry;
import io.github.clemensjl.hengst.mod.common.layout.LayoutMath;
import io.github.clemensjl.hengst.mod.common.layout.MutableRect;
import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.platform.GameView;
import java.util.Arrays;
import java.util.List;

/**
 * Draws the active HUD modules. The module arrays are rebuilt only when the configuration changes;
 * {@link #tick} and {@link #render} allocate nothing.
 */
public final class HudRenderer {
    private static final HudModule[] NO_MODULES = new HudModule[0];
    private static final ModuleConfig[] NO_CONFIGS = new ModuleConfig[0];

    private final FeatureRegistry registry;
    private final GameVersion version;
    private final MutableRect rect = new MutableRect();
    private HudModule[] modules = NO_MODULES;
    private ModuleConfig[] configs = NO_CONFIGS;

    public HudRenderer(FeatureRegistry registry, GameVersion version) {
        this.registry = registry;
        this.version = version;
    }

    /** Collects the enabled, supported modules in number order. Allocates; call only on configuration changes. */
    public void rebuild(Profile profile) {
        List<Feature> supported = registry.supportedOn(version);
        HudModule[] m = new HudModule[supported.size()];
        ModuleConfig[] c = new ModuleConfig[supported.size()];
        int n = 0;
        for (Feature f : supported) {
            if (f instanceof HudModule) {
                ModuleConfig cfg = profile.module(f.id());
                if (cfg.enabled()) {
                    m[n] = (HudModule) f;
                    c[n] = cfg;
                    n++;
                }
            }
        }
        modules = Arrays.copyOf(m, n);
        configs = Arrays.copyOf(c, n);
    }

    public int activeCount() {
        return modules.length;
    }

    public HudModule[] activeModules() {
        return modules;
    }

    public ModuleConfig[] activeConfigs() {
        return configs;
    }

    public void tick(HudContext ctx) {
        HudModule[] m = modules;
        ModuleConfig[] c = configs;
        for (int i = 0; i < m.length; i++) {
            m[i].tick(ctx, c[i]);
        }
    }

    /** Draws nothing when the HUD is hidden (F1), outside a world, or while a screen is open. */
    public void render(Canvas canvas, HudContext ctx) {
        GameView game = ctx.platform().game();
        if (game.hudHidden() || !game.inWorld() || game.screenOpen()) {
            return;
        }
        HudModule[] m = modules;
        ModuleConfig[] c = configs;
        MutableRect r = rect;
        for (int i = 0; i < m.length; i++) {
            HudModule module = m[i];
            ModuleConfig cfg = c[i];
            float scale = cfg.scale();
            int w = Math.round(module.width(canvas, cfg) * scale);
            int h = Math.round(module.height(canvas, cfg) * scale);
            LayoutMath.resolve(cfg, w, h, canvas.width(), canvas.height(), r);
            canvas.pushTransform(r.x, r.y, scale);
            module.render(canvas, cfg, ctx);
            canvas.popTransform();
        }
    }
}
