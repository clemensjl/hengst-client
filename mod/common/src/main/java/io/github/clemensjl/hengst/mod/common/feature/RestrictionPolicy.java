package io.github.clemensjl.hengst.mod.common.feature;

/**
 * Decides whether a feature is blocked on a server. {@code server} is {@code GameView.serverAddress()} as reported
 * (null in single player). Called whenever the feature state is rewritten, never per frame; must still not allocate.
 */
public interface RestrictionPolicy {
    /** Nothing is restricted. The real server list arrives with M4. */
    RestrictionPolicy NONE = new RestrictionPolicy() {
        @Override
        public boolean restricted(Feature feature, String server) {
            return false;
        }
    };

    boolean restricted(Feature feature, String server);
}
