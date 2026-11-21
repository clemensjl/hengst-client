package io.github.clemensjl.hengst.mod.common.hud.modules;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
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
        String pair = "😀";
        StringBuilder in = new StringBuilder();
        for (int i = 0; i < 18; i++) {
            in.append('a');
        }
        in.append(pair); // high surrogate at index 18, low at 19: a cut after 19 chars would split it
        while (in.length() < 30) {
            in.append('b');
        }
        in.append(pair); // high at 30, low at 31: a tail of 19 chars would start inside it
        while (in.length() < 55) {
            in.append('c');
        }
        String result = shown(in.toString(), false);
        assertTrue(result.length() <= 40, "length " + result.length());
        assertTrue(result.contains(".."));
        for (int i = 0; i < result.length(); i++) {
            char c = result.charAt(i);
            if (Character.isHighSurrogate(c)) {
                assertTrue(i + 1 < result.length() && Character.isLowSurrogate(result.charAt(i + 1)), "lone high at " + i);
                i++;
            } else {
                assertFalse(Character.isLowSurrogate(c), "lone low at " + i);
            }
        }
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
    void server_address_keeps_its_text_while_the_address_is_unchanged() {
        ServerAddressModule m = new ServerAddressModule();
        ModuleConfig cfg = ModuleConfig.defaults();
        platform.serverAddress = "mc.example.net:25570";
        m.tick(ctx, cfg);
        String first = m.text();
        for (int i = 0; i < 40; i++) {
            platform.serverAddress = new String("mc.example.net:25570");
            m.tick(ctx, cfg);
        }
        assertSame(first, m.text());
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
