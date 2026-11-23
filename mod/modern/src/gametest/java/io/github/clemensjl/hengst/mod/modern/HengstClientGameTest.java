package io.github.clemensjl.hengst.mod.modern;

import com.google.gson.JsonParser;
import com.mojang.blaze3d.platform.InputConstants;
import io.github.clemensjl.hengst.mod.common.FeatureCatalog;
import io.github.clemensjl.hengst.mod.common.HengstCore;
import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.config.Profile;
import io.github.clemensjl.hengst.mod.common.config.ProfileStore;
import io.github.clemensjl.hengst.mod.common.feature.DefaultKey;
import io.github.clemensjl.hengst.mod.common.feature.Feature;
import io.github.clemensjl.hengst.mod.common.feature.FeatureState;
import io.github.clemensjl.hengst.mod.common.feature.KeySpec;
import io.github.clemensjl.hengst.mod.common.feature.OptionSpec;
import io.github.clemensjl.hengst.mod.common.hud.HudModule;
import io.github.clemensjl.hengst.mod.common.hud.TextHudModule;
import io.github.clemensjl.hengst.mod.common.layout.Anchor;
import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import io.github.clemensjl.hengst.mod.common.platform.GameKey;
import io.github.clemensjl.hengst.mod.common.platform.Platform;
import io.github.clemensjl.hengst.mod.common.platform.UiKey;
import io.github.clemensjl.hengst.mod.common.platform.UiScreen;
import io.github.clemensjl.hengst.mod.common.ui.ScrollList;
import io.github.clemensjl.hengst.mod.common.ui.SettingsModel;
import io.github.clemensjl.hengst.mod.common.ui.SettingsScreen;
import io.github.clemensjl.hengst.mod.common.ui.Widget;
import java.awt.image.BufferedImage;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.UncheckedIOException;
import java.lang.management.ManagementFactory;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Clock;
import java.time.LocalTime;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.CompletableFuture;
import java.util.function.Predicate;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import javax.imageio.ImageIO;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.gametest.v1.FabricClientGameTest;
import net.fabricmc.fabric.api.client.gametest.v1.TestInput;
import net.fabricmc.fabric.api.client.gametest.v1.context.ClientGameTestContext;
import net.fabricmc.fabric.api.client.gametest.v1.context.TestSingleplayerContext;
import net.fabricmc.fabric.api.client.rendering.v1.hud.HudElement;
import net.fabricmc.fabric.api.client.rendering.v1.hud.HudElementRegistry;
import net.fabricmc.fabric.api.client.rendering.v1.hud.VanillaHudElements;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.InactivityFpsLimit;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.client.gui.components.events.GuiEventListener;
import net.minecraft.client.gui.components.debug.DebugScreenEntries;
import net.minecraft.client.gui.components.debug.DebugScreenEntryStatus;
//? if >=26.1 {
import net.minecraft.client.gui.GuiGraphicsExtractor;
//?} else {
/*import net.minecraft.client.gui.GuiGraphics;
*///?}
import net.minecraft.client.gui.screens.ChatScreen;
import net.minecraft.client.gui.screens.PauseScreen;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.inventory.InventoryScreen;
import net.minecraft.client.gui.screens.options.controls.KeyBindsList;
import net.minecraft.client.gui.screens.options.controls.KeyBindsScreen;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.contents.TranslatableContents;
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
        // After the mod's own tick listener (registered at client init): the measured extra key dispatch.
        ClientTickEvents.END_CLIENT_TICK.register(HengstClientGameTest::measureDispatch);
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
                fullPhase(ctx, world);
            }
        }
    }

    private static void fullPhase(ClientGameTestContext ctx, TestSingleplayerContext world) {
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

        input.pressKey(o -> o.keyChat);                           // the chat key, a real key event
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
        input.pressKey(o -> o.keyChat);
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

        keyRegistration(ctx, core);
        infoModules(ctx, core);
        reducedDebugInfo(ctx, world, core);
        settingsWithTheFullCatalog(ctx, core);
        languageSwitch(ctx, core);
        dispatchAllocation(ctx);
        costMeasurement(ctx, core);
        check(core.renderer().activeCount() == 4, "four active modules after the M1a steps, got "
                + core.renderer().activeCount());
        pass("after the M1a steps only the four M0 modules are active");

        dragFpsToBottomRight(ctx, core, layout);
        bindToggleForRestart(ctx);
    }

    // The stopwatch toggle bound to K in the controls screen at the end of the full phase; the restart phase checks
    // that the game loaded it from options.txt and unbinds it again.
    private static final String TOGGLE = "key.hengst.stopwatch.toggle";

    /** Binds the stopwatch toggle to K the way a player does: the controls screen, a real key press, Escape (saves). */
    private static void bindToggleForRestart(ClientGameTestContext ctx) {
        TestInput input = ctx.getInput();
        KeyMapping toggle = ctx.computeOnClient(mc -> mapping(mc, TOGGLE));
        InputConstants.Key k = keyboardKey(InputConstants.KEY_K);
        ctx.setScreen(() -> new KeyBindsScreen(null, Minecraft.getInstance().options));
        ctx.waitForScreen(KeyBindsScreen.class);
        ctx.runOnClient(mc -> ((KeyBindsScreen) ModernPlatform.currentScreen(mc)).selectedKey = toggle);
        input.pressKey(k);
        ctx.waitTick();
        check(ctx.computeOnClient(mc -> toggle.saveString()).equals(k.getName()), "the controls screen bound the toggle to "
                + k.getName() + ", got " + ctx.computeOnClient(mc -> toggle.saveString()));
        input.pressKey(InputConstants.KEY_ESCAPE);
        ctx.waitForScreen(null);
        check(optionLine("key_" + TOGGLE + ":").equals("key_" + TOGGLE + ":" + k.getName()),
                "options.txt after closing the controls screen: " + optionLine("key_" + TOGGLE + ":"));
        pass("the stopwatch toggle bound to K in the controls screen, saved by the game when the screen closed");
    }

    /** The first line of options.txt that starts with the prefix, or "". */
    private static String optionLine(String prefix) {
        Path options = FabricLoader.getInstance().getGameDir().resolve("options.txt");
        try {
            for (String line : Files.readAllLines(options, StandardCharsets.UTF_8)) {
                if (line.startsWith(prefix)) {
                    return line;
                }
            }
            return "";
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
    }

    // ---- M1a: key registration, the seven text modules, the settings with the full catalog, language ----

    private static final String[] HENGST_KEYS = {"key.hengst.settings", "key.hengst.hud_editor",
        "key.hengst.stopwatch.toggle", "key.hengst.stopwatch.reset"};

    /** The game's key binding list holds exactly the catalog's bindings, in catalog order, in the mod's category. */
    private static void keyRegistration(ClientGameTestContext ctx, HengstCore core) {
        List<String> catalog = new ArrayList<>();
        for (KeySpec k : FeatureCatalog.keys(ModernPlatform.gameVersion())) {
            catalog.add(k.translationKey());
        }
        List<String> fromCore = new ArrayList<>();
        for (KeySpec k : core.keySpecs()) {
            fromCore.add(k.translationKey());
        }
        List<String> registered = ctx.computeOnClient(mc -> {
            List<String> names = new ArrayList<>();
            KeyMapping[] mine = ModernKeys.mappings();
            int i = 0;
            for (KeyMapping k : mc.options.keyMappings) {
                if (k.getCategory() == ModernKeys.CATEGORY) {
                    check(i < mine.length && mine[i] == k, "option mapping " + k.getName() + " is the mod's own mapping " + i);
                    names.add(k.getName());
                    i++;
                }
            }
            return names;
        });
        check(catalog.equals(Arrays.asList(HENGST_KEYS)), "catalog keys " + catalog);
        check(fromCore.equals(catalog), "core keys " + fromCore + " vs catalog " + catalog);
        check(registered.equals(catalog), "registered key bindings " + registered + " vs catalog " + catalog);
        pass("the game's key binding list holds exactly the catalog's bindings in catalog order " + registered);
    }

    /** The seven text modules one at a time: switched on through the core, checked by text and pixels, switched off. */
    private static void infoModules(ClientGameTestContext ctx, HengstCore core) {
        TestInput input = ctx.getInput();
        Platform platform = core.context().platform();

        // Direction: yaw 180 looks north.
        face(ctx, 180f);
        setEnabled(ctx, core, "direction", true);
        waitText(ctx, core, "direction", "N 0°"::equals, 2_000, "direction facing north (yaw 180)");
        int[] rect = shownByPixels(ctx, "direction", "hengst-40-direction");
        invisibleNegativeControl(ctx, core, "direction");
        offAndGone(ctx, core, "direction", rect, "hengst-41-direction-off");
        pass("direction: 'N 0°' facing north, drawn (pixels), the pixel check fails for invisible text, gone when off");

        // Speed: 0.0 standing, at least 3 m/s while W is held (walking is about 4.3 m/s).
        setEnabled(ctx, core, "speed", true);
        waitText(ctx, core, "speed", "0.0 m/s"::equals, 5_000, "speed standing");
        input.holdKey(o -> o.keyUp);
        String walking;
        try {
            walking = waitText(ctx, core, "speed", s -> speed(s) >= 3.0, 3_000, "speed while W is held");
            shownByPixels(ctx, "speed", "hengst-42a-speed-walking");
        } finally {
            input.releaseKey(o -> o.keyUp);
        }
        waitText(ctx, core, "speed", "0.0 m/s"::equals, 5_000, "speed after W is released");
        rect = shownByPixels(ctx, "speed", "hengst-42-speed");
        offAndGone(ctx, core, "speed", rect, "hengst-43-speed-off");
        pass("speed: '0.0 m/s' standing, '" + walking + "' while walking, both drawn (pixels), gone when off");

        // Clock: the system time to the minute, 24-hour then 12-hour.
        setEnabled(ctx, core, "clock", true);
        String clock = waitText(ctx, core, "clock", s -> s.matches("\\d\\d:\\d\\d"), 2_000, "clock in the 24-hour format");
        checkClose(clockMinutes(clock), "24-hour clock " + clock);
        rect = shownByPixels(ctx, "clock", "hengst-44-clock");
        setOption(ctx, core, "clock", "format", "12h");
        String am = ctx.computeOnClient(mc -> core.context().i18n().get("hengst.clock.am"));
        String pm = ctx.computeOnClient(mc -> core.context().i18n().get("hengst.clock.pm"));
        String twelve = waitText(ctx, core, "clock", s -> s.matches("\\d{1,2}:\\d\\d (" + am + "|" + pm + ")"), 2_000,
                "clock in the 12-hour format");
        checkClose(clockMinutes(twelve, am, pm), "12-hour clock " + twelve);
        shownByPixels(ctx, "clock", "hengst-44a-clock-12h");
        setOption(ctx, core, "clock", "format", "24h");
        waitText(ctx, core, "clock", s -> s.matches("\\d\\d:\\d\\d"), 2_000, "clock back in the 24-hour format");
        offAndGone(ctx, core, "clock", rect, "hengst-45-clock-off");
        pass("clock: '" + clock + "' and '" + twelve + "' match the system time to the minute, both drawn (pixels), gone when off");

        // Memory: the shown figures against Runtime read just before and after the module's reading.
        setEnabled(ctx, core, "memory", true);
        long before = ctx.computeOnClient(mc -> {
            core.applyConfigForTest();                     // the next tick reads the memory again
            return usedBytes();
        });
        ctx.waitTick();
        long[] after = ctx.computeOnClient(mc -> new long[] {usedBytes(), Runtime.getRuntime().maxMemory()});
        String memory = waitText(ctx, core, "memory", s -> s.matches("RAM \\d+% \\d+/\\d+ MB"), 2_000, "memory format");
        Matcher m = Pattern.compile("RAM (\\d+)% (\\d+)/(\\d+) MB").matcher(memory);
        check(m.matches(), "memory text " + memory);
        long percent = Long.parseLong(m.group(1));
        long usedMb = Long.parseLong(m.group(2));
        long maxMb = Long.parseLong(m.group(3));
        check(maxMb == after[1] >> 20, "memory maximum " + maxMb + " MB vs Runtime " + (after[1] >> 20) + " MB");
        long low = (Math.min(before, after[0]) >> 20) - MEMORY_SLACK_MB;
        long high = (Math.max(before, after[0]) >> 20) + MEMORY_SLACK_MB;
        check(usedMb >= low && usedMb <= high, "memory used " + usedMb + " MB outside Runtime's " + low + " to " + high);
        check(Math.abs(percent - usedMb * 100 / maxMb) <= 1, "memory percent " + percent + " vs " + usedMb + "/" + maxMb);
        rect = shownByPixels(ctx, "memory", "hengst-46-memory");
        offAndGone(ctx, core, "memory", rect, "hengst-47-memory-off");
        pass("memory: '" + memory + "' matches Runtime (maximum exact, used within " + MEMORY_SLACK_MB
                + " MB of the readings around it), drawn (pixels), gone when off");

        // Ping and server address in singleplayer: no server address, so both show nothing and draw no box.
        check(ctx.computeOnClient(mc -> platform.game().serverAddress()) == null, "singleplayer has no server address");
        for (String id : new String[] {"ping", "server_address"}) {
            setEnabled(ctx, core, id, true);
            ctx.waitTicks(3);
            String t = ctx.computeOnClient(mc -> module(core, id).text());
            check(t.isEmpty(), id + " shows nothing in singleplayer, got '" + t + "'");
            Layout l = layout(ctx);
            check(!l.has(id), id + " has no box in singleplayer");
            int[] place = defaultPlace(ctx, core, id, l);
            BufferedImage img = shot(ctx, "hengst-48-" + id + "-singleplayer");
            BufferedImage hidden = f1Baseline(ctx, "hengst-48-" + id + "-singleplayer-f1");
            checkHidden(img, hidden, place, id + " at its default place in singleplayer");
            setEnabled(ctx, core, id, false);
        }
        pass("ping and server address: empty in singleplayer, no box, their default place equals the F1 baseline");

        stopwatch(ctx, core);
    }

    /** Both stopwatch keys bound to letters that vanilla leaves unbound, restored afterwards whatever happens. */
    private static void stopwatch(ClientGameTestContext ctx, HengstCore core) {
        TestInput input = ctx.getInput();
        KeyMapping toggle = ctx.computeOnClient(mc -> mapping(mc, "key.hengst.stopwatch.toggle"));
        KeyMapping reset = ctx.computeOnClient(mc -> mapping(mc, "key.hengst.stopwatch.reset"));
        InputConstants.Key k = keyboardKey(InputConstants.KEY_K);
        InputConstants.Key j = keyboardKey(InputConstants.KEY_J);
        String taken = ctx.computeOnClient(mc -> {
            for (KeyMapping other : mc.options.keyMappings) {
                if (other != toggle && other != reset
                        && (other.saveString().equals(k.getName()) || other.saveString().equals(j.getName()))) {
                    return other.getName() + " on " + other.saveString();
                }
            }
            return null;
        });
        check(taken == null, "K and J are free in this version's bindings, but " + taken);
        setEnabled(ctx, core, "stopwatch", true);
        try {
            ctx.runOnClient(mc -> {
                toggle.setKey(k);
                reset.setKey(j);
                KeyMapping.resetMapping();
            });
            waitText(ctx, core, "stopwatch", "0:00.0"::equals, 2_000, "stopwatch at zero");
            input.pressKey(k);
            String first = waitText(ctx, core, "stopwatch", s -> !s.equals("0:00.0"), 2_000, "the stopwatch runs after K");
            waitText(ctx, core, "stopwatch", s -> !s.equals(first), 1_000, "the running stopwatch advances");
            waitRealTime(ctx, 1_500);
            input.pressKey(k);
            ctx.waitTicks(2);
            String stopped = ctx.computeOnClient(mc -> module(core, "stopwatch").text());
            double seconds = stopwatchSeconds(stopped);
            check(seconds >= 1.0 && seconds <= 2.5, "stopwatch stopped after about 1.5 s, shows " + stopped);
            waitRealTime(ctx, 500);
            check(ctx.computeOnClient(mc -> module(core, "stopwatch").text()).equals(stopped), "a stopped stopwatch holds");
            pass("stopwatch: K starts it, it advances, K stops it at " + stopped + " and it holds");

            TypingChatScreen[] chat = new TypingChatScreen[1];
            ctx.setScreen(() -> chat[0] = new TypingChatScreen());
            ctx.waitForScreen(TypingChatScreen.class);
            input.pressKey(k);
            input.typeChars("k");
            ctx.waitTicks(2);
            String typed = ctx.computeOnClient(mc -> chat[0].typed());
            check(typed.contains("k"), "the letter reached the chat box, got '" + typed + "'");
            ctx.setScreen(() -> null);
            ctx.waitForScreen(null);
            ctx.waitTicks(3);
            check(ctx.computeOnClient(mc -> module(core, "stopwatch").text()).equals(stopped),
                    "K typed in chat does not start the stopwatch");
            input.pressKey(k);
            waitText(ctx, core, "stopwatch", s -> !s.equals(stopped), 2_000, "K after closing the chat starts it");
            input.pressKey(k);
            ctx.waitTicks(2);
            pass("stopwatch: K typed in chat does nothing, K after the chat starts it");

            String held = ctx.computeOnClient(mc -> module(core, "stopwatch").text());
            int[] rect = shownByPixels(ctx, "stopwatch", "hengst-49-stopwatch");
            check(ctx.computeOnClient(mc -> module(core, "stopwatch").text()).equals(held), "stopwatch held during the shot");
            input.pressKey(j);
            waitText(ctx, core, "stopwatch", "0:00.0"::equals, 1_000, "J resets the stopwatch");
            offAndGone(ctx, core, "stopwatch", rect, "hengst-50-stopwatch-off");
            pass("stopwatch: drawn at " + held + " (pixels), J resets it to 0:00.0, gone when off");
        } finally {
            ctx.runOnClient(mc -> {
                toggle.setKey(toggle.getDefaultKey());
                reset.setKey(reset.getDefaultKey());
                KeyMapping.resetMapping();
            });
            setEnabled(ctx, core, "stopwatch", false);
        }
        check(ctx.computeOnClient(mc -> toggle.isUnbound() && reset.isUnbound()), "stopwatch keys unbound again");
    }

    /**
     * The settings with the real catalog: group headings, an overflowing list that moves one row per wheel notch, the
     * scroll indicator, and for the stopwatch (group Comfort) its style rows, its muted hint above its options and
     * option rows that react where they are drawn. Positions come from the screen's own lists.
     */
    private static void settingsWithTheFullCatalog(ClientGameTestContext ctx, HengstCore core) {
        TestInput input = ctx.getInput();
        KeyMapping settingsKey = ctx.computeOnClient(mc -> mapping(mc, "key.hengst.settings"));
        input.pressKey(settingsKey);
        ctx.waitForScreen(ModernScreenHost.class);
        UiScreen ui = settingsUi(ctx);
        ScrollList list = (ScrollList) field(ui, "featureList");
        int[] lb = bounds(list);
        int rowH = (Integer) field(list, "rowHeight");
        int rows = ((List<?>) field(list, "widgets")).size();
        check(list.overflows() && rows * rowH > lb[3], "the full catalog overflows the list: " + rows + " rows of " + rowH
                + " px in " + lb[3] + " px");

        Layout rec = record(ctx, ui);
        BufferedImage top = shot(ctx, "hengst-51-settings-full-catalog");
        Text hud = rec.find(i18n(ctx, core, "hengst.group.hud"), GROUP_TEXT);
        check(hud != null && hud.y >= lb[1] && hud.x >= lb[0] && hud.x < lb[0] + lb[2], "HUD heading in the list, texts "
                + rec.texts);
        int grey = greyTextPixels(top, hud.rect());
        check(grey >= MIN_TEXT_PIXELS, "HUD heading drawn: grey text pixels " + grey);
        int[] thumb = rec.fill(THUMB);
        int[] track = rec.fill(TRACK);
        check(thumb != null && track != null && thumb[1] == lb[1], "scroll indicator at the top, fills " + rec.fillList());
        int[] trackBelow = {track[0], thumb[3], track[2], track[3]};
        double lumaThumb = meanLuma(top, thumb);
        double lumaTrack = meanLuma(top, trackBelow);
        check(lumaThumb > lumaTrack + 30, "scroll indicator drawn: thumb luma " + lumaThumb + ", track " + lumaTrack);
        pass("settings: the full catalog overflows the list, the HUD heading and the scroll indicator are drawn (pixels)");

        double[] s = scale(ctx, rec);
        int cx = lb[0] + lb[2] / 2;
        int cy = lb[1] + lb[3] / 2;
        input.setCursorPos((cx + 0.5) * s[0], (cy + 0.5) * s[1]);
        ctx.waitTick();
        input.scroll(-1.0);
        ctx.waitTick();
        int offset = ctx.computeOnClient(mc -> list.offset());
        check(offset == rowH, "one notch towards the user scrolls by one row (" + rowH + "), offset " + offset);
        BufferedImage scrolled = shot(ctx, "hengst-52-settings-scrolled");
        int[] listRect = {lb[0], lb[1], lb[0] + lb[2], lb[1] + lb[3]};
        check(diffFraction(top, scrolled, listRect) > 0.01, "the scrolled list is drawn differently");
        check(record(ctx, ui).fill(THUMB)[1] > lb[1], "the thumb moved down");
        input.scroll(1.0);
        ctx.waitTick();
        check(ctx.computeOnClient(mc -> list.offset()) == 0, "one notch away from the user scrolls back up");
        pass("settings: one wheel notch moves the list by one row (" + rowH + " px) and back");

        selectStopwatch(ctx, core, ui, list);
        // The stopwatch switched on with a real click on its row's switch; it applies when the settings close.
        Widget stopwatchRow = featureRow((List<?>) field(list, "widgets"), "stopwatch");
        clickGui(ctx, s, stopwatchRow.x + constant(SettingsScreen.class, "TOGGLE_W") / 2, stopwatchRow.y + stopwatchRow.h / 2);
        SettingsModel settingsModel = (SettingsModel) field(ui, "model");
        check(ctx.computeOnClient(mc -> settingsModel.config("stopwatch").enabled()), "a click on the stopwatch's switch"
                + " turns it on in the settings");
        rec = record(ctx, ui);
        Text comfort = rec.find(i18n(ctx, core, "hengst.group.comfort"), GROUP_TEXT);
        check(comfort != null, "Comfort heading drawn at the end of the list, texts " + rec.texts);
        check(rec.fill(THUMB)[3] == lb[1] + lb[3], "the thumb at the bottom at the end of the list");
        ScrollList options = (ScrollList) field(ui, "optionList");
        List<?> optionRows = (List<?>) field(options, "widgets");
        Feature stopwatch = core.registry().byId("stopwatch");
        check(optionRows.size() == 3 + stopwatch.options().size(), "stopwatch options: background, shadow, colour and "
                + stopwatch.options().size() + " own, got " + optionRows.size());
        for (String key : new String[] {"hengst.option.background", "hengst.option.shadow", "hengst.option.text_color"}) {
            String label = i18n(ctx, core, key);
            check(rec.findStart(label) != null, "stopwatch style row '" + label + "' drawn, texts " + rec.texts);
        }
        BufferedImage selected = shot(ctx, "hengst-53-settings-stopwatch");
        checkHint(ctx, core, ui, rec, selected, false);
        pass("settings: Comfort heading at the end of the list, the stopwatch shows its HUD style rows and its muted hint");

        // Option rows react where they are drawn: background (first) and tenths (last), each clicked twice.
        OptionSpec tenths = stopwatch.options().get(0);
        int[] ob = bounds(options);
        for (int index : new int[] {0, optionRows.size() - 1}) {
            Widget w = (Widget) optionRows.get(index);
            check(w.y >= ob[1] && w.y + w.h <= ob[1] + ob[3], "option row " + index + " inside the option list");
            boolean was = ctx.computeOnClient(mc -> index == 0 ? core.activeProfile().module("stopwatch").background()
                    : (Boolean) core.activeProfile().module("stopwatch").option(tenths));
            for (int click = 0; click < 2; click++) {
                clickGui(ctx, s, w.x + w.w - 6, w.y + w.h / 2);
                boolean now = ctx.computeOnClient(mc -> index == 0 ? core.activeProfile().module("stopwatch").background()
                        : (Boolean) core.activeProfile().module("stopwatch").option(tenths));
                check(now == (click == 0 ? !was : was), "click " + (click + 1) + " on option row " + index + ": " + now);
            }
        }
        pass("settings: the stopwatch's option rows below the hint react to clicks where they are drawn");

        // A narrower window: the hint wraps to the option panel and the option list moves down below it.
        int[] window = ctx.computeOnClient(mc -> new int[] {mc.getWindow().getScreenWidth(), mc.getWindow().getScreenHeight()});
        input.resizeWindow(640, 480);
        try {
            ctx.waitTicks(2);
            Layout narrow = record(ctx, ui);
            BufferedImage wrapped = shot(ctx, "hengst-54-settings-hint-wrapped");
            checkHint(ctx, core, ui, narrow, wrapped, true);
        } finally {
            input.resizeWindow(window[0], window[1]);
            ctx.waitTicks(2);
        }
        pass("settings: in a 640x480 window the hint wraps to the option panel (muted, pixels)");

        input.pressKey(InputConstants.KEY_ESCAPE);
        ctx.waitForScreen(null);
        Feature sw = core.registry().byId("stopwatch");
        check(ctx.computeOnClient(mc -> FeatureState.on(sw.number())) && layout(ctx).has("stopwatch"), "the stopwatch"
                + " switched on in the settings is on and laid out after closing them");
        int[] swRect = shownByPixels(ctx, "stopwatch", "hengst-54a-stopwatch-from-settings");
        offAndGone(ctx, core, "stopwatch", swRect, "hengst-54b-stopwatch-off-again");
        pass("settings: the stopwatch switched on with a click on its switch is drawn after closing (pixels)");
    }

    /** Scrolls the feature list to its end with the wheel and clicks the stopwatch's row (beside its switch). */
    private static void selectStopwatch(ClientGameTestContext ctx, HengstCore core, UiScreen ui, ScrollList list) {
        TestInput input = ctx.getInput();
        int[] lb = bounds(list);
        int rowH = (Integer) field(list, "rowHeight");
        List<?> rows = (List<?>) field(list, "widgets");
        Layout rec = record(ctx, ui);
        double[] s = scale(ctx, rec);
        input.setCursorPos((lb[0] + lb[2] / 2 + 0.5) * s[0], (lb[1] + lb[3] / 2 + 0.5) * s[1]);
        ctx.waitTick();
        for (int i = 0; i < rows.size(); i++) {
            input.scroll(-1.0);
        }
        ctx.waitTick();
        int expected = (rows.size() - lb[3] / rowH) * rowH;
        int offset = ctx.computeOnClient(mc -> list.offset());
        check(offset == expected, "wheel to the end of the list: offset " + offset + ", expected " + expected);
        record(ctx, ui);                                       // places the rows as drawn
        Widget row = featureRow(rows, "stopwatch");
        check(row.y >= lb[1] && row.y + row.h <= lb[1] + lb[3], "stopwatch row inside the list at the end");
        clickGui(ctx, s, row.x + row.w * 2 / 3, row.y + row.h / 2);
        SettingsModel model = (SettingsModel) field(ui, "model");
        String selected = ctx.computeOnClient(mc -> model.selected() == null ? null : model.selected().id());
        check("stopwatch".equals(selected), "a click on the stopwatch row selects it, got " + selected);
    }

    /**
     * The selected feature's hint: muted grey, at the option panel's left edge from the list top, every line within
     * the panel width, together the whole hint, and the option list starting below it; grey text pixels on screen.
     */
    private static void checkHint(ClientGameTestContext ctx, HengstCore core, UiScreen ui, Layout rec, BufferedImage img,
            boolean mustWrap) {
        String hint = i18n(ctx, core, "hengst.hint.stopwatch");
        String[] lines = (String[]) field(ui, "hintLines");
        int optX = (Integer) field(ui, "optX");
        int optW = (Integer) field(ui, "optW");
        int lineHeight = (Integer) field(ui, "lineHeight");
        int listY = staticInt(ui, "LIST_Y");
        int gap = staticInt(ui, "GAP");
        check(String.join(" ", lines).equals(hint), "hint lines " + Arrays.toString(lines) + " vs '" + hint + "'");
        check(!mustWrap || lines.length >= 2, "the hint wraps in a narrow panel (" + optW + " px): "
                + Arrays.toString(lines));
        List<Text> drawn = new ArrayList<>();
        for (Text t : rec.texts) {
            if (t.argb == NOTICE && t.x == optX && t.y >= listY && t.y < listY + lines.length * lineHeight) {
                drawn.add(t);
            }
        }
        check(drawn.size() == lines.length, "hint drawn as " + lines.length + " muted lines, got " + drawn);
        for (int i = 0; i < lines.length; i++) {
            Text t = drawn.get(i);
            check(t.text.equals(lines[i]) && t.w <= optW, "hint line " + i + " '" + t.text + "' width " + t.w
                    + " within " + optW);
            int grey = greyTextPixels(img, t.rect());
            check(grey >= MIN_TEXT_PIXELS, "hint line " + i + " drawn: grey text pixels " + grey);
            check(textPixels(img, t.rect()) < grey, "hint line " + i + " is muted, not white");
        }
        int optionsY = bounds((ScrollList) field(ui, "optionList"))[1];
        check(optionsY == listY + lines.length * lineHeight + gap, "option list starts below the hint at " + optionsY);
    }

    /**
     * The game language switched to German and back in a running world: the direction letter for east, the group
     * heading and the stopwatch hint in the settings, and the key binding names in the controls screen follow
     * without a restart.
     */
    private static void languageSwitch(ClientGameTestContext ctx, HengstCore core) {
        face(ctx, -90f);
        setEnabled(ctx, core, "direction", true);
        waitText(ctx, core, "direction", "E 90°"::equals, 2_000, "direction facing east in English");
        controls(ctx, "en_us");
        try {
            setLanguage(ctx, core, "de_de", "Komfort");
            waitText(ctx, core, "direction", "O 90°"::equals, 2_000, "direction facing east in German");
            shownByPixels(ctx, "direction", "hengst-55-direction-german");
            pass("language: German switched in a running world, direction shows 'O 90°' (pixels)");

            KeyMapping settingsKey = ctx.computeOnClient(mc -> mapping(mc, "key.hengst.settings"));
            ctx.getInput().pressKey(settingsKey);
            ctx.waitForScreen(ModernScreenHost.class);
            UiScreen ui = settingsUi(ctx);
            selectStopwatch(ctx, core, ui, (ScrollList) field(ui, "featureList"));
            Layout rec = record(ctx, ui);
            check(rec.find("Komfort", GROUP_TEXT) != null, "German group heading 'Komfort', texts " + rec.texts);
            check(i18n(ctx, core, "hengst.hint.stopwatch").startsWith("Tasten"), "German hint loaded");
            checkHint(ctx, core, ui, rec, shot(ctx, "hengst-56-settings-german"), false);
            ctx.getInput().pressKey(InputConstants.KEY_ESCAPE);
            ctx.waitForScreen(null);
            pass("language: the settings show the German heading 'Komfort' and the German stopwatch hint");

            controls(ctx, "de_de");
        } finally {
            setLanguage(ctx, core, "en_us", "Comfort");
        }
        waitText(ctx, core, "direction", "E 90°"::equals, 2_000, "direction facing east in English again");
        setEnabled(ctx, core, "direction", false);
        pass("language: back to English without a restart, direction shows 'E 90°' again");
    }

    /**
     * The stopwatch's two bindings and the mod's category in the controls screen with translated names: through the
     * game's own translation call (the one KeyBindsList uses) and on the screen's entries.
     */
    private static void controls(ClientGameTestContext ctx, String lang) {
        Map<String, String> names = langFile(lang);
        Set<String> expected = new HashSet<>();
        for (String key : HENGST_KEYS) {
            String shown = ctx.computeOnClient(mc -> Component.translatable(key).getString());
            check(shown.equals(names.get(key)) && !shown.equals(key), lang + ": " + key + " shown as '" + shown + "'");
            expected.add(key + "=" + shown);
        }
        String category = ctx.computeOnClient(mc -> ModernKeys.CATEGORY.label().getString());
        check(category.equals(names.get("key.category.hengst.main")), lang + ": category shown as '" + category + "'");

        ctx.setScreen(() -> new KeyBindsScreen(null, Minecraft.getInstance().options));
        ctx.waitForScreen(KeyBindsScreen.class);
        List<String> entries = ctx.computeOnClient(mc -> controlsEntries(ModernPlatform.currentScreen(mc)));
        int at = entries.indexOf("C:" + category);
        check(at >= 0, lang + ": controls screen has the category '" + category + "': " + entries);
        Set<String> listed = new HashSet<>();
        for (int i = at + 1; i < entries.size() && entries.get(i).startsWith("K:"); i++) {
            listed.add(entries.get(i).substring(2));
        }
        check(listed.equals(expected), lang + ": controls entries under '" + category + "' " + listed + ", expected "
                + expected);
        shot(ctx, "hengst-57-controls-" + lang);
        ctx.setScreen(() -> null);
        ctx.waitForScreen(null);
        pass(lang + ": the controls screen lists the mod's category and its four bindings translated " + listed);
    }

    /** "C:name" for a category heading, "K:translation key=shown name" for a binding, in screen order. */
    private static List<String> controlsEntries(Object screen) {
        List<String> out = new ArrayList<>();
        for (GuiEventListener child : ((Screen) screen).children()) {
            if (!(child instanceof KeyBindsList list)) {
                continue;
            }
            for (KeyBindsList.Entry e : list.children()) {
                if (e instanceof KeyBindsList.CategoryEntry) {
                    out.add("C:" + ((AbstractWidget) e.children().get(0)).getMessage().getString());
                } else if (e instanceof KeyBindsList.KeyEntry) {
                    // By type, not by name: in a production run (intermediary names on 1.21.11) the fields of game
                    // classes are not called "key" and "name".
                    KeyMapping key = (KeyMapping) fieldOfType(e, KeyMapping.class);
                    Component name = nameOf(e, key);
                    out.add("K:" + key.getName() + "=" + name.getString());
                }
            }
        }
        return out;
    }

    /**
     * The mod's per-tick key dispatch ({@code ModernKeys.tick}: the keys and the focus rule) in steady state, called once
     * more per tick from a tick listener with the measurement around that call only. The game's window-active query
     * ({@code Minecraft.isWindowActive()}, the argument {@code ModernKeys.dispatch(Minecraft)} passes) is measured
     * separately and reported, not asserted: it is game code. 100 calls warm up, then two windows of 300 calls; every
     * allocating call is logged with its index and the focus value. The second window starts no earlier than 90 s JVM
     * uptime and must have no allocating call.
     */
    // A single dispatch call of 1192 bytes was seen once per game start at 67 to 80 s JVM uptime (cause unknown, spec
    // 8.8). The asserted window starts no earlier than this; inside it the check stays exact.
    private static final long ASSERTED_WINDOW_UPTIME_MS = 90_000;

    private static void waitForSteadyJvm(ClientGameTestContext ctx) {
        long before = ManagementFactory.getRuntimeMXBean().getUptime();
        long wait = Math.max(0, ASSERTED_WINDOW_UPTIME_MS - before);
        if (wait > 0) {
            waitRealTime(ctx, wait);
        }
        LOGGER.info("[hengst-gametest] dispatch: asserted window starts at JVM uptime {} ms (waited {} ms for {} ms)",
                ManagementFactory.getRuntimeMXBean().getUptime(), wait, ASSERTED_WINDOW_UPTIME_MS);
    }

    private static void dispatchAllocation(ClientGameTestContext ctx) {
        long[][] windows = new long[2][];
        for (int round = 0; round < 3; round++) {                 // round 0: 100 calls to warm up
            int calls = round == 0 ? 100 : 300;
            if (round == 2) {
                waitForSteadyJvm(ctx);
            }
            ctx.runOnClient(mc -> {
                dispatchBytes = 0;
                gameBytes = 0;
                emptyBytes = 0;
                allocatingCalls = 0;
                allocLog.setLength(0);
                focusFlips = 0;
                measured = 0;
                measureLeft = calls;
            });
            long deadline = System.nanoTime() + 60_000_000_000L;
            ctx.waitFor(mc -> measureLeft == 0 || System.nanoTime() > deadline, ClientGameTestContext.NO_TIMEOUT);
            int[] state = ctx.computeOnClient(mc -> new int[] {measureLeft, measured, tickCalls});
            check(state[0] == 0, "measured dispatch calls within 60 s: left " + state[0] + ", measured " + state[1]
                    + ", tick listener calls " + state[2] + ", error " + measureError);
            String log = ctx.computeOnClient(mc -> allocLog.toString());
            long[] r = ctx.computeOnClient(mc -> new long[] {dispatchBytes, gameBytes, emptyBytes, allocatingCalls,
                measured, focusFlips});
            LOGGER.info("[hengst-gametest] dispatch window {} ({} calls, {} focus changes): mod {} bytes in {} calls,"
                    + " game window query {} bytes, empty measurement {} bytes; allocating calls [index focused mod/game"
                    + " bytes, call time, collections during the call, JVM uptime]:{}", round, r[4], r[5], r[0], r[3], r[1], r[2], log.isEmpty() ? " none" : log);
            if (round > 0) {
                windows[round - 1] = r;
            }
        }
        // The focus-loss branch, which a window that keeps its focus never takes: 100 losses to warm up, 300 measured.
        long[] flips = ctx.computeOnClient(mc -> {
            HengstCore core = HengstModernMod.core();
            KeyMapping[] mappings = ModernKeys.mappings();
            long[] out = new long[2];
            for (int i = 0; i < 400; i++) {
                long a = THREADS.getCurrentThreadAllocatedBytes();
                ModernKeys.tick(core, mappings, false);
                ModernKeys.tick(core, mappings, true);
                long used = THREADS.getCurrentThreadAllocatedBytes() - a;
                if (i >= 100) {
                    out[0] += used;
                    out[1] += used != 0 ? 1 : 0;
                }
            }
            ModernKeys.tick(core, mappings, mc.isWindowActive());
            return out;
        });
        check(flips[0] == 0, "300 focus losses and returns allocated " + flips[0] + " bytes in " + flips[1] + " calls");
        check(windows[1][3] == 0 && windows[1][0] == 0, "ModernKeys.tick allocated " + windows[1][0]
                + " bytes in the second window of 300 ticks, in " + windows[1][3] + " calls");
        pass("the mod's per-tick key dispatch allocates nothing in the second window of 300 ticks (first window "
                + windows[0][0] + " bytes in " + windows[0][3] + " calls; the game's window query " + windows[0][1]
                + " and " + windows[1][1] + " bytes), nor in 300 focus losses and returns");
    }

    private static final long MEMORY_SLACK_MB = 64;
    // The core's own colours, read from its constants (private there).
    private static final int GROUP_TEXT = constant(SettingsScreen.class, "GROUP_TEXT");
    private static final int NOTICE = constant(SettingsScreen.class, "NOTICE");
    private static final int THUMB = constant(ScrollList.class, "THUMB");
    private static final int TRACK = constant(ScrollList.class, "TRACK");
    private static final com.sun.management.ThreadMXBean THREADS =
            (com.sun.management.ThreadMXBean) ManagementFactory.getThreadMXBean();
    // Written and read on the client thread.
    private static int measureLeft;
    private static int measured;
    private static long dispatchBytes;
    private static long gameBytes;
    private static long emptyBytes;
    private static int tickCalls;
    private static long allocatingCalls;
    private static final StringBuilder allocLog = new StringBuilder();
    private static int focusFlips;
    // The collectors, read around each measured call: a collection in the call is logged with an allocating call.
    private static final java.lang.management.GarbageCollectorMXBean[] GCS =
            ManagementFactory.getGarbageCollectorMXBeans().toArray(new java.lang.management.GarbageCollectorMXBean[0]);

    private static long collections() {
        long n = 0;
        for (int i = 0; i < GCS.length; i++) {
            n += GCS[i].getCollectionCount();
        }
        return n;
    }
    private static boolean lastFocused = true;
    private static volatile Throwable measureError;

    /** End of every client tick (registered after the mod's own listener): the measured extra dispatch call. */
    private static void measureDispatch(Minecraft mc) {
        tickCalls++;
        if (measureLeft <= 0) {
            return;
        }
        HengstCore core = HengstModernMod.core();
        KeyMapping[] mappings = ModernKeys.mappings();
        long gc0 = collections();
        long t0 = System.nanoTime();
        long a = THREADS.getCurrentThreadAllocatedBytes();
        long b = THREADS.getCurrentThreadAllocatedBytes();
        boolean focused = mc.isWindowActive();
        long c = THREADS.getCurrentThreadAllocatedBytes();
        if (focused != lastFocused) {
            focusFlips++;
            lastFocused = focused;
        }
        try {
            ModernKeys.tick(core, mappings, focused);
        } catch (Throwable e) {
            measureError = e;
            measureLeft = 0;
            return;
        }
        long d = THREADS.getCurrentThreadAllocatedBytes();
        long micros = (System.nanoTime() - t0) / 1000;
        long gcs = collections() - gc0;
        emptyBytes += b - a;
        gameBytes += c - b;
        dispatchBytes += d - c;
        if (d != c || c != b) {
            if (d != c) {
                allocatingCalls++;
            }
            if (allocLog.length() < 2000) {
                allocLog.append(" [").append(measured).append(' ').append(focused).append(' ').append(d - c)
                        .append('/').append(c - b).append(", ").append(micros).append(" us, ").append(gcs).append(" collections, uptime ")
                        .append(ManagementFactory.getRuntimeMXBean().getUptime()).append(" ms]");
            }
        }
        measured++;
        measureLeft--;
    }

    // ---- Task 16: the core's cost per module per frame, in the real game ----

    // 500 frames to settle (JIT, caches), then 2000 measured frames per case; 300 frames for the allocation window.
    private static final int COST_WARMUP = 500;
    private static final int COST_FRAMES = 2000;
    private static final int ALLOC_FRAMES = 300;
    // Frames drawn through the counting canvas before the allocation window counts: a new canvas class reaching the
    // compiled frame path makes the JIT deoptimise it once (seen on 1.8.9 as 896 bytes in one frame).
    private static final int ALLOC_SETTLE = 100;
    private static final int MAX_MODULES = 11;
    private static final String[] M0_IDS = {"fps", "cps", "keystrokes", "coordinates"};
    private static final String[] NEW_IDS = {"direction", "ping", "clock", "server_address", "memory", "speed",
        "stopwatch"};
    // Budgets of the measured quantity (spec 8.8), median per version, in ns. The strict judgement (four M0 modules on
    // the median of at least three runs, each M1a module's marginal cost at 10 us) is made by the reader of the runs;
    // the suite fails only where noise on a loaded machine cannot reach: all eleven above the budget, a marginal cost
    // above twice its budget, the four M0 modules above twice theirs. Each strict miss is logged as a WARN line.
    private static final long M0_BUDGET = 50_000;
    private static final long MODULE_BUDGET = 10_000;
    private static final long ALL_BUDGET = 120_000;

    // Written by the HUD probe on the client thread, read there (computeOnClient) after the window closed.
    private static final long[] COST_TIMES = new long[MAX_MODULES];
    private static final long[] COST_SAMPLES = new long[COST_FRAMES * MAX_MODULES];
    private static final long[] HOOK_SAMPLES = new long[COST_FRAMES];
    private static final long[] EMPTY_SAMPLES = new long[COST_FRAMES];
    private static int costLeft;
    private static int costFrame;
    private static int costUndrawn;
    private static long costFirst;
    private static long costLast;
    private static int allocLeft;
    private static long allocHook;
    private static long allocMod;
    private static long allocCanvas;
    private static int allocFramesWithBytes;
    private static int allocFramesWithBytesNoNewText;
    private static int allocTick0;
    private static final StringBuilder ALLOC_DIAG = new StringBuilder();
    private static final int[] TEXT_CHANGES = new int[MAX_MODULES];
    private static final String[] LAST_TEXT = new String[MAX_MODULES];

    /** Percentile statistics of one series, in ns. */
    private static long[] stats(long[] values) {
        long[] v = values.clone();
        Arrays.sort(v);
        int n = v.length;
        long median = n % 2 == 1 ? v[n / 2] : (v[n / 2 - 1] + v[n / 2]) / 2;
        long p95 = v[Math.max(0, (int) Math.ceil(0.95 * n) - 1)];
        return new long[] {median, p95, v[n - 1]};
    }

    private static String fmt(long[] s) {
        return "median " + s[0] + " ns p95 " + s[1] + " ns max " + s[2] + " ns";
    }

    /** The smallest positive value in the series, or 0: the observed resolution of System.nanoTime. */
    private static long resolution(long[] values, long current) {
        long r = current;
        for (long x : values) {
            if (x > 0 && (r == 0 || x < r)) {
                r = x;
            }
        }
        return r;
    }

    /** Per-frame sum over the modules at the given indexes. */
    private static long[] sums(long[] samples, int[] indexes) {
        long[] out = new long[COST_FRAMES];
        for (int f = 0; f < COST_FRAMES; f++) {
            long sum = 0;
            for (int i : indexes) {
                sum += samples[f * MAX_MODULES + i];
            }
            out[f] = sum;
        }
        return out;
    }

    private static long[] column(long[] samples, int index) {
        long[] out = new long[COST_FRAMES];
        for (int f = 0; f < COST_FRAMES; f++) {
            out[f] = samples[f * MAX_MODULES + index];
        }
        return out;
    }

    private static int[] indexesOf(String[] order, String[] ids) {
        int[] out = new int[ids.length];
        for (int k = 0; k < ids.length; k++) {
            out[k] = Arrays.asList(order).indexOf(ids[k]);
            check(out[k] >= 0, ids[k] + " active in the cost case, active " + Arrays.toString(order));
        }
        return out;
    }

    /**
     * The time the core spends per module and frame ({@code HudRenderer.setRenderTimes}: size, placement, transform and
     * render of each active module, read with System.nanoTime around it). On this version the module submits its draw
     * calls into the GUI render state and the game draws them later, so the number is the submit cost, not the GPU or
     * draw cost. Cases: the four M0 modules (sum per frame), each of the seven M1a text modules alone, all eleven (sum),
     * all off (hook time only). Each case: 500 frames to settle, 2000 measured. Logged per case with median, 95th
     * percentile and maximum; asserted: the medians against the budgets of spec 8.8. Then 300 frames with all eleven
     * on and the allocation of the core's frame call. The game's own options and frame rate are kept.
     */
    private static void costMeasurement(ClientGameTestContext ctx, HengstCore core) {
        long[] res = new long[1];
        String[] all = new String[M0_IDS.length + NEW_IDS.length];
        System.arraycopy(M0_IDS, 0, all, 0, M0_IDS.length);
        System.arraycopy(NEW_IDS, 0, all, M0_IDS.length, NEW_IDS.length);
        long m0;
        long allMedian;
        long[] alone = new long[NEW_IDS.length];
        long[] marginal = new long[NEW_IDS.length];
        // The game limits the frame rate while nobody touches the input ("afk": 30, later 10 frames per second), which
        // a test that does not move would hit; a player at the keyboard does not. Off for the measurement, restored.
        InactivityFpsLimit inactivity = ctx.computeOnClient(mc -> mc.options.inactivityFpsLimit().get());
        ctx.runOnClient(mc -> {
            mc.options.inactivityFpsLimit().set(InactivityFpsLimit.MINIMIZED);
            core.renderer().setRenderTimes(COST_TIMES);
        });
        try {
            m0 = costCase(ctx, core, "m0", M0_IDS, M0_IDS, null, res)[0];
            for (int i = 0; i < NEW_IDS.length; i++) {
                alone[i] = costCase(ctx, core, NEW_IDS[i], new String[] {NEW_IDS[i]}, new String[] {NEW_IDS[i]}, null,
                        res)[0];
            }
            for (int i = 0; i < NEW_IDS.length; i++) {
                String[] on = Arrays.copyOf(M0_IDS, M0_IDS.length + 1);
                on[M0_IDS.length] = NEW_IDS[i];
                long[] r = costCase(ctx, core, "m0+" + NEW_IDS[i], on, on, NEW_IDS[i], res);
                marginal[i] = r[1];
                LOGGER.info("[hengst-gametest] COST marginal {} median {} ns (its own time with the four M0 modules on,"
                        + " drawn after them; case total {} ns minus the m0 case {} ns = {} ns)", NEW_IDS[i], r[1], r[0],
                        m0, r[0] - m0);
            }
            allMedian = costCase(ctx, core, "all", all, all, null, res)[0];
            costCase(ctx, core, "off", new String[0], new String[0], null, res);
            profileCase(ctx, core, "direction", new String[] {"direction"});
            profileCase(ctx, core, "all", all);
            allocationWindow(ctx, core, all);
            ctx.runOnClient(mc -> core.renderer().setRenderTimes(null));
            for (int i : cheapestAndDearest(marginal)) {
                pairedCheck(ctx, core, all, NEW_IDS[i], marginal[i]);
            }
        } finally {
            ctx.runOnClient(mc -> {
                mc.options.inactivityFpsLimit().set(inactivity);
                core.renderer().setRenderTimes(null);
                for (String id : all) {
                    core.activeProfile().module(id).setEnabled(Arrays.asList(M0_IDS).contains(id));
                }
                core.applyConfigForTest();
            });
        }
        LOGGER.info("[hengst-gametest] COST timer resolution observed (smallest positive reading): {} ns", res[0]);
        StringBuilder verdict = new StringBuilder();
        List<String> failures = new ArrayList<>();
        if (m0 > M0_BUDGET) {
            LOGGER.warn("[hengst-gametest] WARN COST m0 median {} ns above the {} ns budget (judged on the median of at least three runs, spec 8.8)", m0, M0_BUDGET);
        }
        if (m0 > 2 * M0_BUDGET) {
            failures.add("m0 median " + m0 + " ns > " + 2 * M0_BUDGET + " ns");
        }
        if (allMedian > ALL_BUDGET) {
            failures.add("all median " + allMedian + " ns > " + ALL_BUDGET + " ns");
        }
        for (int i = 0; i < NEW_IDS.length; i++) {
            verdict.append(i == 0 ? "" : ", ").append(NEW_IDS[i]).append(' ').append(marginal[i]).append(" (alone ")
                    .append(alone[i]).append(')');
            if (marginal[i] > MODULE_BUDGET) {
                LOGGER.warn("[hengst-gametest] WARN COST marginal {} median {} ns above the {} ns budget", NEW_IDS[i], marginal[i], MODULE_BUDGET);
            }
            if (marginal[i] > 2 * MODULE_BUDGET) {
                failures.add("marginal " + NEW_IDS[i] + " median " + marginal[i] + " ns > " + 2 * MODULE_BUDGET + " ns");
            }
        }
        LOGGER.info("[hengst-gametest] COST verdict: m0 {} ns (budget {}), all {} ns (budget {}), marginal per module ns: {}", m0, M0_BUDGET, allMedian, ALL_BUDGET, verdict);
        check(failures.isEmpty(), "cost thresholds missed: " + failures);
        pass("cost per module per frame (submit time): all eleven within " + ALL_BUDGET + " ns, each marginal cost"
                + " within twice " + MODULE_BUDGET + " ns, the four M0 modules within twice " + M0_BUDGET
                + " ns (strict comparison in the COST verdict and WARN lines)");
    }

    /** Switches exactly the given modules on (all others of the catalog off) through the core. */
    private static String[] enableOnly(ClientGameTestContext ctx, HengstCore core, String[] all, String[] on) {
        return ctx.computeOnClient(mc -> {
            for (String id : all) {
                core.activeProfile().module(id).setEnabled(Arrays.asList(on).contains(id));
            }
            core.applyConfigForTest();
            HudModule[] active = core.renderer().activeModules();
            String[] ids = new String[active.length];
            for (int i = 0; i < active.length; i++) {
                ids[i] = active[i].id();
            }
            return ids;
        });
    }

    /**
     * One case; returns the median of the summed modules per frame (the hook time without modules) and, with
     * {@code marginal} set, that module's own median ({@code -1} otherwise).
     */
    private static long[] costCase(ClientGameTestContext ctx, HengstCore core, String name, String[] on,
            String[] summed, String marginal, long[] res) {
        String[] all = new String[M0_IDS.length + NEW_IDS.length];
        System.arraycopy(M0_IDS, 0, all, 0, M0_IDS.length);
        System.arraycopy(NEW_IDS, 0, all, M0_IDS.length, NEW_IDS.length);
        String[] order = enableOnly(ctx, core, all, on);
        check(order.length == on.length, name + ": active " + Arrays.toString(order));
        int start = ctx.computeOnClient(mc -> hudCalls);
        long deadline = System.nanoTime() + 120_000_000_000L;
        ctx.waitFor(mc -> hudCalls >= start + COST_WARMUP || System.nanoTime() > deadline, ClientGameTestContext.NO_TIMEOUT);
        check(ctx.computeOnClient(mc -> hudCalls) >= start + COST_WARMUP, name + ": " + COST_WARMUP
                + " frames to settle within 120 s");
        ctx.runOnClient(mc -> {
            costFrame = 0;
            costUndrawn = 0;
            costLeft = COST_FRAMES;
        });
        long deadline2 = System.nanoTime() + 120_000_000_000L;
        ctx.waitFor(mc -> costLeft == 0 || System.nanoTime() > deadline2, ClientGameTestContext.NO_TIMEOUT);
        long[][] data = ctx.computeOnClient(mc -> new long[][] {COST_SAMPLES.clone(), HOOK_SAMPLES.clone(),
            EMPTY_SAMPLES.clone(), {costLeft, costUndrawn, costLast - costFirst}});
        check(data[3][0] == 0, name + ": " + COST_FRAMES + " measured frames within 120 s, " + data[3][0] + " left");
        check(data[3][1] == 0, name + ": the core drew in every measured frame, " + data[3][1] + " frames without");
        checkPlayerAlive(ctx, name);
        long[] hook = stats(data[1]);
        long[] pair = stats(data[2]);
        res[0] = resolution(data[2], resolution(data[1], res[0]));
        StringBuilder modules = new StringBuilder();
        for (int i = 0; i < order.length; i++) {
            long[] column = column(data[0], i);
            res[0] = resolution(column, res[0]);
            long[] s = stats(column);
            modules.append(i == 0 ? "" : ", ").append(order[i]).append(' ').append(s[0]).append('/').append(s[1])
                    .append('/').append(s[2]);
        }
        double fps = (COST_FRAMES - 1) / ((data[3][data[3].length - 1]) / 1e9);
        String tail = String.format(java.util.Locale.ROOT, "; %.0f frames per second in the window", fps) + "; hook " + fmt(hook) + "; empty probe pair " + fmt(pair) + "; per module median/p95/max ns: "
                + (modules.length() == 0 ? "none" : modules.toString());
        if (summed.length == 0) {
            LOGGER.info("[hengst-gametest] COST {} median {} ns (hook time, no module active{})", name, hook[0], tail);
            return new long[] {hook[0], -1};
        }
        long[] sum = stats(sums(data[0], indexesOf(order, summed)));
        LOGGER.info("[hengst-gametest] COST {} median {} ns ({}, sum of {} modules per frame{})", name, sum[0], fmt(sum),
                summed.length, tail);
        if (summed.length == all.length) {
            long[] m0 = stats(sums(data[0], indexesOf(order, M0_IDS)));
            LOGGER.info("[hengst-gametest] COST all-m0part median {} ns ({}, the four M0 modules with all eleven on)",
                    m0[0], fmt(m0));
        }
        long own = marginal == null ? -1 : stats(column(data[0], indexesOf(order, new String[] {marginal})[0]))[0];
        return new long[] {sum[0], own};
    }

    /** A measured case is valid only with a living player (a dead player's screen is not the HUD being measured). */
    private static void checkPlayerAlive(ClientGameTestContext ctx, String name) {
        float health = ctx.computeOnClient(mc -> mc.player != null && mc.player.isAlive() ? mc.player.getHealth() : -1F);
        check(health > 0, name + ": player alive after the window, health " + health);
        LOGGER.info("[hengst-gametest] COST {} player alive, health {}", name, health);
    }

    // Cross-check of the marginal cost without the per-module timer (render times off): windows A (the four M0
    // modules) and B (A plus one module) alternate, each 60 frames to settle and 120 measured; per window the median of
    // the hook time; per pair B minus A.
    private static final int PAIRS = 24;
    private static final int PAIR_SETTLE = 60;
    private static final int PAIR_FRAMES = 120;
    private static final long[] PAIR_SAMPLES = new long[PAIR_FRAMES];
    private static int pairLeft;
    private static int pairFrame;

    /** Indexes of the drawn module (own time above 0) with the smallest and with the largest marginal cost. */
    private static int[] cheapestAndDearest(long[] marginal) {
        int lo = -1;
        int hi = -1;
        for (int i = 0; i < marginal.length; i++) {
            if (marginal[i] <= 0) {
                continue;                                         // ping and server address draw nothing here
            }
            if (lo < 0 || marginal[i] < marginal[lo]) {
                lo = i;
            }
            if (hi < 0 || marginal[i] > marginal[hi]) {
                hi = i;
            }
        }
        check(lo >= 0, "a drawn module for the paired check");
        return lo == hi ? new int[] {lo} : new int[] {lo, hi};
    }

    private static long pairWindow(ClientGameTestContext ctx, HengstCore core, String[] all, String[] on) {
        String[] order = enableOnly(ctx, core, all, on);
        check(order.length == on.length, "paired window: active " + Arrays.toString(order));
        int start = ctx.computeOnClient(mc -> hudCalls);
        long deadline = System.nanoTime() + 60_000_000_000L;
        ctx.waitFor(mc -> hudCalls >= start + PAIR_SETTLE || System.nanoTime() > deadline, ClientGameTestContext.NO_TIMEOUT);
        ctx.runOnClient(mc -> {
            pairFrame = 0;
            pairLeft = PAIR_FRAMES;
        });
        long deadline2 = System.nanoTime() + 60_000_000_000L;
        ctx.waitFor(mc -> pairLeft == 0 || System.nanoTime() > deadline2, ClientGameTestContext.NO_TIMEOUT);
        check(ctx.computeOnClient(mc -> pairLeft) == 0, "paired window: " + PAIR_FRAMES + " frames within 60 s");
        return stats(ctx.computeOnClient(mc -> PAIR_SAMPLES.clone()))[0];
    }

    /**
     * Paired alternating windows A, B, A, B, ... (logged, not asserted): the median over the pairs of B minus A, next
     * to the module's own time from the per-module timer.
     */
    private static void pairedCheck(ClientGameTestContext ctx, HengstCore core, String[] all, String module,
            long ownMedian) {
        String[] b = Arrays.copyOf(M0_IDS, M0_IDS.length + 1);
        b[M0_IDS.length] = module;
        long[] a = new long[PAIRS];
        long[] bb = new long[PAIRS];
        long[] d = new long[PAIRS];
        for (int p = 0; p < PAIRS; p++) {
            a[p] = pairWindow(ctx, core, all, M0_IDS);
            bb[p] = pairWindow(ctx, core, all, b);
            d[p] = bb[p] - a[p];
        }
        checkPlayerAlive(ctx, "paired " + module);
        long[] ds = d.clone();
        Arrays.sort(ds);
        LOGGER.info("[hengst-gametest] COST paired {}: median of {} paired differences (B minus A, hook time, render"
                + " times off) {} ns, quartiles {} / {} ns, min {} max {} ns; window medians A {} ns, B {} ns; own time"
                + " from the per-module timer {} ns; differences {}", module, PAIRS, stats(d)[0], ds[PAIRS / 4],
                ds[(3 * PAIRS) / 4], ds[0], ds[PAIRS - 1], stats(a)[0], stats(bb)[0], ownMedian, Arrays.toString(d));
    }

    /**
     * All eleven on, 300 frames: bytes allocated on the client thread around the mod's HUD element (vanilla's submit
     * included) and, in a second pass through a canvas that counts the bytes inside its delegated drawing calls, the
     * bytes of the core's frame call without the game's drawing code. Text changes per module in the window are listed.
     */
    private static void allocationWindow(ClientGameTestContext ctx, HengstCore core, String[] all) {
        String[] order = enableOnly(ctx, core, all, all);
        int start = ctx.computeOnClient(mc -> hudCalls);
        long deadline = System.nanoTime() + 120_000_000_000L;
        ctx.waitFor(mc -> hudCalls >= start + COST_WARMUP || System.nanoTime() > deadline, ClientGameTestContext.NO_TIMEOUT);
        ctx.runOnClient(mc -> {
            allocHook = 0;
            allocMod = 0;
            allocCanvas = 0;
            allocFramesWithBytes = 0;
            allocFramesWithBytesNoNewText = 0;
            Arrays.fill(TEXT_CHANGES, 0);
            Arrays.fill(LAST_TEXT, null);
            ALLOC_DIAG.setLength(0);
            allocTick0 = tickCalls;
            allocLeft = ALLOC_FRAMES + ALLOC_SETTLE;
        });
        long deadline2 = System.nanoTime() + 120_000_000_000L;
        ctx.waitFor(mc -> allocLeft == 0 || System.nanoTime() > deadline2, ClientGameTestContext.NO_TIMEOUT);
        long[] r = ctx.computeOnClient(mc -> new long[] {allocLeft, allocHook, allocMod, allocCanvas,
            allocFramesWithBytes, allocFramesWithBytesNoNewText});
        check(r[0] == 0, "allocation window: " + ALLOC_FRAMES + " frames within 120 s");
        int[] changes = ctx.computeOnClient(mc -> TEXT_CHANGES.clone());
        String diag = ctx.computeOnClient(mc -> ALLOC_DIAG.toString());
        StringBuilder changed = new StringBuilder();
        for (int i = 0; i < order.length; i++) {
            if (changes[i] > 0) {
                changed.append(changed.length() == 0 ? "" : ", ").append(order[i]).append(' ').append(changes[i]);
            }
        }
        LOGGER.info("[hengst-gametest] COST alloc {} frames, all eleven on: the core's frame call {} bytes ({} frames with"
                + " bytes), the game's drawing calls inside it {} bytes; the mod's HUD element in the real pass {} bytes;"
                + " text changes in the window: {}; frames with bytes:{}", ALLOC_FRAMES, r[2], r[4], r[3], r[1],
                changed.length() == 0 ? "none" : changed.toString(), diag.isEmpty() ? " none" : diag);
        // Steady state: a frame in which no module shows a new text allocates nothing in the core's frame call (a new
        // text may build its fitted form once; that frame is listed above and not asserted).
        check(r[5] == 0, "the core's frame call allocated in " + r[5] + " frames without a new text: " + diag);
        pass("the core's frame call allocates nothing in frames without a new text (" + ALLOC_FRAMES + " frames, all"
                + " eleven on; " + r[2] + " bytes in " + r[4] + " frames with a new text)");
    }

    private static final int PROFILE_FRAMES = 1000;
    private static final int PROFILE_CALLS = 64;
    private static final String[] CALL_NAMES = {"width", "height", "fontHeight", "textWidth", "fill", "text",
        "pushTransform", "popTransform"};
    private static final long[] PROFILE_NANOS = new long[PROFILE_FRAMES * PROFILE_CALLS];
    private static final int[] PROFILE_KINDS = new int[PROFILE_FRAMES * PROFILE_CALLS];
    private static final int[] PROFILE_COUNT = new int[PROFILE_FRAMES];
    private static int profileLeft;
    private static int profileFrame;

    /**
     * Diagnosis by elimination, logged only: the mod's HUD drawn once per frame through a canvas that times each of its
     * delegated drawing calls (System.nanoTime around the game's call); per call position in the frame, the kind of
     * call and its median and 95th percentile over 1000 frames. Shows which of the game's calls carry the time.
     */
    private static void profileCase(ClientGameTestContext ctx, HengstCore core, String name, String[] on) {
        String[] all = new String[M0_IDS.length + NEW_IDS.length];
        System.arraycopy(M0_IDS, 0, all, 0, M0_IDS.length);
        System.arraycopy(NEW_IDS, 0, all, M0_IDS.length, NEW_IDS.length);
        enableOnly(ctx, core, all, on);
        int start = ctx.computeOnClient(mc -> hudCalls);
        long deadline = System.nanoTime() + 120_000_000_000L;
        ctx.waitFor(mc -> hudCalls >= start + COST_WARMUP || System.nanoTime() > deadline, ClientGameTestContext.NO_TIMEOUT);
        ctx.runOnClient(mc -> {
            profileFrame = 0;
            profileLeft = PROFILE_FRAMES;
        });
        long deadline2 = System.nanoTime() + 120_000_000_000L;
        ctx.waitFor(mc -> profileLeft == 0 || System.nanoTime() > deadline2, ClientGameTestContext.NO_TIMEOUT);
        check(ctx.computeOnClient(mc -> profileLeft) == 0, "profile " + name + ": " + PROFILE_FRAMES + " frames within 120 s");
        long[] nanos = ctx.computeOnClient(mc -> PROFILE_NANOS.clone());
        int[] kinds = ctx.computeOnClient(mc -> PROFILE_KINDS.clone());
        int[] counts = ctx.computeOnClient(mc -> PROFILE_COUNT.clone());
        int n = counts[0];
        StringBuilder out = new StringBuilder();
        for (int k = 0; k < n; k++) {
            long[] column = new long[PROFILE_FRAMES];
            for (int f = 0; f < PROFILE_FRAMES; f++) {
                column[f] = counts[f] > k ? nanos[f * PROFILE_CALLS + k] : 0;
            }
            long[] st = stats(column);
            out.append(k == 0 ? "" : ", ").append(k).append(':').append(CALL_NAMES[kinds[k]]).append(' ').append(st[0])
                    .append('/').append(st[1]);
        }
        LOGGER.info("[hengst-gametest] COST profile {} ({} canvas calls per frame; call position:kind median/p95 ns): {}",
                name, n, out);
    }

    /** Called by the HUD probe instead of the plain inner call while a cost or allocation window is open. */
    private static void measuredHud(Runnable inner, Runnable beginAllocCanvas) {
        HengstCore core = HengstModernMod.core();
        if (profileLeft > 0) {
            beginAllocCanvas.run();
            TIMING_WRAP.n = 0;
            TIMING_WRAP.base = profileFrame * PROFILE_CALLS;
            core.onHudRender(TIMING_WRAP, 0.0F);
            PROFILE_COUNT[profileFrame] = TIMING_WRAP.n;
            profileFrame++;
            profileLeft--;
            return;
        }
        if (pairLeft > 0) {
            long t0 = System.nanoTime();
            inner.run();
            PAIR_SAMPLES[pairFrame++] = System.nanoTime() - t0;
            pairLeft--;
            return;
        }
        if (costLeft > 0) {
            Arrays.fill(COST_TIMES, -1);
            long e0 = System.nanoTime();
            long e1 = System.nanoTime();
            long t0 = System.nanoTime();
            inner.run();
            long t1 = System.nanoTime();
            int f = costFrame;
            if (core.renderer().activeCount() > 0 && COST_TIMES[0] < 0) {
                costUndrawn++;
            }
            if (f == 0) {
                costFirst = t0;
            }
            costLast = t0;
            HOOK_SAMPLES[f] = t1 - t0;
            EMPTY_SAMPLES[f] = e1 - e0;
            System.arraycopy(COST_TIMES, 0, COST_SAMPLES, f * MAX_MODULES, MAX_MODULES);
            costFrame++;
            costLeft--;
            return;
        }
        long a0 = THREADS.getCurrentThreadAllocatedBytes();
        long a1 = THREADS.getCurrentThreadAllocatedBytes();
        inner.run();
        long a2 = THREADS.getCurrentThreadAllocatedBytes();
        beginAllocCanvas.run();
        ALLOC_WRAP.inside = 0;
        long a3 = THREADS.getCurrentThreadAllocatedBytes();
        core.onHudRender(ALLOC_WRAP, 0.0F);
        long a4 = THREADS.getCurrentThreadAllocatedBytes();
        long cost = a1 - a0;
        long mod = (a4 - a3 - cost) - ALLOC_WRAP.inside;
        if (allocLeft > ALLOC_FRAMES) {
            allocLeft--;
            return;
        }
        allocHook += a2 - a1 - cost;
        allocMod += mod;
        allocCanvas += ALLOC_WRAP.inside;
        if (mod != 0) {
            allocFramesWithBytes++;
        }
        HudModule[] active = core.renderer().activeModules();
        int changed = 0;
        for (int i = 0; i < active.length && i < MAX_MODULES; i++) {
            if (active[i] instanceof TextHudModule) {
                String t = ((TextHudModule) active[i]).text();
                if (t != LAST_TEXT[i]) {
                    if (LAST_TEXT[i] != null) {
                        TEXT_CHANGES[i]++;
                        changed |= 1 << i;
                    }
                    LAST_TEXT[i] = t;
                }
            }
        }
        if (mod != 0 && changed == 0) {
            allocFramesWithBytesNoNewText++;
        }
        if (mod != 0 && ALLOC_DIAG.length() < 2000) {
            // Cold path, after the measurement: which modules showed a new text in this frame, ticks since the window
            // opened, JVM uptime (to match a compilation log).
            ALLOC_DIAG.append(" [frame ").append(ALLOC_FRAMES - allocLeft).append(", ").append(mod).append(" bytes, tick ")
                    .append(tickCalls - allocTick0).append(", new text:");
            for (int i = 0; i < active.length && i < MAX_MODULES; i++) {
                if ((changed & (1 << i)) != 0) {
                    ALLOC_DIAG.append(' ').append(active[i].id());
                }
            }
            ALLOC_DIAG.append(changed == 0 ? " none" : "").append(", uptime ")
                    .append(ManagementFactory.getRuntimeMXBean().getUptime()).append(" ms]");
        }
        allocLeft--;
    }

    private static final ModernCanvas ALLOC_CANVAS = new ModernCanvas();
    private static final AllocCanvas ALLOC_WRAP = new AllocCanvas(ALLOC_CANVAS);

    private static final TimingCanvas TIMING_WRAP = new TimingCanvas(ALLOC_CANVAS);

    /** Delegates to the game's canvas and records kind and duration of each delegated call of the frame. */
    private static final class TimingCanvas implements Canvas {
        private final Canvas d;
        int n;
        int base;

        TimingCanvas(Canvas d) {
            this.d = d;
        }

        private void add(int kind, long start) {
            long t = System.nanoTime() - start;
            if (n < PROFILE_CALLS) {
                PROFILE_NANOS[base + n] = t;
                PROFILE_KINDS[base + n] = kind;
                n++;
            }
        }

        @Override
        public int width() {
            long a = System.nanoTime();
            int r = d.width();
            add(0, a);
            return r;
        }

        @Override
        public int height() {
            long a = System.nanoTime();
            int r = d.height();
            add(1, a);
            return r;
        }

        @Override
        public int fontHeight() {
            long a = System.nanoTime();
            int r = d.fontHeight();
            add(2, a);
            return r;
        }

        @Override
        public int textWidth(String text) {
            long a = System.nanoTime();
            int r = d.textWidth(text);
            add(3, a);
            return r;
        }

        @Override
        public void fill(int x1, int y1, int x2, int y2, int argb) {
            long a = System.nanoTime();
            d.fill(x1, y1, x2, y2, argb);
            add(4, a);
        }

        @Override
        public void text(String text, int x, int y, int argb, boolean shadow) {
            long a = System.nanoTime();
            d.text(text, x, y, argb, shadow);
            add(5, a);
        }

        @Override
        public void pushTransform(float translateX, float translateY, float scale) {
            long a = System.nanoTime();
            d.pushTransform(translateX, translateY, scale);
            add(6, a);
        }

        @Override
        public void popTransform() {
            long a = System.nanoTime();
            d.popTransform();
            add(7, a);
        }
    }

    /** Delegates to the game's canvas and adds up the bytes allocated inside each delegated call. */
    private static final class AllocCanvas implements Canvas {
        private final Canvas d;
        long inside;

        AllocCanvas(Canvas d) {
            this.d = d;
        }

        private static long now() {
            return THREADS.getCurrentThreadAllocatedBytes();
        }

        @Override
        public int width() {
            long a = now();
            int r = d.width();
            inside += now() - a;
            return r;
        }

        @Override
        public int height() {
            long a = now();
            int r = d.height();
            inside += now() - a;
            return r;
        }

        @Override
        public int fontHeight() {
            long a = now();
            int r = d.fontHeight();
            inside += now() - a;
            return r;
        }

        @Override
        public int textWidth(String text) {
            long a = now();
            int r = d.textWidth(text);
            inside += now() - a;
            return r;
        }

        @Override
        public void fill(int x1, int y1, int x2, int y2, int argb) {
            long a = now();
            d.fill(x1, y1, x2, y2, argb);
            inside += now() - a;
        }

        @Override
        public void text(String text, int x, int y, int argb, boolean shadow) {
            long a = now();
            d.text(text, x, y, argb, shadow);
            inside += now() - a;
        }

        @Override
        public void pushTransform(float translateX, float translateY, float scale) {
            long a = now();
            d.pushTransform(translateX, translateY, scale);
            inside += now() - a;
        }

        @Override
        public void popTransform() {
            long a = now();
            d.popTransform();
            inside += now() - a;
        }
    }

    /**
     * The server's reduced-debug-info rule (vanilla F3 then hides position and facing): coordinates, direction and speed
     * show nothing while it is set and return when it is cleared.
     */
    private static void reducedDebugInfo(ClientGameTestContext ctx, TestSingleplayerContext world, HengstCore core) {
        String[] ids = {"coordinates", "direction", "speed"};
        boolean[] was = new boolean[ids.length];
        for (int i = 0; i < ids.length; i++) {
            String id = ids[i];
            was[i] = ctx.computeOnClient(mc -> core.activeProfile().module(id).enabled());
            setEnabled(ctx, core, id, true);
            waitText(ctx, core, id, s -> !s.isEmpty(), 3_000, id + " shows a text before the rule");
        }
        world.getServer().runCommand("gamerule reduced_debug_info true");
        waitUntil(ctx, mc -> mc.showOnlyReducedInfo(), 5_000, "the client sees reduced debug info");
        for (String id : ids) {
            waitText(ctx, core, id, String::isEmpty, 2_000, id + " empty under reduced debug info");
        }
        world.getServer().runCommand("gamerule reduced_debug_info false");
        waitUntil(ctx, mc -> !mc.showOnlyReducedInfo(), 5_000, "reduced debug info cleared");
        for (String id : ids) {
            waitText(ctx, core, id, s -> !s.isEmpty(), 3_000, id + " shows a text again after the rule");
        }
        for (int i = 0; i < ids.length; i++) {
            setEnabled(ctx, core, ids[i], was[i]);
        }
        ctx.runOnClient(HengstClientGameTest::clearChat);         // the two command feedback lines
        pass("reduced debug info: coordinates, direction and speed empty while the gamerule is on, back when it is off");
    }

    // ---- M1a helpers ----

    private static TextHudModule module(HengstCore core, String id) {
        return (TextHudModule) core.registry().byId(id);
    }

    private static void setEnabled(ClientGameTestContext ctx, HengstCore core, String id, boolean on) {
        int nr = core.registry().byId(id).number();
        ctx.runOnClient(mc -> {
            core.activeProfile().module(id).setEnabled(on);
            core.applyConfigForTest();
        });
        check(ctx.computeOnClient(mc -> FeatureState.on(nr)) == on, id + (on ? " on" : " off") + " in the feature state");
    }

    private static void setOption(ClientGameTestContext ctx, HengstCore core, String id, String key, Object value) {
        ctx.runOnClient(mc -> {
            core.activeProfile().module(id).setOption(key, value);
            core.applyConfigForTest();
        });
    }

    /** Waits until the module's text passes; returns it. */
    private static String waitText(ClientGameTestContext ctx, HengstCore core, String id, Predicate<String> ok,
            long maxMillis, String what) {
        long deadline = System.nanoTime() + maxMillis * 1_000_000L;
        ctx.waitFor(mc -> ok.test(module(core, id).text()) || System.nanoTime() > deadline, ClientGameTestContext.NO_TIMEOUT);
        String t = ctx.computeOnClient(mc -> module(core, id).text());
        check(ok.test(t), what + " within " + maxMillis + " ms: text '" + t + "'");
        return t;
    }

    /** The module's box, shown against a fresh F1 baseline of the same view; returns the box. */
    private static int[] shownByPixels(ClientGameTestContext ctx, String id, String name) {
        Layout layout = layout(ctx);
        BufferedImage shown = shot(ctx, name);
        BufferedImage hidden = f1Baseline(ctx, name + "-f1");
        checkShown(shown, hidden, layout, id, id + " drawn");
        return layout.rect(id);
    }

    /** Switched off: not laid out, and where its box was the frame equals a fresh F1 baseline. */
    private static void offAndGone(ClientGameTestContext ctx, HengstCore core, String id, int[] rect, String name) {
        setEnabled(ctx, core, id, false);
        ctx.waitTick();
        check(!layout(ctx).has(id), id + " not laid out after switching it off");
        BufferedImage img = shot(ctx, name);
        BufferedImage hidden = f1Baseline(ctx, name + "-f1");
        checkHidden(img, hidden, rect, id + " gone after switching it off");
    }

    /**
     * Negative control of the pixel check: the module drawn without background and shadow in the colour of the sky
     * behind it must fail the shown-check (no white text and no difference from the F1 baseline). Restores the style.
     */
    private static void invisibleNegativeControl(ClientGameTestContext ctx, HengstCore core, String id) {
        int[] r = layout(ctx).rect(id);
        BufferedImage base = f1Baseline(ctx, "hengst-40-" + id + "-control-f1");
        int sky = meanColour(base, r);
        int[] saved = ctx.computeOnClient(mc -> {
            ModuleConfig c = core.activeProfile().module(id);
            return new int[] {c.background() ? 1 : 0, c.shadow() ? 1 : 0, c.textColor()};
        });
        ctx.runOnClient(mc -> {
            ModuleConfig c = core.activeProfile().module(id);
            c.setBackground(false);
            c.setShadow(false);
            c.setTextColor(0xFF000000 | sky);
            core.applyConfigForTest();
        });
        try {
            ctx.waitTick();
            Layout layout = layout(ctx);
            BufferedImage img = shot(ctx, "hengst-40-" + id + "-invisible");
            boolean failed;
            try {
                checkShown(img, base, layout, id, "negative control");
                failed = false;
            } catch (AssertionError expected) {
                failed = true;
            }
            double diff = diffFraction(img, base, r);
            check(failed && diff < 0.2, "negative control: an invisible " + id + " fails the pixel check (differing "
                    + diff + ")");
        } finally {
            ctx.runOnClient(mc -> {
                ModuleConfig c = core.activeProfile().module(id);
                c.setBackground(saved[0] == 1);
                c.setShadow(saved[1] == 1);
                c.setTextColor(saved[2]);
                core.applyConfigForTest();
            });
        }
    }

    /** Where a module with no box would be drawn: its default anchor and offsets, one text line high. */
    private static int[] defaultPlace(ClientGameTestContext ctx, HengstCore core, String id, Layout l) {
        HudModule m = (HudModule) core.registry().byId(id);
        check(m.defaultAnchor() == Anchor.TOP_RIGHT, id + " default anchor " + m.defaultAnchor());
        int w = l.w / 3;
        int h = ctx.computeOnClient(mc -> mc.font.lineHeight) + 4;
        int x2 = l.w + m.defaultOffsetX();
        int y1 = m.defaultOffsetY();
        return new int[] {x2 - w, y1, x2, y1 + h};
    }

    private static void face(ClientGameTestContext ctx, float yaw) {
        ctx.runOnClient(mc -> {
            mc.player.setYRot(yaw);
            mc.player.yRotO = yaw;
        });
        ctx.waitTicks(2);
    }

    private static long usedBytes() {
        Runtime rt = Runtime.getRuntime();
        return rt.totalMemory() - rt.freeMemory();
    }

    /** The number of "4.3 m/s", or -1. */
    private static double speed(String text) {
        int end = text.indexOf(" m/s");
        return end < 0 ? -1 : Double.parseDouble(text.substring(0, end));
    }

    private static int clockMinutes(String hhmm) {
        return Integer.parseInt(hhmm.substring(0, 2)) * 60 + Integer.parseInt(hhmm.substring(3, 5));
    }

    private static int clockMinutes(String twelve, String am, String pm) {
        int colon = twelve.indexOf(':');
        int hour = Integer.parseInt(twelve.substring(0, colon)) % 12;
        int minute = Integer.parseInt(twelve.substring(colon + 1, colon + 3));
        return (twelve.endsWith(pm) ? hour + 12 : hour) * 60 + minute;
    }

    /** Within one minute of the system time, across midnight too. */
    private static void checkClose(int shownMinutes, String what) {
        LocalTime now = LocalTime.now();
        int d = Math.abs(now.getHour() * 60 + now.getMinute() - shownMinutes);
        d = Math.min(d, 1440 - d);
        check(d <= 1, what + " vs system time " + now);
    }

    /** "m:ss.t" or "h:mm:ss.t" in seconds. */
    private static double stopwatchSeconds(String text) {
        String[] parts = text.split(":");
        double s = Double.parseDouble(parts[parts.length - 1]);
        for (int i = parts.length - 2, f = 60; i >= 0; i--, f *= 60) {
            s += Integer.parseInt(parts[i]) * (double) f;
        }
        return s;
    }

    private static InputConstants.Key keyboardKey(int code) {
        //? if >=26.1 {
        return InputConstants.Type.KEYBOARD.getOrCreate(code);
        //?} else {
        /*return InputConstants.Type.KEYSYM.getOrCreate(code);
        *///?}
    }

    /**
     * Switches the game language as the language screen does (selection, option, resource reload; the options are saved
     * as the screen's close does) and waits until the core has loaded it and the loading overlay is gone. Two times are
     * logged: from the switch (includes the game's resource reload) and from the first tick after the reload finished.
     * The mod's promise is its own periodic check: the core has the new language at most one second after the reload.
     */
    private static void setLanguage(ClientGameTestContext ctx, HengstCore core, String code, String comfort) {
        long start = System.nanoTime();
        long[] at = new long[2];                                  // [reload done seen, core picked up], client thread
        CompletableFuture<Void> reload = ctx.computeOnClient(mc -> {
            mc.getLanguageManager().setSelected(code);
            mc.options.languageCode = code;
            mc.options.save();
            return mc.reloadResourcePacks();
        });
        long deadline = System.nanoTime() + 60_000_000_000L;
        ctx.waitFor(mc -> {
            long now = System.nanoTime();
            if (at[0] == 0 && reload.isDone()) {
                at[0] = now;
            }
            if (at[1] == 0 && comfort.equals(core.context().i18n().get("hengst.group.comfort"))) {
                at[1] = now;
            }
            return (at[0] != 0 && at[1] != 0) || now > deadline;
        }, ClientGameTestContext.NO_TIMEOUT);
        check(at[0] != 0 && !reload.isCompletedExceptionally(), "resource reload for " + code + " within 60 s");
        check(at[1] != 0, "the core loads " + code + " after the switch, within 60 s");
        long total = (at[1] - start) / 1_000_000;
        long afterReload = Math.max(0, at[1] - at[0]) / 1_000_000;
        check(afterReload <= LANGUAGE_BOUND_MS, "the core picked up " + code + " " + afterReload
                + " ms after the reload finished (bound " + LANGUAGE_BOUND_MS + " ms)");
        // The loading overlay covers the frame until it has faded out, after the reload itself completed.
        ctx.waitFor(mc -> noOverlay(mc) || System.nanoTime() > deadline, ClientGameTestContext.NO_TIMEOUT);
        check(ctx.computeOnClient(HengstClientGameTest::noOverlay), "loading overlay gone after the reload for " + code);
        pass("language " + code + " picked up by the core " + afterReload + " ms after the reload finished (bound "
                + LANGUAGE_BOUND_MS + " ms), " + total + " ms after the switch; reload and overlay "
                + (System.nanoTime() - start) / 1_000_000 + " ms");
    }

    // The core checks the language once a second, in a tick; the wait observes once per tick (50 ms): one second plus
    // that one tick of observation. The check runs during the reload too, so the core usually has the new language
    // before the reload ends and this measures 0 ms (every run so far): the assertion only catches a pick-up that
    // comes later than one second after the reload, not a slow check that still finishes within the reload.
    private static final long LANGUAGE_BOUND_MS = 1_050;

    private static boolean noOverlay(Minecraft mc) {
        //? if >=26.1 {
        return mc.gui.overlay() == null;
        //?} else {
        /*return mc.getOverlay() == null;
        *///?}
    }

    private static Map<String, String> langFile(String lang) {
        try (InputStream in = HengstClientGameTest.class.getResourceAsStream("/assets/hengst/lang/" + lang + ".json")) {
            check(in != null, "language file " + lang);
            Map<String, String> out = new HashMap<>();
            JsonParser.parseReader(new InputStreamReader(in, StandardCharsets.UTF_8)).getAsJsonObject()
                    .entrySet().forEach(e -> out.put(e.getKey(), e.getValue().getAsString()));
            return out;
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
    }

    private static String i18n(ClientGameTestContext ctx, HengstCore core, String key) {
        return ctx.computeOnClient(mc -> core.context().i18n().get(key));
    }

    /** The core's screen behind the host and the core's SafeScreen wrapper. */
    private static UiScreen settingsUi(ClientGameTestContext ctx) {
        return ctx.computeOnClient(mc -> {
            Object host = ModernPlatform.currentScreen(mc);
            check(host instanceof ModernScreenHost, "a mod screen is open, got " + host);
            Object ui = field(field(host, "ui"), "screen");
            check(ui instanceof SettingsScreen, "the settings are open, got " + ui);
            return (UiScreen) ui;
        });
    }

    /** What the core's screen draws, through a recording canvas of the current GUI size. */
    private static Layout record(ClientGameTestContext ctx, UiScreen ui) {
        return ctx.computeOnClient(mc -> {
            Layout c = new Layout(mc);
            ui.render(c, -1, -1, 0.0F);
            guiWidth = c.w;
            return c;
        });
    }

    /** Window pixels per GUI pixel, x and y. */
    private static double[] scale(ClientGameTestContext ctx, Layout layout) {
        return ctx.computeOnClient(mc -> new double[] {mc.getWindow().getScreenWidth() / (double) layout.w,
                mc.getWindow().getScreenHeight() / (double) layout.h});
    }

    private static void clickGui(ClientGameTestContext ctx, double[] s, int gx, int gy) {
        ctx.getInput().setCursorPos((gx + 0.5) * s[0], (gy + 0.5) * s[1]);
        ctx.waitTick();
        ctx.getInput().pressMouse(LEFT);
        ctx.waitTick();
    }

    private static Widget featureRow(List<?> rows, String id) {
        for (Object row : rows) {
            Field f;
            try {
                f = row.getClass().getDeclaredField("feature");
            } catch (NoSuchFieldException groupHeading) {
                continue;
            }
            if (id.equals(((Feature) get(f, row)).id())) {
                return (Widget) row;
            }
        }
        throw new AssertionError("[hengst-gametest] FAIL no settings row for " + id);
    }

    /** {x, y, w, h} of a scroll list. */
    private static int[] bounds(ScrollList list) {
        return new int[] {(Integer) field(list, "x"), (Integer) field(list, "y"), (Integer) field(list, "w"),
            (Integer) field(list, "h")};
    }

    private static Object field(Object owner, String name) {
        for (Class<?> c = owner.getClass(); c != null; c = c.getSuperclass()) {
            try {
                return get(c.getDeclaredField(name), owner);
            } catch (NoSuchFieldException e) {
                // try the superclass
            }
        }
        throw new AssertionError("[hengst-gametest] FAIL no field " + name + " in " + owner.getClass());
    }

    /** The entry's shown name: its Component field that translates the binding's own key. */
    private static Component nameOf(Object entry, KeyMapping key) {
        for (Field f : entry.getClass().getDeclaredFields()) {
            if (f.getType() == Component.class) {
                Component c = (Component) get(f, entry);
                if (c != null && c.getContents() instanceof TranslatableContents tc && tc.getKey().equals(key.getName())) {
                    return c;
                }
            }
        }
        throw new AssertionError("[hengst-gametest] FAIL no name translating " + key.getName() + " in " + entry.getClass());
    }

    /** The one field declared in the owner's own class with exactly this type. */
    private static Object fieldOfType(Object owner, Class<?> type) {
        Field found = null;
        for (Field f : owner.getClass().getDeclaredFields()) {
            if (f.getType() == type) {
                check(found == null, "one field of type " + type.getSimpleName() + " in " + owner.getClass());
                found = f;
            }
        }
        check(found != null, "a field of type " + type.getSimpleName() + " in " + owner.getClass());
        return get(found, owner);
    }

    private static int constant(Class<?> owner, String name) {
        try {
            Field f = owner.getDeclaredField(name);
            f.setAccessible(true);
            return f.getInt(null);
        } catch (ReflectiveOperationException e) {
            throw new AssertionError("[hengst-gametest] FAIL constant " + owner.getSimpleName() + "." + name, e);
        }
    }

    private static int staticInt(Object owner, String name) {
        try {
            Field f = owner.getClass().getDeclaredField(name);
            f.setAccessible(true);
            return f.getInt(null);
        } catch (ReflectiveOperationException e) {
            throw new AssertionError("[hengst-gametest] FAIL static " + name, e);
        }
    }

    private static Object get(Field f, Object owner) {
        try {
            f.setAccessible(true);
            return f.get(owner);
        } catch (IllegalAccessException e) {
            throw new AssertionError("[hengst-gametest] FAIL field " + f, e);
        }
    }

    /** A chat screen that reports what was typed into its box. */
    private static final class TypingChatScreen extends ChatScreen {
        TypingChatScreen() {
            super("", false);
        }

        String typed() {
            return input.getValue();
        }
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
        check(optionLine("lang:").equals("lang:en_us"), "restart: options.txt " + optionLine("lang:"));
        String[] lang = ctx.computeOnClient(mc -> new String[] {mc.options.languageCode,
            mc.getLanguageManager().getSelected(), core.context().i18n().get("hengst.group.comfort"),
            Component.translatable("key.hengst.settings").getString()});
        check("en_us".equals(lang[0]) && "en_us".equals(lang[1]) && "Comfort".equals(lang[2])
                && langFile("en_us").get("key.hengst.settings").equals(lang[3]), "restart: the game runs in English, got "
                + Arrays.toString(lang));
        pass("restart: options.txt has lang:en_us and the game and the core run in English");
        optionsKeepTheKeyLines();
        KeyMapping toggle = ctx.computeOnClient(mc -> mapping(mc, TOGGLE));
        String k = keyboardKey(InputConstants.KEY_K).getName();
        check(ctx.computeOnClient(mc -> toggle.saveString()).equals(k), "restart: the toggle loaded as "
                + ctx.computeOnClient(mc -> toggle.saveString()) + ", saved " + k);
        ctx.runOnClient(mc -> {
            toggle.setKey(toggle.getDefaultKey());
            KeyMapping.resetMapping();
            mc.options.save();
        });
        check(optionLine("key_" + TOGGLE + ":").endsWith(":key.keyboard.unknown"), "restart: toggle unbound again, "
                + optionLine("key_" + TOGGLE + ":"));
        pass("restart: the stopwatch toggle bound to K in the earlier run is loaded with K, then unbound again");
    }

    /** The game's options.txt, written by the earlier run, keeps one line per binding of the mod. */
    private static void optionsKeepTheKeyLines() {
        Path options = FabricLoader.getInstance().getGameDir().resolve("options.txt");
        List<String> lines;
        try {
            lines = Files.readAllLines(options, StandardCharsets.UTF_8);
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
        List<String> found = new ArrayList<>();
        for (String key : HENGST_KEYS) {
            for (String line : lines) {
                if (line.startsWith("key_" + key + ":")) {
                    found.add(line);
                }
            }
        }
        check(found.size() == HENGST_KEYS.length, "options.txt has one line per mod binding, got " + found);
        check(found.get(0).endsWith(":key.keyboard.right.shift"), "settings key line " + found.get(0));
        for (int i = 1; i < found.size(); i++) {
            String expected = HENGST_KEYS[i].equals(TOGGLE) ? keyboardKey(InputConstants.KEY_K).getName()
                    : "key.keyboard.unknown";
            check(found.get(i).endsWith(":" + expected), "key line " + found.get(i) + ", expected " + expected);
        }
        pass("restart: options.txt keeps the lines of all four bindings " + found);
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

    /** Light grey pixels (all channels at least 140, nearly equal): the muted text colour over the dimmed game. */
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
                if (min >= 140 && max - min <= 20 && max < TEXT_LEVEL) {
                    n++;
                }
            }
        }
        return n;
    }

    /** Mean RGB of the region, as 0xRRGGBB. */
    private static int meanColour(BufferedImage img, int[] r) {
        int[] p = pixels(img, r);
        long red = 0;
        long green = 0;
        long blue = 0;
        int total = 0;
        for (int y = p[1]; y < p[3]; y++) {
            for (int x = p[0]; x < p[2]; x++) {
                int c = img.getRGB(x, y);
                red += (c >> 16) & 255;
                green += (c >> 8) & 255;
                blue += c & 255;
                total++;
            }
        }
        check(total > 0, "empty pixel region " + Arrays.toString(r));
        return (int) (red / total) << 16 | (int) (green / total) << 8 | (int) (blue / total);
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

    /**
     * Calls the core's HUD render once with a recording canvas (layout and draw count only, not visibility). A box
     * belongs to the n-th active module that the renderer does not skip: like HudRenderer, a module whose scaled
     * width or height is 0 (empty text) gets no box.
     */
    private static Layout layout(ClientGameTestContext ctx) {
        return ctx.computeOnClient(mc -> {
            Layout c = new Layout(mc);
            HengstCore core = HengstModernMod.core();
            core.onHudRender(c, 0.0F);
            c.ids = new String[c.boxes.size()];
            HudModule[] modules = core.renderer().activeModules();
            ModuleConfig[] configs = core.renderer().activeConfigs();
            int n = 0;
            for (int i = 0; i < modules.length && n < c.ids.length; i++) {
                float scale = configs[i].scale();
                if (Math.round(modules[i].width(c, configs[i]) * scale) > 0
                        && Math.round(modules[i].height(c, configs[i]) * scale) > 0) {
                    c.ids[n++] = modules[i].id();
                }
            }
            check(n == c.ids.length, "every box belongs to a drawn module: " + n + " of " + c.ids.length);
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

    private static void clearChat(Minecraft mc) {
        //? if >=26.1 {
        mc.gui.hud.getChat().clearMessages(false);
        //?} else {
        /*mc.gui.getChat().clearMessages(false);
        *///?}
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
            if (mod && (costLeft > 0 || allocLeft > 0 || profileLeft > 0 || pairLeft > 0)) {
                measuredHud(() -> inner.extractRenderState(graphics, delta), () -> ALLOC_CANVAS.begin(graphics));
                return;
            }
            inner.extractRenderState(graphics, delta);
        }
        //?} else {
        /*@Override
        public void render(GuiGraphics graphics, DeltaTracker delta) {
            stamp();
            if (mod && (costLeft > 0 || allocLeft > 0 || profileLeft > 0 || pairLeft > 0)) {
                measuredHud(() -> inner.render(graphics, delta), () -> ALLOC_CANVAS.begin(graphics));
                return;
            }
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

    /** One recorded text: GUI rectangle and colour. */
    static final class Text {
        final String text;
        final int x;
        final int y;
        final int w;
        final int h;
        final int argb;

        Text(String text, int x, int y, int w, int h, int argb) {
            this.text = text;
            this.x = x;
            this.y = y;
            this.w = w;
            this.h = h;
            this.argb = argb;
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
     * Where the core places each module, in GUI pixels: a canvas that records the bounding box of everything drawn
     * between a module's push and pop, the fills with their colours and the texts. Says nothing about visibility;
     * the screenshots do.
     */
    private static final class Layout implements Canvas {
        final int w;
        final int h;
        private final Font font;
        final List<int[]> boxes = new ArrayList<>();
        final List<int[]> fills = new ArrayList<>();
        final List<Text> texts = new ArrayList<>();
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

        boolean has(String id) {
            return Arrays.asList(ids).contains(id);
        }

        /** The first text drawn exactly so, in this colour, or null. */
        Text find(String text, int argb) {
            for (Text t : texts) {
                if (t.text.equals(text) && t.argb == argb) {
                    return t;
                }
            }
            return null;
        }

        /** The first text that starts with this label (option rows draw "label: value"), or null. */
        Text findStart(String label) {
            for (Text t : texts) {
                if (t.text.startsWith(label)) {
                    return t;
                }
            }
            return null;
        }

        /** {x1, y1, x2, y2} of the first fill in exactly this colour, or null. */
        int[] fill(int argb) {
            for (int[] f : fills) {
                if (f[4] == argb) {
                    return new int[] {f[0], f[1], f[2], f[3]};
                }
            }
            return null;
        }

        String fillList() {
            StringBuilder b = new StringBuilder();
            for (int[] f : fills) {
                b.append(Arrays.toString(f));
            }
            return b.toString();
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
            return font.lineHeight;
        }

        @Override
        public int textWidth(String text) {
            return font.width(text);
        }

        @Override
        public void fill(int x1, int y1, int x2, int y2, int argb) {
            int[] r = extend(Math.min(x1, x2), Math.min(y1, y2), Math.max(x1, x2), Math.max(y1, y2));
            fills.add(new int[] {r[0], r[1], r[2], r[3], argb});
        }

        @Override
        public void text(String text, int x, int y, int argb, boolean shadow) {
            int[] r = extend(x, y, x + font.width(text), y + font.lineHeight);
            texts.add(new Text(text, r[0], r[1], r[2] - r[0], r[3] - r[1], argb));
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
