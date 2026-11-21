package io.github.clemensjl.hengst.mod.common.hud.modules;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.hud.HudContext;
import io.github.clemensjl.hengst.mod.common.hud.TextHudModule;
import io.github.clemensjl.hengst.mod.common.layout.Anchor;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.platform.GameView;
import java.util.EnumSet;

/**
 * Shows the own latency to the server as "42 ms", the same figure the player list (Tab) shows. Visible only in a world
 * with a server address (checked every tick, empty text otherwise). The value is read when the module becomes visible
 * and then every 20 ticks; {@link GameView#pingMillis()} below 0 (unknown: not reported yet, hidden by the server) is
 * shown as "? ms", never as a number, and a value above 9999 is shown as 9999. The text is rebuilt only when the
 * shown value changed. There is no colour by threshold: the number is the only carrier.
 */
public final class PingModule extends TextHudModule {
    private static final int INTERVAL_TICKS = 20;
    private static final int MAX_SHOWN = 9999;
    private static final int UNKNOWN = -1;
    private static final int NOTHING_SHOWN = -2;

    private final StringBuilder sb = new StringBuilder(8);
    private int untilSample;
    private int shownValue = NOTHING_SHOWN;

    public PingModule() {
        super("ping", 8, EnumSet.of(GameVersion.V1_8_9, GameVersion.V1_21_11, GameVersion.V26_3));
    }

    @Override
    public Anchor defaultAnchor() {
        return Anchor.TOP_RIGHT;
    }

    @Override
    public int defaultOffsetX() {
        return -2;
    }

    @Override
    public int defaultOffsetY() {
        return 52;
    }

    /** The module is not ticked while off: after every apply the next tick reads the ping again. */
    @Override
    public void onConfigChanged(ModuleConfig cfg) {
        untilSample = 0;
    }

    @Override
    public void tick(HudContext ctx, ModuleConfig cfg) {
        GameView game = ctx.platform().game();
        if (!game.inWorld() || game.serverAddress() == null) {
            untilSample = 0;
            shownValue = NOTHING_SHOWN;
            setText("");
            return;
        }
        if (untilSample > 0) {
            untilSample--;
            return;
        }
        untilSample = INTERVAL_TICKS - 1;
        int ping = game.pingMillis();
        int value = ping < 0 ? UNKNOWN : Math.min(ping, MAX_SHOWN);
        if (value == shownValue) {
            return;
        }
        shownValue = value;
        StringBuilder b = sb;
        b.setLength(0);
        if (value == UNKNOWN) {
            b.append('?');
        } else {
            b.append(value);
        }
        b.append(" ms");
        setText(b.toString());
    }
}
