package io.github.clemensjl.hengst.mod.common.platform;

public enum GameVersion {
    V1_8_9("1.8.9"), V1_21_11("1.21.11"), V26_3("26.3");

    private final String id;

    GameVersion(String id) {
        this.id = id;
    }

    public String id() {
        return id;
    }
}
