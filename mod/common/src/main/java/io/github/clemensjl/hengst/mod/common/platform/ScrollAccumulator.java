package io.github.clemensjl.hengst.mod.common.platform;

/**
 * Turns raw wheel values (LWJGL deltas, 120 per notch on Windows; sensitivity-scaled and fractional values on touchpads)
 * into whole notches. The sign is the one the game delivers: positive when the wheel is rolled away from the user. One
 * instance per open screen; {@link #reset()} when the screen closes.
 */
public final class ScrollAccumulator {
    /** At most this many notches per event; the rest of a garbage value is dropped. */
    public static final int MAX_NOTCHES = 100;
    // Ten events of 0.1 sum to 0.9999999999999999 in double: count that as a full notch.
    private static final double EPSILON = 1e-9;

    private final double unitsPerNotch;
    private double remainder;

    /** The raw value of one notch: 1.0 where the game delivers notches, 120.0 for LWJGL 2 wheel deltas. */
    public ScrollAccumulator(double unitsPerNotch) {
        if (!(unitsPerNotch > 0.0) || Double.isInfinite(unitsPerNotch)) {
            throw new IllegalArgumentException("unitsPerNotch must be positive and finite: " + unitsPerNotch);
        }
        this.unitsPerNotch = unitsPerNotch;
    }

    /**
     * Adds one scroll event and returns the signed number of whole notches it completes (often 0). NaN and infinite
     * values give 0 and leave the remainder alone. Any finite value is processed; more than {@link #MAX_NOTCHES}
     * notches are capped to plus or minus {@link #MAX_NOTCHES} and the remainder and the excess are dropped.
     */
    public int add(double amount) {
        if (Double.isNaN(amount) || Double.isInfinite(amount)) {
            return 0;
        }
        remainder += amount / unitsPerNotch;
        if (Double.isInfinite(remainder)) { // a finite amount can overflow once scaled
            remainder = 0.0;
            return amount > 0 ? MAX_NOTCHES : -MAX_NOTCHES;
        }
        int notches = (int) (remainder + (remainder > 0 ? EPSILON : -EPSILON)); // saturates for huge values
        if (notches > MAX_NOTCHES || notches < -MAX_NOTCHES) {
            remainder = 0.0;
            return notches > 0 ? MAX_NOTCHES : -MAX_NOTCHES;
        }
        remainder -= notches;
        return notches;
    }

    /** Forgets a partial notch, for example when the screen closes. */
    public void reset() {
        remainder = 0.0;
    }
}
