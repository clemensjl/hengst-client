package io.github.clemensjl.hengst.mod.common;

import io.github.clemensjl.hengst.mod.common.platform.Platform;
import io.github.clemensjl.hengst.mod.common.platform.TimeSource;
import java.util.function.Supplier;

/**
 * Fault isolation shared by the adapters: starts the core without letting a failure reach the game, and decides what a
 * failed hook means. Callers catch {@code RuntimeException | LinkageError}; VM errors such as OutOfMemoryError and
 * StackOverflowError are deliberately not caught and keep crashing the game. One instance per adapter, used from the
 * client thread.
 */
public final class AdapterGuard {
    /** Where the adapter writes; built by the adapter on its logging framework. */
    public interface Log {
        void info(String message);

        void error(String message, Throwable error);
    }

    /** Different exception classes logged per hook site. */
    public static final int MAX_KINDS = 4;

    private final String[] siteNames;
    private final Log log;
    private final Class<?>[][] loggedKinds;
    private final int[] loggedCount;
    private boolean linkageLogged;

    /** {@code siteNames}: the hook sites, indexed by the adapter's site constants; used in the log lines. */
    public AdapterGuard(String[] siteNames, Log log) {
        this.siteNames = siteNames.clone();
        this.log = log;
        this.loggedKinds = new Class<?>[siteNames.length][MAX_KINDS];
        this.loggedCount = new int[siteNames.length];
    }

    /**
     * Creates the platform, logs the load line and starts the core; null after logging why. Caught: runtime exceptions
     * and linkage errors raised while the platform and the core are created (a common class that fails its static
     * initializer or references something missing). Not caught: VM errors.
     */
    public HengstCore start(Supplier<? extends Platform> platformFactory, TimeSource time) {
        try {
            Platform platform = platformFactory.get();
            log.info("[hengst] loaded on " + platform.version().id());
            return HengstCore.start(platform, time);
        } catch (RuntimeException | LinkageError e) {
            log.error("[hengst] core failed to start; the mod stays inactive and its key bindings do nothing", e);
            return null;
        }
    }

    /**
     * Cold path; nothing thrown by the mod reaches the game. A linkage error (the game or the API mod does not have what
     * the adapter was built against) would repeat on every call: returns true, the caller switches the mod off, and it
     * is logged the first time only. Any other failure returns false and is logged once per site and exception class
     * (the first {@link #MAX_KINDS} classes of a site). Allocates only when it logs.
     */
    public boolean hookFailed(int site, Throwable error) {
        if (error instanceof LinkageError) {
            if (!linkageLogged) {
                linkageLogged = true;
                log.error("[hengst] the " + siteNames[site] + " hook does not match this game; the mod is disabled",
                        error);
            }
            return true;
        }
        Class<?> kind = error.getClass();
        int n = loggedCount[site];
        for (int i = 0; i < n; i++) {
            if (loggedKinds[site][i] == kind) {
                return false;
            }
        }
        if (n == MAX_KINDS) {
            return false;
        }
        loggedKinds[site][n] = kind;
        loggedCount[site] = n + 1;
        log.error("[hengst] the " + siteNames[site] + " hook failed; further " + kind.getSimpleName()
                + " there are not logged", error);
        return false;
    }
}
