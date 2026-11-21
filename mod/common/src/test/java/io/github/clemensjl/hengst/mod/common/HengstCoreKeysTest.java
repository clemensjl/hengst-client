package io.github.clemensjl.hengst.mod.common;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.clemensjl.hengst.mod.common.feature.DefaultKey;
import io.github.clemensjl.hengst.mod.common.feature.Feature;
import io.github.clemensjl.hengst.mod.common.feature.FeatureGroup;
import io.github.clemensjl.hengst.mod.common.feature.KeySpec;
import io.github.clemensjl.hengst.mod.common.feature.RestrictionPolicy;
import io.github.clemensjl.hengst.mod.common.platform.GameVersion;
import io.github.clemensjl.hengst.mod.common.platform.WallClock;
import io.github.clemensjl.hengst.mod.common.testutil.FakePlatform;
import io.github.clemensjl.hengst.mod.common.testutil.FakeTime;
import java.nio.file.Path;
import java.time.Clock;
import java.time.Instant;
import java.time.ZoneOffset;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.EnumSet;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class HengstCoreKeysTest {
    @TempDir Path dir;
    FakePlatform platform;
    FakeTime time;
    Clock clock;
    HengstCore core;
    KeyFeature f;
    int HOLD;
    int PRESS;

    /** Keys "<id>.hold" (HOLD) and "<id>.press" (PRESS); logs onKey as "<key> down" / "<key> up". */
    static final class KeyFeature extends Feature {
        final List<String> events = new ArrayList<String>();
        RuntimeException failure;

        KeyFeature(String id, int number) {
            this(id, number, EnumSet.allOf(GameVersion.class));
        }

        KeyFeature(String id, int number, EnumSet<GameVersion> versions) {
            super(id, number, FeatureGroup.HUD, versions);
        }

        @Override public List<KeySpec> keys() {
            return Arrays.asList(new KeySpec(id() + ".hold", DefaultKey.NONE, KeySpec.Mode.HOLD),
                    new KeySpec(id() + ".press", DefaultKey.NONE, KeySpec.Mode.PRESS));
        }

        @Override public void onKey(int key, boolean down, long nowNanos) {
            events.add(key + (down ? " down" : " up"));
            if (failure != null) { throw failure; }
        }
    }

    @BeforeEach void setUp() {
        platform = new FakePlatform(dir); time = new FakeTime();
        clock = Clock.fixed(Instant.parse("2026-11-20T10:15:30Z"), ZoneOffset.UTC);
    }

    HengstCore start() { return HengstCore.start(platform, time, clock); }

    void startWithKeys() {
        core = start(); platform.inWorld = true; f = new KeyFeature("keys", 47);
        core.activeProfile().module("keys").setEnabled(true);
        int base = core.keySpecs().size();
        core.registerForTest(f);
        assertEquals(base + 2, core.keySpecs().size());
        HOLD = base; PRESS = base + 1;
    }

    /** Registers a second key feature and returns the index of its HOLD key (PRESS is one more). */
    int addSecond(KeyFeature g) {
        core.activeProfile().module(g.id()).setEnabled(true);
        int base = core.keySpecs().size();
        core.registerForTest(g);
        return base;
    }

    static List<String> ids(List<KeySpec> keys) {
        List<String> out = new ArrayList<String>();
        for (KeySpec k : keys) { out.add(k.id()); }
        return out;
    }

    long warningsWith(String text) {
        long n = 0;
        for (String w : platform.warnings) { if (w.contains(text)) { n++; } }
        return n;
    }

    @Test void core_keys_come_first_with_their_persisted_names() {
        List<KeySpec> keys = FeatureCatalog.keys(GameVersion.V1_8_9);
        assertEquals("key.hengst.settings", keys.get(0).translationKey());
        assertEquals(DefaultKey.RIGHT_SHIFT, keys.get(0).defaultKey());
        assertEquals("key.hengst.hud_editor", keys.get(1).translationKey());
        assertEquals(DefaultKey.NONE, keys.get(1).defaultKey());
        assertEquals(KeySpec.Mode.PRESS, keys.get(1).mode());
    }

    @Test void the_settings_key_opens_the_settings_only_in_a_world_without_a_screen() {
        core = start();
        core.onFeatureKey(0, true); core.onFeatureKey(0, false);
        assertNull(platform.openedScreen);
        platform.inWorld = true;
        core.onFeatureKey(0, true);
        assertNotNull(platform.openedScreen);
    }

    @Test void the_hud_editor_key_opens_the_editor() {
        core = start(); platform.inWorld = true;
        core.onFeatureKey(1, true);
        assertNotNull(platform.openedScreen);
    }

    @Test void hold_reports_press_and_release_press_only_presses() {
        startWithKeys();
        core.onFeatureKey(HOLD, true); core.onFeatureKey(HOLD, false);
        core.onFeatureKey(PRESS, true); core.onFeatureKey(PRESS, false);
        assertEquals(Arrays.asList("0 down", "0 up", "1 down"), f.events);
    }

    @Test void a_repeated_press_while_held_counts_once() {
        startWithKeys();
        for (int i = 0; i < 5; i++) { core.onFeatureKey(PRESS, true); }
        assertEquals(Arrays.asList("1 down"), f.events);
    }

    @Test void a_release_without_a_press_does_nothing() {
        startWithKeys();
        core.onFeatureKey(HOLD, false);
        assertTrue(f.events.isEmpty());
    }

    @Test void a_press_while_a_screen_is_open_is_ignored_and_needs_a_release() {
        startWithKeys();
        platform.screenOpen = true; core.onFeatureKey(PRESS, true); core.onClientTick();
        platform.screenOpen = false; core.onClientTick(); core.onFeatureKey(PRESS, true);
        assertTrue(f.events.isEmpty());
        core.onFeatureKey(PRESS, false); core.onFeatureKey(PRESS, true);
        assertEquals(Arrays.asList("1 down"), f.events);
    }

    @Test void a_press_outside_a_world_is_ignored() {
        startWithKeys();
        platform.inWorld = false;
        core.onFeatureKey(PRESS, true);
        assertTrue(f.events.isEmpty());
    }

    @Test void opening_a_screen_releases_a_held_hold_key() {
        startWithKeys();
        core.onFeatureKey(HOLD, true);
        platform.screenOpen = true; core.onClientTick();
        core.onFeatureKey(HOLD, false);
        assertEquals(Arrays.asList("0 down", "0 up"), f.events);
    }

    @Test void leaving_the_world_releases_a_held_hold_key() {
        startWithKeys();
        core.onFeatureKey(HOLD, true);
        platform.inWorld = false; core.onClientTick();
        assertEquals(Arrays.asList("0 down", "0 up"), f.events);
    }

    @Test void a_released_hold_key_is_not_released_again_by_later_ticks() {
        startWithKeys();
        core.onFeatureKey(HOLD, true);
        platform.screenOpen = true; core.onClientTick(); core.onClientTick();
        assertEquals(Arrays.asList("0 down", "0 up"), f.events);
    }

    @Test void keys_of_a_disabled_feature_do_nothing() {
        startWithKeys();
        core.activeProfile().module("keys").setEnabled(false); core.applyConfigForTest();
        core.onFeatureKey(PRESS, true);
        assertTrue(f.events.isEmpty());
    }

    @Test void toggling_the_feature_takes_effect_without_a_restart() {
        startWithKeys();
        core.activeProfile().module("keys").setEnabled(false); core.applyConfigForTest();
        core.onFeatureKey(PRESS, true); core.onFeatureKey(PRESS, false);
        core.activeProfile().module("keys").setEnabled(true); core.applyConfigForTest();
        core.onFeatureKey(PRESS, true);
        assertEquals(Arrays.asList("1 down"), f.events);
    }

    @Test void a_hold_key_pressed_while_on_still_gets_its_release_after_the_feature_is_switched_off() {
        startWithKeys();
        core.onFeatureKey(HOLD, true);
        core.activeProfile().module("keys").setEnabled(false); core.applyConfigForTest();
        core.onFeatureKey(HOLD, false);
        assertEquals(Arrays.asList("0 down", "0 up"), f.events);
    }

    @Test void out_of_range_indexes_and_bad_ids() {
        startWithKeys();
        core.onFeatureKey(-1, true); core.onFeatureKey(core.keySpecs().size(), true); core.onFeatureKey(99, true);
        core.onFeatureKey(-1, false); core.onFeatureKey(99, false);
        assertTrue(f.events.isEmpty());
        assertThrows(IllegalArgumentException.class, () -> new KeySpec("Bad Id", DefaultKey.NONE, KeySpec.Mode.PRESS));
        assertThrows(IllegalArgumentException.class, () -> new KeySpec("", DefaultKey.NONE, KeySpec.Mode.PRESS));
        assertEquals("key.hengst.a.b_1", new KeySpec("a.b_1", DefaultKey.NONE, KeySpec.Mode.HOLD).translationKey());
    }

    @Test void the_catalog_has_unique_ids_and_matches_the_core() {
        Set<String> ids = new HashSet<String>();
        for (KeySpec k : FeatureCatalog.keys(GameVersion.V26_3)) { assertTrue(ids.add(k.id()), k.id()); }
        core = start();
        assertEquals(ids(FeatureCatalog.keys(GameVersion.V1_8_9)), ids(core.keySpecs()));
    }

    @Test void a_throwing_press_handler_stops_neither_other_handlers_nor_later_presses_and_logs_once() {
        startWithKeys();
        KeyFeature g = new KeyFeature("other", 48);
        int gBase = addSecond(g);
        f.failure = new IllegalStateException("boom");
        core.onFeatureKey(PRESS, true); core.onFeatureKey(PRESS, false);
        core.onFeatureKey(PRESS, true); core.onFeatureKey(PRESS, false);
        core.onFeatureKey(gBase + 1, true);
        assertEquals(Arrays.asList("1 down", "1 down"), f.events);
        assertEquals(Arrays.asList("1 down"), g.events);
        assertEquals(1, warningsWith("boom"), platform.warnings.toString());
    }

    @Test void a_throwing_release_in_the_tick_does_not_stop_the_other_release() {
        startWithKeys();
        KeyFeature g = new KeyFeature("other", 48);
        int gBase = addSecond(g);
        core.onFeatureKey(HOLD, true);
        core.onFeatureKey(gBase, true);
        f.failure = new IllegalStateException("boom");
        platform.screenOpen = true;
        core.onClientTick();
        assertEquals(Arrays.asList("0 down", "0 up"), f.events);
        assertEquals(Arrays.asList("0 down", "0 up"), g.events);
        assertEquals(1, warningsWith("boom"));
    }

    @Test void a_second_feature_with_the_same_exception_class_is_logged_too() {
        startWithKeys();
        KeyFeature g = new KeyFeature("other", 48);
        int gBase = addSecond(g);
        f.failure = new IllegalStateException("boom-f");
        g.failure = new IllegalStateException("boom-g");
        core.onFeatureKey(PRESS, true);
        core.onFeatureKey(gBase + 1, true);
        assertEquals(1, warningsWith("boom-f"));
        assertEquals(1, warningsWith("boom-g"));
    }

    @Test void a_failure_is_logged_again_after_a_healthy_press() {
        startWithKeys();
        f.failure = new IllegalStateException("boom");
        core.onFeatureKey(PRESS, true); core.onFeatureKey(PRESS, false);
        f.failure = null;
        core.onFeatureKey(PRESS, true); core.onFeatureKey(PRESS, false);
        f.failure = new IllegalStateException("boom");
        core.onFeatureKey(PRESS, true);
        assertEquals(2, warningsWith("boom"));
    }

    /** Mutation caught: adding keys of unsupported features, or letting appendKeys shift later indexes. */
    @Test void an_unsupported_feature_adds_no_key_and_shifts_no_index() {
        core = start(); platform.inWorld = true;
        int base = core.keySpecs().size();
        List<String> before = ids(core.keySpecs());
        KeyFeature other = new KeyFeature("modernonly", 47, EnumSet.of(GameVersion.V26_3));
        core.activeProfile().module("modernonly").setEnabled(true);
        core.registerForTest(other);
        assertEquals(base, core.keySpecs().size());
        assertEquals(before, ids(core.keySpecs()));
        KeyFeature mine = new KeyFeature("mine", 49);
        core.activeProfile().module("mine").setEnabled(true);
        core.registerForTest(mine);
        assertEquals(base + 2, core.keySpecs().size());
        core.onFeatureKey(base + 1, true);
        assertEquals(Arrays.asList("1 down"), mine.events);
        assertTrue(other.events.isEmpty());
    }

    /** Mutation caught: dispatch that checks the profile instead of FeatureState (policy ignored), or no re-check when the server changes. */
    @Test void a_restricted_feature_gets_no_press_until_the_restriction_ends() {
        RestrictionPolicy policy = (feature, server) -> "blocked.example".equals(server) && feature.id().equals("keys");
        platform.inWorld = true;
        platform.serverAddress = "blocked.example";
        core = HengstCore.start(platform, time, clock, WallClock.SYSTEM, policy);
        f = new KeyFeature("keys", 47);
        core.activeProfile().module("keys").setEnabled(true);
        int base = core.keySpecs().size();
        core.registerForTest(f);
        core.onClientTick();
        core.onFeatureKey(base + 1, true); core.onFeatureKey(base + 1, false);
        assertTrue(f.events.isEmpty());
        platform.serverAddress = "free.example";
        core.onClientTick();
        core.onFeatureKey(base + 1, true);
        assertEquals(Arrays.asList("1 down"), f.events);
        core.onFeatureKey(base + 1, false);
        platform.serverAddress = "blocked.example";
        core.onClientTick();
        core.onFeatureKey(base + 1, true);
        assertEquals(Arrays.asList("1 down"), f.events);
    }

    /** Mutation caught: a second release from the tick after the feature-off path, or none at all. */
    @Test void switched_off_while_held_then_the_world_is_left_releases_exactly_once() {
        startWithKeys();
        core.onFeatureKey(HOLD, true);
        core.activeProfile().module("keys").setEnabled(false); core.applyConfigForTest();
        platform.inWorld = false;
        core.onClientTick(); core.onClientTick();
        core.onFeatureKey(HOLD, false);
        assertEquals(Arrays.asList("0 down", "0 up"), f.events);
    }

    /** Mutation caught: either derivation of the key list (core from the registry, catalog from create) drifting apart or out of number order. */
    @Test void the_core_key_list_equals_the_catalog_for_every_version() {
        for (GameVersion v : GameVersion.values()) {
            platform.version = v;
            core = start();
            assertEquals(ids(FeatureCatalog.keys(v)), ids(core.keySpecs()), v.toString());
        }
        int last = Integer.MIN_VALUE;
        for (Feature feature : FeatureCatalog.create()) {
            assertTrue(feature.number() > last, "the catalog lists " + feature.id() + " out of number order");
            last = feature.number();
        }
    }
}
