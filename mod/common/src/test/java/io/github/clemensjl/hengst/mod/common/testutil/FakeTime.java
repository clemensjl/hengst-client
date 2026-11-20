package io.github.clemensjl.hengst.mod.common.testutil;

import io.github.clemensjl.hengst.mod.common.platform.TimeSource;

public class FakeTime implements TimeSource {
    public long now;

    @Override
    public long nanoTime() {
        return now;
    }

    public void advanceMillis(long ms) {
        now += ms * 1_000_000L;
    }
}
