package io.github.clemensjl.hengst.mod.legacy;

import net.fabricmc.api.ClientModInitializer;
import org.apache.logging.log4j.LogManager;
import org.apache.logging.log4j.Logger;

public final class HengstLegacyMod implements ClientModInitializer {
    private static final Logger LOGGER = LogManager.getLogger("hengst");

    @Override
    public void onInitializeClient() {
        LOGGER.info("[hengst] loaded on 1.8.9");
    }
}
