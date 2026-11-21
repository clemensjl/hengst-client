package io.github.clemensjl.hengst.mod.common;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.config.Profile;
import io.github.clemensjl.hengst.mod.common.config.ProfileStore;
import io.github.clemensjl.hengst.mod.common.config.Settings;
import io.github.clemensjl.hengst.mod.common.config.SettingsStore;
import io.github.clemensjl.hengst.mod.common.editor.HudEditorClose;
import io.github.clemensjl.hengst.mod.common.editor.HudEditorModel;
import io.github.clemensjl.hengst.mod.common.editor.HudEditorScreen;
import io.github.clemensjl.hengst.mod.common.feature.Feature;
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
 * and the counters, and turns the adapters' hooks into work.
 *
 * <p>Contract for adapter authors:
 * <ul>
 * <li>All methods, including {@link #start} and {@link #shutdown}, run on the client thread. Never call
 * {@link #shutdown} from a JVM shutdown hook.</li>
 * <li>{@link #start} may be called from the client mod initializer. It uses only the platform's version, config
 * directory and {@code logWarning}, and reads files synchronously. No hook may fire before it returned. If it
 * throws, the adapter keeps no core and treats every hook as a no-op.</li>
 * <li>Hooks are safe after {@link #shutdown}; a change made later is saved again by the tick.</li>
 * <li>{@link Platform#game()} and {@link Platform#input()} return long-lived objects and must not allocate.</li>
 * <li>No exception from a module or screen leaves a hook. The frame and tick hooks allocate nothing in the healthy
 * case.</li>
 * <li>Saves are rate-limited: a pending change is written on the next tick at least 2 s after the previous save
 * attempt, on every screen close and in {@link #shutdown}. A failing save is retried every 2 s and logged once.</li>
 * <li>The platform's screen host: see {@link #onOpenSettingsKey} and {@link Platform#closeScreen()} for the removal
 * rule. The screens handed to {@link Platform#openScreen} never throw.</li>
 * <li>First start: the four M0 modules are enabled in memory at their default places and nothing is written until
 * the first change. A broken profile file that was moved aside as a backup counts as a first start; an unreadable
 * file or one from a newer version does not (nothing is written over it, no module is enabled).</li>
 * </ul>
 */
public final class HengstCore {
    private static final long SAVE_INTERVAL_NANOS = 2_000_000_000L;

    // Failure sites. Each logs once per distinct exception class and is re-armed by the next success of that site.
    private static final int SITE_RENDER = 0;
    private static final int SITE_TICK = 1;
    private static final int SITE_MOUSE = 2;
    private static final int SITE_SCREEN = 3;
    private static final int SITE_OPEN = 4;
    private static final int SITE_SAVE = 5;
    private static final int SITE_FRAME = 6;
    private static final int SITE_SETTINGS = 7;
    private static final int SITES = 8;
    private static final int MAX_KINDS = 4;

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
    private final Class<?>[][] loggedKinds = new Class<?>[SITES][MAX_KINDS];
    private final int[] loggedCount = new int[SITES];
    private final boolean[] held = new boolean[2];

    private Profile profile;
    // The name the loaded profile belongs to; saves go there, and only while it is still the active one.
    private String profileName;
    private boolean dirty;
    // The loaded profile exists only in memory (first start): the next save request writes it even if nothing changed.
    private boolean unwritten;
    // settings.json does not yet name the active profile (a write after a delete failed): retried with the saves.
    private boolean settingsDirty;
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
            unwritten = true;
        }
        frames = new FrameCounter(time);
        cps = new CpsCounter(time);
        ctx = new HudContext(platform, time, frames, cps);
        renderer = new HudRenderer(registry, platform.version());
        renderer.rebuild(profile);
        lastSaveNanos = time.nanoTime();
        registry.seal();
    }

    /** See the class comment for when to call it. Throws only if the platform itself fails. */
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

    /**
     * Exactly once per rendered game frame, from a per-frame hook that also fires under F1 and while screens are
     * open. It feeds the FPS counter; {@link #onHudRender} does not count frames, because the versions' HUD hooks
     * stop under F1. Allocation free.
     */
    public void onFrame() {
        try {
            frames.onFrame();
            ok(SITE_FRAME);
        } catch (RuntimeException e) {
            failure(SITE_FRAME, "frame counter", e);
        }
    }

    /**
     * From the version's HUD hook, with a valid canvas of the current GUI scale. Draws the active modules unless the
     * HUD is hidden (F1), the player is not in a world, the debug overlay (F3) is open or one of the mod's own
     * screens is open; vanilla screens such as chat and inventory do not hide it. {@code partialTick} is currently
     * unused. Allocation free.
     */
    public void onHudRender(Canvas canvas, float partialTick) {
        try {
            renderer.render(canvas, ctx);
            ok(SITE_RENDER);
        } catch (RuntimeException e) {
            failure(SITE_RENDER, "HUD render", e);
        }
    }

    /**
     * At the end of every client tick (20 Hz), also outside a world and while any screen is open. Ticks the modules,
     * counts nothing, writes pending changes (at most every 2 s) and ends a mod screen the game dropped without
     * telling us. Allocation free unless a displayed value changed or a save runs.
     */
    public void onClientTick() {
        try {
            GameView game = platform.game();
            boolean screenOpen = game.screenOpen();
            if (screenOpen) {
                // A release may have gone to the screen: do not let a stale "held" swallow the next click.
                held[0] = false;
                held[1] = false;
            }
            Session s = current;
            if (s != null && !screenOpen) {
                // The game removed our screen without calling closed(): end the session as a close would.
                s.run();
                s = null;
            }
            renderer.tick(ctx);
            if (s != null && s.editorModel != null && s.editorModel.dirty()) {
                dirty = true;
            }
            if ((dirty || settingsDirty) && time.nanoTime() - lastSaveNanos >= SAVE_INTERVAL_NANOS) {
                trySave();
            }
            ok(SITE_TICK);
        } catch (RuntimeException e) {
            failure(SITE_TICK, "client tick", e);
        }
    }

    /**
     * Every physical mouse button transition, press and release, regardless of key bindings and of open screens:
     * 0 left, 1 right, 2 middle (ignored). Only the press edge of left and right counts, and only in a world without
     * any screen open; two presses without a release in between count once. Releases the game routes to a screen
     * may be missing: the core tolerates that.
     *
     * <p>1.8.9: use {@code KeyBinding.setKeyPressed(IZ)V} (MCP {@code setKeyBindState}), which sees press and release
     * ({@code onKeyPressed} fires on press only), mapping {@code -100} to 0 and {@code -99} to 1 and dropping
     * keyboard codes, plus {@code Screen.handleInput()V} for the events an open screen drains from LWJGL's queue.
     * Modern versions: {@code MouseHandler.onButton}.
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
            ok(SITE_MOUSE);
        } catch (RuntimeException e) {
            failure(SITE_MOUSE, "mouse button", e);
        }
    }

    /**
     * Once per key-binding press of the settings key. Opening only in a world is the adapter's duty: it checks that
     * before calling, the core does not. Does nothing if one of the mod's screens is already open.
     *
     * <p>Screen removal rule: when the game removes a mod screen (any reason), the adapter first marks its host
     * screen as removed and then forwards {@code closed()} to the {@link UiScreen}. The core answers with
     * {@link Platform#closeScreen()}, which must do nothing for a host that is removed or no longer current, so the
     * game's set-screen is never called re-entrantly. As a safety net the tick ends a session whose screen is gone.
     *
     * <p>{@link UiScreen} conventions: mouse coordinates are integer scaled GUI pixels, origin top left, the same
     * space as {@link Canvas} (modern adapters floor the double positions); {@code mouseDragged} receives the
     * absolute position; the scroll amount is in wheel notches, positive when the wheel is rolled away from the user
     * (1.8.9: the sign of the wheel delta); {@code keyPressed} receives key repeats as further presses on all
     * versions (1.8.9: enable repeat events while the host is open); Escape is forwarded to the screen, not handled
     * by the host.
     */
    public void onOpenSettingsKey() {
        if (current != null) {
            return;
        }
        try {
            openSettings();
            ok(SITE_OPEN);
        } catch (RuntimeException e) {
            failure(SITE_OPEN, "opening the settings", e);
        }
    }

    /** Like {@link #onOpenSettingsKey()} for the HUD editor key. */
    public void onOpenHudEditorKey() {
        if (current != null) {
            return;
        }
        try {
            openEditor();
            ok(SITE_OPEN);
        } catch (RuntimeException e) {
            failure(SITE_OPEN, "opening the HUD editor", e);
        }
    }

    /**
     * When the client stops (client thread, never from a JVM shutdown hook): writes pending changes. Safe to call
     * more than once; hooks stay usable afterwards.
     */
    public void shutdown() {
        Session s = current;
        if (s != null && s.editorModel != null && s.editorModel.dirty()) {
            dirty = true;
        }
        if (dirty || settingsDirty) {
            trySave();
        }
    }

    // ---- for tests ----
    // Test access only: adapters and features must not use these. They are public because the adapters' in-game tests
    // live in other packages.

    /** Test access only, not for adapters or features: the loaded profile. */
    public Profile activeProfile() {
        return profile;
    }

    /** Test access only, not for adapters or features: the HUD renderer. */
    public HudRenderer renderer() {
        return renderer;
    }

    /** Test access only, not for adapters or features: the shared counters and platform. */
    public HudContext context() {
        return ctx;
    }

    /**
     * Test access only, not for adapters or features: the registry, sealed since {@link #start}. Registering on it
     * throws {@link IllegalStateException}; use {@link #registerForTest}.
     */
    public FeatureRegistry registry() {
        return registry;
    }

    /** Test access only, not for adapters or features: registers an extra feature after start and rebuilds the HUD. */
    public void registerForTest(Feature feature) {
        registry.unseal();
        try {
            registry.register(feature);
        } finally {
            registry.seal();
        }
        renderer.rebuild(profile);
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
                unwritten = false;
                lastSaveNanos = time.nanoTime();
                ok(SITE_SAVE);
            }
        }
    }

    // ---- profile and saving ----

    /**
     * The settings model's switch hook. Normally atomic: on a failed settings.json write nothing changes and the
     * exception goes up. If the loaded profile is no longer the active one (the model already selected "default"
     * after deleting it), the in-memory switch is carried out anyway and settings.json is retried by the saves.
     */
    private void switchProfile(String name) {
        boolean forced = !ProfileStore.sameName(profileName, settings.activeProfile());
        String old = settings.activeProfile();
        Profile loaded = profileStore.load(name);
        settings.setActiveProfile(name);
        boolean written = true;
        try {
            settingsStore.save(settings);
        } catch (IOException e) {
            if (!forced) {
                settings.setActiveProfile(old);
                throw new UncheckedIOException(e);
            }
            written = false;
            failure(SITE_SETTINGS, "writing settings.json", e);
        }
        settingsDirty = !written;
        profile = loaded;
        profileName = name;
        dirty = false;
        unwritten = false;
        renderer.rebuild(profile);
    }

    /** The settings model's save hook: an IO failure goes up as a RuntimeException, the state stays dirty. */
    private void saveForModel() {
        if (!dirty && !unwritten) {
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
        unwritten = false;
        ok(SITE_SAVE);
        Session s = current;
        if (s != null && s.editorModel != null) {
            s.editorModel.markSaved();
        }
    }

    private void saveSettings() throws IOException {
        try {
            settingsStore.save(settings);
        } finally {
            lastSaveNanos = time.nanoTime();
        }
        settingsDirty = false;
        ok(SITE_SETTINGS);
    }

    /** Batched save for tick and shutdown: a failure is logged and leaves the state pending. */
    private void trySave() {
        if (dirty) {
            try {
                saveProfile();
            } catch (IOException | RuntimeException e) {
                failure(SITE_SAVE, "saving profile " + profileName, e);
            }
        }
        if (settingsDirty) {
            try {
                saveSettings();
            } catch (IOException | RuntimeException e) {
                failure(SITE_SETTINGS, "saving settings.json", e);
            }
        }
    }

    /** Healthy path of a site: re-arms its failure log. No allocation. */
    private void ok(int site) {
        if (loggedCount[site] != 0) {
            loggedCount[site] = 0;
        }
    }

    /** Logs a failure once per site and exception class until {@link #ok} re-arms the site. */
    private void failure(int site, String what, Exception e) {
        Class<?> kind = e.getClass();
        int n = loggedCount[site];
        for (int i = 0; i < n; i++) {
            if (loggedKinds[site][i] == kind) {
                return;
            }
        }
        if (n >= MAX_KINDS) {
            // The table is full: further classes at this site stay silent until a success re-arms it.
            return;
        }
        loggedKinds[site][n] = kind;
        loggedCount[site] = n + 1;
        platform.logWarning("Hengst: " + what + " failed: " + e);
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
                ok(SITE_SCREEN);
            } catch (RuntimeException e) {
                failure(SITE_SCREEN, "drawing a screen", e);
            }
        }

        @Override
        public boolean mouseClicked(int mouseX, int mouseY, int button) {
            try {
                boolean r = screen.mouseClicked(mouseX, mouseY, button);
                ok(SITE_SCREEN);
                return r;
            } catch (RuntimeException e) {
                failure(SITE_SCREEN, "a mouse click on a screen", e);
                return false;
            }
        }

        @Override
        public boolean mouseReleased(int mouseX, int mouseY, int button) {
            try {
                boolean r = screen.mouseReleased(mouseX, mouseY, button);
                ok(SITE_SCREEN);
                return r;
            } catch (RuntimeException e) {
                failure(SITE_SCREEN, "a mouse release on a screen", e);
                return false;
            }
        }

        @Override
        public boolean mouseDragged(int mouseX, int mouseY, int button) {
            try {
                boolean r = screen.mouseDragged(mouseX, mouseY, button);
                ok(SITE_SCREEN);
                return r;
            } catch (RuntimeException e) {
                failure(SITE_SCREEN, "a mouse drag on a screen", e);
                return false;
            }
        }

        @Override
        public boolean mouseScrolled(int mouseX, int mouseY, double amount) {
            try {
                boolean r = screen.mouseScrolled(mouseX, mouseY, amount);
                ok(SITE_SCREEN);
                return r;
            } catch (RuntimeException e) {
                failure(SITE_SCREEN, "scrolling on a screen", e);
                return false;
            }
        }

        @Override
        public boolean keyPressed(UiKey key, boolean shift, boolean control) {
            try {
                boolean r = screen.keyPressed(key, shift, control);
                ok(SITE_SCREEN);
                return r;
            } catch (RuntimeException e) {
                failure(SITE_SCREEN, "a key press on a screen", e);
                return false;
            }
        }

        @Override
        public void closed() {
            try {
                screen.closed();
                ok(SITE_SCREEN);
            } catch (RuntimeException e) {
                failure(SITE_SCREEN, "closing a screen", e);
            }
        }
    }
}
