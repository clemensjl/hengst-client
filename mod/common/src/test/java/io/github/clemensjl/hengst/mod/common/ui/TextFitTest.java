package io.github.clemensjl.hengst.mod.common.ui;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.testutil.CountingCanvas;
import io.github.clemensjl.hengst.mod.common.testutil.FakeCanvas;
import java.lang.management.ManagementFactory;
import org.junit.jupiter.api.Test;

class TextFitTest {
    FakeCanvas c = new FakeCanvas();

    @Test void prefix_keeps_a_fitting_text_and_cuts_the_end_otherwise() {
        String s = "Hello";
        assertSame(s, TextFit.prefix(c, s, 30));
        assertEquals("Hel", TextFit.prefix(c, s, 18));
        assertEquals("", TextFit.prefix(c, s, 5));
        assertEquals("", TextFit.prefix(c, null, 30));
    }

    @Test void middle_keeps_start_and_end() {
        assertEquals("abc..ij", TextFit.middle(c, "abcdefghij", 42));
        assertEquals("abcdefghij", TextFit.middle(c, "abcdefghij", 60));
        assertEquals("..", TextFit.middle(c, "abcdefghij", 13));
    }

    @Test void wrap_by_words_and_long_words_alone() {
        assertArrayEquals(new String[] {"aa bb", "cc"}, TextFit.wrap(c, "aa bb cc", 30));
        assertArrayEquals(new String[] {"a", "verylongword", "b"}, TextFit.wrap(c, "a verylongword b", 30));
        assertArrayEquals(new String[0], TextFit.wrap(c, "   ", 30));
    }

    @org.junit.jupiter.api.Tag("interpreted")
    @Test void fitting_text_allocates_nothing() {
        com.sun.management.ThreadMXBean mx = (com.sun.management.ThreadMXBean) ManagementFactory.getThreadMXBean();
        long id = Thread.currentThread().getId();
        CountingCanvas cc = new CountingCanvas();
        String s = "Hello";
        for (int i = 0; i < 20000; i++) {
            TextFit.prefix(cc, s, 30);
            TextFit.middle(cc, s, 30);
        }
        long before = mx.getThreadAllocatedBytes(id);
        for (int i = 0; i < 10000; i++) {
            TextFit.prefix(cc, s, 30);
            TextFit.middle(cc, s, 30);
        }
        long used = mx.getThreadAllocatedBytes(id) - before;
        assertTrue(used < 1024, "allocated " + used + " bytes");
    }
}
