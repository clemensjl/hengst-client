package io.github.clemensjl.hengst.mod.common.hud.modules;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.hud.CpsCounter;
import io.github.clemensjl.hengst.mod.common.hud.FrameCounter;
import io.github.clemensjl.hengst.mod.common.hud.HudContext;
import io.github.clemensjl.hengst.mod.common.testutil.FakeCanvas;
import io.github.clemensjl.hengst.mod.common.testutil.FakePlatform;
import io.github.clemensjl.hengst.mod.common.testutil.FakeTime;
import java.nio.file.Path;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class ServerAddressModuleTest {
    @TempDir Path dir;
    FakePlatform platform;
    HudContext ctx;

    @BeforeEach
    void setUp() {
        platform = new FakePlatform(dir);
        platform.inWorld = true;
        FakeTime time = new FakeTime();
        ctx = new HudContext(platform, time, new FrameCounter(time), new CpsCounter(time));
    }

    private String shown(String address, boolean showPort) {
        ServerAddressModule m = new ServerAddressModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        cfg.setOption("show_port", showPort);
        platform.serverAddress = address;
        m.tick(ctx, cfg);
        return m.text();
    }

    @Test
    void server_address_port_handling() {
        String[][] cases = {
            {"mc.example.net:25570", "mc.example.net"},
            {"[2001:db8::1]:25565", "[2001:db8::1]"},
            {"2001:db8::1", "2001:db8::1"},
            {"Example.NET", "Example.NET"},
            {"[2001:db8::1]", "[2001:db8::1]"},
            {"[2001:db8::1", "[2001:db8::1"},
            {"example.net.", "example.net."},
            {"EXAMPLE.net.:25565", "EXAMPLE.net."},
            {"münchen.example:25565", "münchen.example"},
            {"xn--mnchen-3ya.example", "xn--mnchen-3ya.example"},
        };
        for (String[] c : cases) {
            assertEquals(c[1], shown(c[0], false), c[0]);
            assertEquals(c[0], shown(c[0], true), c[0] + " with port");
        }
    }

    @Test
    void a_long_server_address_is_cut_in_the_middle() {
        String s = "aaaaaaaaaa.bbbbbbbbbb.cccccccccc.dddddddddd.eeeee";
        assertEquals(49, s.length());
        String expected = s.substring(0, 19) + ".." + s.substring(30);
        assertEquals("aaaaaaaaaa.bbbbbbbb..cc.dddddddddd.eeeee", expected);
        assertEquals("aaaaaaaaaa.bbbbbbbb..cc.dddddddddd.eeeee", shown(s, false));
        assertEquals(40, shown(s, false).length());
    }

    @Test
    void forty_characters_are_kept_and_forty_one_are_cut() {
        StringBuilder b = new StringBuilder();
        for (int i = 0; i < 40; i++) {
            b.append((char) ('a' + i % 26));
        }
        String forty = b.toString();
        assertEquals(forty, shown(forty, false));
        String fortyOne = forty + "z";
        String cut = shown(fortyOne, false);
        assertEquals(40, cut.length());
        assertEquals(fortyOne.substring(0, 19) + ".." + fortyOne.substring(22), cut);
    }

    @Test
    void the_cut_applies_after_the_port_is_dropped_and_with_the_port_shown() {
        String host = "aaaaaaaaaa.bbbbbbbbbb.cccccccccc.dddddddddd.eeeee";
        assertEquals("aaaaaaaaaa.bbbbbbbb..cc.dddddddddd.eeeee", shown(host + ":25565", false));
        String withPort = shown(host + ":25565", true);
        assertEquals(40, withPort.length());
        assertTrue(withPort.endsWith(":25565"));
    }

    @Test
    void the_cut_never_splits_a_surrogate_pair() {
        // 55 chars: a pair at 18/19 (a head of 19 chars would end inside it) and a pair at 35/36 (index len - 20: a
        // tail of 19 chars would start at 36, inside it)
        char[] in = new char[55];
        java.util.Arrays.fill(in, 'c');
        in[18] = '\uD83D';
        in[19] = '\uDE00';
        in[35] = '\uD83D';
        in[36] = '\uDE00';
        String result = shown(new String(in), false);
        assertEquals(18 + 2 + 18, result.length(), "both pairs are dropped from the cut: 18 + .. + 18");
        assertTrue(result.startsWith("cccccccccccccccccc..") && result.endsWith("cccccccccccccccccc"), result);
        assertNoLoneSurrogate(result);
        // pairs on the kept side stay whole: one at 17/18 and one at 37/38
        java.util.Arrays.fill(in, 'c');
        in[17] = '\uD83D';
        in[18] = '\uDE00';
        in[37] = '\uD83D';
        in[38] = '\uDE00';
        result = shown(new String(in), false);
        assertEquals(19 + 2 + 19, result.length());
        assertNoLoneSurrogate(result);
    }

    private static void assertNoLoneSurrogate(String s) {
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (Character.isHighSurrogate(c)) {
                assertTrue(i + 1 < s.length() && Character.isLowSurrogate(s.charAt(i + 1)), "lone high at " + i);
                i++;
            } else {
                assertFalse(Character.isLowSurrogate(c), "lone low at " + i);
            }
        }
    }

    @Test
    void a_section_sign_before_an_ipv6_bracket_does_not_keep_the_port_visible() {
        assertEquals("[::1]", shown("§[::1]:25565", false));
        assertEquals("[::1]", shown("§§[::1]:25565", false));
        assertEquals("[::1]:25565", shown("§[::1]:25565", true));
        assertEquals("host", shown("§host:25565", false));
        assertEquals("::1", shown("§::1", false)); // a plain IPv6 literal has no port
    }

    @Test
    void the_section_sign_is_never_shown() {
        assertEquals("cRed.example", shown("§cRed.example:25565", false));
        assertEquals("cRed.example:25565", shown("§cRed.example:25565", true));
        assertEquals("a.example", shown("a.§§example", false));
        assertEquals("", shown("§", false));
        // the cut counts what is shown: 41 chars with one sign are 40 shown chars and stay whole
        String forty = "abcdefghijklmnopqrstuvwxyzabcdefghijklmn";
        assertEquals(40, forty.length());
        assertEquals(forty, shown(forty.substring(0, 20) + "§" + forty.substring(20), false));
    }

    @Test
    void applying_the_config_clears_the_text_and_the_remembered_address() {
        ServerAddressModule m = new ServerAddressModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        platform.serverAddress = "a.example.net";
        m.tick(ctx, cfg);
        assertEquals("a.example.net", m.text());
        m.onConfigChanged(cfg);
        assertEquals("", m.text());
        platform.serverAddress = "a.example.net"; // the same address again must be drawn again
        m.tick(ctx, cfg);
        assertEquals("a.example.net", m.text());
        m.onConfigChanged(cfg);
        platform.serverAddress = null; // left the server while the module was off
        m.tick(ctx, cfg);
        assertEquals("", m.text());
        platform.serverAddress = "b.example.net";
        m.tick(ctx, cfg);
        assertEquals("b.example.net", m.text());
    }

    @Test
    void the_widest_text_stays_within_forty_characters() {
        StringBuilder b = new StringBuilder();
        for (int i = 0; i < 300; i++) {
            b.append('W');
        }
        assertEquals(40, shown(b.toString(), true).length());
        assertEquals(40, shown("[" + b + "]:25565", true).length());
        assertEquals(0, new ServerAddressModule().width(new FakeCanvas(), ModuleConfig.defaults()));
    }

    @Test
    void server_address_is_hidden_without_a_server() {
        ServerAddressModule m = new ServerAddressModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        m.tick(ctx, cfg);
        assertEquals("", m.text());
        platform.serverAddress = "mc.example.net";
        m.tick(ctx, cfg);
        assertEquals("mc.example.net", m.text());
        platform.inWorld = false;
        m.tick(ctx, cfg);
        assertEquals("", m.text());
        platform.inWorld = true;
        m.tick(ctx, cfg);
        assertEquals("mc.example.net", m.text());
        platform.serverAddress = null;
        m.tick(ctx, cfg);
        assertEquals("", m.text());
        assertEquals(0, m.width(new FakeCanvas(), cfg));
    }

    @Test
    void a_changed_address_shows_after_at_most_twenty_ticks() {
        ServerAddressModule m = new ServerAddressModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        platform.serverAddress = "a.example.net";
        m.tick(ctx, cfg);
        platform.serverAddress = "b.example.net";
        for (int i = 0; i < 19; i++) {
            m.tick(ctx, cfg);
        }
        assertEquals("a.example.net", m.text());
        m.tick(ctx, cfg);
        assertEquals("b.example.net", m.text());
    }

    @Test
    void the_option_applies_at_the_next_tick() {
        ServerAddressModule m = new ServerAddressModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        platform.serverAddress = "mc.example.net:25570";
        m.tick(ctx, cfg);
        assertEquals("mc.example.net", m.text());
        cfg.setOption("show_port", true);
        m.tick(ctx, cfg);
        assertEquals("mc.example.net:25570", m.text());
        cfg.setOption("show_port", false);
        m.tick(ctx, cfg);
        assertEquals("mc.example.net", m.text());
    }

    @Test
    void the_address_is_read_again_when_the_config_is_applied() {
        ServerAddressModule m = new ServerAddressModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        platform.serverAddress = "a.example.net";
        m.tick(ctx, cfg);
        platform.serverAddress = "b.example.net";
        m.tick(ctx, cfg);
        m.tick(ctx, cfg);
        assertEquals("a.example.net", m.text());
        m.onConfigChanged(cfg);
        m.tick(ctx, cfg);
        assertEquals("b.example.net", m.text());
    }

    @Test
    void the_address_is_read_at_once_when_the_module_becomes_visible_again() {
        ServerAddressModule m = new ServerAddressModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        platform.serverAddress = "a.example.net";
        m.tick(ctx, cfg);
        platform.serverAddress = null;
        m.tick(ctx, cfg);
        platform.serverAddress = "b.example.net";
        m.tick(ctx, cfg);
        assertEquals("b.example.net", m.text());
    }
}
