package io.github.clemensjl.hengst.mod.legacy;

import io.github.clemensjl.hengst.mod.common.HengstCore;
import io.github.clemensjl.hengst.mod.common.config.Profile;
import io.github.clemensjl.hengst.mod.common.config.ProfileStore;
import io.github.clemensjl.hengst.mod.common.feature.Feature;
import io.github.clemensjl.hengst.mod.common.feature.FeatureGroup;
import io.github.clemensjl.hengst.mod.common.hud.HudModule;
import io.github.clemensjl.hengst.mod.common.layout.Anchor;
import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import io.github.clemensjl.hengst.mod.common.platform.GameKey;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.platform.Platform;
import io.github.clemensjl.hengst.mod.common.platform.UiKey;
import io.github.clemensjl.hengst.mod.common.platform.UiScreen;
import io.github.clemensjl.hengst.mod.common.ui.ScrollList;
import java.awt.image.BufferedImage;
import java.io.File;
import java.lang.reflect.Field;
import java.nio.ByteBuffer;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Clock;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.EnumSet;
import java.util.List;
import java.util.concurrent.Callable;
import java.util.concurrent.TimeUnit;
import java.util.function.BooleanSupplier;
import javax.imageio.ImageIO;
import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.loader.api.FabricLoader;
import net.legacyfabric.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.ChatScreen;
import net.minecraft.client.gui.screen.GameMenuScreen;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.screen.TitleScreen;
import net.minecraft.client.gui.screen.ingame.SurvivalInventoryScreen;
import net.minecraft.client.util.ScreenshotUtils;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.text.LiteralText;
import net.minecraft.world.level.LevelGeneratorType;
import net.minecraft.world.level.LevelInfo;
import org.apache.logging.log4j.LogManager;
import org.apache.logging.log4j.Logger;
import org.lwjgl.input.Keyboard;
import org.lwjgl.input.Mouse;
import org.lwjgl.opengl.GL11;

/**
 * The mod in a real singleplayer world on 1.8.9, driven by a test thread. 1.8.9 has no client gametest API, so the
 * harness does what Fabric's gametest does on modern versions, by hand: it runs every game access on the client
 * thread ({@code MinecraftClient.execute}), creates a flat world through {@code startIntegratedServer}, and gives
 * input by writing events into LWJGL 2's own mouse and keyboard queues, so the game's real input paths
 * ({@code MinecraftClient.tick}, {@code Screen.handleInput}) and the mod's mixins handle them. Asserts through the
 * mod's objects and on the pixels of framebuffer screenshots taken with the game's {@code ScreenshotUtils}: a
 * module's rectangle, taken from the layout the core reports, must show the mod's text where the HUD is expected and
 * match the F1 baseline where it is not. A failed check logs FAIL and exits the game with code 1.
 *
 * <p>Two phases ({@code -Dhengst.gametest.phase}): {@code full} runs every check with a fresh run directory and ends
 * by dragging the FPS module to the bottom right in the HUD editor; {@code restart} runs with that directory kept
 * and checks that the saved layout is loaded and drawn.
 */
public final class LegacyGameTest implements ClientModInitializer {
    private static final Logger LOGGER = LogManager.getLogger("hengst-gametest");
    private static final String WORLD = "hengst-test";
    // Pixel thresholds, as in the modern test. Text is white (all channels >= TEXT_LEVEL).
    private static final int TEXT_LEVEL = 225;
    private static final int MIN_TEXT_PIXELS = 8;
    private static final int SAME_TOLERANCE = 24;
    private static final int PRESSED_FILL = 0xB0FFFFFF;
    // Black at alpha 25 (0.098): below the alpha test's 0.1 threshold the font renderer leaves enabled.
    private static final int FAINT_FILL = 0x19000000;
    private static final long CLIENT_TIMEOUT_S = 120;

    // LWJGL 2 event records (2.9.4+legacyfabric.15, Mouse.next / Keyboard.readNext).
    private static final int MOUSE_EVENT_SIZE = 22;
    private static final int KEY_EVENT_SIZE = 18;

    static volatile int ticks;
    public static volatile int frames;

    // Probes, written and read on the client thread.
    private static int seq;
    private static int hudSeq;
    private static int chatSeq;
    private static int screenSeq;
    private static int hudCalls;
    private static final List<int[]> MOUSE_LOG = new ArrayList<int[]>();

    @Override
    public void onInitializeClient() {
        final String phase = System.getProperty("hengst.gametest.phase");
        if (phase == null) {
            return;
        }
        ClientTickEvents.END_CLIENT_TICK.register(c -> ticks++);
        Thread test = new Thread(() -> {
            try {
                runTest("restart".equals(phase));
                LOGGER.info("[hengst-gametest] ALL PASSED ({} phase)", phase);
                onClient(() -> {
                    MinecraftClient.getInstance().scheduleStop();
                    return null;
                });
            } catch (Throwable e) {
                LOGGER.error("[hengst-gametest] FAIL " + e.getMessage(), e);
                System.exit(1);
            }
        }, "hengst-gametest");
        test.setDaemon(true);
        test.start();
        Thread watchdog = new Thread(() -> {
            try {
                Thread.sleep(TimeUnit.MINUTES.toMillis(8));
            } catch (InterruptedException e) {
                return;
            }
            LOGGER.error("[hengst-gametest] FAIL watchdog: the test did not finish within 8 minutes");
            System.exit(2);
        }, "hengst-gametest-watchdog");
        watchdog.setDaemon(true);
        watchdog.start();
    }

    // ---- probes (client thread) ----

    public static void stampHud() {
        hudSeq = ++seq;
        hudCalls++;
        depthAtHud = GL11.glIsEnabled(GL11.GL_DEPTH_TEST);
    }

    // GL state around the mod's HUD draw and the faint-fill probe (client thread).
    private static boolean depthAtHud;
    private static boolean depthAfterHud;
    private static boolean alphaTestBeforeFaint;
    private static int[] faintRect;
    private static final LegacyCanvas FAINT_CANVAS = new LegacyCanvas();

    /** At the end of the mod's HUD hook: records the depth state and, when asked, draws one faint fill. */
    public static void afterHud() {
        depthAfterHud = GL11.glIsEnabled(GL11.GL_DEPTH_TEST);
        int[] r = faintRect;
        if (r != null) {
            MinecraftClient mc = MinecraftClient.getInstance();
            int s = LegacyInput.guiScale(mc.width, mc.height, mc.options.guiScale, mc.forcesUnicodeFont());
            FAINT_CANVAS.begin(mc.textRenderer, LegacyInput.guiSize(mc.width, s), LegacyInput.guiSize(mc.height, s));
            alphaTestBeforeFaint = GL11.glIsEnabled(GL11.GL_ALPHA_TEST);
            FAINT_CANVAS.fill(r[0], r[1], r[2], r[3], FAINT_FILL);
        }
    }

    public static void stampChat() {
        chatSeq = ++seq;
    }

    public static void recordMouse(int lwjglButton, boolean pressed) {
        MinecraftClient mc = MinecraftClient.getInstance();
        MOUSE_LOG.add(new int[] {lwjglButton, pressed ? 1 : 0, mc.currentScreen != null ? 1 : 0});
    }

    // ---- the test ----

    private static void runTest(boolean restart) throws Exception {
        waitUntil(() -> MinecraftClient.getInstance().currentScreen instanceof TitleScreen, 120_000, "title screen");
        waitFrames(5);
        shot("hengst-00-title");
        check(onClient(() -> !Mouse.isGrabbed()), "mouse not grabbed (harness keeps the user's cursor free)");
        pass("title screen reached");

        onClient(() -> {
            LevelInfo info = new LevelInfo(0L, LevelInfo.GameMode.SURVIVAL, false, false, LevelGeneratorType.FLAT)
                    .enableCommands();
            MinecraftClient.getInstance().startIntegratedServer(WORLD, WORLD, info);
            return null;
        });
        waitUntil(() -> {
            MinecraftClient mc = MinecraftClient.getInstance();
            return mc.world != null && mc.player != null && mc.currentScreen == null;
        }, 60_000, "in the world");
        waitTicks(20);
        onClient(() -> {
            MinecraftClient mc = MinecraftClient.getInstance();
            mc.player.sendChatMessage("/time set 6000");
            mc.player.sendChatMessage("/gamerule doDaylightCycle false");
            // The "Press E to open your inventory" hint would cover the top right (the settings' buttons).
            mc.player.sendChatMessage("/achievement give achievement.openInventory");
            return null;
        });
        waitTicks(40);
        onClient(() -> {
            MinecraftClient mc = MinecraftClient.getInstance();
            mc.inGameHud.getChatHud().clear();
            mc.notification.reset();
            return null;
        });
        waitUntil(() -> hudCalls > 0, 10_000, "HUD hook drawn");
        settle();
        pass("in a flat world, HUD hook called");
        if (restart) {
            restartPhase();
        } else {
            fullPhase();
        }
    }

    private static void fullPhase() throws Exception {
        final HengstCore core = HengstLegacyMod.core();
        final Platform platform = core.context().platform();
        check(HengstLegacyMod.ENABLED, "mod enabled");
        check(onClient(() -> core.renderer().activeCount()) == 4, "four active modules");

        Layout layout = layout();
        check(layout.draws > 0, "the core draws the HUD in a world");
        BufferedImage hud = shot("hengst-01-hud");

        key(Keyboard.KEY_F1, (char) 0);
        settle();
        check(onClient(() -> platform.game().hudHidden()), "F1 hides the HUD");
        check(layout().draws == 0, "the core draws nothing under F1");
        BufferedImage hidden = shot("hengst-02-f1");
        for (String id : new String[] {"fps", "cps", "coordinates", "keystrokes"}) {
            checkShown(hud, hidden, layout, id, "HUD vs F1 baseline");
        }
        checkSame(hud, hidden, layout.control(), "sky away from any HUD, HUD vs F1 baseline");
        checkFramesAdvance(core, "under F1");
        key(Keyboard.KEY_F1, (char) 0);
        settle();
        check(!onClient(() -> platform.game().hudHidden()), "F1 again shows the HUD");
        pass("four modules drawn by the game (pixels), F1 hides them, frames counted under F1");

        waitUntil(() -> core.context().frames().fps() > 0, 5_000, "frame counter reports FPS");
        int callsBefore = onClient(() -> hudCalls);
        int framesBefore = frames;
        waitRealTime(500);
        int calls = onClient(() -> hudCalls);
        int framesNow = frames;
        check(calls > callsBefore + 10, "HUD hook every frame: " + callsBefore + " -> " + calls);
        check(calls - callsBefore <= framesNow - framesBefore + 2, "HUD hook at most once per frame");
        pass("frame counter advances (" + onClient(() -> core.context().frames().fps()) + " FPS), HUD hook "
                + (calls - callsBefore) + " calls in " + (framesNow - framesBefore) + " frames");

        vanillaHudUnchanged(core, layout, "hengst-03");
        pass("vanilla hotbar, hearts and chat line identical with and without the mod HUD (GL state intact)");

        waitUntil(() -> cps(0) == 0 && cps(1) == 0, 3_000, "CPS idle before clicking");
        onClient(() -> {
            MOUSE_LOG.clear();
            return null;
        });
        for (int i = 0; i < 5; i++) {
            click(0);
        }
        for (int i = 0; i < 3; i++) {
            click(1);
        }
        waitUntil(() -> cps(0) == 5 && cps(1) == 3, 1_000, "CPS reads 5 left and 3 right");
        List<int[]> log = onClient(() -> new ArrayList<int[]>(MOUSE_LOG));
        check(count(log, 0, 1, 0) == 5 && count(log, 0, 0, 0) == 5 && count(log, 1, 1, 0) == 3
                && count(log, 1, 0, 0) == 3, "every press and release reached the mod without a screen: " + dump(log));
        shot("hengst-04-cps-5-3");
        waitUntil(() -> cps(0) == 0 && cps(1) == 0, 3_000, "CPS back to 0 after the one-second window");
        shot("hengst-05-cps-0");
        pass("CPS counts 5 left and 3 right separately and falls to 0");

        onClient(() -> {
            MinecraftClient.getInstance().setScreen(new ChatScreen());
            MOUSE_LOG.clear();
            return null;
        });
        settle();
        for (int i = 0; i < 3; i++) {
            click(0);
        }
        waitTicks(3);
        log = onClient(() -> new ArrayList<int[]>(MOUSE_LOG));
        int cpsInChat = onClient(() -> cps(0));
        check(cpsInChat == 0, "clicks with chat open do not count, got " + cpsInChat);
        check(count(log, 0, 1, 1) == 3 && count(log, 0, 0, 1) == 3,
                "presses and releases with chat open reached the mod (screen path): " + dump(log));
        onClient(() -> {
            MinecraftClient.getInstance().setScreen(null);
            return null;
        });
        settle();
        pass("clicks with chat open reach the mod but do not raise CPS");

        Layout held = null;
        BufferedImage released = shot("hengst-06-w-released");
        keyDown(Keyboard.KEY_W, 'w', true);
        waitTicks(10);
        check(onClient(() -> platform.input().isDown(GameKey.FORWARD)), "W held is down");
        held = layout();
        int[] wCell = held.fillWithColour(PRESSED_FILL);
        check(wCell != null, "keystrokes draws a pressed cell while W is held");
        BufferedImage wHeld = shot("hengst-07-w-held");
        double lumaHeld = meanLuma(wHeld, wCell);
        double lumaReleased = meanLuma(released, wCell);
        check(lumaHeld > lumaReleased + 40, "W cell lit in the frame: luma " + lumaReleased + " -> " + lumaHeld);
        keyDown(Keyboard.KEY_W, 'w', false);
        waitTicks(2);
        check(!onClient(() -> platform.input().isDown(GameKey.FORWARD)), "W released is up");
        pass("W held lights the key (pixels)");

        // The player walked: a fresh F1 baseline for the same view, once the player has stopped sliding (a baseline
        // taken while the camera still moved made the F3 comparison fail now and then on the grass texture).
        waitTicks(20);
        settle();
        hidden = f1Baseline("hengst-08-f1-baseline");

        registerDummyFeatures(core);
        key(Keyboard.KEY_RSHIFT, (char) 0);
        waitUntil(() -> MinecraftClient.getInstance().currentScreen instanceof LegacyScreenHost, 3_000,
                "settings key opens the mod's screen");
        settle();
        check(layout().draws == 0, "HUD suppressed while the settings are open");
        check(onClient(Keyboard::areRepeatEventsEnabled), "key repeats on while the host is open");
        BufferedImage settings = shot("hengst-09-settings");
        List<Text> texts = screenTexts();
        Text notice = null;
        for (Text t : texts) {
            if (t.text.contains("Not an official Minecraft product")) {
                notice = t;
            }
        }
        check(notice != null, "settings screen draws the notice, texts " + texts);
        int grey = greyTextPixels(settings, notice.rect());
        check(grey >= MIN_TEXT_PIXELS, "notice in the settings screenshot: grey text pixels " + grey);
        checkFramesAdvance(core, "with the settings screen open");
        settingsWheel();
        key(Keyboard.KEY_ESCAPE, (char) 0);
        waitUntil(() -> MinecraftClient.getInstance().currentScreen == null, 3_000, "Escape closes the settings");
        settle();
        check(!onClient(Keyboard::areRepeatEventsEnabled), "key repeats restored after the host closed");
        check(layout().draws > 0, "HUD unsuppressed after Escape");
        BufferedImage afterEscape = shot("hengst-12-after-escape");
        checkShown(afterEscape, hidden, layout, "fps", "after Escape");
        pass("settings key opens the settings with the notice, Escape closes them, the HUD is back");
        vanillaHudUnchanged(core, layout, "hengst-13");
        pass("after the settings: vanilla hotbar, hearts and chat line identical with and without the mod HUD");

        key(Keyboard.KEY_F3, (char) 0);
        settle();
        check(onClient(() -> platform.game().debugOverlayOpen()), "F3 open");
        check(layout().draws == 0, "the core draws nothing with F3 open");
        BufferedImage f3 = shot("hengst-14-f3");
        int[] keysLow = layout.lowerPart("keystrokes");
        checkHidden(f3, hidden, keysLow, "keystrokes with F3 open vs F1 baseline");
        key(Keyboard.KEY_F3, (char) 0);
        settle();
        check(!onClient(() -> platform.game().debugOverlayOpen()), "F3 closed");
        BufferedImage f3Closed = shot("hengst-15-f3-closed");
        checkText(f3Closed, keysLow, "keystrokes after F3 closed");
        checkShown(f3Closed, hidden, layout, "fps", "after F3 closed");
        pass("F3 hides the mod HUD (pixels) and it returns when F3 closes");

        onClient(() -> {
            MinecraftClient.getInstance().inGameHud.getChatHud()
                    .addMessage(new LiteralText("Hengst in-game test: this chat line draws over the mod HUD"));
            MinecraftClient.getInstance().setScreen(new OrderChatScreen());
            return null;
        });
        settle();
        checkDrawOrder("chat open");
        shot("hengst-16-chat");
        onClient(() -> {
            MinecraftClient.getInstance().setScreen(null);
            return null;
        });
        settle();
        pass("with chat open the mod HUD is drawn before the chat lines and the screen");

        onClient(() -> {
            MinecraftClient mc = MinecraftClient.getInstance();
            mc.setScreen(new OrderInventoryScreen(mc.player));
            return null;
        });
        settle();
        checkDrawOrder("inventory open");
        shot("hengst-17-inventory");
        onClient(() -> {
            MinecraftClient.getInstance().setScreen(null);
            return null;
        });
        settle();
        pass("with the inventory open the mod HUD is drawn before the chat lines and the screen with its dim");

        key(Keyboard.KEY_ESCAPE, (char) 0);
        waitUntil(() -> MinecraftClient.getInstance().currentScreen instanceof GameMenuScreen, 3_000, "pause menu");
        settle();
        shot("hengst-18-pause-menu");
        onClient(() -> {
            MinecraftClient.getInstance().setScreen(null);
            return null;
        });
        settle();
        pass("pause menu screenshot taken");

        probeScreen(platform);
        inputAfterClose(platform);
        faintFill(layout);

        dragFpsToBottomRight(core, layout);
        moduleOverHotbarItem(core);
    }

    /**
     * A fill with alpha 25 drawn after the mod's text, where the font renderer has left the alpha test on with its
     * 0.1 threshold: it must still darken the frame (translucent backgrounds are honoured).
     */
    private static void faintFill(Layout layout) throws Exception {
        final int[] r = layout.control();
        BufferedImage plain = shot("hengst-30-faint-fill-off");
        onClient(() -> {
            faintRect = r;
            return null;
        });
        settle();
        BufferedImage faint = shot("hengst-31-faint-fill-on");
        onClient(() -> {
            faintRect = null;
            return null;
        });
        check(onClient(() -> alphaTestBeforeFaint), "alpha test on before the faint fill, as the font renderer leaves it");
        double before = meanLuma(plain, r);
        double after = meanLuma(faint, r);
        check(after < before - 5, "a fill with alpha 25 darkens the frame: luma " + before + " -> " + after);
        pass("fill with alpha 25 drawn although the alpha test was on (luma " + Math.round(before) + " -> "
                + Math.round(after) + ")");
    }

    /**
     * A module dragged over a filled hotbar slot: the item icon is drawn earlier in the HUD at a positive z and writes
     * depth, the mod draws at z 0 afterwards; its text must stay visible over the item.
     */
    private static void moduleOverHotbarItem(final HengstCore core) throws Exception {
        onClient(() -> {
            // One item: a stack count would draw white digits into the checked region itself.
            MinecraftClient.getInstance().player.sendChatMessage("/give @p minecraft:obsidian 1");
            return null;
        });
        waitTicks(20);
        onClient(() -> {
            MinecraftClient.getInstance().inGameHud.getChatHud().clear();
            return null;
        });
        settle();
        check(onClient(() -> MinecraftClient.getInstance().player.inventory.main[0] != null), "hotbar slot 1 filled");
        Layout layout = layout();
        int w = layout.w;
        int h = layout.h;
        // Item icon of hotbar slot 1 (InGameHud.renderHotbar: x = w / 2 - 90 + 2, y = h - 16 - 3, 16 x 16).
        int[] item = {w / 2 - 88, h - 19, w / 2 - 72, h - 3};
        int[] cps = layout.rect("cps");
        onClient(() -> {
            core.onOpenHudEditorKey();
            return null;
        });
        settle();
        int x = cps[0] + 2;
        int y = cps[1] + 2;
        int tx = item[0] + 2;
        int ty = item[1] + 2;
        mouse(0, true, x, y, 0);
        waitTicks(1);
        for (int i = 1; i <= 5; i++) {
            mouse(-1, false, x + (tx - x) * i / 5, y + (ty - y) * i / 5, 0);
            waitTicks(1);
        }
        mouse(0, false, tx, ty, 0);
        waitTicks(2);
        key(Keyboard.KEY_ESCAPE, (char) 0);
        waitUntil(() -> MinecraftClient.getInstance().currentScreen == null, 3_000, "Escape closes the editor");
        settle();
        int[] moved = layout().rect("cps");
        int[] overlap = {Math.max(moved[0], item[0]), Math.max(moved[1], item[1]), Math.min(moved[2], item[2]),
                Math.min(moved[3], item[3])};
        check(overlap[2] - overlap[0] >= 8 && overlap[3] - overlap[1] >= 6, "CPS module over the hotbar item, module "
                + Arrays.toString(moved) + " item " + Arrays.toString(item));
        BufferedImage img = shot("hengst-32-cps-over-hotbar-item");
        checkText(img, overlap, "CPS text over the filled hotbar slot");
        check(onClient(() -> depthAtHud && depthAfterHud), "depth test on at the HUD hook and again after the mod drew");
        pass("a module over a filled hotbar slot stays visible (pixels); depth state restored after the mod HUD");
    }

    /**
     * Screen.handleInput keeps draining both LWJGL queues on the same screen object after a handler closed it: events
     * queued in the same tick after the closing one must not reach the closed core screen, while the raw-click hook
     * still sees the physical buttons.
     */
    private static void inputAfterClose(final Platform platform) throws Exception {
        final ProbeUi keysProbe = new ProbeUi(platform);
        keysProbe.closeOnEscape = true;
        onClient(() -> {
            platform.openScreen(keysProbe);
            return null;
        });
        settle();
        onClient(() -> {
            queueKey(Keyboard.KEY_ESCAPE, (char) 0, true);
            queueKey(Keyboard.KEY_ESCAPE, (char) 0, false);
            queueKey(Keyboard.KEY_A, 'a', true);
            queueKey(Keyboard.KEY_A, 'a', false);
            return null;
        });
        waitTicks(3);
        List<UiKey> keys = onClient(() -> new ArrayList<UiKey>(keysProbe.keys));
        check(keys.equals(Arrays.asList(UiKey.ESCAPE)), "keys after Escape closed the screen in the same tick, got "
                + keys);
        check(onClient(() -> MinecraftClient.getInstance().currentScreen == null), "Escape closed the probe screen");

        final ProbeUi clickProbe = new ProbeUi(platform);
        clickProbe.closeOnClick = true;
        onClient(() -> {
            platform.openScreen(clickProbe);
            return null;
        });
        settle();
        final int[] c = onClient(() -> {
            Screen s = MinecraftClient.getInstance().currentScreen;
            return new int[] {s.width / 2, s.height / 2};
        });
        onClient(() -> {
            MOUSE_LOG.clear();
            queueMouse(0, true, c[0], c[1], 0);
            queueMouse(0, false, c[0], c[1], 0);
            queueMouse(-1, false, c[0], c[1], 120);
            queueKey(Keyboard.KEY_A, 'a', true);
            queueKey(Keyboard.KEY_A, 'a', false);
            return null;
        });
        waitTicks(3);
        List<int[]> clicks = onClient(() -> new ArrayList<int[]>(clickProbe.clicks));
        List<Double> scrolls = onClient(() -> new ArrayList<Double>(clickProbe.scrolls));
        List<UiKey> clickKeys = onClient(() -> new ArrayList<UiKey>(clickProbe.keys));
        check(clicks.size() == 1 && clicks.get(0)[3] == 1, "only the closing press reached the screen, got "
                + dump(clicks));
        check(scrolls.isEmpty() && clickKeys.isEmpty(), "no wheel or key after the close, got " + scrolls + " "
                + clickKeys);
        List<int[]> log = onClient(() -> new ArrayList<int[]>(MOUSE_LOG));
        check(count(log, 0, 1, 1) == 1 && count(log, 0, 0, 0) + count(log, 0, 0, 1) == 1,
                "the raw-click hook saw the press and the release: " + dump(log));
        settle();
        pass("input queued after the closing event of a tick does not reach the closed screen; raw clicks still seen");
    }

    /** Wheel over the settings' feature list: one notch moves it by one row (20 px, SettingsScreen.ROW). */
    private static void settingsWheel() throws Exception {
        final ScrollList list = onClient(() -> {
            UiScreen settings = unwrap(((LegacyScreenHost) MinecraftClient.getInstance().currentScreen).ui());
            return (ScrollList) field(settings, "featureList");
        });
        int[] bounds = onClient(() -> new int[] {(Integer) field(list, "x"), (Integer) field(list, "y"),
                (Integer) field(list, "w"), (Integer) field(list, "h")});
        int gx = bounds[0] + bounds[2] / 2;
        int gy = bounds[1] + bounds[3] / 2;
        int start = onClient(list::offset);
        check(start == 0, "feature list starts at the top, offset " + start);
        BufferedImage top = shot("hengst-10-settings-list-top");
        wheel(gx, gy, -120);
        waitTicks(2);
        int down = onClient(list::offset);
        check(down == 20, "one notch towards the user scrolls the feature list down by one row, offset " + down);
        settle();
        BufferedImage scrolled = shot("hengst-11-settings-list-scrolled");
        int[] listRect = {bounds[0], bounds[1], bounds[0] + bounds[2], bounds[1] + bounds[3]};
        check(diffFraction(top, scrolled, listRect) > 0.01, "the scrolled list is drawn differently");
        wheel(gx, gy, 60);
        waitTicks(2);
        check(onClient(list::offset) == 20, "half a notch does nothing yet");
        wheel(gx, gy, 60);
        waitTicks(2);
        int back = onClient(list::offset);
        check(back == 0, "two half notches away from the user scroll back up by one step, offset " + back);
        pass("wheel over the settings list: one row (20 px) per notch, half notches accumulate");
    }

    /** Scroll notches, key repeat and the removal rule on a probe screen hosted by the adapter. */
    private static void probeScreen(final Platform platform) throws Exception {
        final ProbeUi probe = new ProbeUi(platform);
        onClient(() -> {
            platform.openScreen(probe);
            return null;
        });
        check(onClient(() -> MinecraftClient.getInstance().currentScreen instanceof LegacyScreenHost),
                "openScreen takes effect synchronously");
        settle();
        int[] centre = onClient(() -> {
            Screen s = MinecraftClient.getInstance().currentScreen;
            return new int[] {s.width / 2, s.height / 2};
        });
        wheel(centre[0], centre[1], 120);
        wheel(centre[0], centre[1], -120);
        for (int i = 0; i < 4; i++) {
            wheel(centre[0], centre[1], 30);
        }
        wheel(centre[0], centre[1], 360);
        waitTicks(2);
        List<Double> scrolls = onClient(() -> new ArrayList<Double>(probe.scrolls));
        check(scrolls.equals(Arrays.asList(1.0, -1.0, 1.0, 1.0, 1.0, 1.0)), "scroll notches, got " + scrolls);
        List<int[]> at = onClient(() -> new ArrayList<int[]>(probe.scrollAt));
        check(at.get(0)[0] == centre[0] && at.get(0)[1] == centre[1],
                "scroll position in GUI pixels from the top left, got " + Arrays.toString(at.get(0)) + " for "
                        + Arrays.toString(centre));
        pass("wheel arrives as whole notches " + scrolls + " at the GUI position of the cursor");

        // A click and a release at a known GUI pixel: the host forwards vanilla's converted position.
        click(0, centre[0] / 2, centre[1] / 2);
        waitTicks(2);
        List<int[]> clicks = onClient(() -> new ArrayList<int[]>(probe.clicks));
        check(clicks.size() == 2 && clicks.get(0)[0] == centre[0] / 2 && clicks.get(0)[1] == centre[1] / 2
                && clicks.get(0)[3] == 1 && clicks.get(1)[3] == 0,
                "click and release forwarded at GUI " + (centre[0] / 2) + "," + (centre[1] / 2) + ", got " + dump(clicks));
        key(Keyboard.KEY_ESCAPE, (char) 0);
        waitTicks(2);
        check(onClient(() -> probe.keys.contains(UiKey.ESCAPE)), "Escape forwarded to the core's screen");
        check(onClient(() -> MinecraftClient.getInstance().currentScreen instanceof LegacyScreenHost),
                "the host itself does not close on Escape");
        pass("clicks, releases and Escape forwarded to the core's screen");

        onClient(() -> {
            MinecraftClient.getInstance().setScreen(new ChatScreen());
            return null;
        });
        int closed = onClient(() -> probe.closed);
        check(closed == 1, "replaced host screen: closed() once, got " + closed);
        check(!onClient(() -> probe.reentrant), "closeScreen() inside closed() changed the screen");
        onClient(() -> {
            platform.closeScreen();
            return null;
        });
        check(onClient(() -> MinecraftClient.getInstance().currentScreen instanceof ChatScreen),
                "closeScreen() left a vanilla screen alone");
        onClient(() -> {
            MinecraftClient.getInstance().setScreen(null);
            return null;
        });
        settle();
        pass("screen removal rule: one close, no re-entrant set-screen");
    }

    /** Last step of the full phase: the layout it saves is what the restart phase checks. */
    private static void dragFpsToBottomRight(final HengstCore core, Layout layout) throws Exception {
        int[] fps = layout.rect("fps");
        BufferedImage before = shot("hengst-19-before-editor");
        onClient(() -> {
            core.onOpenHudEditorKey();
            return null;
        });
        check(onClient(() -> MinecraftClient.getInstance().currentScreen instanceof LegacyScreenHost),
                "HUD editor open");
        settle();
        BufferedImage editor = shot("hengst-20-editor");
        double lumaBefore = meanLuma(before, layout.control());
        double lumaEditor = meanLuma(editor, layout.control());
        check(lumaEditor < 0.75 * lumaBefore, "editor dims the screen: luma " + lumaBefore + " -> " + lumaEditor);
        int[] centre = {layout.w * 3 / 10, layout.h * 2 / 5, layout.w * 7 / 10, layout.h * 3 / 5};
        checkText(editor, centre, "editor hint in the centre");
        pass("editor dims the screen and centres the hint (pixels)");

        int x = fps[0] + 3;
        int y = fps[1] + 3;
        int tx = layout.w - 2;
        int ty = layout.h - 2;
        mouse(0, true, x, y, 0);
        waitTicks(1);
        for (int i = 1; i <= 5; i++) {
            mouse(-1, false, x + (tx - x) * i / 5, y + (ty - y) * i / 5, 0);
            waitTicks(1);
        }
        mouse(0, false, tx, ty, 0);
        waitTicks(2);
        settle();
        shot("hengst-21-editor-dragged");
        key(Keyboard.KEY_ESCAPE, (char) 0);
        waitUntil(() -> MinecraftClient.getInstance().currentScreen == null, 3_000, "Escape closes the editor");
        settle();

        Path profiles = FabricLoader.getInstance().getConfigDir().resolve("hengst").resolve("profiles");
        check(Files.isRegularFile(profiles.resolve("default.json")), "profile file written: " + profiles);
        Profile saved = new ProfileStore(profiles, Clock.systemUTC(), msg -> LOGGER.warn(msg)).load(ProfileStore.DEFAULT);
        Anchor anchor = saved.module("fps").anchor();
        check(anchor == Anchor.BOTTOM_RIGHT, "saved FPS anchor, got " + anchor);
        checkFpsBottomRight("hengst-22-fps-bottom-right", "hengst-23-fps-bottom-right-f1");
        pass("HUD editor drag saved the FPS module at " + anchor + " and the game draws it there (pixels)");
    }

    private static void restartPhase() throws Exception {
        final HengstCore core = HengstLegacyMod.core();
        Anchor anchor = onClient(() -> core.activeProfile().module("fps").anchor());
        check(anchor == Anchor.BOTTOM_RIGHT, "restart: loaded FPS anchor (run runGameTest first), got " + anchor);
        checkFpsBottomRight("hengst-24-restart-fps-bottom-right", "hengst-25-restart-f1");
        pass("restart: the FPS module is drawn at the saved position (pixels)");
    }

    // ---- checks ----

    /**
     * Mod HUD suppressed vs drawn, a few frames apart: the vanilla HUD drawn before the mod (hotbar, hearts) and after
     * it (a chat line) must look the same. A GL state the canvas left behind (colour, texture, blend) would tint or
     * blank them, in this frame (chat) or the next (hotbar).
     */
    private static void vanillaHudUnchanged(final HengstCore core, Layout layout, String prefix) throws Exception {
        int w = layout.w;
        int h = layout.h;
        int[] hotbar = {w / 2 - 91, h - 22, w / 2 + 91, h};
        int[] hearts = {w / 2 - 91, h - 39, w / 2 - 10, h - 30};
        int keysRight = layout.rect("keystrokes")[2];
        // The newest chat line (unfocused chat): GUI y h - 38 to h - 29, the hearts row. So the hearts are compared
        // in a pair without a chat line and the chat line in a second pair: each check sees only its own element.
        int[] chat = {keysRight + 2, h - 38, 200, h - 29};
        // The hand is drawn by the world pass of the next frame: state the canvas leaked would show there too.
        int[] hand = {w / 2 + 100, h * 3 / 4, w - 50, h - 2};

        BufferedImage[] pair = suppressedPair(core, prefix + "-vanilla-only", prefix + "-with-mod");
        checkSame(pair[1], pair[0], hand, "hand (next frame's world render) with and without the mod HUD");
        checkSame(pair[1], pair[0], hotbar, "hotbar with and without the mod HUD");
        checkSame(pair[1], pair[0], hearts, "hearts with and without the mod HUD");

        onClient(() -> {
            MinecraftClient.getInstance().inGameHud.getChatHud()
                    .addMessage(new LiteralText("Hengst in-game test: a vanilla chat line next to the mod HUD"));
            return null;
        });
        pair = suppressedPair(core, prefix + "-chat-vanilla-only", prefix + "-chat-with-mod");
        checkText(pair[0], chat, "chat line drawn (vanilla only)");
        checkSame(pair[1], pair[0], chat, "chat line with and without the mod HUD");
        onClient(() -> {
            MinecraftClient.getInstance().inGameHud.getChatHud().clear();
            return null;
        });
    }

    /** Two frames, the mod HUD suppressed in the first and drawn in the second. */
    private static BufferedImage[] suppressedPair(final HengstCore core, String vanillaName, String withModName)
            throws Exception {
        onClient(() -> {
            core.renderer().setSuppressed(true);
            return null;
        });
        settle();
        BufferedImage vanilla = shot(vanillaName);
        onClient(() -> {
            core.renderer().setSuppressed(false);
            return null;
        });
        settle();
        return new BufferedImage[] {vanilla, shot(withModName)};
    }

    private static void checkFpsBottomRight(String shownName, String hiddenName) throws Exception {
        Layout layout = layout();
        int[] r = layout.rect("fps");
        check(r[0] > layout.w / 2 && r[1] > layout.h / 2, "fps laid out at the bottom right, rect "
                + Arrays.toString(r) + " on " + layout.w + "x" + layout.h);
        BufferedImage shown = shot(shownName);
        BufferedImage hidden = f1Baseline(hiddenName);
        checkShown(shown, hidden, layout, "fps", "FPS at the bottom right");
    }

    private static BufferedImage f1Baseline(String name) throws Exception {
        key(Keyboard.KEY_F1, (char) 0);
        settle();
        BufferedImage hidden = shot(name);
        key(Keyboard.KEY_F1, (char) 0);
        settle();
        check(!onClient(() -> HengstLegacyMod.core().context().platform().game().hudHidden()),
                "F1 baseline taken, HUD shown again");
        return hidden;
    }

    /** onFrame keeps feeding the FPS counter: after 2.5 s it would read 0 if no frame had been counted. */
    private static void checkFramesAdvance(final HengstCore core, String state) throws Exception {
        waitRealTime(2_500);
        int fps = onClient(() -> core.context().frames().fps());
        check(fps > 0, "frames counted " + state + ", FPS " + fps);
        pass("frames counted " + state + " (" + fps + " FPS)");
    }

    /** One frame's order: the mod HUD hook, then vanilla's chat lines, then the screen: consecutive stamps. */
    private static void checkDrawOrder(String where) throws Exception {
        int[] s = onClient(() -> new int[] {hudSeq, chatSeq, screenSeq});
        check(s[0] + 1 == s[1] && s[1] + 1 == s[2], where + ": order mod HUD, chat, screen expected, got "
                + Arrays.toString(s));
    }

    private static void checkShown(BufferedImage shown, BufferedImage hidden, Layout layout, String id, String what) {
        int[] r = layout.rect(id);
        checkText(shown, r, what + ": " + id);
        int baseline = textPixels(hidden, r);
        check(baseline == 0, what + ": " + id + " baseline has no text pixels, got " + baseline);
        double diff = diffFraction(shown, hidden, r);
        check(diff >= 0.2, what + ": " + id + " differs from the baseline, fraction " + diff);
        // Glyphs, not solid blocks: white covers about a third of the text cells when the font texture is used;
        // with texturing left off every glyph is drawn as a filled box.
        int ink = 0;
        int cells = 0;
        for (Text t : layout.texts) {
            if (t.x >= r[0] && t.y >= r[1] && t.x + t.w <= r[2] && t.y + t.h <= r[3]) {
                int[] cell = {t.x, t.y, t.x + t.w, t.y + t.h};
                ink += textPixels(shown, cell);
                cells += area(shown, cell);
            }
        }
        check(cells > 0, what + ": " + id + " has text");
        double ratio = ink / (double) cells;
        check(ratio < 0.45, what + ": " + id + " text drawn as glyphs, white fraction of the text cells " + ratio);
    }

    private static void checkHidden(BufferedImage img, BufferedImage hidden, int[] r, String what) {
        int text = textPixels(img, r);
        check(text == 0, what + ": no text pixels, got " + text);
        checkSame(img, hidden, r, what);
    }

    private static void checkSame(BufferedImage a, BufferedImage b, int[] r, String what) {
        double diff = diffFraction(a, b, r);
        check(diff <= 0.05, what + ": same as baseline, differing fraction " + diff);
    }

    private static void checkText(BufferedImage img, int[] r, String what) {
        int text = textPixels(img, r);
        int total = area(img, r);
        check(text >= MIN_TEXT_PIXELS, what + ": text pixels " + text);
        check(text < total * 0.7, what + ": region is not one flat bright colour (" + text + "/" + total + ")");
    }

    // ---- pixels ----

    // GUI width of the last layout; screenshots are scaled from it.
    private static int guiWidth = 1;

    private static int[] pixels(BufferedImage img, int[] r) {
        double s = img.getWidth() / (double) guiWidth;
        int x1 = Math.max(0, (int) Math.ceil(r[0] * s) + 1);
        int y1 = Math.max(0, (int) Math.ceil(r[1] * s) + 1);
        int x2 = Math.min(img.getWidth(), (int) Math.floor(r[2] * s) - 1);
        int y2 = Math.min(img.getHeight(), (int) Math.floor(r[3] * s) - 1);
        return new int[] {x1, y1, x2, y2};
    }

    private static int area(BufferedImage img, int[] r) {
        int[] p = pixels(img, r);
        return Math.max(0, p[2] - p[0]) * Math.max(0, p[3] - p[1]);
    }

    private static int textPixels(BufferedImage img, int[] r) {
        int[] p = pixels(img, r);
        int n = 0;
        for (int y = p[1]; y < p[3]; y++) {
            for (int x = p[0]; x < p[2]; x++) {
                int c = img.getRGB(x, y);
                if (((c >> 16) & 255) >= TEXT_LEVEL && ((c >> 8) & 255) >= TEXT_LEVEL && (c & 255) >= TEXT_LEVEL) {
                    n++;
                }
            }
        }
        return n;
    }

    /** Light grey pixels (all channels at least 140, nearly equal): the notice's text colour over the dimmed game. */
    private static int greyTextPixels(BufferedImage img, int[] r) {
        int[] p = pixels(img, r);
        int n = 0;
        for (int y = p[1]; y < p[3]; y++) {
            for (int x = p[0]; x < p[2]; x++) {
                int c = img.getRGB(x, y);
                int red = (c >> 16) & 255;
                int green = (c >> 8) & 255;
                int blue = c & 255;
                int max = Math.max(red, Math.max(green, blue));
                int min = Math.min(red, Math.min(green, blue));
                if (min >= 140 && max - min <= 20) {
                    n++;
                }
            }
        }
        return n;
    }

    private static double diffFraction(BufferedImage a, BufferedImage b, int[] r) {
        int[] p = pixels(a, r);
        int differing = 0;
        int total = 0;
        for (int y = p[1]; y < p[3]; y++) {
            for (int x = p[0]; x < p[2]; x++) {
                int c = a.getRGB(x, y);
                int d = b.getRGB(x, y);
                int m = Math.max(Math.abs(((c >> 16) & 255) - ((d >> 16) & 255)),
                        Math.max(Math.abs(((c >> 8) & 255) - ((d >> 8) & 255)), Math.abs((c & 255) - (d & 255))));
                if (m > SAME_TOLERANCE) {
                    differing++;
                }
                total++;
            }
        }
        check(total > 0, "empty pixel region " + Arrays.toString(r));
        return differing / (double) total;
    }

    private static double meanLuma(BufferedImage img, int[] r) {
        int[] p = pixels(img, r);
        double sum = 0;
        int total = 0;
        for (int y = p[1]; y < p[3]; y++) {
            for (int x = p[0]; x < p[2]; x++) {
                int c = img.getRGB(x, y);
                sum += 0.299 * ((c >> 16) & 255) + 0.587 * ((c >> 8) & 255) + 0.114 * (c & 255);
                total++;
            }
        }
        check(total > 0, "empty pixel region " + Arrays.toString(r));
        return sum / total;
    }

    // ---- input into LWJGL's queues (client thread) ----

    private static Field mouseBuffer;
    private static Field keyBuffer;

    /** Appends one record to a queue that Mouse.next / Keyboard.next read from (the buffer is in read mode). */
    private static void append(Field field, int size, Writer writer) throws Exception {
        ByteBuffer b = (ByteBuffer) field.get(null);
        int pos = b.position();
        int lim = b.limit();
        check(b.capacity() - lim >= size, "LWJGL queue full");
        b.limit(b.capacity());
        b.position(lim);
        writer.write(b);
        check(b.position() == lim + size, "LWJGL event record size");
        b.limit(b.position());
        b.position(pos);
    }

    private interface Writer {
        void write(ByteBuffer b);
    }

    /** A mouse event at a GUI position (converted to LWJGL's raw window coordinates, y from the bottom). */
    private static void mouse(final int button, final boolean state, final int guiX, final int guiY, final int wheel)
            throws Exception {
        onClient(() -> {
            queueMouse(button, state, guiX, guiY, wheel);
            return null;
        });
    }

    /** Client thread only: appends one mouse event; several calls in one task land in the same tick. */
    private static void queueMouse(final int button, final boolean state, final int guiX, final int guiY,
            final int wheel) throws Exception {
        MinecraftClient mc = MinecraftClient.getInstance();
        int scale = LegacyInput.guiScale(mc.width, mc.height, mc.options.guiScale, mc.forcesUnicodeFont());
        int guiH = LegacyInput.guiSize(mc.height, scale);
        final int rawX = guiX * scale + scale / 2;
        final int rawY = (guiH - 1 - guiY) * scale + scale / 2;
        check(LegacyInput.guiX(rawX, LegacyInput.guiSize(mc.width, scale), mc.width) == guiX
                && LegacyInput.guiY(rawY, guiH, mc.height) == guiY, "raw position round trip");
        if (mouseBuffer == null) {
            mouseBuffer = Mouse.class.getDeclaredField("readBuffer");
            mouseBuffer.setAccessible(true);
        }
        append(mouseBuffer, MOUSE_EVENT_SIZE, b -> b.put((byte) button).put((byte) (state ? 1 : 0)).putInt(rawX)
                .putInt(rawY).putInt(wheel).putLong(System.nanoTime()));
    }

    private static void click(int button) throws Exception {
        int[] c = onClient(() -> {
            MinecraftClient mc = MinecraftClient.getInstance();
            int scale = LegacyInput.guiScale(mc.width, mc.height, mc.options.guiScale, mc.forcesUnicodeFont());
            return new int[] {LegacyInput.guiSize(mc.width, scale) / 2, LegacyInput.guiSize(mc.height, scale) / 2};
        });
        click(button, c[0], c[1]);
    }

    private static void click(int button, int guiX, int guiY) throws Exception {
        mouse(button, true, guiX, guiY, 0);
        mouse(button, false, guiX, guiY, 0);
        waitTicks(1);
    }

    private static void wheel(int guiX, int guiY, int delta) throws Exception {
        mouse(-1, false, guiX, guiY, delta);
    }

    private static void keyDown(final int key, final char ch, final boolean state) throws Exception {
        onClient(() -> {
            queueKey(key, ch, state);
            return null;
        });
    }

    /** Client thread only: appends one keyboard event. */
    private static void queueKey(final int key, final char ch, final boolean state) throws Exception {
        if (keyBuffer == null) {
            keyBuffer = Keyboard.class.getDeclaredField("readBuffer");
            keyBuffer.setAccessible(true);
        }
        append(keyBuffer, KEY_EVENT_SIZE, b -> b.putInt(key).put((byte) (state ? 1 : 0)).putInt(ch)
                .putLong(System.nanoTime()).put((byte) 0));
    }

    /** A key press and release, handled in the next client tick. */
    private static void key(int key, char ch) throws Exception {
        keyDown(key, ch, true);
        keyDown(key, ch, false);
        waitTicks(2);
    }

    // ---- client thread and waits ----

    private static <T> T onClient(Callable<T> task) throws Exception {
        try {
            return MinecraftClient.getInstance().execute(task).get(CLIENT_TIMEOUT_S, TimeUnit.SECONDS);
        } catch (java.util.concurrent.ExecutionException e) {
            Throwable cause = e.getCause();
            if (cause instanceof Exception) {
                throw (Exception) cause;
            }
            if (cause instanceof Error) {
                throw (Error) cause;
            }
            throw e;
        }
    }

    private static void waitTicks(int n) throws InterruptedException {
        int target = ticks + n;
        while (ticks < target) {
            Thread.sleep(5);
        }
    }

    private static void waitFrames(int n) throws InterruptedException {
        int target = frames + n;
        while (frames < target) {
            Thread.sleep(5);
        }
    }

    /** Two ticks and then two whole frames: a change made on the client thread is in the framebuffer. */
    private static void settle() throws InterruptedException {
        waitTicks(2);
        waitFrames(3);
    }

    private static void waitRealTime(long millis) throws InterruptedException {
        Thread.sleep(millis);
    }

    private static void waitUntil(final BooleanSupplier condition, long maxMillis, String what) throws Exception {
        long deadline = System.nanoTime() + maxMillis * 1_000_000L;
        while (true) {
            if (onClient(condition::getAsBoolean)) {
                return;
            }
            if (System.nanoTime() > deadline) {
                throw new AssertionError("[hengst-gametest] FAIL " + what + " within " + maxMillis + " ms");
            }
            Thread.sleep(20);
        }
    }

    // ---- helpers ----

    /** Client thread only. */
    private static int cps(int button) {
        return HengstLegacyMod.core().context().cps().cps(button);
    }

    private static int count(List<int[]> log, int button, int pressed, int screen) {
        int n = 0;
        for (int[] e : log) {
            if (e[0] == button && e[1] == pressed && e[2] == screen) {
                n++;
            }
        }
        return n;
    }

    private static String dump(List<int[]> log) {
        StringBuilder sb = new StringBuilder();
        for (int[] e : log) {
            sb.append(Arrays.toString(e));
        }
        return sb.toString();
    }

    /** Enough extra rows that the settings' feature list can scroll on a 240 px high GUI. */
    private static void registerDummyFeatures(final HengstCore core) throws Exception {
        onClient(() -> {
            for (int i = 0; i < 16; i++) {
                core.registry().register(new Feature("gametest_row_" + i, 900 + i, FeatureGroup.COMFORT,
                        EnumSet.of(GameVersion.V1_8_9)) {
                });
            }
            return null;
        });
    }

    /** The core's screen behind its SafeScreen wrapper (HengstCore's private inner class, field "screen"). */
    private static UiScreen unwrap(UiScreen safe) throws Exception {
        return (UiScreen) field(safe, "screen");
    }

    private static Object field(Object owner, String name) throws Exception {
        Field f = owner.getClass().getDeclaredField(name);
        f.setAccessible(true);
        return f.get(owner);
    }

    /**
     * A second {@code onHudRender} on the client thread, outside the frame, into a recording canvas. It cannot change
     * core state the checks read: HudRenderer.render only reads the profile and the game, and the only fields the
     * modules write while rendering are their text and label width caches, which the recording canvas fills with the
     * same font renderer metrics the HUD canvas uses.
     */
    private static Layout layout() throws Exception {
        return onClient(() -> {
            MinecraftClient mc = MinecraftClient.getInstance();
            Layout c = new Layout(mc);
            HengstLegacyMod.core().onHudRender(c, 0.0F);
            c.ids = new String[c.boxes.size()];
            HudModule[] modules = HengstLegacyMod.core().renderer().activeModules();
            for (int i = 0; i < c.ids.length && i < modules.length; i++) {
                c.ids[i] = modules[i].id();
            }
            guiWidth = c.w;
            return c;
        });
    }

    /** What the current host's core screen draws, through a recording canvas. */
    private static List<Text> screenTexts() throws Exception {
        return onClient(() -> {
            MinecraftClient mc = MinecraftClient.getInstance();
            LegacyScreenHost host = (LegacyScreenHost) mc.currentScreen;
            Layout c = new Layout(mc);
            host.ui().render(c, 0, 0, 0.0F);
            return c.texts;
        });
    }

    private static BufferedImage shot(final String name) throws Exception {
        File file = onClient(() -> {
            MinecraftClient mc = MinecraftClient.getInstance();
            ScreenshotUtils.saveScreenshot(mc.runDirectory, name + ".png", mc.width, mc.height, mc.getFramebuffer());
            return new File(new File(mc.runDirectory, "screenshots"), name + ".png");
        });
        LOGGER.info("[hengst-gametest] screenshot {}", file);
        BufferedImage img = ImageIO.read(file);
        check(img != null, "screenshot readable: " + file);
        return img;
    }

    private static void pass(String step) {
        LOGGER.info("[hengst-gametest] PASS {}", step);
    }

    private static void check(boolean condition, String what) {
        if (!condition) {
            throw new AssertionError("[hengst-gametest] FAIL " + what);
        }
    }

    // ---- screens and canvases ----

    /** The chat screen, stamping when it starts to draw. */
    private static final class OrderChatScreen extends ChatScreen {
        @Override
        public void render(int mouseX, int mouseY, float tickDelta) {
            screenSeq = ++seq;
            super.render(mouseX, mouseY, tickDelta);
        }
    }

    /** The survival inventory (draws the dim), stamping when it starts to draw. */
    private static final class OrderInventoryScreen extends SurvivalInventoryScreen {
        OrderInventoryScreen(PlayerEntity player) {
            super(player);
        }

        @Override
        public void render(int mouseX, int mouseY, float tickDelta) {
            screenSeq = ++seq;
            super.render(mouseX, mouseY, tickDelta);
        }
    }

    /** A screen of the core's kind that records its input and behaves like the core on close. */
    private static final class ProbeUi implements UiScreen {
        private final Platform platform;
        final List<Double> scrolls = new ArrayList<Double>();
        final List<int[]> scrollAt = new ArrayList<int[]>();
        final List<int[]> clicks = new ArrayList<int[]>();
        final List<UiKey> keys = new ArrayList<UiKey>();
        int closed;
        boolean reentrant;
        // Behave like the core's Done button and Escape: close the screen from inside the handler.
        boolean closeOnEscape;
        boolean closeOnClick;

        ProbeUi(Platform platform) {
            this.platform = platform;
        }

        @Override
        public void render(Canvas canvas, int mouseX, int mouseY, float partialTick) {
        }

        @Override
        public boolean mouseClicked(int mouseX, int mouseY, int button) {
            clicks.add(new int[] {mouseX, mouseY, button, 1});
            if (closeOnClick) {
                platform.closeScreen();
            }
            return true;
        }

        @Override
        public boolean mouseReleased(int mouseX, int mouseY, int button) {
            clicks.add(new int[] {mouseX, mouseY, button, 0});
            return true;
        }

        @Override
        public boolean mouseDragged(int mouseX, int mouseY, int button) {
            return false;
        }

        @Override
        public boolean mouseScrolled(int mouseX, int mouseY, double amount) {
            scrolls.add(amount);
            scrollAt.add(new int[] {mouseX, mouseY});
            return true;
        }

        @Override
        public boolean keyPressed(UiKey key, boolean shift, boolean control) {
            keys.add(key);
            if (closeOnEscape && key == UiKey.ESCAPE) {
                platform.closeScreen();
            }
            return true;
        }

        @Override
        public void closed() {
            closed++;
            MinecraftClient mc = MinecraftClient.getInstance();
            Object before = mc.currentScreen;
            platform.closeScreen();
            if (mc.currentScreen != before) {
                reentrant = true;
            }
        }
    }

    static final class Text {
        final String text;
        final int x;
        final int y;
        final int w;
        final int h;

        Text(String text, int x, int y, int w, int h) {
            this.text = text;
            this.x = x;
            this.y = y;
            this.w = w;
            this.h = h;
        }

        int[] rect() {
            return new int[] {x - 1, y - 1, x + w + 1, y + h + 1};
        }

        @Override
        public String toString() {
            return "'" + text + "'@" + x + "," + y;
        }
    }

    /**
     * Where the core places things, in GUI pixels: a canvas that records the bounding box of everything drawn
     * between a module's push and pop, the fills with their colours, and the texts. Says nothing about visibility;
     * the screenshots do.
     */
    private static final class Layout implements Canvas {
        final int w;
        final int h;
        private final net.minecraft.client.font.TextRenderer font;
        final List<int[]> boxes = new ArrayList<int[]>();
        final List<int[]> fills = new ArrayList<int[]>();
        final List<Text> texts = new ArrayList<Text>();
        String[] ids = new String[0];
        int draws;
        private float ox;
        private float oy;
        private float scale = 1.0F;
        private int[] box;

        Layout(MinecraftClient mc) {
            int s = LegacyInput.guiScale(mc.width, mc.height, mc.options.guiScale, mc.forcesUnicodeFont());
            w = LegacyInput.guiSize(mc.width, s);
            h = LegacyInput.guiSize(mc.height, s);
            font = mc.textRenderer;
        }

        int[] rect(String id) {
            for (int i = 0; i < ids.length; i++) {
                if (id.equals(ids[i])) {
                    return boxes.get(i);
                }
            }
            throw new AssertionError("[hengst-gametest] FAIL module " + id + " not drawn");
        }

        int[] lowerPart(String id) {
            int[] r = rect(id);
            return new int[] {r[0], r[3] - (r[3] - r[1]) * 2 / 5, r[2], r[3]};
        }

        /** Open sky at the top centre: no vanilla HUD, no mod module, no hand, no crosshair. */
        int[] control() {
            return new int[] {w * 2 / 5, h / 12, w * 3 / 5, h / 6};
        }

        /** The first fill drawn with exactly this colour, in GUI pixels. */
        int[] fillWithColour(int argb) {
            for (int[] f : fills) {
                if (f[4] == argb) {
                    return new int[] {f[0], f[1], f[2], f[3]};
                }
            }
            return null;
        }

        private int[] extend(float x1, float y1, float x2, float y2) {
            draws++;
            int[] r = {Math.round(ox + x1 * scale), Math.round(oy + y1 * scale), Math.round(ox + x2 * scale),
                    Math.round(oy + y2 * scale)};
            if (box != null) {
                box[0] = Math.min(box[0], r[0]);
                box[1] = Math.min(box[1], r[1]);
                box[2] = Math.max(box[2], r[2]);
                box[3] = Math.max(box[3], r[3]);
            }
            return r;
        }

        @Override
        public int width() {
            return w;
        }

        @Override
        public int height() {
            return h;
        }

        @Override
        public int fontHeight() {
            return font.fontHeight;
        }

        @Override
        public int textWidth(String text) {
            return font.getStringWidth(text);
        }

        @Override
        public void fill(int x1, int y1, int x2, int y2, int argb) {
            int[] r = extend(Math.min(x1, x2), Math.min(y1, y2), Math.max(x1, x2), Math.max(y1, y2));
            fills.add(new int[] {r[0], r[1], r[2], r[3], argb});
        }

        @Override
        public void text(String text, int x, int y, int argb, boolean shadow) {
            int[] r = extend(x, y, x + font.getStringWidth(text), y + font.fontHeight);
            texts.add(new Text(text, r[0], r[1], r[2] - r[0], r[3] - r[1]));
        }

        @Override
        public void pushTransform(float translateX, float translateY, float scale) {
            ox = translateX;
            oy = translateY;
            this.scale = scale;
            box = new int[] {Integer.MAX_VALUE, Integer.MAX_VALUE, Integer.MIN_VALUE, Integer.MIN_VALUE};
            boxes.add(box);
        }

        @Override
        public void popTransform() {
            box = null;
            ox = 0;
            oy = 0;
            scale = 1.0F;
        }
    }
}
