package io.github.clemensjl.hengst.mod.common.hud.modules;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.feature.OptionSpec;
import io.github.clemensjl.hengst.mod.common.hud.HudContext;
import io.github.clemensjl.hengst.mod.common.hud.TextHudModule;
import io.github.clemensjl.hengst.mod.common.layout.Anchor;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.platform.GameView;
import java.util.Collections;
import java.util.EnumSet;
import java.util.List;

/**
 * Shows the address of the connected server as it was typed (case is kept). Without {@code show_port} a trailing
 * port is dropped: "[v6]:port" becomes "[v6]", an address with exactly one ':' loses everything from it, anything
 * else (a plain IPv6 literal, no port) stays. A shown text longer than 40 chars is cut to the first 19, "..", and the
 * last 19 (never inside a surrogate pair), so at most 40 chars. The address is only displayed: never resolved, never
 * sent, never logged. Visibility is checked every tick; the address is compared (reference, then {@code equals}) when
 * the module becomes visible, then every 20 ticks and when the option changes, and the text rebuilt only on change.
 */
public final class ServerAddressModule extends TextHudModule {
    private static final OptionSpec SHOW_PORT = OptionSpec.toggle("show_port", false);
    private static final List<OptionSpec> OPTIONS = Collections.singletonList(SHOW_PORT);
    private static final int INTERVAL_TICKS = 20;
    private static final int MAX_CHARS = 40;
    private static final int HEAD = 19;
    private static final int TAIL = 19;

    private int untilRead;
    private String lastRaw;
    private boolean builtShowPort;

    public ServerAddressModule() {
        super("server_address", 10, EnumSet.of(GameVersion.V1_8_9, GameVersion.V1_21_11, GameVersion.V26_3));
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
        return 80;
    }

    @Override
    public List<OptionSpec> options() {
        return OPTIONS;
    }

    /** The module is not ticked while off: after every apply the next tick reads the address again. */
    @Override
    public void onConfigChanged(ModuleConfig cfg) {
        untilRead = 0;
    }

    @Override
    public void tick(HudContext ctx, ModuleConfig cfg) {
        GameView game = ctx.platform().game();
        String raw = game.inWorld() ? game.serverAddress() : null;
        if (raw == null) {
            untilRead = 0;
            lastRaw = null;
            setText("");
            return;
        }
        boolean showPort = cfg.bool(SHOW_PORT);
        boolean optionChanged = showPort != builtShowPort;
        if (untilRead > 0 && !optionChanged) {
            untilRead--;
            return;
        }
        untilRead = INTERVAL_TICKS - 1;
        if (!optionChanged && lastRaw != null && (raw == lastRaw || raw.equals(lastRaw))) {
            return;
        }
        builtShowPort = showPort;
        lastRaw = raw;
        String shown = showPort ? raw : withoutPort(raw);
        setText(shown.length() > MAX_CHARS ? cutMiddle(shown) : shown);
    }

    private static String withoutPort(String s) {
        if (s.length() > 0 && s.charAt(0) == '[') {
            int close = s.indexOf(']');
            return close < 0 ? s : s.substring(0, close + 1);
        }
        int colon = s.indexOf(':');
        if (colon >= 0 && s.indexOf(':', colon + 1) < 0) {
            return s.substring(0, colon);
        }
        return s;
    }

    private static String cutMiddle(String s) {
        int head = HEAD;
        if (Character.isHighSurrogate(s.charAt(head - 1)) && Character.isLowSurrogate(s.charAt(head))) {
            head--;
        }
        int tailStart = s.length() - TAIL;
        if (Character.isHighSurrogate(s.charAt(tailStart - 1)) && Character.isLowSurrogate(s.charAt(tailStart))) {
            tailStart++;
        }
        return s.substring(0, head) + ".." + s.substring(tailStart);
    }
}
