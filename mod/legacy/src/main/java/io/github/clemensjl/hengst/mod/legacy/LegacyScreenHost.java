package io.github.clemensjl.hengst.mod.legacy;

import io.github.clemensjl.hengst.mod.common.platform.ScrollAccumulator;
import io.github.clemensjl.hengst.mod.common.platform.UiScreen;
import net.minecraft.client.gui.screen.Screen;
import org.lwjgl.input.Keyboard;
import org.lwjgl.input.Mouse;

/**
 * The game screen that hosts one of the core's screens. It draws nothing of its own (no background, no buttons),
 * forwards every input to the {@link UiScreen} and handles none itself, Escape included.
 *
 * <p>Mouse input arrives through vanilla's {@code Screen.handleMouse}, which converts LWJGL's raw position (y from
 * the bottom of the window) into scaled GUI pixels from the top left and calls {@link #mouseClicked},
 * {@link #mouseReleased} and {@link #mouseDragged} (absolute position); the wheel is read after it from the same
 * event.
 *
 * <p>After removal the host forwards nothing: {@code Screen.handleInput} keeps draining both LWJGL queues on this
 * object after a handler closed it, so events queued in the same tick after Escape or a Done click would otherwise
 * reach the closed core screen. {@code super.handleMouse()} still runs, and the raw-click hook (ScreenMixin, at the
 * call site in handleInput) sees every physical button either way.
 */
public final class LegacyScreenHost extends Screen {
    private final UiScreen ui;
    private final LegacyCanvas canvas = new LegacyCanvas();
    private final ScrollAccumulator scroll = new ScrollAccumulator(120.0);
    private boolean removed;
    private boolean repeatSaved;
    private boolean repeatBefore;

    public LegacyScreenHost(UiScreen ui) {
        this.ui = ui;
    }

    /** True once the game removed this screen; {@link LegacyPlatform#closeScreen()} then leaves it alone. */
    boolean isRemoved() {
        return removed;
    }

    /** For the dev in-game test: the hosted screen. */
    UiScreen ui() {
        return ui;
    }

    /**
     * From MinecraftClient.setScreen (and on every resize). Key repeats on while the host is open: LWJGL 2 delivers
     * repeats as further key-down events only when enabled. The state before the first init is restored on removal.
     */
    @Override
    public void init() {
        if (!repeatSaved) {
            repeatBefore = Keyboard.areRepeatEventsEnabled();
            repeatSaved = true;
        }
        Keyboard.enableRepeatEvents(true);
    }

    /**
     * No vanilla background (the dim of {@code renderBackground}) and no buttons: the host draws only what the core's
     * screen draws. The mouse position is the one GameRenderer.render computed the same way as handleMouse.
     */
    @Override
    public void render(int mouseX, int mouseY, float tickDelta) {
        canvas.begin(client.textRenderer, width, height);
        ui.render(canvas, mouseX, mouseY, tickDelta);
    }

    @Override
    protected void mouseClicked(int mouseX, int mouseY, int button) {
        if (removed) {
            return;
        }
        int b = LegacyInput.button(button);
        if (b != LegacyInput.OTHER_BUTTON) {
            ui.mouseClicked(mouseX, mouseY, b);
        }
    }

    @Override
    protected void mouseReleased(int mouseX, int mouseY, int button) {
        if (removed) {
            return;
        }
        int b = LegacyInput.button(button);
        if (b != LegacyInput.OTHER_BUTTON) {
            ui.mouseReleased(mouseX, mouseY, b);
        }
    }

    /** Called for move events while a button is held, with the absolute position. */
    @Override
    protected void mouseDragged(int mouseX, int mouseY, int button, long mouseLastClicked) {
        if (removed) {
            return;
        }
        int b = LegacyInput.button(button);
        if (b != LegacyInput.OTHER_BUTTON) {
            ui.mouseDragged(mouseX, mouseY, b);
        }
    }

    /**
     * One LWJGL mouse event: buttons and drags through vanilla, then the wheel as whole notches (+1 or -1 per call,
     * positive away from the user; 120 per notch, smaller touchpad steps accumulated).
     */
    @Override
    public void handleMouse() {
        super.handleMouse();
        int wheel = Mouse.getEventDWheel();
        if (wheel == 0 || removed) {
            return;
        }
        int notches = scroll.add(wheel);
        if (notches == 0) {
            return;
        }
        int mx = LegacyInput.guiX(Mouse.getEventX(), width, client.width);
        int my = LegacyInput.guiY(Mouse.getEventY(), height, client.height);
        for (; notches > 0; notches--) {
            ui.mouseScrolled(mx, my, 1.0);
        }
        for (; notches < 0; notches++) {
            ui.mouseScrolled(mx, my, -1.0);
        }
    }

    /**
     * Presses and, while the host is open, key repeats (Screen.handleKeyboard calls this for every key-down event);
     * Escape goes to the core's screen like any other key. super is not called: it would close the screen on Escape.
     */
    @Override
    protected void keyPressed(char id, int code) {
        if (removed) {
            return;
        }
        ui.keyPressed(LegacyInput.uiKey(code), hasShiftDown(), hasControlDown());
    }

    @Override
    public boolean shouldPauseGame() {
        return false;
    }

    /**
     * Removal rule: mark removed first, then tell the core's screen, exactly once. MinecraftClient.setScreen calls
     * this while the host is still the current screen.
     */
    @Override
    public void removed() {
        if (removed) {
            return;
        }
        removed = true;
        scroll.reset();
        if (repeatSaved) {
            Keyboard.enableRepeatEvents(repeatBefore);
        }
        ui.closed();
    }
}
