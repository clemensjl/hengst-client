package io.github.clemensjl.hengst.mod.common.platform;

/** Read-only view of the game state. Call from the client thread only. */
public interface GameView {
    boolean inWorld();

    double playerX();

    double playerY();

    double playerZ();

    float playerYaw();

    /** F1. */
    boolean hudHidden();

    /** F3. */
    boolean debugOverlayOpen();

    /** Any screen is open (chat, inventory, menu). */
    boolean screenOpen();

    /** Null in single player or the main menu. */
    String serverAddress();

    /**
     * The own player's latency as reported by the server, in milliseconds; -1 in single player, without a connection
     * or without the own entry in the player list. Allocation free.
     */
    int pingMillis();

    /**
     * The client's world instance, compared by reference only; null without a world. A new instance on joining a world
     * and on a dimension change. Allocation free. Consumers compare it with the value they stored and replace that
     * value every tick; they must not keep it beyond the world's life (it references the world object).
     */
    Object worldIdentity();

    /**
     * The server (or the client's own option) asks for reduced debug info: vanilla's debug screen then hides the
     * position and the facing. Modules that show what vanilla withholds there (coordinates, direction, speed) show
     * nothing while it is set. False outside a world. Allocation free.
     */
    boolean reducedDebugInfo();

    /**
     * The JVM's used heap in bytes (total minus free). The default asks {@link Runtime}, which does not allocate; no
     * adapter work. The F3 screen shows the same figure.
     */
    default long memoryUsedBytes() {
        Runtime r = Runtime.getRuntime();
        return r.totalMemory() - r.freeMemory();
    }

    /** The JVM's maximum heap in bytes; {@link Long#MAX_VALUE} without a limit. Does not allocate. */
    default long memoryMaxBytes() {
        return Runtime.getRuntime().maxMemory();
    }
}
