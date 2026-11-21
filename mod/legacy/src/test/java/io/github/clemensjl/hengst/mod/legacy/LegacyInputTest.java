package io.github.clemensjl.hengst.mod.legacy;

import static org.junit.jupiter.api.Assertions.assertEquals;

import io.github.clemensjl.hengst.mod.common.platform.UiKey;
import org.junit.jupiter.api.Test;
import org.lwjgl.input.Keyboard;

class LegacyInputTest {
    @Test
    void lwjglButtonsAreZeroBasedLeftRightMiddle() {
        assertEquals(0, LegacyInput.button(0));
        assertEquals(1, LegacyInput.button(1));
        assertEquals(2, LegacyInput.button(2));
        assertEquals(LegacyInput.OTHER_BUTTON, LegacyInput.button(3));
        // Mouse.getEventButton() is -1 for move and wheel events.
        assertEquals(LegacyInput.OTHER_BUTTON, LegacyInput.button(-1));
    }

    @Test
    void keyBindingCodesOfMouseButtonsAreTheButtonMinus100() {
        // GameOptions: attackKey -100, useKey -99, pickItemKey -98; MinecraftClient.tick passes button - 100.
        assertEquals(0, LegacyInput.buttonOfKeyCode(-100));
        assertEquals(1, LegacyInput.buttonOfKeyCode(-99));
        assertEquals(2, LegacyInput.buttonOfKeyCode(-98));
        // A move or wheel event (button -1) arrives as -101.
        assertEquals(LegacyInput.OTHER_BUTTON, LegacyInput.buttonOfKeyCode(-101));
        assertEquals(LegacyInput.OTHER_BUTTON, LegacyInput.buttonOfKeyCode(-97));
    }

    @Test
    void keyboardCodesAreNotMouseButtons() {
        assertEquals(LegacyInput.OTHER_BUTTON, LegacyInput.buttonOfKeyCode(0));
        assertEquals(LegacyInput.OTHER_BUTTON, LegacyInput.buttonOfKeyCode(Keyboard.KEY_W));
        assertEquals(LegacyInput.OTHER_BUTTON, LegacyInput.buttonOfKeyCode(256 + 'a'));
    }

    @Test
    void mapsEveryUiKey() {
        assertEquals(UiKey.ESCAPE, LegacyInput.uiKey(Keyboard.KEY_ESCAPE));
        assertEquals(UiKey.ENTER, LegacyInput.uiKey(Keyboard.KEY_RETURN));
        assertEquals(UiKey.ENTER, LegacyInput.uiKey(Keyboard.KEY_NUMPADENTER));
        assertEquals(UiKey.BACKSPACE, LegacyInput.uiKey(Keyboard.KEY_BACK));
        assertEquals(UiKey.DELETE, LegacyInput.uiKey(Keyboard.KEY_DELETE));
        assertEquals(UiKey.TAB, LegacyInput.uiKey(Keyboard.KEY_TAB));
        assertEquals(UiKey.LEFT, LegacyInput.uiKey(Keyboard.KEY_LEFT));
        assertEquals(UiKey.RIGHT, LegacyInput.uiKey(Keyboard.KEY_RIGHT));
        assertEquals(UiKey.UP, LegacyInput.uiKey(Keyboard.KEY_UP));
        assertEquals(UiKey.DOWN, LegacyInput.uiKey(Keyboard.KEY_DOWN));
    }

    @Test
    void lwjglScancodesOfTheUiKeys() {
        // LWJGL 2 uses DirectInput scancodes.
        assertEquals(UiKey.ESCAPE, LegacyInput.uiKey(1));
        assertEquals(UiKey.ENTER, LegacyInput.uiKey(28));
        assertEquals(UiKey.ENTER, LegacyInput.uiKey(156));
        assertEquals(UiKey.BACKSPACE, LegacyInput.uiKey(14));
        assertEquals(UiKey.DELETE, LegacyInput.uiKey(211));
        assertEquals(UiKey.TAB, LegacyInput.uiKey(15));
        assertEquals(UiKey.LEFT, LegacyInput.uiKey(203));
        assertEquals(UiKey.RIGHT, LegacyInput.uiKey(205));
        assertEquals(UiKey.UP, LegacyInput.uiKey(200));
        assertEquals(UiKey.DOWN, LegacyInput.uiKey(208));
    }

    @Test
    void otherKeysAreOther() {
        assertEquals(UiKey.OTHER, LegacyInput.uiKey(Keyboard.KEY_A));
        assertEquals(UiKey.OTHER, LegacyInput.uiKey(Keyboard.KEY_NONE));
        assertEquals(UiKey.OTHER, LegacyInput.uiKey(Keyboard.KEY_RSHIFT));
    }

    @Test
    void rawXScalesLikeTheVanillaScreen() {
        // Screen.handleMouse: x = Mouse.getEventX() * width / client.width (gui 427 on an 854 px window, scale 2).
        assertEquals(0, LegacyInput.guiX(0, 427, 854));
        assertEquals(0, LegacyInput.guiX(1, 427, 854));
        assertEquals(1, LegacyInput.guiX(2, 427, 854));
        assertEquals(426, LegacyInput.guiX(853, 427, 854));
        // Odd window width: gui width is ceil(855 / 2) = 428.
        assertEquals(427, LegacyInput.guiX(854, 428, 855));
    }

    @Test
    void rawYIsFlippedToTopLeftLikeTheVanillaScreen() {
        // Screen.handleMouse: y = height - Mouse.getEventY() * height / client.height - 1; LWJGL's y starts at the
        // bottom of the window.
        assertEquals(239, LegacyInput.guiY(0, 240, 480));
        assertEquals(239, LegacyInput.guiY(1, 240, 480));
        assertEquals(238, LegacyInput.guiY(2, 240, 480));
        assertEquals(0, LegacyInput.guiY(479, 240, 480));
        assertEquals(120, LegacyInput.guiY(238, 240, 480));
    }

    @Test
    void guiScaleFollowsTheVanillaWindowRules() {
        // Window: the largest factor up to the option (0 = auto) that keeps at least 320 x 240 GUI pixels.
        assertEquals(2, LegacyInput.guiScale(854, 480, 0, false));
        assertEquals(1, LegacyInput.guiScale(854, 480, 1, false));
        assertEquals(2, LegacyInput.guiScale(854, 480, 3, false));
        assertEquals(4, LegacyInput.guiScale(1920, 1080, 0, false));
        assertEquals(3, LegacyInput.guiScale(1920, 1080, 3, false));
        assertEquals(1, LegacyInput.guiScale(300, 200, 0, false));
        // The unicode font forces an even factor (except 1).
        assertEquals(4, LegacyInput.guiScale(1920, 1080, 0, true));
        assertEquals(2, LegacyInput.guiScale(1920, 1080, 3, true));
        assertEquals(1, LegacyInput.guiScale(854, 480, 1, true));
    }

    @Test
    void guiSizeIsRoundedUpLikeTheVanillaWindow() {
        assertEquals(427, LegacyInput.guiSize(854, 2));
        assertEquals(428, LegacyInput.guiSize(855, 2));
        assertEquals(360, LegacyInput.guiSize(1080, 3));
        assertEquals(361, LegacyInput.guiSize(1081, 3));
        assertEquals(854, LegacyInput.guiSize(854, 1));
    }
}
