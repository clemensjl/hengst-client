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
}
