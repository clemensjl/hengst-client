package io.github.clemensjl.hengst.mod.common.hud.modules;

import io.github.clemensjl.hengst.mod.common.I18n;
import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.feature.OptionSpec;
import io.github.clemensjl.hengst.mod.common.hud.HudContext;
import io.github.clemensjl.hengst.mod.common.hud.TextHudModule;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.platform.GameView;
import java.util.Collections;
import java.util.EnumSet;
import java.util.List;

/**
 * Shows the facing as a compass course, like the "Facing" line of the F3 screen. Yaw 0 looks south, 180 north, so
 * the course is {@code h = ((yaw + 180) mod 360 + 360) mod 360}, shown as {@code d = round(h) mod 360} degrees (a
 * half degree rounds up) and the letters of the eighth {@code ((2d + 45) / 90) mod 8}. The 360 texts of the current
 * style and language are built once when the style or the language generation changes; a tick only picks one.
 * Empty outside a world, "?" for a yaw that is not a finite number.
 */
public final class DirectionModule extends TextHudModule {
    private static final OptionSpec STYLE =
            OptionSpec.choice("style", "cardinal_degrees", "cardinal", "cardinal_degrees", "degrees");
    private static final List<OptionSpec> OPTIONS = Collections.singletonList(STYLE);
    private static final int CARDINAL = 0;
    private static final int DEGREES = 2;
    private static final String[] LETTER_KEYS = {
        "hengst.direction.n", "hengst.direction.ne", "hengst.direction.e", "hengst.direction.se",
        "hengst.direction.s", "hengst.direction.sw", "hengst.direction.w", "hengst.direction.nw"
    };
    private static final int NO_COURSE = -1;
    private static final int NOT_BUILT = -2;

    private final String[] table = new String[360];
    private int builtStyle = NOT_BUILT;
    private int builtGeneration = -1;
    private int lastCourse = NOT_BUILT;

    public DirectionModule() {
        super("direction", 7, EnumSet.of(GameVersion.V1_8_9, GameVersion.V1_21_11, GameVersion.V26_3));
    }

    @Override
    public int defaultOffsetX() {
        return 2;
    }

    @Override
    public int defaultOffsetY() {
        return 44;
    }

    @Override
    public List<OptionSpec> options() {
        return OPTIONS;
    }

    @Override
    public void tick(HudContext ctx, ModuleConfig cfg) {
        GameView game = ctx.platform().game();
        if (!game.inWorld()) {
            lastCourse = NOT_BUILT;
            setText("");
            return;
        }
        int style = cfg.choice(STYLE);
        if (style != builtStyle || ctx.languageGeneration() != builtGeneration) {
            build(ctx.i18n(), style);
            builtStyle = style;
            builtGeneration = ctx.languageGeneration();
            lastCourse = NOT_BUILT;
        }
        int course = course(game.playerYaw());
        if (course == lastCourse) {
            return;
        }
        lastCourse = course;
        setText(course == NO_COURSE ? "?" : table[course]);
    }

    /** The displayed degrees 0 to 359, or {@link #NO_COURSE} for a yaw that is NaN or infinite. No allocation. */
    static int course(float yaw) {
        if (Float.isNaN(yaw) || Float.isInfinite(yaw)) {
            return NO_COURSE;
        }
        float h = (yaw + 180f) % 360f; // exact remainder: also right for huge yaws
        if (h < 0f) {
            h += 360f;
        }
        return Math.round(h) % 360;
    }

    private void build(I18n i18n, int style) {
        String[] letters = new String[8];
        for (int i = 0; i < 8; i++) {
            letters[i] = i18n.get(LETTER_KEYS[i]);
        }
        StringBuilder b = new StringBuilder(16);
        for (int d = 0; d < 360; d++) {
            b.setLength(0);
            if (style != DEGREES) {
                b.append(letters[((2 * d + 45) / 90) % 8]);
            }
            if (style != CARDINAL) {
                if (style != DEGREES) {
                    b.append(' ');
                }
                b.append(d).append('°');
            }
            table[d] = b.toString();
        }
    }
}
