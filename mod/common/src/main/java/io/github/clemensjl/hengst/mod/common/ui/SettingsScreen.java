package io.github.clemensjl.hengst.mod.common.ui;

import io.github.clemensjl.hengst.mod.common.I18n;
import io.github.clemensjl.hengst.mod.common.config.ModuleConfig;
import io.github.clemensjl.hengst.mod.common.feature.Feature;
import io.github.clemensjl.hengst.mod.common.feature.OptionSpec;
import io.github.clemensjl.hengst.mod.common.platform.Canvas;
import io.github.clemensjl.hengst.mod.common.platform.UiKey;
import io.github.clemensjl.hengst.mod.common.platform.UiScreen;
import java.util.ArrayList;
import java.util.List;
import java.util.function.BooleanSupplier;
import java.util.function.Consumer;
import java.util.function.DoubleConsumer;
import java.util.function.DoubleSupplier;
import java.util.function.Supplier;

/**
 * The settings screen: feature list with switches on the left, the options of the selected feature on the right,
 * profile controls on top. Drawn by common; the adapters only forward input. It does not close itself:
 * {@code onClose} (see {@link SettingsClose}) saves, rebuilds the renderer and closes the game screen.
 * Labels, layout and fitted texts are built on open, on profile or option changes and on resize, not per frame.
 */
public final class SettingsScreen implements UiScreen {
    private static final int MARGIN = 8;
    private static final int ROW = 20;
    private static final int GAP = 4;
    private static final int HEADER_Y = 8;
    private static final int HEADER_H = 18;
    private static final int LIST_Y = 36;
    private static final int BOTTOM = 36;
    private static final int DONE_H = 18;
    private static final int TOGGLE_W = 20;
    private static final int DIM = 0xC0101010;

    private static final String[] COLOR_KEYS = {"white", "yellow", "lime", "aqua", "red", "orange", "gray", "black"};
    private static final int[] COLORS = {
        0xFFFFFFFF, 0xFFFFFF55, 0xFF55FF55, 0xFF55FFFF, 0xFFFF5555, 0xFFFFAA00, 0xFFAAAAAA, 0xFF000000
    };

    private final SettingsModel model;
    private final I18n i18n;
    private final Runnable onClose;
    private final String notice;
    private final String[] colorNames = new String[COLORS.length];
    private final ScrollList featureList = new ScrollList(0, 0, 0, 0);
    private final ScrollList optionList = new ScrollList(0, 0, 0, 0);
    private final List<Widget> header = new ArrayList<Widget>();
    private final ButtonWidget done;
    private boolean closed;

    private int sw;
    private int sh;
    private float noticeScale = 1f;
    private float noticeY;

    public SettingsScreen(SettingsModel model, I18n i18n, Runnable onClose) {
        this.model = model;
        this.i18n = i18n;
        this.onClose = onClose;
        this.notice = i18n.get("hengst.notice");
        for (int i = 0; i < COLORS.length; i++) {
            colorNames[i] = i18n.get("hengst.color." + COLOR_KEYS[i]);
        }
        this.done = new ButtonWidget(i18n.get("hengst.button.done"), new Runnable() {
            @Override
            public void run() {
                close();
            }
        });
        List<Widget> rows = new ArrayList<Widget>();
        for (Feature f : model.features()) {
            rows.add(new FeatureRow(f));
        }
        featureList.setWidgets(rows, ROW);
        refresh();
        model.warnLoadProblems();
    }

    /** Rebuilds everything that depends on the active profile. */
    private void refresh() {
        buildHeader();
        buildOptions();
        if (sw > 0) {
            positionHeader();
        }
    }

    private void buildHeader() {
        header.clear();
        List<String> names = model.profiles();
        header.add(new ChoiceWidget(i18n.get("hengst.profile"), names.toArray(new String[0]),
                new Supplier<String>() {
                    @Override
                    public String get() {
                        return model.activeProfileName();
                    }
                },
                new Consumer<String>() {
                    @Override
                    public void accept(String name) {
                        model.switchTo(name);
                        refresh();
                    }
                }));
        header.add(new ButtonWidget(i18n.get("hengst.button.new_profile"), new Runnable() {
            @Override
            public void run() {
                model.copyActive();
                refresh();
            }
        }));
        header.add(new ButtonWidget(i18n.get("hengst.button.delete_profile"), new Runnable() {
            @Override
            public void run() {
                model.deleteActive();
                refresh();
            }
        }));
        header.add(new ButtonWidget(i18n.get("hengst.button.edit_hud"), new Runnable() {
            @Override
            public void run() {
                // The editor replaces this screen; the adapter's closed() for it must not close the editor.
                closed = true;
                model.openHudEditor();
            }
        }));
    }

    private void buildOptions() {
        List<Widget> out = new ArrayList<Widget>();
        final Feature f = model.selected();
        if (f != null) {
            final String id = f.id();
            out.add(new ToggleWidget(i18n.get("hengst.option.background"), new BooleanSupplier() {
                @Override
                public boolean getAsBoolean() {
                    return model.config(id).background();
                }
            }, new Consumer<Boolean>() {
                @Override
                public void accept(Boolean b) {
                    model.config(id).setBackground(b);
                    model.markChanged();
                }
            }));
            out.add(new ToggleWidget(i18n.get("hengst.option.shadow"), new BooleanSupplier() {
                @Override
                public boolean getAsBoolean() {
                    return model.config(id).shadow();
                }
            }, new Consumer<Boolean>() {
                @Override
                public void accept(Boolean b) {
                    model.config(id).setShadow(b);
                    model.markChanged();
                }
            }));
            out.add(colorChoice(id));
            for (OptionSpec spec : f.options()) {
                out.add(optionWidget(id, spec));
            }
        }
        optionList.setWidgets(out, ROW);
    }

    private Widget colorChoice(final String id) {
        return new ChoiceWidget(i18n.get("hengst.option.text_color"), colorNames,
                new Supplier<String>() {
                    @Override
                    public String get() {
                        int argb = model.config(id).textColor();
                        for (int i = 0; i < COLORS.length; i++) {
                            if (COLORS[i] == argb) {
                                return colorNames[i];
                            }
                        }
                        // A colour outside the table shows the first entry; the stored value is not changed.
                        return colorNames[0];
                    }
                },
                new Consumer<String>() {
                    @Override
                    public void accept(String name) {
                        for (int i = 0; i < COLORS.length; i++) {
                            if (colorNames[i].equals(name)) {
                                model.config(id).setTextColor(COLORS[i]);
                                model.markChanged();
                                return;
                            }
                        }
                    }
                });
    }

    private Widget optionWidget(final String id, final OptionSpec spec) {
        String label = i18n.get("hengst.option." + spec.key());
        switch (spec.kind()) {
            case TOGGLE:
                return new ToggleWidget(label, new BooleanSupplier() {
                    @Override
                    public boolean getAsBoolean() {
                        return (Boolean) model.config(id).option(spec);
                    }
                }, new Consumer<Boolean>() {
                    @Override
                    public void accept(Boolean b) {
                        model.config(id).setOption(spec.key(), spec.coerce(b));
                        model.markChanged();
                    }
                });
            case CHOICE:
                return new ChoiceWidget(label, spec.values(), new Supplier<String>() {
                    @Override
                    public String get() {
                        return (String) model.config(id).option(spec);
                    }
                }, new Consumer<String>() {
                    @Override
                    public void accept(String s) {
                        model.config(id).setOption(spec.key(), spec.coerce(s));
                        model.markChanged();
                    }
                });
            default:
                // The slider reads a cached value: option() boxes a number on every call.
                final double[] cache = {((Number) model.config(id).option(spec)).doubleValue()};
                return new SliderWidget(label, spec.min(), spec.max(), spec.step(),
                        new DoubleSupplier() {
                            @Override
                            public double getAsDouble() {
                                return cache[0];
                            }
                        },
                        new DoubleConsumer() {
                            @Override
                            public void accept(double v) {
                                ModuleConfig cfg = model.config(id);
                                Object stored = spec.coerce(v);
                                cfg.setOption(spec.key(), stored);
                                cache[0] = ((Number) stored).doubleValue();
                                model.markChanged();
                            }
                        });
        }
    }

    private void layout(Canvas c) {
        sw = c.width();
        sh = c.height();
        int leftW = sw * 40 / 100;
        int bottomY = sh - BOTTOM;
        featureList.setBounds(MARGIN, LIST_Y, leftW, bottomY - LIST_Y);
        int rightX = MARGIN + leftW + MARGIN;
        optionList.setBounds(rightX, LIST_Y, sw - MARGIN - rightX, bottomY - LIST_Y);
        done.w = Math.min(100, sw - 2 * MARGIN);
        done.h = DONE_H;
        done.x = sw - MARGIN - done.w;
        done.y = sh - MARGIN - DONE_H;
        // The legal notice is one line; it is scaled down to the screen width instead of being cut.
        int nw = c.textWidth(notice);
        int avail = sw - 2 * MARGIN;
        noticeScale = nw > avail && nw > 0 ? avail / (float) nw : 1f;
        noticeY = bottomY + (c.fontHeight() - c.fontHeight() * noticeScale) / 2f;
        positionHeader();
    }

    private void positionHeader() {
        int pw = Math.max(60, sw * 30 / 100);
        Widget choice = header.get(0);
        choice.x = MARGIN;
        choice.y = HEADER_Y;
        choice.w = pw;
        choice.h = HEADER_H;
        int start = MARGIN + pw + GAP;
        int n = header.size() - 1;
        int bw = (sw - MARGIN - start - (n - 1) * GAP) / n;
        for (int i = 1; i < header.size(); i++) {
            Widget b = header.get(i);
            b.x = start + (i - 1) * (bw + GAP);
            b.y = HEADER_Y;
            b.w = bw;
            b.h = HEADER_H;
        }
    }

    @Override
    public void render(Canvas c, int mx, int my, float partialTick) {
        if (c.width() != sw || c.height() != sh) {
            layout(c);
        }
        c.fill(0, 0, sw, sh, DIM);
        for (int i = 0; i < header.size(); i++) {
            header.get(i).render(c, mx, my);
        }
        featureList.render(c, mx, my);
        optionList.render(c, mx, my);
        c.pushTransform(MARGIN, noticeY, noticeScale);
        c.text(notice, 0, 0, 0xFFAAAAAA, true);
        c.popTransform();
        done.render(c, mx, my);
    }

    @Override
    public boolean mouseClicked(int mx, int my, int button) {
        for (int i = 0; i < header.size(); i++) {
            if (header.get(i).mouseClicked(mx, my, button)) {
                return true;
            }
        }
        return featureList.mouseClicked(mx, my, button)
                || optionList.mouseClicked(mx, my, button)
                || done.mouseClicked(mx, my, button);
    }

    @Override
    public boolean mouseReleased(int mx, int my, int button) {
        featureList.mouseReleased();
        optionList.mouseReleased();
        return true;
    }

    @Override
    public boolean mouseDragged(int mx, int my, int button) {
        return featureList.mouseDragged(mx, my) || optionList.mouseDragged(mx, my);
    }

    @Override
    public boolean mouseScrolled(int mx, int my, double amount) {
        if (featureList.contains(mx, my)) {
            featureList.scroll(amount);
            return true;
        }
        if (optionList.contains(mx, my)) {
            optionList.scroll(amount);
            return true;
        }
        return false;
    }

    @Override
    public boolean keyPressed(UiKey key, boolean shift, boolean control) {
        if (key == UiKey.ESCAPE) {
            close();
            return true;
        }
        return false;
    }

    @Override
    public void closed() {
        close();
    }

    private void close() {
        if (!closed) {
            closed = true;
            onClose.run();
        }
    }

    // Package-private for tests.
    List<Widget> headerWidgets() {
        return header;
    }

    ScrollList featureList() {
        return featureList;
    }

    ScrollList optionList() {
        return optionList;
    }

    Widget doneButton() {
        return done;
    }

    /** One list row: the switch on the left 20 px, the rest selects the feature. */
    private final class FeatureRow extends Widget {
        private final Feature feature;
        private final String name;
        private final ToggleWidget toggle;

        FeatureRow(final Feature feature) {
            this.feature = feature;
            this.name = i18n.get("hengst.feature." + feature.id());
            final String id = feature.id();
            this.toggle = new ToggleWidget("", new BooleanSupplier() {
                @Override
                public boolean getAsBoolean() {
                    return model.config(id).enabled();
                }
            }, new Consumer<Boolean>() {
                @Override
                public void accept(Boolean b) {
                    model.config(id).setEnabled(b);
                    model.markChanged();
                }
            });
        }

        @Override
        public void render(Canvas c, int mx, int my) {
            boolean selected = model.selected() == feature;
            c.fill(x, y, x + w, y + h, selected ? 0xFF4A4A70 : inside(mx, my) ? BG_HOVER : BG);
            toggle.x = x;
            toggle.y = y;
            toggle.w = TOGGLE_W;
            toggle.h = h;
            toggle.render(c, mx, my);
            String s = fitted(c, name, w - TOGGLE_W - 6);
            c.text(s, x + TOGGLE_W + 4, y + (h - c.fontHeight()) / 2, TEXT, true);
        }

        @Override
        public boolean mouseClicked(int mx, int my, int button) {
            if (button != 0 || !inside(mx, my)) {
                return false;
            }
            toggle.x = x;
            toggle.y = y;
            toggle.w = TOGGLE_W;
            toggle.h = h;
            if (toggle.mouseClicked(mx, my, button)) {
                return true;
            }
            model.select(feature.id());
            buildOptions();
            optionList.scroll(1_000_000);
            return true;
        }
    }
}
