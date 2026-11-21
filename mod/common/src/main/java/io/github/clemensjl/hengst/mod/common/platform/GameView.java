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
     * and on a dimension change. Allocation free.
     */
    Object worldIdentity();
}
