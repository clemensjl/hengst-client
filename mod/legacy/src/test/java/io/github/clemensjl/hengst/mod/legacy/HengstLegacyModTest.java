package io.github.clemensjl.hengst.mod.legacy;

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
import io.github.clemensjl.hengst.mod.common.ui.Widget;
import java.lang.management.ManagementFactory;
import java.lang.reflect.Field;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.EnumSet;
import java.util.List;
import net.minecraft.client.option.KeyBinding;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/** What belongs to the adapter itself; start and failure handling of the guard are tested in common. */
class HengstLegacyModTest {
    @AfterEach
    void reset() {
        HengstLegacyMod.ENABLED = false;
    }

    @Test
    void aRuntimeExceptionInAHookKeepsTheModRunning() {
        HengstLegacyMod.ENABLED = true;
        // No core in the unit test: the hook bodies fail with a NullPointerException, which the guard absorbs.
        assertDoesNotThrow(HengstLegacyMod::onFrame);
        assertDoesNotThrow(HengstLegacyMod::onFrame);
        assertTrue(HengstLegacyMod.ENABLED);
    }

    @Test
    void hooksNeverThrowIntoTheGame() {
        // No core (start failed or not started): the hook bodies catch what the missing core causes.
        assertDoesNotThrow(HengstLegacyMod::onFrame);
        assertDoesNotThrow(() -> HengstLegacyMod.onMouseButton(0, true));
    }

    @Test
    void stopWritesPendingChangesEvenAfterAHookFailureSwitchedTheModOff(@TempDir Path dir) throws Exception {
        UiScreen[] opened = new UiScreen[1];
        HengstCore core = HengstLegacyMod.GUARD.start(() -> workingPlatform(dir, opened), TimeSource.SYSTEM);
        core.onOpenSettingsKey();
        opened[0].render(CANVAS, 0, 0, 0f);
        int[] fps = featureSwitch(opened[0], "fps");
        opened[0].mouseClicked(fps[0], fps[1], 0);                 // the FPS switch: a change waiting to be saved
        Path profile = dir.resolve("hengst/profiles/default.json");
        assertFalse(Files.exists(profile));
        HengstLegacyMod.ENABLED = true;
        assertTrue(HengstLegacyMod.GUARD.hookFailed(HengstLegacyMod.SITE_HUD, new NoSuchMethodError("DrawableHelper.fill")));
        HengstLegacyMod.ENABLED = false;                                      // what the hook does on true
        HengstLegacyMod.stopCore(core);
        assertTrue(Files.exists(profile));
    }

    @Test
    void stopWithoutACoreDoesNothing() {
        assertDoesNotThrow(() -> HengstLegacyMod.stopCore(null));
    }

    @Test
    void aLinkageErrorInAHookDisablesTheMod() {
        HengstLegacyMod.ENABLED = true;
        HengstLegacyMod.onHookFailure(HengstLegacyMod.SITE_FRAME, new NoSuchMethodError("Minecraft.level"));
        assertFalse(HengstLegacyMod.ENABLED);
    }

    @Test
    void aRuntimeExceptionInAHookDoesNotDisableTheMod() {
        HengstLegacyMod.ENABLED = true;
        HengstLegacyMod.onHookFailure(HengstLegacyMod.SITE_FRAME, new IllegalStateException("test"));
        assertTrue(HengstLegacyMod.ENABLED);
    }

    @Test
    void theSettingsKeyDoesNothingWhileAnotherScreenIsOpenAndWorksAfterARelease(@TempDir Path dir) {
        UiScreen[] opened = new UiScreen[1];
        HengstCore core = HengstCore.start(workingPlatform(dir, opened), TimeSource.SYSTEM);
        KeyBinding[] maps = mappings(core);
        opened[0] = OTHER_SCREEN;                                  // a game screen such as chat is open
        press(maps[0]);
        LegacyKeys.dispatch(core, maps);
        assertSame(OTHER_SCREEN, opened[0]);
        release(maps[0]);
        LegacyKeys.dispatch(core, maps);
        opened[0] = null;                                          // the screen closed
        LegacyKeys.dispatch(core, maps);
        assertNull(opened[0], "the press with the screen open does not fire later");
        press(maps[0]);
        LegacyKeys.dispatch(core, maps);
        assertNotNull(opened[0]);
        assertNotSame(OTHER_SCREEN, opened[0]);
    }

    @Test
    void dispatchReportsChangesOnlyAndAHeldKeyOnce(@TempDir Path dir) {
        KeyFeature f = new KeyFeature();
        HengstCore core = startWithKeys(dir, new UiScreen[1], f);
        KeyBinding[] maps = mappings(core);
        KeyBinding hold = maps[maps.length - 2];
        press(hold);
        LegacyKeys.dispatch(core, maps);
        LegacyKeys.dispatch(core, maps);                           // still down, no click: nothing
        KeyBinding.onKeyPressed(hold.getCode());                   // a second press while held
        LegacyKeys.dispatch(core, maps);
        assertEquals(Arrays.asList("0 down"), f.events);
        release(hold);
        LegacyKeys.dispatch(core, maps);
        LegacyKeys.dispatch(core, maps);
        assertEquals(Arrays.asList("0 down", "0 up"), f.events);
    }

    @Test
    void aTapWithinOneTickIsAPressAndARelease(@TempDir Path dir) {
        KeyFeature f = new KeyFeature();
        HengstCore core = startWithKeys(dir, new UiScreen[1], f);
        KeyBinding[] maps = mappings(core);
        KeyBinding hold = maps[maps.length - 2];
        press(hold);
        release(hold);
        LegacyKeys.dispatch(core, maps);
        assertEquals(Arrays.asList("0 down", "0 up"), f.events);
    }

    @Test
    void aPressKeyWithAScreenOpenDoesNothing(@TempDir Path dir) {
        KeyFeature f = new KeyFeature();
        UiScreen[] opened = new UiScreen[1];
        HengstCore core = startWithKeys(dir, opened, f);
        KeyBinding[] maps = mappings(core);
        core.onOpenSettingsKey();                                  // a screen is open (as with chat)
        assertNotNull(opened[0]);
        press(maps[maps.length - 1]);
        LegacyKeys.dispatch(core, maps);
        assertTrue(f.events.isEmpty());
    }

    @Test
    void losingTheWindowFocusReleasesAHeldHoldKey(@TempDir Path dir) {
        KeyFeature f = new KeyFeature();
        HengstCore core = startWithKeys(dir, new UiScreen[1], f);
        KeyBinding[] maps = mappings(core);
        KeyBinding hold = maps[maps.length - 2];
        LegacyKeys.focus(core, maps, true);
        press(hold);
        LegacyKeys.dispatch(core, maps);
        LegacyKeys.focus(core, maps, false);
        assertEquals(Arrays.asList("0 down", "0 up"), f.events);
        LegacyKeys.focus(core, maps, false);
        LegacyKeys.dispatch(core, maps);                           // the mapping still reads down: no new press
        LegacyKeys.focus(core, maps, true);
        LegacyKeys.dispatch(core, maps);
        assertEquals(Arrays.asList("0 down", "0 up"), f.events);
        release(hold);
        LegacyKeys.dispatch(core, maps);
        press(hold);
        LegacyKeys.dispatch(core, maps);
        assertEquals(Arrays.asList("0 down", "0 up", "0 down"), f.events);
    }

    @Test
    void aFocusLossReleaseComesAfterAPressOfTheSameTick(@TempDir Path dir) {
        KeyFeature f = new KeyFeature();
        HengstCore core = startWithKeys(dir, new UiScreen[1], f);
        FakeKey[] maps = fakes(core);
        FakeKey hold = maps[maps.length - 2];
        LegacyKeys.tick(core, maps, true);
        hold.clicks = 1;                                           // pressed in the tick the window loses focus
        hold.down = true;
        LegacyKeys.tick(core, maps, false);
        assertEquals(Arrays.asList("0 down", "0 up"), f.events);
        LegacyKeys.tick(core, maps, false);                            // still reads down: the forced release is final
        LegacyKeys.tick(core, maps, true);
        assertEquals(Arrays.asList("0 down", "0 up"), f.events);
    }

    @Test
    void aThrowingFeatureInTheKeyCatalogLeavesTheModInert() {
        KeyBinding[] before = LegacyKeys.bindings();
        boolean[] platformCreated = new boolean[1];
        HengstCore core = assertDoesNotThrow(() -> HengstLegacyMod.start(() -> new ThrowingFeature().keys(), LegacyKeys::code,
                () -> {
                    platformCreated[0] = true;
                    return null;
                }));
        assertNull(core);
        assertFalse(platformCreated[0], "the core is not started");
        assertSame(before, LegacyKeys.bindings(), "no binding array assigned");
    }

    @Test
    void anUnmappedDefaultKeyLeavesTheModInert() {
        KeyBinding[] before = LegacyKeys.bindings();
        boolean[] platformCreated = new boolean[1];
        List<KeyBinding> registered = new ArrayList<>();
        HengstCore core = assertDoesNotThrow(() -> HengstLegacyMod.start(() -> FeatureCatalog.keys(GameVersion.V1_8_9), key -> {
            throw new IllegalArgumentException("no key code for " + key);
        }, m -> {
            registered.add(m);
            return m;
        }, () -> {
            platformCreated[0] = true;
            return null;
        }));
        assertNull(core);
        assertFalse(platformCreated[0], "the core is not started");
        assertTrue(registered.isEmpty(), "every key code is computed before the first registration");
        assertSame(before, LegacyKeys.bindings(), "no binding array assigned");
    }

    @Test
    void aHelperThatThrowsAfterTheFirstBindingAssignsNoBindingsAndLeavesTheModInert() {
        KeyBinding[] before = LegacyKeys.bindings();
        boolean[] platformCreated = new boolean[1];
        List<KeyBinding> registered = new ArrayList<>();
        HengstCore core = assertDoesNotThrow(() -> HengstLegacyMod.start(() -> FeatureCatalog.keys(GameVersion.V1_8_9),
                LegacyKeys::code, m -> {
                    if (!registered.isEmpty()) {
                        throw new IllegalArgumentException("Attempted to register two key bindings with equal ID: "
                                + m.getTranslationKey());
                    }
                    registered.add(m);
                    return m;
                }, () -> {
                    platformCreated[0] = true;
                    return null;
                }));
        assertNull(core);
        assertEquals(1, registered.size(), "the first binding went through the helper");
        assertFalse(platformCreated[0], "the core is not started");
        assertSame(before, LegacyKeys.bindings(), "no binding array assigned after a partial registration");
    }

    @Test
    void a_hold_key_held_through_a_focus_loss_allocates_nothing(@TempDir Path dir) {
        KeyFeature f = new KeyFeature(true);
        HengstCore core = startWithKeys(dir, new UiScreen[1], f);
        FakeKey[] maps = fakes(core);
        FakeKey hold = maps[maps.length - 2];
        for (int i = 0; i < 2000; i++) {
            heldThroughFocusLoss(core, maps, hold, f);             // warm up
        }
        f.downs = 0;
        f.ups = 0;
        int releasedByFocus = 0;
        com.sun.management.ThreadMXBean mx = (com.sun.management.ThreadMXBean) ManagementFactory.getThreadMXBean();
        long before = mx.getThreadAllocatedBytes(Thread.currentThread().getId());
        for (int i = 0; i < 1000; i++) {
            releasedByFocus += heldThroughFocusLoss(core, maps, hold, f);
        }
        long allocated = mx.getThreadAllocatedBytes(Thread.currentThread().getId()) - before;
        assertEquals(1000, f.downs, "a press of the held HOLD key per cycle");
        assertEquals(1000, releasedByFocus, "its release, sent by the focus loss while the key still reads pressed");
        assertEquals(1000, f.ups, "no second release when the key then reads up");
        assertTrue(allocated < 2048, "allocated " + allocated + " bytes");
    }

    /**
     * The HOLD key pressed and held, the focus lost while it still reads pressed, then the key released; returns the
     * releases the focus-loss tick delivered.
     */
    private static int heldThroughFocusLoss(HengstCore core, FakeKey[] maps, FakeKey hold, KeyFeature f) {
        hold.clicks = 1;
        hold.down = true;
        LegacyKeys.tick(core, maps, true);
        int ups = f.ups;
        LegacyKeys.tick(core, maps, false);
        int released = f.ups - ups;
        hold.down = false;
        LegacyKeys.tick(core, maps, true);
        return released;
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
        long before = mx.getThreadAllocatedBytes(Thread.currentThread().getId());
        for (int i = 0; i < 1000; i++) {
            tapAndTick(core, maps, i);
        }
        long allocated = mx.getThreadAllocatedBytes(Thread.currentThread().getId()) - before;
        assertEquals(2000, f.downs, "a press of the HOLD and the PRESS key every tick");
        assertEquals(1000, f.ups, "a release of the HOLD key every tick");
        assertTrue(allocated < 2048, "allocated " + allocated + " bytes");
    }

    /** A click on the HOLD and the PRESS key, released in the same tick; the focus is lost every second tick. */
    private static void tapAndTick(HengstCore core, FakeKey[] maps, int i) {
        maps[maps.length - 2].clicks = 1;
        maps[maps.length - 1].clicks = 1;
        LegacyKeys.tick(core, maps, (i & 1) == 0);
    }

    @Test
    void key_dispatch_allocates_nothing(@TempDir Path dir) {
        HengstCore core = HengstCore.start(workingPlatform(dir, new UiScreen[1]), TimeSource.SYSTEM);
        KeyBinding[] maps = mappings(core);
        for (int i = 0; i < 2000; i++) {
            LegacyKeys.dispatch(core, maps);                       // warm up
        }
        com.sun.management.ThreadMXBean mx = (com.sun.management.ThreadMXBean) ManagementFactory.getThreadMXBean();
        long before = mx.getThreadAllocatedBytes(Thread.currentThread().getId());
        for (int i = 0; i < 2000; i++) {
            LegacyKeys.dispatch(core, maps);
        }
        long allocated = mx.getThreadAllocatedBytes(Thread.currentThread().getId()) - before;
        assertTrue(allocated < 2048, "allocated " + allocated + " bytes");   // an iterator per call: over 60 KB
    }

    /**
     * A point on the switch of a feature's row in the open settings screen, from the list's own placement after a
     * render (rows, offset and indicator gutter as drawn), not from literals: the list may overflow and scroll.
     */
    private static int[] featureSwitch(UiScreen safe, String id) throws Exception {
        Object settings = field(safe, "screen");
        Object list = field(settings, "featureList");
        for (Object row : (List<?>) field(list, "widgets")) {
            Field feature;
            try {
                feature = row.getClass().getDeclaredField("feature");
            } catch (NoSuchFieldException groupHeading) {
                continue;
            }
            feature.setAccessible(true);
            if (id.equals(((Feature) feature.get(row)).id())) {
                Widget w = (Widget) row;
                int top = (Integer) field(list, "y");
                assertTrue(w.y >= top && w.y + w.h <= top + (Integer) field(list, "h"), id + " row drawn");
                return new int[] {w.x + 2, w.y + w.h / 2};
            }
        }
        throw new AssertionError("no row for " + id);
    }

    private static Object field(Object owner, String name) throws Exception {
        Field f = owner.getClass().getDeclaredField(name);
        f.setAccessible(true);
        return f.get(owner);
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
    private static final class FakeKey extends KeyBinding {
        int clicks;
        boolean down;

        FakeKey(String name, int code) {
            super(name, code, LegacyKeys.CATEGORY);
        }

        @Override
        public boolean wasPressed() {
            if (clicks == 0) {
                return false;
            }
            clicks--;
            return true;
        }

        @Override
        public boolean isPressed() {
            return down;
        }
    }

    private static FakeKey[] fakes(HengstCore core) {
        FakeKey[] maps = new FakeKey[core.keySpecs().size()];
        for (int i = 0; i < maps.length; i++) {
            maps[i] = new FakeKey(core.keySpecs().get(i).translationKey(), 200 + 50 + i);
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

    /** One binding per key of the core, each on its own key code, in the mod's category. */
    private static KeyBinding[] mappings(HengstCore core) {
        KeyBinding[] maps = new KeyBinding[core.keySpecs().size()];
        for (int i = 0; i < maps.length; i++) {
            maps[i] = new KeyBinding(core.keySpecs().get(i).translationKey(), 200 + i, LegacyKeys.CATEGORY);
        }
        return maps;
    }

    /** What the tick's keyboard loop does for a press without a screen. */
    private static void press(KeyBinding m) {
        KeyBinding.setKeyPressed(m.getCode(), true);
        KeyBinding.onKeyPressed(m.getCode());
    }

    private static void release(KeyBinding m) {
        KeyBinding.setKeyPressed(m.getCode(), false);
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
