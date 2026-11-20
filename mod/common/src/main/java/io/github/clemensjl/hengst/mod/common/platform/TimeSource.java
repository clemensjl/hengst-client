package io.github.clemensjl.hengst.mod.common.platform;

public interface TimeSource {
    TimeSource SYSTEM = System::nanoTime;

    long nanoTime();
}
