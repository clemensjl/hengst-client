package io.github.clemensjl.hengst.mod.common;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.config.Profile;
import io.github.clemensjl.hengst.mod.common.config.ProfileStore;
import io.github.clemensjl.hengst.mod.common.config.Settings;
import io.github.clemensjl.hengst.mod.common.config.SettingsStore;
import io.github.clemensjl.hengst.mod.common.editor.HudEditorClose;
import io.github.clemensjl.hengst.mod.common.editor.HudEditorModel;
import io.github.clemensjl.hengst.mod.common.editor.HudEditorScreen;
import io.github.clemensjl.hengst.mod.common.feature.FeatureRegistry;
import io.github.clemensjl.hengst.mod.common.hud.CpsCounter;
import io.github.clemensjl.hengst.mod.common.hud.FrameCounter;
import io.github.clemensjl.hengst.mod.common.hud.HudContext;
import io.github.clemensjl.hengst.mod.common.hud.HudRenderer;
import io.github.clemensjl.hengst.mod.common.hud.modules.CoordinatesModule;
import io.github.clemensjl.hengst.mod.common.hud.modules.CpsModule;
import io.github.clemensjl.hengst.mod.common.hud.modules.FpsModule;
import io.github.clemensjl.hengst.mod.common.hud.modules.KeystrokesModule;
import io.github.clemensjl.hengst.mod.common.layout.Anchor;
import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import io.github.clemensjl.hengst.mod.common.platform.GameView;
import io.github.clemensjl.hengst.mod.common.platform.Platform;
import io.github.clemensjl.hengst.mod.common.platform.TimeSource;
import io.github.clemensjl.hengst.mod.common.platform.UiKey;
import io.github.clemensjl.hengst.mod.common.platform.UiScreen;
import io.github.clemensjl.hengst.mod.common.ui.SettingsClose;
import io.github.clemensjl.hengst.mod.common.ui.SettingsModel;
import io.github.clemensjl.hengst.mod.common.ui.SettingsScreen;
import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.file.Path;
import java.time.Clock;

/**
 * The one object the game adapters talk to. It owns the registry, the stored settings and profile, the HUD renderer
 * and the counters, and turns the adapters' hooks into work. Client thread only.
 *
 * <p>Saving is batched: when a screen closes, at most every two seconds from the client tick while changes are
 * pending, and in {@link #shutdown}. A failed save keeps the pending state, so the next attempt retries.
 * The frame and tick hooks allocate nothing in the healthy case, and no exception from a module or screen leaves them.
 */
public final class HengstCore {
    private static final long SAVE_INTERVAL_NANOS = 2_000_000_000L;

    // Failure sites: each logs once (the save site logs again after a success followed by a new failure).
    private static final int SITE_RENDER = 0;
    private static final int SITE_TICK = 1;
    private static final int SITE_MOUSE = 2;
    private static final int SITE_SCREEN = 3;
    private static final int SITE_OPEN = 4;
    private static final int SITE_SAVE = 5;
    private static final int SITES = 6;

    private final Platform platform;
    private final TimeSource time;
    private final FeatureRegistry registry = new FeatureRegistry();
    private final SettingsStore settingsStore;
    private final ProfileStore profileStore;
    private final Settings settings;
    private final FrameCounter frames;
    private final CpsCounter cps;
    private final HudContext ctx;
    private final HudRenderer renderer;
    private final boolean[] failureLogged = new boolean[SITES];
    private final boolean[] held = new boolean[2];

    private Profile profile;
    // The name the loaded profile belongs to; saves go there, and only while it is still the active one.
    private String profileName;
    private boolean dirty;
    private long lastSaveNanos;
    private Session current;

    private HengstCore(Platform platform, TimeSource time, Clock clock) {
        this.platform = platform;
        this.time = time;
        registry.register(new FpsModule());
        registry.register(new CpsModule());
        registry.register(new KeystrokesModule());
        registry.register(new CoordinatesModule());
        Path root = platform.configDir().resolve("hengst");
        settingsStore = new SettingsStore(root.resolve("settings.json"), clock, platform::logWarning);
        profileStore = new ProfileStore(root.resolve("profiles"), clock, platform::logWarning);
        settings = settingsStore.load();
        profileName = settings.activeProfile();
        profile = profileStore.load(profileName);
        boolean firstStart = ProfileStore.sameName(profileName, ProfileStore.DEFAULT)
                && profileStore.loadedFromDefaults()
                && !profileStore.loadFailed(profileName)
                && !settingsStore.settingsLoadFailed();
        if (firstStart) {
            applyFirstStartDefaults(profile);
        }
        frames = new FrameCounter(time);
        cps = new CpsCounter(time);
        ctx = new HudContext(platform, time, frames, cps);
        renderer = new HudRenderer(registry, platform.version());
        renderer.rebuild(profile);
        lastSaveNanos = time.nanoTime();
    }

    public static HengstCore start(Platform platform, TimeSource time) {
        return start(platform, time, Clock.systemDefaultZone());
    }

    static HengstCore start(Platform platform, TimeSource time, Clock clock) {
        return new HengstCore(platform, time, clock);
    }

    /** The four M0 modules, on, at their default places. Memory only: nothing is written until the first change. */
    private static void applyFirstStartDefaults(Profile p) {
        place(p.module("fps"), Anchor.TOP_LEFT, 2, 2);
        place(p.module("cps"), Anchor.TOP_LEFT, 2, 16);
        place(p.module("coordinates"), Anchor.TOP_LEFT, 2, 30);
        place(p.module("keystrokes"), Anchor.BOTTOM_LEFT, 2, -2);
    }

    private static void place(ModuleConfig c, Anchor anchor, int x, int y) {
        c.setEnabled(true);
        c.setAnchor(anchor);
        c.setOffsetX(x);
        c.setOffsetY(y);
    }

    // ---- hooks for the adapters ----

    /** Once per rendered frame, after the game drew its own HUD. Counts the frame even if nothing is drawn. */
    public void onHudRender(Canvas canvas, float partialTick) {
        frames.onFrame();
        try {
            renderer.render(canvas, ctx);
        } catch (RuntimeException e) {
            failure(SITE_RENDER, "HUD render", e);
        }
    }

    /** Once per client tick (20 Hz), also outside a world. */
    public void onClientTick() {
        try {
            GameView game = platform.game();
            if (game.screenOpen()) {
                // A release may have gone to the screen: do not let a stale "held" swallow the next click.
                held[0] = false;
                held[1] = false;
            }
            if (game.inWorld()) {
                renderer.tick(ctx);
            }
            Session s = current;
            if (s != null && s.editorModel != null && s.editorModel.dirty()) {
                dirty = true;
            }
            if (dirty && time.nanoTime() - lastSaveNanos >= SAVE_INTERVAL_NANOS) {
                trySave();
            }
        } catch (RuntimeException e) {
            failure(SITE_TICK, "client tick", e);
        }
    }

    /**
     * Every mouse button event of the game, in a world or not, screen open or not. Only the press edge of the left
     * (0) and right (1) button counts, and only in a world without any screen open.
     */
    public void onMouseButton(int button, boolean pressed) {
        if (button < 0 || button > 1) {
            return;
        }
        try {
            if (!pressed) {
                held[button] = false;
                return;
            }
            GameView game = platform.game();
            if (!game.inWorld() || game.screenOpen()) {
                held[0] = false;
                held[1] = false;
                return;
            }
            if (held[button]) {
                return;
            }
            held[button] = true;
            cps.onPress(button);
        } catch (RuntimeException e) {
            failure(SITE_MOUSE, "mouse button", e);
        }
    }

    /** The settings key was pressed. Does nothing if one of the mod's screens is already open. */
    public void onOpenSettingsKey() {
        if (current != null) {
            return;
        }
        try {
            openSettings();
        } catch (RuntimeException e) {
            failure(SITE_OPEN, "opening the settings", e);
        }
    }

    /** The HUD editor key was pressed. Does nothing if one of the mod's screens is already open. */
    public void onOpenHudEditorKey() {
        if (current != null) {
            return;
        }
        try {
            openEditor();
        } catch (RuntimeException e) {
            failure(SITE_OPEN, "opening the HUD editor", e);
        }
    }

    /** The game is closing: saves pending changes. Safe to call more than once. */
    public void shutdown() {
        Session s = current;
        if (s != null && s.editorModel != null && s.editorModel.dirty()) {
            dirty = true;
        }
        if (dirty) {
            trySave();
        }
    }

    // ---- for tests ----

    public Profile activeProfile() {
        return profile;
    }

    public HudRenderer renderer() {
        return renderer;
    }

    public HudContext context() {
        return ctx;
    }

    public FeatureRegistry registry() {
        return registry;
    }

    // ---- screens ----

    private void openSettings() {
        SettingsModel model = new SettingsModel(registry, platform.version(), profileStore, settings,
                () -> profile,
                this::switchProfile,
                () -> dirty = true,
                this::openEditorFromSettings,
                this::saveForModel,
                platform::logWarning);
        model.setSettingsLoadFailedCheck(settingsStore::settingsLoadFailed);
        Session session = new Session(null);
        session.delegate = new SettingsClose(model, renderer, () -> profile, platform);
        show(session, new SettingsScreen(model, I18n.load(platform.languageCode()), session));
    }

    private void openEditor() {
        HudEditorModel model = new HudEditorModel(renderer.activeModules(), renderer.activeConfigs());
        Session session = new Session(model);
        session.delegate = new HudEditorClose(model, profileStore, profileName, profile, renderer, platform);
        show(session, new HudEditorScreen(model, ctx, I18n.load(platform.languageCode()), session));
    }

    /** The editor replaces the settings screen: the settings session ends without closing anything. */
    private void openEditorFromSettings() {
        Session old = current;
        if (old != null) {
            old.done = true;
        }
        current = null;
        try {
            renderer.rebuild(profile);
            openEditor();
        } catch (RuntimeException e) {
            if (old != null) {
                old.done = false;
                current = old;
                renderer.setSuppressed(true);
            }
            throw e;
        }
    }

    private void show(Session session, UiScreen screen) {
        current = session;
        renderer.setSuppressed(true);
        try {
            platform.openScreen(new SafeScreen(screen));
        } catch (RuntimeException e) {
            current = null;
            renderer.setSuppressed(false);
            throw e;
        }
    }

    /** The close callback of one screen. Runs once; later calls (the platform calling closed() as well) do nothing. */
    private final class Session implements Runnable {
        final HudEditorModel editorModel;
        Runnable delegate;
        boolean done;

        Session(HudEditorModel editorModel) {
            this.editorModel = editorModel;
        }

        @Override
        public void run() {
            if (done) {
                return;
            }
            done = true;
            if (current == this) {
                current = null;
                renderer.setSuppressed(false);
            }
            try {
                if (editorModel == null) {
                    delegate.run();
                } else {
                    closeEditor(this);
                }
            } catch (RuntimeException e) {
                failure(SITE_SCREEN, "closing a screen", e);
            }
        }
    }

    private void closeEditor(Session s) {
        HudEditorModel model = s.editorModel;
        // Changes made in the settings screen that could not be saved before: HudEditorClose only saves the layout.
        if (!model.dirty() && dirty) {
            trySave();
        }
        boolean layoutPending = model.dirty();
        s.delegate.run();
        if (layoutPending) {
            // HudEditorClose saved it (no longer dirty) or failed (still dirty: the tick retries).
            dirty = model.dirty();
            if (!dirty) {
                lastSaveNanos = time.nanoTime();
                failureLogged[SITE_SAVE] = false;
            }
        }
    }

    // ---- profile and saving ----

    private void switchProfile(String name) {
        String old = settings.activeProfile();
        Profile loaded = profileStore.load(name);
        settings.setActiveProfile(name);
        try {
            settingsStore.save(settings);
        } catch (IOException e) {
            settings.setActiveProfile(old);
            throw new UncheckedIOException(e);
        }
        profile = loaded;
        profileName = name;
        dirty = false;
        renderer.rebuild(profile);
    }

    /** The settings model's save hook: an IO failure goes up as a RuntimeException, the state stays dirty. */
    private void saveForModel() {
        if (!dirty) {
            return;
        }
        try {
            saveProfile();
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
    }

    private void saveProfile() throws IOException {
        try {
            if (!ProfileStore.sameName(profileName, settings.activeProfile())) {
                throw new IOException("the loaded profile " + profileName + " is not the active one");
            }
            profileStore.save(profileName, profile);
        } finally {
            lastSaveNanos = time.nanoTime();
        }
        dirty = false;
        failureLogged[SITE_SAVE] = false;
        Session s = current;
        if (s != null && s.editorModel != null) {
            s.editorModel.markSaved();
        }
    }

    /** Batched save for tick and shutdown: a failure is logged once and leaves the state dirty. */
    private void trySave() {
        try {
            saveProfile();
        } catch (IOException | RuntimeException e) {
            failure(SITE_SAVE, "saving profile " + profileName, e);
        }
    }

    private void failure(int site, String what, Exception e) {
        if (!failureLogged[site]) {
            failureLogged[site] = true;
            platform.logWarning("Hengst: " + what + " failed: " + e);
        }
    }

    /** Hands a screen to the platform with every call fenced: an exception never reaches the game. */
    private final class SafeScreen implements UiScreen {
        private final UiScreen screen;

        SafeScreen(UiScreen screen) {
            this.screen = screen;
        }

        @Override
        public void render(Canvas canvas, int mouseX, int mouseY, float partialTick) {
            try {
                screen.render(canvas, mouseX, mouseY, partialTick);
            } catch (RuntimeException e) {
                failure(SITE_SCREEN, "drawing a screen", e);
            }
        }

        @Override
        public boolean mouseClicked(int mouseX, int mouseY, int button) {
            try {
                return screen.mouseClicked(mouseX, mouseY, button);
            } catch (RuntimeException e) {
                failure(SITE_SCREEN, "a mouse click on a screen", e);
                return false;
            }
        }

        @Override
        public boolean mouseReleased(int mouseX, int mouseY, int button) {
            try {
                return screen.mouseReleased(mouseX, mouseY, button);
            } catch (RuntimeException e) {
                failure(SITE_SCREEN, "a mouse release on a screen", e);
                return false;
            }
        }

        @Override
        public boolean mouseDragged(int mouseX, int mouseY, int button) {
            try {
                return screen.mouseDragged(mouseX, mouseY, button);
            } catch (RuntimeException e) {
                failure(SITE_SCREEN, "a mouse drag on a screen", e);
                return false;
            }
        }

        @Override
        public boolean mouseScrolled(int mouseX, int mouseY, double amount) {
            try {
                return screen.mouseScrolled(mouseX, mouseY, amount);
            } catch (RuntimeException e) {
                failure(SITE_SCREEN, "scrolling on a screen", e);
                return false;
            }
        }

        @Override
        public boolean keyPressed(UiKey key, boolean shift, boolean control) {
            try {
                return screen.keyPressed(key, shift, control);
            } catch (RuntimeException e) {
                failure(SITE_SCREEN, "a key press on a screen", e);
                return false;
            }
        }

        @Override
        public void closed() {
            try {
                screen.closed();
            } catch (RuntimeException e) {
                failure(SITE_SCREEN, "closing a screen", e);
            }
        }
    }
}
