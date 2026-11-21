package io.github.clemensjl.hengst.mod.modern;

import com.mojang.blaze3d.platform.InputConstants;
import io.github.clemensjl.hengst.mod.common.platform.UiKey;

/**
 * Pure translations from the game's input codes to the core's conventions. The InputConstants values are
 * compile-time constants of the node's game version (SDL3 scancodes and 1-based buttons on 26.x, GLFW codes and
 * 0-based buttons on 1.21.11), so this class compiles to the right numbers on both nodes without a version switch.
 */
final class ModernInput {
    /** Returned by {@link #button} for buttons other than left, right and middle. */
    static final int OTHER_BUTTON = -1;

    private ModernInput() {
    }

    /** The game's mouse button code to the core's 0 left, 1 right, 2 middle; {@link #OTHER_BUTTON} for the rest. */
    static int button(int raw) {
        switch (raw) {
            case InputConstants.MOUSE_BUTTON_LEFT:
                return 0;
            case InputConstants.MOUSE_BUTTON_RIGHT:
                return 1;
            case InputConstants.MOUSE_BUTTON_MIDDLE:
                return 2;
            default:
                return OTHER_BUTTON;
        }
    }

    /** The game's key code (KeyEvent.key()) to the keys the mod's screens understand. */
    static UiKey uiKey(int key) {
        switch (key) {
            case InputConstants.KEY_ESCAPE:
                return UiKey.ESCAPE;
            case InputConstants.KEY_RETURN:
            case InputConstants.KEY_NUMPADENTER:
                return UiKey.ENTER;
            case InputConstants.KEY_BACKSPACE:
                return UiKey.BACKSPACE;
            case InputConstants.KEY_DELETE:
                return UiKey.DELETE;
            case InputConstants.KEY_TAB:
                return UiKey.TAB;
            case InputConstants.KEY_LEFT:
                return UiKey.LEFT;
            case InputConstants.KEY_RIGHT:
                return UiKey.RIGHT;
            case InputConstants.KEY_UP:
                return UiKey.UP;
            case InputConstants.KEY_DOWN:
                return UiKey.DOWN;
            default:
                return UiKey.OTHER;
        }
    }

    /** A scaled GUI position as the integer pixel it lies in (floor, so -0.5 is -1, not 0). */
    static int guiPixel(double position) {
        return (int) Math.floor(position);
    }
}
