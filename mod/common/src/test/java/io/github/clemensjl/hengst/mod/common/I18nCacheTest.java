package io.github.clemensjl.hengst.mod.common;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

/** The resolution of a code is remembered, unsupported codes included, so the class path is read once per file. */
class I18nCacheTest {
    @Test
    void an_unsupported_code_is_resolved_once_and_reads_nothing_the_second_time() {
        I18n first = I18n.load("fr_fr");
        int reads = I18n.fileReads();
        for (int i = 0; i < 1000; i++) {
            assertSame(first, I18n.load("fr_fr"));
        }
        assertEquals(reads, I18n.fileReads());
    }

    @Test
    void a_first_load_reads_at_most_the_two_bundled_files_and_a_switch_back_reads_none() {
        int before = I18n.fileReads();
        I18n.load("de_at");
        assertTrue(I18n.fileReads() - before <= 2);
        int reads = I18n.fileReads();
        I18n.load("de_de");
        I18n.load("en_us");
        I18n.load("de_at");
        I18n.load("en_gb");
        assertEquals(reads + 0, I18n.fileReads());
    }

    @Test
    void hostile_codes_share_one_entry_and_never_read() {
        I18n.load("en_us");
        I18n.load("de_de");
        int reads = I18n.fileReads();
        I18n a = I18n.load("../../etc/passwd");
        assertSame(a, I18n.load("x\0y"));
        assertSame(a, I18n.load(new String(new char[100000]).replace('\0', 'a')));
        assertEquals(reads, I18n.fileReads());
    }
}
