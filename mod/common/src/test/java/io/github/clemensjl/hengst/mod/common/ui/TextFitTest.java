package io.github.clemensjl.hengst.mod.common.ui;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.sun.management.ThreadMXBean;
import io.github.clemensjl.hengst.mod.common.testutil.CountingCanvas;
import io.github.clemensjl.hengst.mod.common.testutil.FakeCanvas;
import java.lang.management.ManagementFactory;
import org.junit.jupiter.api.Tag;
import org.junit.jupiter.api.Test;

class TextFitTest {
    private static final String SMILE = "😀";

    FakeCanvas c = new FakeCanvas();

    @Test void prefix_keeps_a_fitting_text_and_cuts_the_end_otherwise() {
        String s = "Hello";
        assertSame(s, TextFit.prefix(c, s, 30));
        assertEquals("Hel", TextFit.prefix(c, s, 18));
        assertEquals("", TextFit.prefix(c, s, 5));
        assertEquals("", TextFit.prefix(c, null, 30));
    }

    @Test void prefix_edges() {
        assertEquals("", TextFit.prefix(c, "Hello", 0));
        assertEquals("", TextFit.prefix(c, "Hello", -7));
        String empty = "";
        assertSame(empty, TextFit.prefix(c, empty, 0));
        assertEquals("", TextFit.prefix(c, empty, -1));
        assertEquals("", TextFit.prefix(c, null, 0));
        assertEquals("Hell", TextFit.prefix(c, "Hello", 29));
    }

    @Test void middle_keeps_start_and_end() {
        assertEquals("abc..ij", TextFit.middle(c, "abcdefghij", 42));
        assertEquals("abcdefghij", TextFit.middle(c, "abcdefghij", 60));
        assertEquals("ab", TextFit.middle(c, "abcdefghij", 13));
    }

    @Test void middle_counts_the_ellipsis_and_puts_the_extra_char_of_an_odd_split_in_front() {
        assertEquals("ab..ij", TextFit.middle(c, "abcdefghij", 41));
        assertEquals("abc..hij", TextFit.middle(c, "abcdefghij", 48));
        assertEquals("ab..ij", TextFit.middle(c, "abcdefghij", 36));
        assertEquals("ab..j", TextFit.middle(c, "abcdefghij", 35));
        assertEquals("a..j", TextFit.middle(c, "abcdefghij", 24));
        assertEquals("a..", TextFit.middle(c, "abcdefghij", 18));
    }

    @Test void middle_edges() {
        String fits = "abcdefghij";
        assertSame(fits, TextFit.middle(c, fits, 60));
        assertSame(fits, TextFit.middle(c, fits, 100));
        String empty = "";
        assertSame(empty, TextFit.middle(c, empty, 0));
        assertEquals("", TextFit.middle(c, "abcdefghij", 0));
        assertEquals("", TextFit.middle(c, "abcdefghij", -5));
        assertEquals("a", TextFit.middle(c, "abcdefghij", 11));
        assertSame(empty, TextFit.middle(c, empty, -1));
    }

    @Test void middle_is_never_wider_than_the_space_when_not_even_the_ellipsis_plus_a_char_fits() {
        assertEquals("", TextFit.middle(c, "W", 3));
        // ".." fits exactly (12 px) but no character fits next to it (18 px): plain cut by the rule
        assertEquals("ab", TextFit.middle(c, "abcdefghij", 12));
        assertEquals("ab", TextFit.middle(c, "abcdefghij", 17));
        assertEquals("a..", TextFit.middle(c, "abcdefghij", 18));
        for (int avail = -3; avail < 70; avail++) {
            assertTrue(c.textWidth(TextFit.middle(c, "abcdefghij", avail)) <= Math.max(avail, 0), "avail " + avail);
        }
    }

    @Test void adjacent_surrogate_pairs_at_the_cut_stay_whole() {
        // prefix: the cut would land between the two pairs' halves
        assertEquals("a" + SMILE, TextFit.prefix(c, "a" + SMILE + SMILE + "b", 24));
        assertEquals("a", TextFit.prefix(c, "a" + SMILE + SMILE + "b", 12));
        // middle: head and tail boundaries both fall inside pairs
        String four = SMILE + SMILE + SMILE + SMILE;
        assertEquals(SMILE + "..", TextFit.middle(c, four, 30));
        assertNoLoneSurrogate(TextFit.middle(c, four, 30));
        for (int avail = 0; avail < 50; avail++) {
            assertNoLoneSurrogate(TextFit.middle(c, four, avail));
            assertNoLoneSurrogate(TextFit.prefix(c, four, avail));
        }
    }

    @Test void a_too_narrow_choice_widget_never_draws_text_wider_than_its_text_area() {
        final int[] widest = {0};
        FakeCanvas rec = new FakeCanvas() {
            @Override
            public void text(String text, int x, int y, int argb, boolean shadow) {
                widest[0] = Math.max(widest[0], textWidth(text));
            }
        };
        for (int w = 0; w < 80; w++) {
            ChoiceWidget cw = new ChoiceWidget("Profile", new String[] {"Competitive 12"}, () -> "Competitive 12", s -> { });
            cw.x = 0; cw.y = 0; cw.w = w; cw.h = 20;
            widest[0] = 0;
            cw.render(rec, 0, 0);
            assertTrue(widest[0] <= Math.max(w - 4, 0), "w " + w + " drew " + widest[0]);
        }
    }

    @Test void wrap_by_words_and_long_words_alone() {
        assertArrayEquals(new String[] {"aa bb", "cc"}, TextFit.wrap(c, "aa bb cc", 30));
        assertArrayEquals(new String[] {"a", "verylongword", "b"}, TextFit.wrap(c, "a verylongword b", 30));
        assertArrayEquals(new String[0], TextFit.wrap(c, "   ", 30));
    }

    @Test void wrap_edges() {
        assertArrayEquals(new String[] {"aa", "bb"}, TextFit.wrap(c, "aa     bb", 12));
        assertArrayEquals(new String[] {"aa bb"}, TextFit.wrap(c, "aa     bb", 30));
        assertArrayEquals(new String[] {"aa", "bb"}, TextFit.wrap(c, "  aa bb  ", 12));
        assertArrayEquals(new String[0], TextFit.wrap(c, "", 30));
        // a line that is exactly as wide as the width stays together; one pixel less breaks it
        assertArrayEquals(new String[] {"aa bb"}, TextFit.wrap(c, "aa bb", 30));
        assertArrayEquals(new String[] {"aa", "bb"}, TextFit.wrap(c, "aa bb", 29));
        assertArrayEquals(new String[] {"aaaaa", "bbbbb"}, TextFit.wrap(c, "aaaaa bbbbb", 30));
        // a word exactly as wide as the line is fine, a longer one stands alone and is not broken
        assertArrayEquals(new String[] {"aaaaa", "b"}, TextFit.wrap(c, "aaaaa b", 30));
        assertArrayEquals(new String[] {"aaaaaaaa"}, TextFit.wrap(c, "aaaaaaaa", 30));
        assertArrayEquals(new String[] {"x", "aaaaaaaa", "y"}, TextFit.wrap(c, "x aaaaaaaa y", 30));
    }

    @Test void prefix_never_splits_a_surrogate_pair() {
        // "ab" + smiley + "cd": 18 px would keep the high surrogate alone
        assertEquals("ab", TextFit.prefix(c, "ab" + SMILE + "cd", 18));
        assertEquals("ab" + SMILE, TextFit.prefix(c, "ab" + SMILE + "cd", 24));
        assertEquals("", TextFit.prefix(c, SMILE + "cd", 6));
    }

    @Test void middle_never_splits_a_surrogate_pair_in_either_half() {
        // head of 3 would end between the surrogates: one char earlier
        assertEquals("ab..def", TextFit.middle(c, "ab" + SMILE + "cdef", 42));
        // tail of 2 would start between the surrogates: one char later
        assertEquals("abc..g", TextFit.middle(c, "abcdef" + SMILE + "g", 42));
        assertNoLoneSurrogate(TextFit.middle(c, "ab" + SMILE + "cdef", 42));
        assertNoLoneSurrogate(TextFit.middle(c, "abcdef" + SMILE + "g", 42));
    }

    @Test void wrap_keeps_surrogate_pairs_whole() {
        String word = SMILE + SMILE + SMILE + SMILE;
        String[] lines = TextFit.wrap(c, "a " + word + " b", 30);
        assertArrayEquals(new String[] {"a", word, "b"}, lines);
        for (String line : lines) {
            assertNoLoneSurrogate(line);
        }
    }

    @Test void a_fitting_text_is_measured_once_by_a_widget() {
        final int[] measured = {0};
        FakeCanvas counting = new FakeCanvas() {
            @Override
            public int textWidth(String text) {
                measured[0]++;
                return super.textWidth(text);
            }
        };
        ToggleWidget t = new ToggleWidget("Hi", () -> true, b -> { });
        t.x = 0; t.y = 0; t.w = 100; t.h = 20;
        t.render(counting, 0, 0);
        assertEquals(1, measured[0]);
        t.render(counting, 0, 0);
        assertEquals(1, measured[0]);
    }

    @Tag("interpreted")
    @Test void fitting_text_allocates_nothing() {
        ThreadMXBean mx = (ThreadMXBean) ManagementFactory.getThreadMXBean();
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
        assertEquals(0, used, "allocated " + used + " bytes");
    }

    @Tag("interpreted")
    @Test void widgets_allocate_nothing_on_the_second_frame_with_the_same_too_wide_text() {
        ThreadMXBean mx = (ThreadMXBean) ManagementFactory.getThreadMXBean();
        long id = Thread.currentThread().getId();
        CountingCanvas cc = new CountingCanvas();
        String wideLabel = "A label much too wide for this toggle";
        String wideValue = "Competitive Season Setup 12";
        ToggleWidget toggle = new ToggleWidget(wideLabel, () -> true, b -> { });
        toggle.x = 0; toggle.y = 0; toggle.w = 100; toggle.h = 20;
        ChoiceWidget choice = new ChoiceWidget("Profile", new String[] {wideValue}, () -> wideValue, s -> { });
        choice.x = 0; choice.y = 30; choice.w = 100; choice.h = 20;
        toggle.render(cc, 0, 0);
        choice.render(cc, 0, 0);
        assertEquals(2, cc.texts);
        long before = mx.getThreadAllocatedBytes(id);
        for (int i = 0; i < 1000; i++) {
            toggle.render(cc, 0, 0);
            choice.render(cc, 0, 0);
        }
        long used = mx.getThreadAllocatedBytes(id) - before;
        assertEquals(0, used, "allocated " + used + " bytes");
    }

    private static void assertNoLoneSurrogate(String s) {
        for (int i = 0; i < s.length(); i++) {
            char ch = s.charAt(i);
            if (Character.isHighSurrogate(ch)) {
                assertTrue(i + 1 < s.length() && Character.isLowSurrogate(s.charAt(i + 1)), "lone high surrogate in " + s);
                i++;
            } else {
                assertFalse(Character.isLowSurrogate(ch), "lone low surrogate in " + s);
            }
        }
    }
}
