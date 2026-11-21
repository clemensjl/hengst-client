package io.github.clemensjl.hengst.mod.common.platform;

public interface InputView {
    boolean isDown(GameKey key);

    /** Display name of the current binding, for example "W". */
    String keyLabel(GameKey key);
}
