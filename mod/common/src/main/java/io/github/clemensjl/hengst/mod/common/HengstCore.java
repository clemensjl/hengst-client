package io.github.clemensjl.hengst.mod.common;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.config.Profile;
import io.github.clemensjl.hengst.mod.common.config.ProfileStore;
import io.github.clemensjl.hengst.mod.common.config.Settings;
import io.github.clemensjl.hengst.mod.common.config.SettingsStore;
import io.github.clemensjl.hengst.mod.common.editor.HudEditorModel;
import io.github.clemensjl.hengst.mod.common.editor.HudEditorScreen;
import io.github.clemensjl.hengst.mod.common.feature.Feature;
import io.github.clemensjl.hengst.mod.common.feature.FeatureGate;
import io.github.clemensjl.hengst.mod.common.feature.FeatureRegistry;
import io.github.clemensjl.hengst.mod.common.feature.FeatureState;
import io.github.clemensjl.hengst.mod.common.feature.KeySpec;
import io.github.clemensjl.hengst.mod.common.feature.RestrictionPolicy;
import io.github.clemensjl.hengst.mod.common.hud.CpsCounter;
import io.github.clemensjl.hengst.mod.common.hud.FrameCounter;
import io.github.clemensjl.hengst.mod.common.hud.HudContext;
import io.github.clemensjl.hengst.mod.common.hud.HudModule;
import io.github.clemensjl.hengst.mod.common.hud.HudRenderer;
import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import io.github.clemensjl.hengst.mod.common.platform.GameView;
import io.github.clemensjl.hengst.mod.common.platform.Platform;
import io.github.clemensjl.hengst.mod.common.platform.TimeSource;
import io.github.clemensjl.hengst.mod.common.platform.UiKey;
import io.github.clemensjl.hengst.mod.common.platform.UiScreen;
import io.github.clemensjl.hengst.mod.common.platform.WallClock;
import io.github.clemensjl.hengst.mod.common.ui.ScreenClose;
import io.github.clemensjl.hengst.mod.common.ui.SettingsModel;
import io.github.clemensjl.hengst.mod.common.ui.SettingsScreen;
import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.file.Path;
import java.time.Clock;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.function.Function;

/**
 * The one object the game adapters talk to. It owns the registry, the stored settings and profile, the HUD renderer
 * and the counters, and turns the adapters' hooks into work.
 *
 * <p>Contract for adapter authors:
 * <ul>
 * <li>All methods, including {@link #start} and {@link #shutdown}, run on the client thread. Never call
 * {@link #shutdown} from a JVM shutdown hook.</li>
 * <li>{@link #start} may be called from the client mod initializer. It uses only the platform's version, config
 * directory, {@code logWarning} and {@code languageCode} (a failing read is logged and counts as English until the
 * first tick check), and reads files synchronously. No hook may fire before it returned. If it
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
 * the first change. Every other HUD module without an entry in the loaded profile gets one at its default place,
 * switched off. A broken profile file that was moved aside as a backup counts as a first start; an unreadable
 * file or one from a newer version does not (nothing is written over it, no module is enabled).</li>
 * </ul>
 */
public final class HengstCore {
    private static final long SAVE_INTERVAL_NANOS = 2_000_000_000L;
    private static final long LANGUAGE_INTERVAL_NANOS = 1_000_000_000L;
    // How often a failing language load is tried for one code (one per periodic check) before it is given up.
    private static final int MAX_LANGUAGE_ATTEMPTS = 3;
    // "No language loaded yet": a code read that failed at start. Compared by reference, equal to no real code.
    private static final String UNREAD = new String("unread");

    // Failure sites. Each logs once per distinct exception class and is re-armed by the next success of that site.
    private static final int SITE_RENDER = 0;
    private static final int SITE_TICK = 1;
    private static final int SITE_MOUSE = 2;
    private static final int SITE_SCREEN = 3;
    private static final int SITE_OPEN = 4;
    private static final int SITE_SAVE = 5;
    private static final int SITE_FRAME = 6;
    private static final int SITE_SETTINGS = 7;
    private static final int SITE_SERVER = 8;
    private static final int SITE_LANGUAGE = 9;
    private static final int SITE_HUD_TICK = 10;
    private static final int SITE_KEY = 11;
    private static final int SITES = 12;
    private static final int MAX_CONFIG_KINDS = 32;
    private static final int MAX_KINDS = 4;
    // Key handler failures remembered at once (feature and exception class); further ones stay silent until a success.
    private static final int MAX_KEY_FAILURES = 8;

    private final Platform platform;
    private final TimeSource time;
    private final FeatureRegistry registry = new FeatureRegistry();
    private final FeatureGate gate;
    private final SettingsStore settingsStore;
    private final ProfileStore profileStore;
    private final Settings settings;
    private final FrameCounter frames;
    private final CpsCounter cps;
    private final HudContext ctx;
    private final HudRenderer renderer;
    // Failures of applyConfig already logged, "step or feature|exception class"; cleared by a clean pass.
    private final Set<String> configLogged = new HashSet<String>();
    private final Class<?>[][] loggedKinds = new Class<?>[SITES][MAX_KINDS];
    private final int[] loggedCount = new int[SITES];
    private final boolean[] held = new boolean[2];
    // Key bindings, indexed like keySpecs: the owning feature (null for the two core keys), the key's index in the
    // owner's keys(), whether it is a HOLD key, whether it is held now and whether its press was delivered (HOLD only).
    private final List<KeySpec> keySpecs = new ArrayList<KeySpec>();
    private Feature[] keyOwner = new Feature[0];
    private int[] keyLocal = new int[0];
    private boolean[] keyHold = new boolean[0];
    private boolean[] keyDown = new boolean[0];
    private boolean[] delivered = new boolean[0];
    private final Feature[] keyFailFeature = new Feature[MAX_KEY_FAILURES];
    private final Class<?>[] keyFailKind = new Class<?>[MAX_KEY_FAILURES];
    private int keyFailCount;

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
    // Change detection in the tick: the language code the texts were loaded for, when it was last read, and the server
    // the gate knows. Compared by reference first, then equals; nothing is built per tick.
    private String loadedLanguage;
    private long lastLanguageCheckNanos;
    private String lastServer;
    // A language code whose load failed, and how often it was tried; given up after MAX_LANGUAGE_ATTEMPTS.
    private String failedLanguage;
    private int languageAttempts;
    // The language loader; replaced only by tests.
    private Function<String, I18n> languageLoader;

    private HengstCore(Platform platform, TimeSource time, Clock clock, WallClock wallClock, RestrictionPolicy policy,
            Function<String, I18n> loader) {
        this.platform = platform;
        this.time = time;
        gate = new FeatureGate(platform.version(), policy);
        for (Feature f : FeatureCatalog.create()) {
            registry.register(f);
        }
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
        addDefaultEntries();
        if (firstStart) {
            applyFirstStartDefaults(profile);
            unwritten = true;
        }
        frames = new FrameCounter(time);
        cps = new CpsCounter(time);
        languageLoader = loader;
        String code = readLanguage();
        I18n initial;
        try {
            initial = loader.apply(code == UNREAD ? null : code);
            loadedLanguage = code;
        } catch (RuntimeException | LinkageError e) {
            // start must not throw for this: keys as texts until the periodic check loads the real language.
            platform.logWarning("Hengst: loading the language failed, showing keys until the next check: " + e);
            initial = I18n.of(Collections.<String, Object>emptyMap(), Collections.<String, Object>emptyMap());
            loadedLanguage = UNREAD;
        }
        ctx = new HudContext(platform, time, wallClock, frames, cps, initial);
        renderer = new HudRenderer(registry, gate);
        applyConfig();
        lastSaveNanos = time.nanoTime();
        // The first periodic check runs in the first tick.
        lastLanguageCheckNanos = lastSaveNanos - LANGUAGE_INTERVAL_NANOS;
        registry.seal();
        appendKeys(null, FeatureCatalog.coreKeys());
        for (Feature f : registry.supportedOn(platform.version())) {
            appendKeys(f, f.keys());
        }
    }

    /** Adds key bindings after the existing ones; the state arrays grow with them. Cold path. */
    private void appendKeys(Feature owner, List<KeySpec> specs) {
        int n = keyOwner.length;
        int total = n + specs.size();
        keyOwner = Arrays.copyOf(keyOwner, total);
        keyLocal = Arrays.copyOf(keyLocal, total);
        keyHold = Arrays.copyOf(keyHold, total);
        keyDown = Arrays.copyOf(keyDown, total);
        delivered = Arrays.copyOf(delivered, total);
        for (int i = 0; i < specs.size(); i++) {
            KeySpec spec = specs.get(i);
            keySpecs.add(spec);
            keyOwner[n + i] = owner;
            keyLocal[n + i] = i;
            keyHold[n + i] = spec.mode() == KeySpec.Mode.HOLD;
        }
    }

    /** See the class comment for when to call it. Throws only if the platform itself fails. */
    public static HengstCore start(Platform platform, TimeSource time) {
        return start(platform, time, Clock.systemDefaultZone());
    }

    static HengstCore start(Platform platform, TimeSource time, Clock clock) {
        return start(platform, time, clock, RestrictionPolicy.NONE);
    }

    static HengstCore start(Platform platform, TimeSource time, Clock clock, RestrictionPolicy policy) {
        return start(platform, time, clock, WallClock.SYSTEM, policy);
    }

    static HengstCore start(Platform platform, TimeSource time, Clock clock, WallClock wallClock,
            RestrictionPolicy policy) {
        return new HengstCore(platform, time, clock, wallClock, policy, I18n::load);
    }

    /** For tests: like the other overload, with the loader used at start and in the checks. */
    static HengstCore start(Platform platform, TimeSource time, Clock clock, WallClock wallClock,
            RestrictionPolicy policy, Function<String, I18n> loader) {
        return new HengstCore(platform, time, clock, wallClock, policy, loader);
    }

    /** The code once at start. A throw is logged once; English until the first tick check reads it again. */
    private String readLanguage() {
        try {
            return platform.languageCode();
        } catch (RuntimeException e) {
            platform.logWarning("Hengst: reading the language failed, using English until the next check: " + e);
            return UNREAD;
        }
    }

    /** The four M0 modules, on (their places come from {@link #addDefaultEntries}). Memory only: nothing is written until the first change. */
    private static void applyFirstStartDefaults(Profile p) {
        p.module("fps").setEnabled(true);
        p.module("cps").setEnabled(true);
        p.module("coordinates").setEnabled(true);
        p.module("keystrokes").setEnabled(true);
    }

    /**
     * Gives every HUD module of this version that the loaded profile has no entry for one at its default place,
     * switched off. Memory only: it is written with the next save.
     */
    private void addDefaultEntries() {
        Set<String> known = profile.moduleIds();
        for (Feature f : registry.supportedOn(platform.version())) {
            if (f instanceof HudModule && !known.contains(f.id())) {
                HudModule m = (HudModule) f;
                ModuleConfig c = profile.module(m.id());
                c.setEnabled(false);
                c.setAnchor(m.defaultAnchor());
                c.setOffsetX(m.defaultOffsetX());
                c.setOffsetY(m.defaultOffsetY());
            }
        }
    }

    /**
     * Applies the loaded profile in three independent steps: rebuilds the renderer, tells every supported feature its
     * configuration, and publishes the feature state. A step that throws (a runtime exception or a linkage error) is
     * logged once and does not stop the others. A feature whose notification threw is off in the published state, so it
     * never runs with stale option fields; if the publishing itself fails, the previous state stays. Cold path; the
     * one place that follows a configuration change.
     */
    private void applyConfig() {
        boolean failed = false;
        try {
            renderer.rebuild(profile);
        } catch (RuntimeException | LinkageError e) {
            failed = true;
            configFailure("rebuilding the HUD (keeping the previous HUD)", e);
        }
        List<Feature> all = registry.all();
        List<Feature> healthy = new ArrayList<Feature>(all.size());
        List<Feature> broken = new ArrayList<Feature>();
        for (Feature f : all) {
            if (!f.supports(platform.version())) {
                healthy.add(f);
                continue;
            }
            try {
                f.onConfigChanged(profile.module(f.id()));
                healthy.add(f);
            } catch (RuntimeException | LinkageError e) {
                failed = true;
                broken.add(f);
                configFailure("the configuration of " + f.id(), e);
            }
        }
        try {
            FeatureState.rewrite(healthy, profile, gate);
        } catch (RuntimeException | LinkageError e) {
            failed = true;
            configFailure("publishing the feature state", e);
            // The previous state stays, except for features whose fields are half updated.
            if (!broken.isEmpty()) {
                FeatureState.withdraw(broken);
            }
        }
        if (!failed) {
            configLogged.clear();
        }
    }

    /** Logs a failure of {@link #applyConfig} once per step or feature and exception class until a clean pass. */
    private void configFailure(String what, Throwable e) {
        if (configLogged.size() < MAX_CONFIG_KINDS && configLogged.add(what + "|" + e.getClass().getName())) {
            platform.logWarning("Hengst: " + what + " failed: " + e);
        }
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
            if (screenOpen || !game.inWorld()) {
                releaseHeldKeys();
            }
            Session s = current;
            if (s != null && !screenOpen) {
                // The game removed our screen without calling closed(): end the session as a close would.
                s.run();
                s = null;
            }
            // Each check is isolated: a failure in one never skips the HUD tick or the save below.
            try {
                checkServer(game);
                ok(SITE_SERVER);
            } catch (RuntimeException e) {
                failure(SITE_SERVER, "the server check", e);
            }
            try {
                checkLanguage();
                ok(SITE_LANGUAGE);
            } catch (RuntimeException e) {
                failure(SITE_LANGUAGE, "the language check", e);
            }
            try {
                renderer.tick(ctx);
                ok(SITE_HUD_TICK);
            } catch (RuntimeException e) {
                failure(SITE_HUD_TICK, "the HUD tick", e);
            }
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

    /** Gives every delivered, still held HOLD key its release: a screen opened or the world was left. */
    private void releaseHeldKeys() {
        try {
            boolean[] d = delivered;
            for (int i = 0; i < d.length; i++) {
                if (d[i]) {
                    d[i] = false;
                    deliverKey(i, false);
                }
            }
            ok(SITE_KEY);
        } catch (RuntimeException e) {
            failure(SITE_KEY, "releasing the held keys", e);
        }
    }

    /**
     * A join, a leave or a switch from one server to another (single player is "no server"): feeds the gate and
     * follows with the one configuration path. Same string, even another object: nothing happens, nothing is built.
     */
    private void checkServer(GameView game) {
        String server = game.serverAddress();
        String last = lastServer;
        if (server == last || (server != null && server.equals(last))) {
            return;
        }
        // Recorded first: a failure below is logged once and not retried every tick.
        lastServer = server;
        gate.setServer(server);
        try {
            applyConfig();
        } catch (RuntimeException | LinkageError e) {
            configFailure("applying the server change", e);
        }
    }

    /**
     * At most once a second (the first time in the first tick): reads the language code and, only if it differs from
     * the one the texts were loaded for, loads the new language. The one allowed file read in the tick (spec 8.6):
     * once per language change, never from a frame hook. If the load fails the old texts stay; it is tried again at
     * the next checks, {@link #MAX_LANGUAGE_ATTEMPTS} times in all, then given up (one log line) until the code
     * changes. After a load every module drops what it cached from text measurements; a module that throws there is
     * logged once and does not stop the others.
     */
    private void checkLanguage() {
        long now = time.nanoTime();
        if (now - lastLanguageCheckNanos < LANGUAGE_INTERVAL_NANOS) {
            return;
        }
        lastLanguageCheckNanos = now;
        String code = platform.languageCode();
        if (sameCode(code, loadedLanguage)) {
            // Back on the loaded language: a give-up for another code is forgotten, so selecting it again retries.
            failedLanguage = null;
            languageAttempts = 0;
            return;
        }
        if (!sameCode(code, failedLanguage)) {
            failedLanguage = code;
            languageAttempts = 0;
        } else if (languageAttempts >= MAX_LANGUAGE_ATTEMPTS) {
            return;
        }
        I18n loaded;
        try {
            loaded = languageLoader.apply(code);
        } catch (RuntimeException | LinkageError e) {
            languageAttempts++;
            if (languageAttempts >= MAX_LANGUAGE_ATTEMPTS) {
                platform.logWarning("Hengst: loading the language " + code + " failed " + languageAttempts
                        + " times, keeping the current texts until it changes: " + e);
            }
            return;
        }
        loadedLanguage = code;
        failedLanguage = null;
        languageAttempts = 0;
        ctx.setI18n(loaded);
        for (Feature f : registry.supportedOn(platform.version())) {
            if (f instanceof HudModule) {
                try {
                    ((HudModule) f).onLanguageChanged();
                } catch (RuntimeException | LinkageError e) {
                    configFailure("the language change of " + f.id(), e);
                }
            }
        }
    }

    private static boolean sameCode(String a, String b) {
        return a == b || (a != null && a.equals(b));
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
     * Opens the settings. {@link #onFeatureKey} (index 0) calls it after its own checks; it does not check the world
     * itself. Does nothing if one of the mod's screens is already open.
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

    /**
     * Every state change of a key binding of {@link #keySpecs()}, by index. Contract for the adapters (per tick and
     * key): first {@code onFeatureKey(i, true)} if at least one click was counted since the last tick, then
     * {@code onFeatureKey(i, false)} if the key is no longer down. The core keeps the held state: a press of a held
     * key and a release of a key that is not held do nothing. A press counts only in a world without an open screen
     * and, for a feature key, while {@link FeatureState#on} is true for its feature (a disabled, unsupported or
     * restricted feature does nothing); it then needs a real release before the next press counts. Index 0 opens the
     * settings, 1 the HUD editor. A release goes only to HOLD keys whose press was delivered; the tick sends it when a
     * screen opens or the world is left. Indexes outside the list are ignored. Allocation free in the healthy case; a
     * throwing handler is logged once per feature and exception class and stops nothing.
     */
    public void onFeatureKey(int index, boolean down) {
        boolean[] downs = keyDown;
        if (index < 0 || index >= downs.length) {
            return;
        }
        try {
            if (!down) {
                downs[index] = false;
                if (delivered[index]) {
                    delivered[index] = false;
                    deliverKey(index, false);
                }
                return;
            }
            if (downs[index]) {
                return;
            }
            downs[index] = true;
            GameView game = platform.game();
            if (!game.inWorld() || game.screenOpen()) {
                return;
            }
            Feature owner = keyOwner[index];
            if (owner == null) {
                if (index == 0) {
                    onOpenSettingsKey();
                } else {
                    onOpenHudEditorKey();
                }
                return;
            }
            if (!FeatureState.on(owner.number())) {
                return;
            }
            if (keyHold[index]) {
                delivered[index] = true;
            }
            deliverKey(index, true);
            ok(SITE_KEY);
        } catch (RuntimeException e) {
            failure(SITE_KEY, "a feature key", e);
        }
    }

    /** One handler call, fenced: a throw is logged once per feature and exception class and goes no further. */
    private void deliverKey(int index, boolean down) {
        Feature owner = keyOwner[index];
        try {
            owner.onKey(keyLocal[index], down, time.nanoTime());
            if (keyFailCount != 0) {
                keyHealthy(owner);
            }
        } catch (RuntimeException | LinkageError e) {
            keyFailure(owner, e);
        }
    }

    private void keyFailure(Feature owner, Throwable e) {
        Class<?> kind = e.getClass();
        for (int i = 0; i < keyFailCount; i++) {
            if (keyFailFeature[i] == owner && keyFailKind[i] == kind) {
                return;
            }
        }
        if (keyFailCount >= MAX_KEY_FAILURES) {
            return;
        }
        keyFailFeature[keyFailCount] = owner;
        keyFailKind[keyFailCount] = kind;
        keyFailCount++;
        platform.logWarning("Hengst: a key of " + owner.id() + " failed: " + e);
    }

    /** A handler of this feature ran cleanly: its failures are logged again if they return. */
    private void keyHealthy(Feature owner) {
        int kept = 0;
        for (int i = 0; i < keyFailCount; i++) {
            if (keyFailFeature[i] != owner) {
                keyFailFeature[kept] = keyFailFeature[i];
                keyFailKind[kept] = keyFailKind[i];
                kept++;
            }
        }
        for (int i = kept; i < keyFailCount; i++) {
            keyFailFeature[i] = null;
            keyFailKind[i] = null;
        }
        keyFailCount = kept;
    }

    /** The key bindings the adapters register, in index order for {@link #onFeatureKey}; the same as {@link FeatureCatalog#keys}. */
    public List<KeySpec> keySpecs() {
        return Collections.unmodifiableList(keySpecs);
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

    /** Test access only, not for adapters or features: replaces the function that loads a language. */
    void setLanguageLoaderForTest(Function<String, I18n> loader) {
        languageLoader = loader;
    }

    /** Test access only, not for adapters or features: applies the loaded profile as a close of the settings does. */
    public void applyConfigForTest() {
        applyConfig();
    }

    /** Test access only, not for adapters or features: registers an extra feature after start and applies the config. */
    public void registerForTest(Feature feature) {
        registry.unseal();
        try {
            registry.register(feature);
        } finally {
            registry.seal();
        }
        if (feature.supports(platform.version())) {
            appendKeys(feature, feature.keys());
        }
        applyConfig();
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
        session.delegate = new ScreenClose(model::saveIfDirty, this::applyConfig, platform, "the settings");
        show(session, new SettingsScreen(model, ctx.i18n(), session));
    }

    private void openEditor() {
        HudEditorModel model = new HudEditorModel(renderer.activeModules(), renderer.activeConfigs());
        Session session = new Session(model);
        final String name = profileName;
        final Profile edited = profile;
        session.delegate = new ScreenClose(model.saver(profileStore, name, edited), this::applyConfig, platform,
                "the HUD editor (profile " + name + ")");
        show(session, new HudEditorScreen(model, ctx, ctx.i18n(), session));
    }

    /** The editor replaces the settings screen: the settings session ends without closing anything. */
    private void openEditorFromSettings() {
        Session old = current;
        if (old != null) {
            old.done = true;
        }
        current = null;
        try {
            applyConfig();
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
        // Changes made in the settings screen that could not be saved before: the editor close only saves the layout.
        if (!model.dirty() && dirty) {
            trySave();
        }
        boolean layoutPending = model.dirty();
        s.delegate.run();
        if (layoutPending) {
            // The close saved it (no longer dirty) or failed (still dirty: the tick retries).
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
        addDefaultEntries();
        applyConfig();
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
