package io.github.clemensjl.hengst.mod.legacy;

import io.github.clemensjl.hengst.mod.common.platform.UiKey;
import org.lwjgl.input.Keyboard;

/**
 * Pure translations from LWJGL 2 input values to the core's conventions. The Keyboard values are compile-time
 * constants, so nothing here loads LWJGL.
 */
final class LegacyInput {
    /** Returned by {@link #button} and {@link #buttonOfKeyCode} for anything but left, right and middle. */
    static final int OTHER_BUTTON = -1;

    /**
     * Key-binding code of LWJGL mouse button 0: MinecraftClient.tick passes {@code Mouse.getEventButton() - 100} to
     * KeyBinding.setKeyPressed, and GameOptions binds attack to -100, use to -99, pick block to -98.
     */
    static final int MOUSE_KEY_CODE_OFFSET = -100;

    private LegacyInput() {
    }

    /**
     * {@code Mouse.getEventButton()} (0 left, 1 right, 2 middle, -1 for move and wheel events) to the core's 0 left,
     * 1 right, 2 middle; {@link #OTHER_BUTTON} for the rest.
     */
    static int button(int lwjglButton) {
        return lwjglButton >= 0 && lwjglButton <= 2 ? lwjglButton : OTHER_BUTTON;
    }

    /** A key-binding code to the mouse button it stands for; keyboard codes are {@link #OTHER_BUTTON}. */
    static int buttonOfKeyCode(int code) {
        return code < 0 ? button(code - MOUSE_KEY_CODE_OFFSET) : OTHER_BUTTON;
    }

    /** An LWJGL 2 key code ({@code Keyboard.getEventKey()}) to the keys the mod's screens understand. */
    static UiKey uiKey(int key) {
        switch (key) {
            case Keyboard.KEY_ESCAPE:
                return UiKey.ESCAPE;
            case Keyboard.KEY_RETURN:
            case Keyboard.KEY_NUMPADENTER:
                return UiKey.ENTER;
            case Keyboard.KEY_BACK:
                return UiKey.BACKSPACE;
            case Keyboard.KEY_DELETE:
                return UiKey.DELETE;
            case Keyboard.KEY_TAB:
                return UiKey.TAB;
            case Keyboard.KEY_LEFT:
                return UiKey.LEFT;
            case Keyboard.KEY_RIGHT:
                return UiKey.RIGHT;
            case Keyboard.KEY_UP:
                return UiKey.UP;
            case Keyboard.KEY_DOWN:
                return UiKey.DOWN;
            default:
                return UiKey.OTHER;
        }
    }

    /** Raw window x (pixels from the left) to a scaled GUI x, as Screen.handleMouse computes it. */
    static int guiX(int rawX, int guiWidth, int displayWidth) {
        return rawX * guiWidth / displayWidth;
    }

    /** Raw window y (LWJGL counts from the bottom) to a scaled GUI y from the top, as Screen.handleMouse computes it. */
    static int guiY(int rawY, int guiHeight, int displayHeight) {
        return guiHeight - rawY * guiHeight / displayHeight - 1;
    }

    /**
     * The GUI scale factor exactly as the constructor of {@code net.minecraft.client.util.Window} computes it, without
     * allocating a Window: the largest factor up to the option (0 = auto) that keeps at least 320 x 240 GUI pixels,
     * made even (unless 1) when the unicode font is forced.
     */
    static int guiScale(int width, int height, int guiScaleOption, boolean unicodeFont) {
        int max = guiScaleOption == 0 ? 1000 : guiScaleOption;
        int scale = 1;
        while (scale < max && width / (scale + 1) >= 320 && height / (scale + 1) >= 240) {
            scale++;
        }
        if (unicodeFont && scale % 2 != 0 && scale != 1) {
            scale--;
        }
        return scale;
    }

    /** Window.getWidth()/getHeight(): the window size divided by the scale, rounded up. */
    static int guiSize(int size, int scale) {
        return (size + scale - 1) / scale;
    }

    /**
     * Turns LWJGL wheel deltas ({@code Mouse.getEventDWheel()}, 120 per notch on Windows, smaller steps on some
     * touchpads) into whole notches, positive when the wheel is rolled away from the user (the sign LWJGL delivers).
     * One instance per open screen.
     */
    static final class ScrollNotches {
        static final int WHEEL_DELTA = 120;
        /** At most this many notches per event; the rest of a garbage delta is dropped. */
        static final int MAX_NOTCHES = 100;
        private long remainder;

        /** Adds one wheel event and returns the signed number of whole notches it completes (often 0). */
        int add(int delta) {
            remainder += delta;
            long notches = remainder / WHEEL_DELTA;
            remainder -= notches * WHEEL_DELTA;
            if (notches > MAX_NOTCHES || notches < -MAX_NOTCHES) {
                remainder = 0;
                return notches > 0 ? MAX_NOTCHES : -MAX_NOTCHES;
            }
            return (int) notches;
        }

        /** Forgets a partial notch, for example when the screen closes. */
        void reset() {
            remainder = 0;
        }
    }
}
