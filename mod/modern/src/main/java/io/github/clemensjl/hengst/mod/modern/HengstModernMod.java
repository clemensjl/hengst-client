package io.github.clemensjl.hengst.mod.modern;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.loader.api.FabricLoader;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

public final class HengstModernMod implements ClientModInitializer {
    private static final Logger LOGGER = LoggerFactory.getLogger("hengst");

    @Override
    public void onInitializeClient() {
        String mcVersion = FabricLoader.getInstance()
                .getModContainer("minecraft")
                .map(mod -> mod.getMetadata().getVersion().getFriendlyString())
                .orElse("unknown");
        LOGGER.info("[hengst] loaded on {}", mcVersion);
    }
}
