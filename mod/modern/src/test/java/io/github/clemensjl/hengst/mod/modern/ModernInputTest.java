package io.github.clemensjl.hengst.mod.modern;

import static org.junit.jupiter.api.Assertions.assertEquals;

import com.mojang.blaze3d.platform.InputConstants;
import io.github.clemensjl.hengst.mod.common.platform.UiKey;
import org.junit.jupiter.api.Test;

class ModernInputTest {
    @Test
    void mapsTheThreeMainButtonsToCoreNumbers() {
        assertEquals(0, ModernInput.button(InputConstants.MOUSE_BUTTON_LEFT));
        assertEquals(1, ModernInput.button(InputConstants.MOUSE_BUTTON_RIGHT));
        assertEquals(2, ModernInput.button(InputConstants.MOUSE_BUTTON_MIDDLE));
        assertEquals(ModernInput.OTHER_BUTTON, ModernInput.button(InputConstants.MOUSE_BUTTON_4));
        // Not MOUSE_BUTTON_8: 1.21.11 defines it as 0, the same code as the left button.
        assertEquals(ModernInput.OTHER_BUTTON, ModernInput.button(InputConstants.MOUSE_BUTTON_5));
    }

    //? if >=26.1 {
    @Test
    void sdlButtonsAreOneBasedWithRightAsThree() {
        // SDL3: SDL_BUTTON_LEFT 1, SDL_BUTTON_MIDDLE 2, SDL_BUTTON_RIGHT 3 (MouseHandler.onButton in 26.3).
        assertEquals(0, ModernInput.button(1));
        assertEquals(2, ModernInput.button(2));
        assertEquals(1, ModernInput.button(3));
        assertEquals(ModernInput.OTHER_BUTTON, ModernInput.button(0));
    }

    @Test
    void sdlScancodesOfTheUiKeys() {
        assertEquals(UiKey.ESCAPE, ModernInput.uiKey(41));
        assertEquals(UiKey.ENTER, ModernInput.uiKey(40));
        assertEquals(UiKey.ENTER, ModernInput.uiKey(88));
        assertEquals(UiKey.BACKSPACE, ModernInput.uiKey(42));
        assertEquals(UiKey.DELETE, ModernInput.uiKey(76));
        assertEquals(UiKey.TAB, ModernInput.uiKey(43));
        assertEquals(UiKey.RIGHT, ModernInput.uiKey(79));
        assertEquals(UiKey.LEFT, ModernInput.uiKey(80));
        assertEquals(UiKey.DOWN, ModernInput.uiKey(81));
        assertEquals(UiKey.UP, ModernInput.uiKey(82));
    }
    //?}

    @Test
    void mapsEveryUiKey() {
        assertEquals(UiKey.ESCAPE, ModernInput.uiKey(InputConstants.KEY_ESCAPE));
        assertEquals(UiKey.ENTER, ModernInput.uiKey(InputConstants.KEY_RETURN));
        assertEquals(UiKey.ENTER, ModernInput.uiKey(InputConstants.KEY_NUMPADENTER));
        assertEquals(UiKey.BACKSPACE, ModernInput.uiKey(InputConstants.KEY_BACKSPACE));
        assertEquals(UiKey.DELETE, ModernInput.uiKey(InputConstants.KEY_DELETE));
        assertEquals(UiKey.TAB, ModernInput.uiKey(InputConstants.KEY_TAB));
        assertEquals(UiKey.LEFT, ModernInput.uiKey(InputConstants.KEY_LEFT));
        assertEquals(UiKey.RIGHT, ModernInput.uiKey(InputConstants.KEY_RIGHT));
        assertEquals(UiKey.UP, ModernInput.uiKey(InputConstants.KEY_UP));
        assertEquals(UiKey.DOWN, ModernInput.uiKey(InputConstants.KEY_DOWN));
    }

    @Test
    void otherKeysAreOther() {
        assertEquals(UiKey.OTHER, ModernInput.uiKey(InputConstants.KEY_A));
        assertEquals(UiKey.OTHER, ModernInput.uiKey(InputConstants.KEY_RSHIFT));
        assertEquals(UiKey.OTHER, ModernInput.uiKey(InputConstants.KEY_SPACE));
    }

    @Test
    void guiPixelFloorsTowardsNegativeInfinity() {
        assertEquals(10, ModernInput.guiPixel(10.0));
        assertEquals(10, ModernInput.guiPixel(10.99));
        assertEquals(0, ModernInput.guiPixel(0.4));
        assertEquals(-1, ModernInput.guiPixel(-0.5));
        assertEquals(-2, ModernInput.guiPixel(-1.01));
    }
}
