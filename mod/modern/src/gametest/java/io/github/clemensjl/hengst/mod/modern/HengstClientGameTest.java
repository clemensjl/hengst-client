package io.github.clemensjl.hengst.mod.modern;

import com.mojang.blaze3d.platform.InputConstants;
import io.github.clemensjl.hengst.mod.common.HengstCore;
import io.github.clemensjl.hengst.mod.common.config.Profile;
import io.github.clemensjl.hengst.mod.common.config.ProfileStore;
import io.github.clemensjl.hengst.mod.common.hud.HudModule;
import io.github.clemensjl.hengst.mod.common.layout.Anchor;
import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import io.github.clemensjl.hengst.mod.common.platform.GameKey;
import io.github.clemensjl.hengst.mod.common.platform.Platform;
import io.github.clemensjl.hengst.mod.common.platform.UiKey;
import io.github.clemensjl.hengst.mod.common.platform.UiScreen;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Clock;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
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
//? if >=26.1 {
import net.minecraft.client.gui.GuiGraphicsExtractor;
//?} else {
/*import net.minecraft.client.gui.GuiGraphics;
*///?}
import net.minecraft.client.gui.screens.ChatScreen;
import net.minecraft.client.gui.screens.inventory.InventoryScreen;
import net.minecraft.network.chat.Component;
import net.minecraft.sounds.SoundSource;
import net.minecraft.world.entity.player.Player;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * The mod in a real singleplayer world, driven with real input through Fabric's client gametest API. Asserts through
 * the mod's own objects and takes a framebuffer screenshot at each step (run directory, folder screenshots). Any
 * failed check throws, which crashes the client and fails the Gradle task.
 *
 * <p>Two phases: the default one runs every check with a fresh run directory and ends by dragging the FPS module to
 * the bottom right in the HUD editor; the phase {@code -Dhengst.gametest.phase=restart} runs with that directory
 * kept and checks that the saved layout is loaded.
 */
public final class HengstClientGameTest implements FabricClientGameTest {
    private static final Logger LOGGER = LoggerFactory.getLogger("hengst-gametest");
    private static final int LEFT = InputConstants.MOUSE_BUTTON_LEFT;

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
            ctx.waitFor(mc -> hudCalls > 0);
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
        check(core.renderer().activeCount() == 4, "four active modules, got " + core.renderer().activeCount());
        check(renderOnce(ctx).draws > 0, "the core draws the HUD in a world");
        shot(ctx, "hengst-01-hud");
        pass("four modules drawn");

        int callsBefore = ctx.computeOnClient(mc -> hudCalls);
        ctx.waitTicks(30);
        int fps = ctx.computeOnClient(mc -> core.context().frames().fps());
        int calls = ctx.computeOnClient(mc -> hudCalls);
        check(fps > 0, "frame counter reports FPS, got " + fps);
        check(calls > callsBefore + 10, "HUD drawn every frame: " + callsBefore + " -> " + calls);
        pass("frame counter advances (" + fps + " FPS)");

        for (int i = 0; i < 5; i++) {
            input.pressMouse(LEFT);
        }
        int cps = leftCps(ctx);
        check(cps == 5, "CPS after 5 clicks, got " + cps);
        shot(ctx, "hengst-02-cps-5");
        ctx.waitTicks(30);
        cps = leftCps(ctx);
        check(cps == 0, "CPS after the window, got " + cps);
        shot(ctx, "hengst-03-cps-0");
        pass("CPS counts 5 clicks and falls to 0");

        ctx.setScreen(() -> new ChatScreen("", false));
        ctx.waitForScreen(ChatScreen.class);
        for (int i = 0; i < 3; i++) {
            input.pressMouse(LEFT);
        }
        cps = leftCps(ctx);
        check(cps == 0, "clicks with chat open do not count, got " + cps);
        ctx.setScreen(() -> null);
        ctx.waitForScreen(null);
        pass("clicks with chat open do not raise CPS");

        input.holdKey(o -> o.keyUp);
        ctx.waitTicks(10);
        check(ctx.computeOnClient(mc -> platform.input().isDown(GameKey.FORWARD)), "W held is down");
        shot(ctx, "hengst-04-w-held");
        input.releaseKey(o -> o.keyUp);
        ctx.waitTick();
        check(!ctx.computeOnClient(mc -> platform.input().isDown(GameKey.FORWARD)), "W released is up");
        pass("W held marks the key as down");

        KeyMapping settingsKey = ctx.computeOnClient(mc -> mapping(mc, "key.hengst.settings"));
        input.pressKey(settingsKey);
        ctx.waitForScreen(ModernScreenHost.class);
        check(renderOnce(ctx).draws == 0, "HUD suppressed while the settings are open");
        shot(ctx, "hengst-05-settings");
        input.pressKey(InputConstants.KEY_ESCAPE);
        ctx.waitForScreen(null);
        check(renderOnce(ctx).draws > 0, "HUD unsuppressed after Escape");
        shot(ctx, "hengst-06-after-escape");
        pass("settings key opens the mod's screen, Escape closes it");

        input.pressKey(o -> o.keyToggleGui);
        ctx.waitTick();
        check(ctx.computeOnClient(mc -> platform.game().hudHidden()), "F1 hides the HUD");
        check(renderOnce(ctx).draws == 0, "the core draws nothing under F1");
        shot(ctx, "hengst-07-f1");
        input.pressKey(o -> o.keyToggleGui);
        ctx.waitTick();
        check(!ctx.computeOnClient(mc -> platform.game().hudHidden()), "F1 again shows the HUD");
        pass("F1 hides the HUD");

        input.pressKey(o -> o.keyDebugOverlay);
        ctx.waitTicks(2);
        check(ctx.computeOnClient(mc -> platform.game().debugOverlayOpen()), "F3 open");
        shot(ctx, "hengst-08-f3");
        input.pressKey(o -> o.keyDebugOverlay);
        ctx.waitTicks(2);
        check(!ctx.computeOnClient(mc -> platform.game().debugOverlayOpen()), "F3 closed");
        pass("F3 screenshot taken");

        ctx.runOnClient(mc -> addChatLine(mc, "Hengst in-game test: this chat line draws over the mod HUD"));
        ctx.setScreen(OrderChatScreen::new);
        ctx.waitForScreen(OrderChatScreen.class);
        checkDrawOrder(ctx, "chat open");
        shot(ctx, "hengst-09-chat");
        ctx.setScreen(() -> null);
        ctx.waitForScreen(null);
        pass("with chat open the HUD is drawn before the chat and the screen");

        ctx.setScreen(() -> new OrderInventoryScreen(Minecraft.getInstance().player));
        ctx.waitForScreen(OrderInventoryScreen.class);
        checkDrawOrder(ctx, "inventory open");
        shot(ctx, "hengst-10-inventory");
        ctx.setScreen(() -> null);
        ctx.waitForScreen(null);
        pass("with the inventory open the HUD is drawn before the screen and its dim");

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

        dragFpsToBottomRight(ctx, core);
    }

    /** Last step of the full phase: the layout it saves is what the restart phase checks. */
    private static void dragFpsToBottomRight(ClientGameTestContext ctx, HengstCore core) {
        TestInput input = ctx.getInput();
        RecordingCanvas before = renderOnce(ctx);
        int[] origin = before.origin(ctx, "fps");
        double[] scale = ctx.computeOnClient(mc -> new double[] {
                (double) mc.getWindow().getScreenWidth() / mc.getWindow().getGuiScaledWidth(),
                (double) mc.getWindow().getScreenHeight() / mc.getWindow().getGuiScaledHeight(),
                mc.getWindow().getScreenWidth(), mc.getWindow().getScreenHeight()});
        ctx.runOnClient(mc -> core.onOpenHudEditorKey());
        ctx.waitForScreen(ModernScreenHost.class);
        double x = (origin[0] + 3) * scale[0];
        double y = (origin[1] + 3) * scale[1];
        input.setCursorPos(x, y);
        ctx.waitTick();
        input.holdMouse(LEFT);
        ctx.waitTick();
        double tx = scale[2] - 4;
        double ty = scale[3] - 4;
        for (int i = 1; i <= 5; i++) {
            input.setCursorPos(x + (tx - x) * i / 5, y + (ty - y) * i / 5);
            ctx.waitTick();
        }
        input.releaseMouse(LEFT);
        ctx.waitTick();
        shot(ctx, "hengst-11-editor-dragged");
        input.pressKey(InputConstants.KEY_ESCAPE);
        ctx.waitForScreen(null);

        Path profiles = FabricLoader.getInstance().getConfigDir().resolve("hengst").resolve("profiles");
        check(Files.isRegularFile(profiles.resolve("default.json")), "profile file written: " + profiles);
        Profile saved = new ProfileStore(profiles, Clock.systemUTC(), msg -> LOGGER.warn(msg)).load(ProfileStore.DEFAULT);
        Anchor anchor = saved.module("fps").anchor();
        check(anchor == Anchor.BOTTOM_RIGHT, "saved FPS anchor, got " + anchor);
        checkBottomRight(ctx, "fps");
        shot(ctx, "hengst-12-fps-bottom-right");
        pass("HUD editor drag saved the FPS module at " + anchor);
    }

    private static void restartPhase(ClientGameTestContext ctx) {
        HengstCore core = HengstModernMod.core();
        Anchor anchor = ctx.computeOnClient(mc -> core.activeProfile().module("fps").anchor());
        check(anchor == Anchor.BOTTOM_RIGHT, "restart: loaded FPS anchor (run runClientGameTest first), got " + anchor);
        checkBottomRight(ctx, "fps");
        shot(ctx, "hengst-13-restart-fps-bottom-right");
        pass("restart: the FPS module is drawn at the saved position");
    }

    private static void checkBottomRight(ClientGameTestContext ctx, String id) {
        RecordingCanvas c = renderOnce(ctx);
        int[] o = c.origin(ctx, id);
        check(o[0] > c.w / 2 && o[1] > c.h / 2, id + " drawn at the bottom right, origin " + Arrays.toString(o)
                + " on " + c.w + "x" + c.h);
    }

    /** One frame's HUD pass consists of the mod element, then vanilla's chat, then the screen: consecutive numbers. */
    private static void checkDrawOrder(ClientGameTestContext ctx, String where) {
        ctx.waitTicks(2);
        int[] s = ctx.computeOnClient(mc -> new int[] {hudSeq, chatSeq, screenSeq});
        check(s[0] + 1 == s[1] && s[1] + 1 == s[2], where + ": order mod HUD, chat, screen expected, got "
                + Arrays.toString(s));
    }

    private static int leftCps(ClientGameTestContext ctx) {
        return ctx.computeOnClient(mc -> HengstModernMod.core().context().cps().cps(0));
    }

    /** Calls the core's HUD render once with a counting canvas, as the HUD element does. */
    private static RecordingCanvas renderOnce(ClientGameTestContext ctx) {
        return ctx.computeOnClient(mc -> {
            RecordingCanvas c = new RecordingCanvas(mc);
            HengstModernMod.core().onHudRender(c, 0.0F);
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

    private static void shot(ClientGameTestContext ctx, String name) {
        LOGGER.info("[hengst-gametest] screenshot {}", ctx.takeScreenshot(name));
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

    /** Counts what the core draws and where each module is placed. */
    private static final class RecordingCanvas implements Canvas {
        final int w;
        final int h;
        private final Font font;
        final List<int[]> origins = new ArrayList<>();
        int draws;

        RecordingCanvas(Minecraft mc) {
            w = mc.getWindow().getGuiScaledWidth();
            h = mc.getWindow().getGuiScaledHeight();
            font = mc.font;
        }

        /** The placement of an active module, in GUI pixels. */
        int[] origin(ClientGameTestContext ctx, String id) {
            HudModule[] modules = ctx.computeOnClient(mc -> HengstModernMod.core().renderer().activeModules());
            for (int i = 0; i < modules.length; i++) {
                if (modules[i].id().equals(id)) {
                    check(i < origins.size(), id + " was not drawn");
                    return origins.get(i);
                }
            }
            throw new AssertionError("[hengst-gametest] FAIL module " + id + " not active");
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
            draws++;
        }

        @Override
        public void text(String text, int x, int y, int argb, boolean shadow) {
            draws++;
        }

        @Override
        public void pushTransform(float translateX, float translateY, float scale) {
            origins.add(new int[] {Math.round(translateX), Math.round(translateY)});
        }

        @Override
        public void popTransform() {
        }
    }
}
