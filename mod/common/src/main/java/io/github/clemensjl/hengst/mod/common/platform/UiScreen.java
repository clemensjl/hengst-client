package io.github.clemensjl.hengst.mod.common.platform;

/** A screen drawn by common; the adapter only provides the shell. */
public interface UiScreen {
    void render(Canvas canvas, int mouseX, int mouseY, float partialTick);

    boolean mouseClicked(int mouseX, int mouseY, int button);

    boolean mouseReleased(int mouseX, int mouseY, int button);

    boolean mouseDragged(int mouseX, int mouseY, int button);

    boolean mouseScrolled(int mouseX, int mouseY, double amount);

    boolean keyPressed(UiKey key, boolean shift, boolean control);

    void closed();
}
