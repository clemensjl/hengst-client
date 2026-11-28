package io.github.clemensjl.hengst.mod.common.testutil;

import io.github.clemensjl.hengst.mod.common.platform.WallClock;

public final class FakeWallClock implements WallClock {
    public long utcMillis;
    public int offset;
    public long switchAtUtc = Long.MAX_VALUE;
    public int offsetAfterSwitch;
    public int currentCalls;
    public int offsetCalls;

    @Override
    public long currentTimeMillis() {
        currentCalls++;
        return utcMillis;
    }

    @Override
    public int offsetMillis(long utcMillis) {
        offsetCalls++;
        return utcMillis < switchAtUtc ? offset : offsetAfterSwitch;
    }
}
