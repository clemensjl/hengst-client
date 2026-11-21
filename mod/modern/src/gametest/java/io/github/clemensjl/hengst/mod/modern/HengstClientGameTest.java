package io.github.clemensjl.hengst.mod.modern;

import com.mojang.blaze3d.platform.InputConstants;
import io.github.clemensjl.hengst.mod.common.HengstCore;
import io.github.clemensjl.hengst.mod.common.config.Profile;
import io.github.clemensjl.hengst.mod.common.config.ProfileStore;
import io.github.clemensjl.hengst.mod.common.feature.DefaultKey;
import io.github.clemensjl.hengst.mod.common.feature.FeatureState;
import io.github.clemensjl.hengst.mod.common.hud.HudModule;
import io.github.clemensjl.hengst.mod.common.layout.Anchor;
import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import io.github.clemensjl.hengst.mod.common.platform.GameKey;
import io.github.clemensjl.hengst.mod.common.platform.Platform;
import io.github.clemensjl.hengst.mod.common.platform.UiKey;
import io.github.clemensjl.hengst.mod.common.platform.UiScreen;
import java.awt.image.BufferedImage;
import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Clock;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.function.Predicate;
import javax.imageio.ImageIO;
import net.fabricmc.fabric.api.client.gametest.v1.FabricClientGameTest;
import net.fabricmc.fabric.api.client.gametest.v1.TestInput;
import net.fabricmc.fabric.api.client.gametest.v1.context.ClientGameTestContext;
import net.fabricmc.fabric.api.client.gametest.v1.context.TestSingleplayerContext;
import net.fabricmc.fabric.api.client.rendering.v1.hud.HudElement;
import net.fabricmc.fabric.api.client.rendering.v1.hud.HudElementRegistry;
import net.fabricmc.fabric.api.client.rendering.v1.hud.VanillaHudElements;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.components.debug.DebugScreenEntries;
import net.minecraft.client.gui.components.debug.DebugScreenEntryStatus;
//? if >=26.1 {
import net.minecraft.client.gui.GuiGraphicsExtractor;
//?} else {
/*import net.minecraft.client.gui.GuiGraphics;
*///?}
import net.minecraft.client.gui.screens.ChatScreen;
import net.minecraft.client.gui.screens.PauseScreen;
import net.minecraft.client.gui.screens.inventory.InventoryScreen;
import net.minecraft.network.chat.Component;
import net.minecraft.sounds.SoundSource;
import net.minecraft.world.entity.player.Player;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * The mod in a real singleplayer world, driven with real input through Fabric's client gametest API. Asserts through
 * the mod's own objects and on the pixels of framebuffer screenshots (run directory, folder screenshots): a module's
 * rectangle, taken from the layout the core reports, must show the mod's text where the HUD is expected and match
 * the F1 baseline where it is not. Any failed check throws, which crashes the client and fails the Gradle task.
 *
 * <p>Two phases: the default one runs every check with a fresh run directory and ends by dragging the FPS module to
 * the bottom right in the HUD editor; the phase {@code -Dhengst.gametest.phase=restart} runs with that directory
 * kept and checks that the saved layout is loaded and drawn.
 */
public final class HengstClientGameTest implements FabricClientGameTest {
    private static final Logger LOGGER = LoggerFactory.getLogger("hengst-gametest");
    private static final int LEFT = InputConstants.MOUSE_BUTTON_LEFT;
    // Pixel thresholds. Text is white (all channels >= TEXT_LEVEL); nothing else in the test scene is that bright.
    private static final int TEXT_LEVEL = 225;
    private static final int MIN_TEXT_PIXELS = 8;
    private static final int SAME_TOLERANCE = 24;

    // Draw-order probes, written on the render thread and read on it (computeOnClient).
    private static int seq;
    private static int hudSeq;
    private static int chatSeq;
    private static int screenSeq;
    private static int hudCalls;
    private static final ProbeElement HUD_PROBE = new ProbeElement(true);
    private static final ProbeElement CHAT_PROBE = new ProbeElement(false);

    @Override
    public void runTest(ClientGameTestContext ctx) {
        boolean restart = "restart".equals(System.getProperty("hengst.gametest.phase"));
        ctx.runOnClient(mc -> {
            mc.options.getSoundSourceOptionInstance(SoundSource.MASTER).set(0.0);
            // The vignette is part of the vanilla HUD (hidden by F1) and darkens the whole frame: off, so that the F1
            // baseline differs from a normal frame only where a HUD element is.
            mc.options.vignette().set(false);
            // The mod's element (fails if it is not registered) and vanilla's chat, wrapped to record draw order.
            HudElementRegistry.replaceElement(HengstModernMod.HUD_ID, e -> HUD_PROBE.wrap(e));
            HudElementRegistry.replaceElement(VanillaHudElements.CHAT, e -> CHAT_PROBE.wrap(e));
        });
        try (TestSingleplayerContext world = ctx.worldBuilder().create()) {
            //? if >=26.1 {
            world.getConnection().waitForChunksRender();
            //?} else {
            /*world.getClientWorld().waitForChunksRender();
            *///?}
            waitUntil(ctx, mc -> hudCalls > 0, 10_000, "HUD element drawn");
            pass("HUD element drawn");
            if (restart) {
                restartPhase(ctx);
            } else {
                fullPhase(ctx);
            }
        }
    }

    private static void fullPhase(ClientGameTestContext ctx) {
        TestInput input = ctx.getInput();
        HengstCore core = HengstModernMod.core();
        Platform platform = core.context().platform();

        check(HengstModernMod.ENABLED, "mod enabled");
        // Singleplayer: no server address (the multiplayer case, an address only once the world exists, is not tested).
        check(ctx.computeOnClient(mc -> platform.game().serverAddress()) == null, "no server address in singleplayer");
        check(core.renderer().activeCount() == 4, "four active modules, got " + core.renderer().activeCount());
        Layout layout = layout(ctx);
        check(layout.draws > 0, "the core draws the HUD in a world");
        BufferedImage hud = shot(ctx, "hengst-01-hud");

        check(ctx.computeOnClient(mc -> FeatureState.on(1) && FeatureState.on(2) && FeatureState.on(3)
                && FeatureState.on(6)), "M0 modules on in the feature state");
        check(!ctx.computeOnClient(mc -> FeatureState.on(7)), "direction off in the feature state");
        ctx.runOnClient(mc -> {
            core.activeProfile().module("fps").setEnabled(false);
            core.applyConfigForTest();
        });
        check(!ctx.computeOnClient(mc -> FeatureState.on(1)), "FPS off in the feature state after switching it off");
        ctx.runOnClient(mc -> {
            core.activeProfile().module("fps").setEnabled(true);
            core.applyConfigForTest();
        });
        check(ctx.computeOnClient(mc -> FeatureState.on(1)), "FPS on again in the feature state");
        check(ctx.computeOnClient(mc -> platform.game().pingMillis()) == -1, "no ping in singleplayer");
        check(ctx.computeOnClient(mc -> platform.game().worldIdentity()) != null, "world identity in a world");
        pass("feature state follows the profile; no ping in singleplayer, a world identity in a world");
        check(ModernKeys.code(DefaultKey.NONE) == InputConstants.UNKNOWN.getValue(),
                "the unbound key code is the game's unknown key, got " + ModernKeys.code(DefaultKey.NONE));
        pass("the unbound key code matches InputConstants.UNKNOWN");

        input.pressKey(o -> o.keyToggleGui);
        ctx.waitTick();
        check(ctx.computeOnClient(mc -> platform.game().hudHidden()), "F1 hides the HUD");
        check(layout(ctx).draws == 0, "the core draws nothing under F1");
        BufferedImage hidden = shot(ctx, "hengst-02-f1");
        for (String id : new String[] {"fps", "cps", "coordinates", "keystrokes"}) {
            checkShown(hud, hidden, layout, id, "HUD vs F1 baseline");
        }
        checkSame(hud, hidden, layout.control(), "sky away from any HUD, HUD vs F1 baseline");
        checkFramesAdvance(ctx, core, "under F1");
        input.pressKey(o -> o.keyToggleGui);
        ctx.waitTick();
        check(!ctx.computeOnClient(mc -> platform.game().hudHidden()), "F1 again shows the HUD");
        pass("HUD drawn by the game (pixels), F1 hides it, frames counted under F1");

        waitUntil(ctx, mc -> core.context().frames().fps() > 0, 5_000, "frame counter reports FPS");
        int callsBefore = ctx.computeOnClient(mc -> hudCalls);
        waitRealTime(ctx, 500);
        int calls = ctx.computeOnClient(mc -> hudCalls);
        check(calls > callsBefore + 10, "HUD element drawn every frame: " + callsBefore + " -> " + calls);
        pass("frame counter advances (" + ctx.computeOnClient(mc -> core.context().frames().fps()) + " FPS)");

        waitUntil(ctx, mc -> leftCps() == 0, 3_000, "CPS idle before clicking");
        for (int i = 0; i < 5; i++) {
            input.pressMouse(LEFT);
        }
        waitUntil(ctx, mc -> leftCps() == 5, 1_000, "CPS reads 5 after 5 clicks");
        shot(ctx, "hengst-03-cps-5");
        waitUntil(ctx, mc -> leftCps() == 0, 3_000, "CPS back to 0 after the one-second window");
        shot(ctx, "hengst-04-cps-0");
        pass("CPS counts 5 clicks and falls to 0");

        ctx.setScreen(() -> new ChatScreen("", false));
        ctx.waitForScreen(ChatScreen.class);
        for (int i = 0; i < 3; i++) {
            input.pressMouse(LEFT);
        }
        int cps = ctx.computeOnClient(mc -> leftCps());
        check(cps == 0, "clicks with chat open do not count, got " + cps);
        ctx.setScreen(() -> null);
        ctx.waitForScreen(null);
        pass("clicks with chat open do not raise CPS");

        input.holdKey(o -> o.keyUp);
        ctx.waitTicks(10);
        check(ctx.computeOnClient(mc -> platform.input().isDown(GameKey.FORWARD)), "W held is down");
        shot(ctx, "hengst-05-w-held");
        input.releaseKey(o -> o.keyUp);
        ctx.waitTick();
        check(!ctx.computeOnClient(mc -> platform.input().isDown(GameKey.FORWARD)), "W released is up");
        pass("W held marks the key as down");

        KeyMapping settingsKey = ctx.computeOnClient(mc -> mapping(mc, "key.hengst.settings"));
        ctx.setScreen(() -> new ChatScreen("", false));
        ctx.waitForScreen(ChatScreen.class);
        input.pressKey(settingsKey);
        ctx.waitTicks(2);
        check(ctx.computeOnClient(mc -> ModernPlatform.currentScreen(mc) instanceof ChatScreen),
                "the settings key with chat open leaves the chat open");
        ctx.setScreen(() -> null);
        ctx.waitForScreen(null);
        ctx.waitTicks(2);
        check(ctx.computeOnClient(mc -> ModernPlatform.currentScreen(mc) == null),
                "the settings key pressed in chat does not open the settings later");
        pass("the settings key does nothing while chat is open");

        input.pressKey(settingsKey);
        ctx.waitForScreen(ModernScreenHost.class);
        check(layout(ctx).draws == 0, "HUD suppressed while the settings are open");
        shot(ctx, "hengst-06-settings");
        checkFramesAdvance(ctx, core, "with the settings screen open");
        input.pressKey(InputConstants.KEY_ESCAPE);
        ctx.waitForScreen(null);
        check(layout(ctx).draws > 0, "HUD unsuppressed after Escape");
        BufferedImage afterEscape = shot(ctx, "hengst-07-after-escape");
        checkShown(afterEscape, hidden, layout, "fps", "after Escape");
        pass("settings key opens the mod's screen, frames counted, Escape closes it and the HUD is back");

        // The player walked in the W step: a fresh F1 baseline for the same view.
        hidden = f1Baseline(ctx, "hengst-07b-f1-baseline");
        input.pressKey(o -> o.keyDebugOverlay);
        ctx.waitTicks(2);
        check(ctx.computeOnClient(mc -> platform.game().debugOverlayOpen()), "F3 open");
        check(layout(ctx).draws == 0, "the core draws nothing with F3 open");
        BufferedImage f3 = shot(ctx, "hengst-08-f3");
        // F3 text covers the top left; the lower part of the keystrokes box is clear of it.
        int[] keysLow = layout.lowerPart("keystrokes");
        checkHidden(f3, hidden, keysLow, "keystrokes with F3 open vs F1 baseline");
        input.pressKey(o -> o.keyDebugOverlay);
        ctx.waitTicks(2);
        check(!ctx.computeOnClient(mc -> platform.game().debugOverlayOpen()), "F3 closed");
        BufferedImage f3Closed = shot(ctx, "hengst-09-f3-closed");
        checkText(f3Closed, keysLow, "keystrokes after F3 closed");
        checkShown(f3Closed, hidden, layout, "fps", "after F3 closed");
        pass("F3 hides the mod HUD (pixels) and it returns when F3 closes");

        // A vanilla debug entry set to "always on" (debug profiles, e.g. the Performance profile) is drawn without
        // F3; the debug screen is not open then, so the mod HUD must stay.
        ctx.runOnClient(mc -> mc.debugEntries.setStatus(DebugScreenEntries.FPS, DebugScreenEntryStatus.ALWAYS_ON));
        ctx.waitTicks(2);
        check(!ctx.computeOnClient(mc -> platform.game().debugOverlayOpen()),
                "an always-on debug entry does not count as the open debug screen");
        check(layout(ctx).draws > 0, "the core draws with an always-on debug entry");
        BufferedImage alwaysOn = shot(ctx, "hengst-09b-debug-entry-always-on");
        checkText(alwaysOn, keysLow, "keystrokes with an always-on debug entry");
        ctx.runOnClient(mc -> mc.debugEntries.setStatus(DebugScreenEntries.FPS, DebugScreenEntryStatus.IN_OVERLAY));
        ctx.waitTicks(2);
        pass("an always-on vanilla debug entry (F3 closed) leaves the mod HUD drawn (pixels)");

        ctx.runOnClient(mc -> addChatLine(mc, "Hengst in-game test: this chat line draws over the mod HUD"));
        ctx.setScreen(OrderChatScreen::new);
        ctx.waitForScreen(OrderChatScreen.class);
        checkDrawOrder(ctx, "chat open");
        shot(ctx, "hengst-10-chat");
        ctx.setScreen(() -> null);
        ctx.waitForScreen(null);
        pass("with chat open the HUD is drawn before the chat and the screen");

        ctx.setScreen(() -> new OrderInventoryScreen(Minecraft.getInstance().player));
        ctx.waitForScreen(OrderInventoryScreen.class);
        checkDrawOrder(ctx, "inventory open");
        shot(ctx, "hengst-11-inventory");
        ctx.setScreen(() -> null);
        ctx.waitForScreen(null);
        pass("with the inventory open the HUD is drawn before the screen and its dim");

        input.pressKey(InputConstants.KEY_ESCAPE);
        ctx.waitForScreen(PauseScreen.class);
        shot(ctx, "hengst-12-pause-menu");
        input.pressKey(InputConstants.KEY_ESCAPE);
        ctx.waitForScreen(null);
        pass("pause menu screenshot taken");

        ProbeUi probe = new ProbeUi(platform);
        ctx.runOnClient(mc -> platform.openScreen(probe));
        ctx.waitForScreen(ModernScreenHost.class);
        input.scroll(1.0);
        input.scroll(-1.0);
        for (int i = 0; i < 10; i++) {
            input.scroll(0.1);
        }
        input.scroll(3.0);
        List<Double> scrolls = ctx.computeOnClient(mc -> new ArrayList<>(probe.scrolls));
        check(scrolls.equals(Arrays.asList(1.0, -1.0, 1.0, 1.0, 1.0, 1.0)), "scroll notches, got " + scrolls);
        pass("scroll arrives as whole notches " + scrolls);
        ctx.setScreen(() -> new ChatScreen("", false));
        ctx.waitForScreen(ChatScreen.class);
        int closed = ctx.computeOnClient(mc -> probe.closed);
        check(closed == 1, "replaced host screen: closed() once, got " + closed);
        check(!ctx.computeOnClient(mc -> probe.reentrant), "closeScreen() inside closed() changed the screen");
        ctx.runOnClient(mc -> platform.closeScreen());
        check(ctx.computeOnClient(mc -> ModernPlatform.currentScreen(mc) instanceof ChatScreen),
                "closeScreen() left a vanilla screen alone");
        ctx.setScreen(() -> null);
        ctx.waitForScreen(null);
        pass("screen removal rule: one close, no re-entrant set-screen");

        dragFpsToBottomRight(ctx, core, layout);
    }

    /** Last step of the full phase: the layout it saves is what the restart phase checks. */
    private static void dragFpsToBottomRight(ClientGameTestContext ctx, HengstCore core, Layout layout) {
        TestInput input = ctx.getInput();
        int[] fps = layout.rect("fps");
        double[] window = ctx.computeOnClient(mc -> new double[] {
                mc.getWindow().getScreenWidth(), mc.getWindow().getScreenHeight()});
        double sx = window[0] / layout.w;
        double sy = window[1] / layout.h;
        BufferedImage before = shot(ctx, "hengst-13-before-editor");
        ctx.runOnClient(mc -> core.onOpenHudEditorKey());
        ctx.waitForScreen(ModernScreenHost.class);
        BufferedImage editor = shot(ctx, "hengst-14-editor");
        double lumaBefore = meanLuma(before, layout.control());
        double lumaEditor = meanLuma(editor, layout.control());
        check(lumaEditor < 0.75 * lumaBefore, "editor dims the screen: luma " + lumaBefore + " -> " + lumaEditor);
        int[] centre = {layout.w * 3 / 10, layout.h * 2 / 5, layout.w * 7 / 10, layout.h * 3 / 5};
        checkText(editor, centre, "editor hint in the centre");
        pass("editor dims the screen and centres the hint (pixels)");

        double x = (fps[0] + 3) * sx;
        double y = (fps[1] + 3) * sy;
        input.setCursorPos(x, y);
        ctx.waitTick();
        input.holdMouse(LEFT);
        ctx.waitTick();
        double tx = window[0] - 4;
        double ty = window[1] - 4;
        for (int i = 1; i <= 5; i++) {
            input.setCursorPos(x + (tx - x) * i / 5, y + (ty - y) * i / 5);
            ctx.waitTick();
        }
        input.releaseMouse(LEFT);
        ctx.waitTick();
        shot(ctx, "hengst-15-editor-dragged");
        input.pressKey(InputConstants.KEY_ESCAPE);
        ctx.waitForScreen(null);

        Path profiles = FabricLoader.getInstance().getConfigDir().resolve("hengst").resolve("profiles");
        check(Files.isRegularFile(profiles.resolve("default.json")), "profile file written: " + profiles);
        Profile saved = new ProfileStore(profiles, Clock.systemUTC(), msg -> LOGGER.warn(msg)).load(ProfileStore.DEFAULT);
        Anchor anchor = saved.module("fps").anchor();
        check(anchor == Anchor.BOTTOM_RIGHT, "saved FPS anchor, got " + anchor);
        checkFpsBottomRight(ctx, "hengst-16-fps-bottom-right", "hengst-17-fps-bottom-right-f1");
        pass("HUD editor drag saved the FPS module at " + anchor + " and the game draws it there (pixels)");
    }

    private static void restartPhase(ClientGameTestContext ctx) {
        HengstCore core = HengstModernMod.core();
        Anchor anchor = ctx.computeOnClient(mc -> core.activeProfile().module("fps").anchor());
        check(anchor == Anchor.BOTTOM_RIGHT, "restart: loaded FPS anchor (run runClientGameTest first), got " + anchor);
        checkFpsBottomRight(ctx, "hengst-18-restart-fps-bottom-right", "hengst-19-restart-f1");
        pass("restart: the FPS module is drawn at the saved position (pixels)");
    }

    /** FPS laid out in the bottom-right quadrant and visible there in the framebuffer, against a fresh F1 baseline. */
    private static void checkFpsBottomRight(ClientGameTestContext ctx, String shownName, String hiddenName) {
        Layout layout = layout(ctx);
        int[] r = layout.rect("fps");
        check(r[0] > layout.w / 2 && r[1] > layout.h / 2, "fps laid out at the bottom right, rect "
                + Arrays.toString(r) + " on " + layout.w + "x" + layout.h);
        BufferedImage shown = shot(ctx, shownName);
        BufferedImage hidden = f1Baseline(ctx, hiddenName);
        checkShown(shown, hidden, layout, "fps", "FPS at the bottom right");
    }

    /** A screenshot of the same view with F1 on (no HUD at all), then F1 off again. */
    private static BufferedImage f1Baseline(ClientGameTestContext ctx, String name) {
        ctx.getInput().pressKey(o -> o.keyToggleGui);
        ctx.waitTick();
        BufferedImage hidden = shot(ctx, name);
        ctx.getInput().pressKey(o -> o.keyToggleGui);
        ctx.waitTick();
        check(!ctx.computeOnClient(mc -> HengstModernMod.core().context().platform().game().hudHidden()),
                "F1 baseline taken, HUD shown again");
        return hidden;
    }

    /** onFrame keeps feeding the FPS counter: after 2.5 s it would read 0 if no frame had been counted. */
    private static void checkFramesAdvance(ClientGameTestContext ctx, HengstCore core, String state) {
        waitRealTime(ctx, 2_500);
        int fps = ctx.computeOnClient(mc -> core.context().frames().fps());
        check(fps > 0, "frames counted " + state + ", FPS " + fps);
        pass("frames counted " + state + " (" + fps + " FPS)");
    }

    // ---- pixel checks ----

    /** The module's rectangle shows white text pixels that the hidden baseline does not, and differs from it. */
    private static void checkShown(BufferedImage shown, BufferedImage hidden, Layout layout, String id, String what) {
        int[] r = layout.rect(id);
        checkText(shown, r, what + ": " + id);
        int baseline = textPixels(hidden, r);
        check(baseline == 0, what + ": " + id + " baseline has no text pixels, got " + baseline);
        double diff = diffFraction(shown, hidden, r);
        check(diff >= 0.2, what + ": " + id + " differs from the baseline, fraction " + diff);
    }

    /** Nothing of the mod in the region: same as the hidden baseline and no text pixels. */
    private static void checkHidden(BufferedImage img, BufferedImage hidden, int[] r, String what) {
        int text = textPixels(img, r);
        check(text == 0, what + ": no text pixels, got " + text);
        checkSame(img, hidden, r, what);
    }

    private static void checkSame(BufferedImage a, BufferedImage b, int[] r, String what) {
        double diff = diffFraction(a, b, r);
        check(diff <= 0.05, what + ": same as baseline, differing fraction " + diff);
    }

    /** White text over a darker background: enough text pixels, and not one flat colour. */
    private static void checkText(BufferedImage img, int[] r, String what) {
        int text = textPixels(img, r);
        int total = area(img, r);
        check(text >= MIN_TEXT_PIXELS, what + ": text pixels " + text);
        check(text < total * 0.7, what + ": region is not one flat bright colour (" + text + "/" + total + ")");
    }

    /** GUI rectangle {x1, y1, x2, y2} to image pixels, inset by one pixel, clamped. */
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
        return sum / total;
    }

    // ---- helpers ----

    /** One frame's HUD pass consists of the mod element, then vanilla's chat, then the screen: consecutive numbers. */
    private static void checkDrawOrder(ClientGameTestContext ctx, String where) {
        ctx.waitTicks(2);
        int[] s = ctx.computeOnClient(mc -> new int[] {hudSeq, chatSeq, screenSeq});
        check(s[0] + 1 == s[1] && s[1] + 1 == s[2], where + ": order mod HUD, chat, screen expected, got "
                + Arrays.toString(s));
    }

    /** Waits on real time (the CPS and FPS windows use the system clock), bounded by {@code maxMillis}. */
    private static void waitUntil(ClientGameTestContext ctx, Predicate<Minecraft> condition, long maxMillis,
            String what) {
        long deadline = System.nanoTime() + maxMillis * 1_000_000L;
        ctx.waitFor(mc -> condition.test(mc) || System.nanoTime() > deadline, ClientGameTestContext.NO_TIMEOUT);
        check(ctx.computeOnClient(condition::test), what + " within " + maxMillis + " ms");
    }

    private static void waitRealTime(ClientGameTestContext ctx, long millis) {
        long deadline = System.nanoTime() + millis * 1_000_000L;
        ctx.waitFor(mc -> System.nanoTime() >= deadline, ClientGameTestContext.NO_TIMEOUT);
    }

    /** Client thread only. */
    private static int leftCps() {
        return HengstModernMod.core().context().cps().cps(0);
    }

    // GUI width of the last layout; screenshots are scaled from it.
    private static int guiWidth;

    /** Calls the core's HUD render once with a recording canvas (layout and draw count only, not visibility). */
    private static Layout layout(ClientGameTestContext ctx) {
        return ctx.computeOnClient(mc -> {
            Layout c = new Layout(mc);
            HengstModernMod.core().onHudRender(c, 0.0F);
            c.ids = new String[c.boxes.size()];
            HudModule[] modules = HengstModernMod.core().renderer().activeModules();
            for (int i = 0; i < c.ids.length && i < modules.length; i++) {
                c.ids[i] = modules[i].id();
            }
            guiWidth = c.w;
            return c;
        });
    }

    private static KeyMapping mapping(Minecraft mc, String name) {
        for (KeyMapping k : mc.options.keyMappings) {
            if (k.getName().equals(name)) {
                return k;
            }
        }
        throw new AssertionError("key mapping " + name + " not registered");
    }

    private static void addChatLine(Minecraft mc, String text) {
        //? if >=26.1 {
        mc.gui.hud.getChat().addClientSystemMessage(Component.literal(text));
        //?} else {
        /*mc.gui.getChat().addMessage(Component.literal(text));
        *///?}
    }

    private static BufferedImage shot(ClientGameTestContext ctx, String name) {
        Path path = ctx.takeScreenshot(name);
        LOGGER.info("[hengst-gametest] screenshot {}", path);
        try {
            BufferedImage img = ImageIO.read(path.toFile());
            check(img != null, "screenshot readable: " + path);
            return img;
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
    }

    private static void pass(String step) {
        LOGGER.info("[hengst-gametest] PASS {}", step);
    }

    private static void check(boolean condition, String what) {
        if (!condition) {
            throw new AssertionError("[hengst-gametest] FAIL " + what);
        }
    }

    /** Wraps a HUD element and stamps when it is drawn. */
    private static final class ProbeElement implements HudElement {
        private final boolean mod;
        private HudElement inner;

        ProbeElement(boolean mod) {
            this.mod = mod;
        }

        HudElement wrap(HudElement element) {
            inner = element;
            return this;
        }

        private void stamp() {
            if (mod) {
                hudSeq = ++seq;
                hudCalls++;
            } else {
                chatSeq = ++seq;
            }
        }

        //? if >=26.1 {
        @Override
        public void extractRenderState(GuiGraphicsExtractor graphics, DeltaTracker delta) {
            stamp();
            inner.extractRenderState(graphics, delta);
        }
        //?} else {
        /*@Override
        public void render(GuiGraphics graphics, DeltaTracker delta) {
            stamp();
            inner.render(graphics, delta);
        }
        *///?}
    }

    /** The chat screen, stamping when it starts to draw. */
    private static final class OrderChatScreen extends ChatScreen {
        OrderChatScreen() {
            super("", false);
        }

        //? if >=26.1 {
        @Override
        public void extractRenderState(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a) {
            screenSeq = ++seq;
            super.extractRenderState(graphics, mouseX, mouseY, a);
        }
        //?} else {
        /*@Override
        public void render(GuiGraphics graphics, int mouseX, int mouseY, float a) {
            screenSeq = ++seq;
            super.render(graphics, mouseX, mouseY, a);
        }
        *///?}
    }

    /** The inventory (an in-game UI screen with a dim), stamping when it starts to draw. */
    private static final class OrderInventoryScreen extends InventoryScreen {
        OrderInventoryScreen(Player player) {
            super(player);
        }

        //? if >=26.1 {
        @Override
        public void extractRenderState(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a) {
            screenSeq = ++seq;
            super.extractRenderState(graphics, mouseX, mouseY, a);
        }
        //?} else {
        /*@Override
        public void render(GuiGraphics graphics, int mouseX, int mouseY, float a) {
            screenSeq = ++seq;
            super.render(graphics, mouseX, mouseY, a);
        }
        *///?}
    }

    /** A screen of the core's kind that counts closes and records scroll amounts; behaves like the core on close. */
    private static final class ProbeUi implements UiScreen {
        private final Platform platform;
        final List<Double> scrolls = new ArrayList<>();
        int closed;
        boolean reentrant;

        ProbeUi(Platform platform) {
            this.platform = platform;
        }

        @Override
        public void render(Canvas canvas, int mouseX, int mouseY, float partialTick) {
        }

        @Override
        public boolean mouseClicked(int mouseX, int mouseY, int button) {
            return false;
        }

        @Override
        public boolean mouseReleased(int mouseX, int mouseY, int button) {
            return false;
        }

        @Override
        public boolean mouseDragged(int mouseX, int mouseY, int button) {
            return false;
        }

        @Override
        public boolean mouseScrolled(int mouseX, int mouseY, double amount) {
            scrolls.add(amount);
            return true;
        }

        @Override
        public boolean keyPressed(UiKey key, boolean shift, boolean control) {
            return false;
        }

        @Override
        public void closed() {
            closed++;
            Minecraft mc = Minecraft.getInstance();
            Object before = ModernPlatform.currentScreen(mc);
            platform.closeScreen();
            if (ModernPlatform.currentScreen(mc) != before) {
                reentrant = true;
            }
        }
    }

    /**
     * Where the core places each module, in GUI pixels: a canvas that records the bounding box of everything drawn
     * between a module's push and pop. Says nothing about visibility; the screenshots do.
     */
    private static final class Layout implements Canvas {
        final int w;
        final int h;
        private final Font font;
        final List<int[]> boxes = new ArrayList<>();
        String[] ids = new String[0];
        int draws;
        private float ox;
        private float oy;
        private float scale = 1.0F;
        private int[] box;

        Layout(Minecraft mc) {
            w = mc.getWindow().getGuiScaledWidth();
            h = mc.getWindow().getGuiScaledHeight();
            font = mc.font;
        }

        /** {x1, y1, x2, y2} of an active module. */
        int[] rect(String id) {
            for (int i = 0; i < ids.length; i++) {
                if (id.equals(ids[i])) {
                    return boxes.get(i);
                }
            }
            throw new AssertionError("[hengst-gametest] FAIL module " + id + " not drawn");
        }

        /** The lower 40 percent of a module's box. */
        int[] lowerPart(String id) {
            int[] r = rect(id);
            return new int[] {r[0], r[3] - (r[3] - r[1]) * 2 / 5, r[2], r[3]};
        }

        /** Open sky at the top centre: no vanilla HUD, no mod module, no hand, no crosshair. */
        int[] control() {
            return new int[] {w * 2 / 5, h / 12, w * 3 / 5, h / 6};
        }

        private void extend(float x1, float y1, float x2, float y2) {
            draws++;
            if (box == null) {
                return;
            }
            box[0] = Math.min(box[0], Math.round(ox + x1 * scale));
            box[1] = Math.min(box[1], Math.round(oy + y1 * scale));
            box[2] = Math.max(box[2], Math.round(ox + x2 * scale));
            box[3] = Math.max(box[3], Math.round(oy + y2 * scale));
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
            return font.lineHeight;
        }

        @Override
        public int textWidth(String text) {
            return font.width(text);
        }

        @Override
        public void fill(int x1, int y1, int x2, int y2, int argb) {
            extend(Math.min(x1, x2), Math.min(y1, y2), Math.max(x1, x2), Math.max(y1, y2));
        }

        @Override
        public void text(String text, int x, int y, int argb, boolean shadow) {
            extend(x, y, x + font.width(text), y + font.lineHeight);
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
