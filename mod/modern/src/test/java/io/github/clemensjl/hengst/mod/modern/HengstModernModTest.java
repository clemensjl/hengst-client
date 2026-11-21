package io.github.clemensjl.hengst.mod.modern;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNotSame;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.FeatureCatalog;
import io.github.clemensjl.hengst.mod.common.feature.DefaultKey;
import io.github.clemensjl.hengst.mod.common.feature.Feature;
import io.github.clemensjl.hengst.mod.common.feature.FeatureGroup;
import io.github.clemensjl.hengst.mod.common.feature.KeySpec;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.platform.GameView;
import io.github.clemensjl.hengst.mod.common.platform.InputView;
import io.github.clemensjl.hengst.mod.common.platform.Platform;
import io.github.clemensjl.hengst.mod.common.platform.TimeSource;
import io.github.clemensjl.hengst.mod.common.platform.UiKey;
import io.github.clemensjl.hengst.mod.common.platform.UiScreen;
import io.github.clemensjl.hengst.mod.common.HengstCore;
import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import java.lang.management.ManagementFactory;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.EnumSet;
import java.util.List;
import net.minecraft.client.KeyMapping;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/** What belongs to the adapter itself; start and failure handling of the guard are tested in common. */
class HengstModernModTest {
    @AfterEach
    void reset() {
        HengstModernMod.ENABLED = false;
    }

    @Test
    void aRuntimeExceptionInAHookKeepsTheModRunning() {
        HengstModernMod.ENABLED = true;
        // No core in the unit test: the hook bodies fail with a NullPointerException, which the guard absorbs.
        assertDoesNotThrow(HengstModernMod::onFrame);
        assertDoesNotThrow(HengstModernMod::onFrame);
        assertTrue(HengstModernMod.ENABLED);
    }

    @Test
    void hooksNeverThrowIntoTheGame() {
        // No core (start failed or not started): the hook bodies catch what the missing core causes.
        assertDoesNotThrow(HengstModernMod::onFrame);
        assertDoesNotThrow(() -> HengstModernMod.onMouseButton(0, true));
    }

    @Test
    void stopWritesPendingChangesEvenAfterAHookFailureSwitchedTheModOff(@TempDir Path dir) throws Exception {
        UiScreen[] opened = new UiScreen[1];
        HengstCore core = HengstModernMod.GUARD.start(() -> workingPlatform(dir, opened), TimeSource.SYSTEM);
        core.onOpenSettingsKey();
        opened[0].render(CANVAS, 0, 0, 0f);
        opened[0].mouseClicked(12, 66, 0);                        // the FPS switch: a change waiting to be saved
        Path profile = dir.resolve("hengst/profiles/default.json");
        assertFalse(Files.exists(profile));
        HengstModernMod.ENABLED = true;
        assertTrue(HengstModernMod.GUARD.hookFailed(HengstModernMod.SITE_HUD, new NoSuchMethodError("GuiGraphics.fill")));
        HengstModernMod.ENABLED = false;                                      // what the hook does on true
        HengstModernMod.stopCore(core);
        assertTrue(Files.exists(profile));
    }

    @Test
    void stopWithoutACoreDoesNothing() {
        assertDoesNotThrow(() -> HengstModernMod.stopCore(null));
    }

    @Test
    void aLinkageErrorInAHookDisablesTheMod() {
        HengstModernMod.ENABLED = true;
        HengstModernMod.onHookFailure(HengstModernMod.SITE_FRAME, new NoSuchMethodError("Minecraft.level"));
        assertFalse(HengstModernMod.ENABLED);
    }

    @Test
    void aRuntimeExceptionInAHookDoesNotDisableTheMod() {
        HengstModernMod.ENABLED = true;
        HengstModernMod.onHookFailure(HengstModernMod.SITE_FRAME, new IllegalStateException("test"));
        assertTrue(HengstModernMod.ENABLED);
    }

    @Test
    void theSettingsKeyDoesNothingWhileAnotherScreenIsOpenAndWorksAfterARelease(@TempDir Path dir) {
        UiScreen[] opened = new UiScreen[1];
        HengstCore core = HengstCore.start(workingPlatform(dir, opened), TimeSource.SYSTEM);
        KeyMapping[] maps = mappings(core);
        opened[0] = OTHER_SCREEN;                                  // a game screen such as chat is open
        press(maps[0]);
        ModernKeys.dispatch(core, maps);
        assertSame(OTHER_SCREEN, opened[0]);
        release(maps[0]);
        ModernKeys.dispatch(core, maps);
        opened[0] = null;                                          // the screen closed
        ModernKeys.dispatch(core, maps);
        assertNull(opened[0], "the press with the screen open does not fire later");
        press(maps[0]);
        ModernKeys.dispatch(core, maps);
        assertNotNull(opened[0]);
        assertNotSame(OTHER_SCREEN, opened[0]);
    }

    @Test
    void dispatchReportsChangesOnlyAndAHeldKeyOnce(@TempDir Path dir) {
        KeyFeature f = new KeyFeature();
        HengstCore core = startWithKeys(dir, new UiScreen[1], f);
        KeyMapping[] maps = mappings(core);
        KeyMapping hold = maps[maps.length - 2];
        press(hold);
        ModernKeys.dispatch(core, maps);
        ModernKeys.dispatch(core, maps);                           // still down, no click: nothing
        KeyMapping.click(hold.getDefaultKey());                    // a key repeat while held
        ModernKeys.dispatch(core, maps);
        assertEquals(Arrays.asList("0 down"), f.events);
        release(hold);
        ModernKeys.dispatch(core, maps);
        ModernKeys.dispatch(core, maps);
        assertEquals(Arrays.asList("0 down", "0 up"), f.events);
    }

    @Test
    void aTapWithinOneTickIsAPressAndARelease(@TempDir Path dir) {
        KeyFeature f = new KeyFeature();
        HengstCore core = startWithKeys(dir, new UiScreen[1], f);
        KeyMapping[] maps = mappings(core);
        KeyMapping hold = maps[maps.length - 2];
        press(hold);
        release(hold);
        ModernKeys.dispatch(core, maps);
        assertEquals(Arrays.asList("0 down", "0 up"), f.events);
    }

    @Test
    void aPressKeyWithAScreenOpenDoesNothing(@TempDir Path dir) {
        KeyFeature f = new KeyFeature();
        UiScreen[] opened = new UiScreen[1];
        HengstCore core = startWithKeys(dir, opened, f);
        KeyMapping[] maps = mappings(core);
        core.onOpenSettingsKey();                                  // a screen is open (as with chat)
        assertNotNull(opened[0]);
        press(maps[maps.length - 1]);
        ModernKeys.dispatch(core, maps);
        assertTrue(f.events.isEmpty());
    }

    @Test
    void losingTheWindowFocusReleasesAHeldHoldKey(@TempDir Path dir) {
        KeyFeature f = new KeyFeature();
        HengstCore core = startWithKeys(dir, new UiScreen[1], f);
        KeyMapping[] maps = mappings(core);
        KeyMapping hold = maps[maps.length - 2];
        ModernKeys.focus(core, maps, true);
        press(hold);
        ModernKeys.dispatch(core, maps);
        ModernKeys.focus(core, maps, false);
        assertEquals(Arrays.asList("0 down", "0 up"), f.events);
        ModernKeys.focus(core, maps, false);
        ModernKeys.dispatch(core, maps);                           // the mapping still reads down: no new press
        ModernKeys.focus(core, maps, true);
        ModernKeys.dispatch(core, maps);
        assertEquals(Arrays.asList("0 down", "0 up"), f.events);
        release(hold);
        ModernKeys.dispatch(core, maps);
        press(hold);
        ModernKeys.dispatch(core, maps);
        assertEquals(Arrays.asList("0 down", "0 up", "0 down"), f.events);
    }

    @Test
    void aFocusLossReleaseComesAfterAPressOfTheSameTick(@TempDir Path dir) {
        KeyFeature f = new KeyFeature();
        HengstCore core = startWithKeys(dir, new UiScreen[1], f);
        FakeKey[] maps = fakes(core);
        FakeKey hold = maps[maps.length - 2];
        ModernKeys.tick(core, maps, true);
        hold.clicks = 1;                                           // pressed in the tick the window loses focus
        hold.down = true;
        ModernKeys.tick(core, maps, false);
        assertEquals(Arrays.asList("0 down", "0 up"), f.events);
        ModernKeys.tick(core, maps, false);                            // still reads down: the forced release is final
        ModernKeys.tick(core, maps, true);
        assertEquals(Arrays.asList("0 down", "0 up"), f.events);
    }

    @Test
    void aThrowingFeatureInTheKeyCatalogLeavesTheModInert() {
        boolean[] platformCreated = new boolean[1];
        HengstCore core = assertDoesNotThrow(() -> HengstModernMod.start(() -> new ThrowingFeature().keys(), ModernKeys::code,
                () -> {
                    platformCreated[0] = true;
                    return null;
                }));
        assertNull(core);
        assertFalse(platformCreated[0], "the core is not started");
        assertFalse(HengstModernMod.ENABLED);
    }

    @Test
    void anUnmappedDefaultKeyLeavesTheModInert() {
        boolean[] platformCreated = new boolean[1];
        HengstCore core = assertDoesNotThrow(() -> HengstModernMod.start(() -> FeatureCatalog.keys(ModernPlatform.gameVersion()), key -> {
            throw new IllegalArgumentException("no key code for " + key);
        }, () -> {
            platformCreated[0] = true;
            return null;
        }));
        assertNull(core);
        assertFalse(platformCreated[0], "the core is not started");
        assertFalse(HengstModernMod.ENABLED);
    }

    @Test
    void the_press_deliver_and_focus_paths_allocate_nothing(@TempDir Path dir) {
        KeyFeature f = new KeyFeature(true);
        HengstCore core = startWithKeys(dir, new UiScreen[1], f);
        FakeKey[] maps = fakes(core);
        for (int i = 0; i < 2000; i++) {
            tapAndTick(core, maps, i);                             // warm up
        }
        f.downs = 0;
        f.ups = 0;
        com.sun.management.ThreadMXBean mx = (com.sun.management.ThreadMXBean) ManagementFactory.getThreadMXBean();
        long before = mx.getCurrentThreadAllocatedBytes();
        for (int i = 0; i < 1000; i++) {
            tapAndTick(core, maps, i);
        }
        long allocated = mx.getCurrentThreadAllocatedBytes() - before;
        assertEquals(2000, f.downs, "a press of the HOLD and the PRESS key every tick");
        assertEquals(1000, f.ups, "a release of the HOLD key every tick");
        assertTrue(allocated < 2048, "allocated " + allocated + " bytes");
    }

    /** A click on the HOLD and the PRESS key, released in the same tick; the focus is lost every second tick. */
    private static void tapAndTick(HengstCore core, FakeKey[] maps, int i) {
        maps[maps.length - 2].clicks = 1;
        maps[maps.length - 1].clicks = 1;
        ModernKeys.tick(core, maps, (i & 1) == 0);
    }

    @Test
    void key_dispatch_allocates_nothing(@TempDir Path dir) {
        HengstCore core = HengstCore.start(workingPlatform(dir, new UiScreen[1]), TimeSource.SYSTEM);
        KeyMapping[] maps = mappings(core);
        for (int i = 0; i < 2000; i++) {
            ModernKeys.dispatch(core, maps);                       // warm up
        }
        com.sun.management.ThreadMXBean mx = (com.sun.management.ThreadMXBean) ManagementFactory.getThreadMXBean();
        long before = mx.getCurrentThreadAllocatedBytes();
        for (int i = 0; i < 2000; i++) {
            ModernKeys.dispatch(core, maps);
        }
        long allocated = mx.getCurrentThreadAllocatedBytes() - before;
        assertTrue(allocated < 2048, "allocated " + allocated + " bytes");   // an iterator per call: over 60 KB
    }

    /** A feature with one HOLD and one PRESS key: the last two indexes of the core's key list. */
    private static final class KeyFeature extends Feature {
        final List<String> events = new ArrayList<>();
        final boolean counting;
        int downs;
        int ups;

        KeyFeature() {
            this(false);
        }

        /** {@code counting}: count instead of recording, for the allocation test. */
        KeyFeature(boolean counting) {
            super("keys", 47, FeatureGroup.HUD, EnumSet.allOf(GameVersion.class));
            this.counting = counting;
        }

        @Override
        public List<KeySpec> keys() {
            return Arrays.asList(new KeySpec("test.hold", DefaultKey.NONE, KeySpec.Mode.HOLD),
                    new KeySpec("test.press", DefaultKey.NONE, KeySpec.Mode.PRESS));
        }

        @Override
        public void onKey(int key, boolean down, long nowNanos) {
            if (counting) {
                if (down) {
                    downs++;
                } else {
                    ups++;
                }
                return;
            }
            events.add(key + (down ? " down" : " up"));
        }
    }

    /** A feature whose constructor fails, as a broken feature in the catalog would. */
    private static final class ThrowingFeature extends Feature {
        ThrowingFeature() {
            super("throwing", 48, FeatureGroup.HUD, EnumSet.allOf(GameVersion.class));
            throw new IllegalStateException("feature constructor failed");
        }
    }

    /** A key binding whose clicks and down state the test sets directly; its reads allocate nothing. */
    private static final class FakeKey extends KeyMapping {
        int clicks;
        boolean down;

        FakeKey(String name, int code) {
            super(name, code, ModernKeys.CATEGORY);
        }

        @Override
        public boolean consumeClick() {
            if (clicks == 0) {
                return false;
            }
            clicks--;
            return true;
        }

        @Override
        public boolean isDown() {
            return down;
        }
    }

    private static FakeKey[] fakes(HengstCore core) {
        FakeKey[] maps = new FakeKey[core.keySpecs().size()];
        for (int i = 0; i < maps.length; i++) {
            maps[i] = new FakeKey(core.keySpecs().get(i).translationKey(), 3000 + 50 + i);
        }
        return maps;
    }

    /** Another screen of the game (chat, inventory): the core sees a screen that is not its own. */
    private static final UiScreen OTHER_SCREEN = new UiScreen() {
        @Override public void render(Canvas canvas, int mouseX, int mouseY, float partialTick) { }
        @Override public boolean mouseClicked(int mouseX, int mouseY, int button) { return false; }
        @Override public boolean mouseReleased(int mouseX, int mouseY, int button) { return false; }
        @Override public boolean mouseDragged(int mouseX, int mouseY, int button) { return false; }
        @Override public boolean mouseScrolled(int mouseX, int mouseY, double amount) { return false; }
        @Override public boolean keyPressed(UiKey key, boolean shift, boolean control) { return false; }
        @Override public void closed() { }
    };

    private static HengstCore startWithKeys(Path dir, UiScreen[] opened, KeyFeature f) {
        HengstCore core = HengstCore.start(workingPlatform(dir, opened), TimeSource.SYSTEM);
        core.activeProfile().module("keys").setEnabled(true);
        core.registerForTest(f);
        return core;
    }

    /** One mapping per key of the core, each on its own key code, in the mod's category. */
    private static KeyMapping[] mappings(HengstCore core) {
        KeyMapping[] maps = new KeyMapping[core.keySpecs().size()];
        for (int i = 0; i < maps.length; i++) {
            maps[i] = new KeyMapping(core.keySpecs().get(i).translationKey(), 3000 + i, ModernKeys.CATEGORY);
        }
        return maps;
    }

    /** What the keyboard handler does for a press without a screen. */
    private static void press(KeyMapping m) {
        m.setDown(true);
        KeyMapping.click(m.getDefaultKey());
    }

    private static void release(KeyMapping m) {
        m.setDown(false);
    }

    private static final Canvas CANVAS = new Canvas() {
        @Override public int width() { return 320; }
        @Override public int height() { return 240; }
        @Override public int fontHeight() { return 9; }
        @Override public int textWidth(String text) { return 6 * text.length(); }
        @Override public void fill(int x1, int y1, int x2, int y2, int argb) { }
        @Override public void text(String text, int x, int y, int argb, boolean shadow) { }
        @Override public void pushTransform(float translateX, float translateY, float scale) { }
        @Override public void popTransform() { }
    };

    /** A platform that works: a world, an open screen is kept in {@code opened[0]}, a temporary config directory. */
    private static Platform workingPlatform(Path dir, UiScreen[] opened) {
        GameView view = new GameView() {
            @Override public boolean inWorld() { return true; }
            @Override public double playerX() { return 0; }
            @Override public double playerY() { return 0; }
            @Override public double playerZ() { return 0; }
            @Override public float playerYaw() { return 0; }
            @Override public boolean hudHidden() { return false; }
            @Override public boolean debugOverlayOpen() { return false; }
            @Override public boolean screenOpen() { return opened[0] != null; }
            @Override public String serverAddress() { return null; }
            @Override public int pingMillis() { return -1; }
            @Override public Object worldIdentity() { return null; }
        };
        return new Platform() {
            @Override public GameVersion version() { return GameVersion.V1_8_9; }
            @Override public GameView game() { return view; }
            @Override public InputView input() { return null; }
            @Override public Path configDir() { return dir; }
            @Override public String languageCode() { return "en_us"; }
            @Override public void logWarning(String message) { }
            @Override public void openScreen(UiScreen screen) { opened[0] = screen; }
            @Override public void closeScreen() { opened[0] = null; }
        };
    }
}
