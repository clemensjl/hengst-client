package io.github.clemensjl.hengst.mod.common.ui;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.testutil.FakePlatform;
import java.io.IOException;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import org.junit.jupiter.api.Test;

class ScreenCloseTest {
    @Test void save_rebuild_close_in_this_order_and_close_even_after_failures() {
        // FakePlatform is final: the order is recorded by the save and rebuild lambdas, the close is seen through closeCount.
        List<String> order = new ArrayList<>();
        FakePlatform p = new FakePlatform(null);
        new ScreenClose(() -> { order.add("save closeCount=" + p.closeCount); throw new IOException("disk"); },
                () -> { order.add("rebuild closeCount=" + p.closeCount); throw new IllegalStateException(); },
                p, "the settings").run();
        assertEquals(Arrays.asList("save closeCount=0", "rebuild closeCount=0"), order);
        assertEquals(1, p.closeCount);                                    // closed last, in the finally block
        assertEquals(Arrays.asList("Could not save the settings: java.io.IOException: disk",
                "Could not rebuild the HUD after closing the settings: java.lang.IllegalStateException"), p.warnings);
    }

    @Test void a_clean_close_saves_rebuilds_and_closes_without_warnings() {
        List<String> order = new ArrayList<>();
        FakePlatform p = new FakePlatform(null);
        new ScreenClose(() -> order.add("save closeCount=" + p.closeCount), () -> order.add("rebuild closeCount=" + p.closeCount),
                p, "the HUD editor (profile default)").run();
        assertEquals(Arrays.asList("save closeCount=0", "rebuild closeCount=0"), order);
        assertEquals(1, p.closeCount);
        assertTrue(p.warnings.isEmpty());
    }

    @Test void an_unchecked_failure_while_saving_is_logged_and_the_rebuild_still_runs() {
        List<String> order = new ArrayList<>();
        FakePlatform p = new FakePlatform(null);
        new ScreenClose(() -> { throw new IllegalArgumentException("x"); }, () -> order.add("rebuild"), p, "the settings").run();
        assertEquals(Arrays.asList("rebuild"), order);
        assertEquals(Arrays.asList("Could not save the settings: java.lang.IllegalArgumentException: x"), p.warnings);
        assertEquals(1, p.closeCount);
    }
}
