# Hengst Client design

Design specification for the desktop app (`apps/desktop`, Tauri 2, React, Tailwind CSS v4, shadcn/ui on Base UI). Every UI task builds against this file. Values here are decisions, not suggestions. If a value has to change, change it here in the same commit as the code.

Scope: v0.1 (phase 4a and 4b). Windows 10/11 x64 only.

## 1. Direction

Hengst Client is a calm, fast tool that people open several times a day to start a game, often for less than ten seconds. It should feel like a well-made Windows utility: quiet surfaces, readable text, one obvious action, and an immediate response to every click. It must not feel like a game menu, a store or a marketing site: no promotions, no decorative motion, no artwork from the game, nothing that competes with the Play button.

Rules that follow from this:

- One place may stand out: the start page (4b) with the instance name in large type and the Play button. Everything else stays still.
- Surfaces are separated by background steps and 1 px lines, not by shadows. No grid of shadowed cards, no gradients, no glass, no glow.
- No uppercase labels, no eyebrow labels above headings, no numbered markers (01, 02) unless the content is a real sequence (installer steps are), no "→" on buttons or links, no meta strings joined with middle dots.
- One accent colour, used for the primary action, the selected state and focus. Nowhere else.
- No emojis anywhere. Icons from one set (lucide) are fine.
- No assets from the game or from other launchers: no Minecraft fonts, textures, blocks, logos, skin renders or colour schemes, and nothing modelled on Lunar Client, Feather, Badlion, Prism Launcher or Modrinth App.

Mode: this is app UI (design-unslop mode A). It uses a real design system, shadcn/ui, with its tokens remapped to the values below. No marketing "taste" style.

## 2. Colour tokens

All colours are CSS custom properties on `:root` (light) and `.dark` (dark) in `apps/desktop/src/styles/index.css` (Task 3). Hex values are authoritative. Neither theme uses pure black or pure white for a surface or for text.

### 2.1 Token table

| Token | Light | Dark | Use |
|---|---|---|---|
| `--bg` | `#F3F4F6` | `#121417` | Window background, sidebar, native window background colour at creation |
| `--surface` | `#FAFAFB` | `#1A1D21` | Content column (header and view), log area |
| `--surface-raised` | `#FDFDFE` | `#22262B` | Inputs, dialogs, popovers, menus, toasts, error panels |
| `--border` | `#D9DCE1` | `#2E333A` | Hairlines: sidebar edge, header line when scrolled, list header, panel outlines. Decorative, no contrast requirement |
| `--border-strong` | `#858C96` | `#6E7680` | Boundaries of interactive controls: input, select trigger, checkbox, radio, secondary button, segment; dark-theme outline of level-1 layers |
| `--text` | `#1A1D21` | `#E7E9EC` | Primary text, icons next to primary text |
| `--text-muted` | `#59616D` | `#9AA3AE` | Secondary text, help lines, column labels, row meta, inactive nav, icons in nav |
| `--text-disabled` | `#8B929C` | `#6B737D` | Text and icons of disabled controls only |
| `--accent` | `#1D6F80` | `#4FB0C2` | Primary button fill, selected check icon, active nav icon, links, progress fill, "Recommended" badge text |
| `--accent-hover` | `#185E6D` | `#69BECD` | Primary button hover |
| `--accent-pressed` | `#134E5B` | `#86CCD8` | Primary button pressed |
| `--on-accent` | `#F6FBFC` | `#0B1A1E` | Text and icons on `--accent`, `--accent-hover`, `--accent-pressed` |
| `--focus` | `#1D6F80` | `#69BECD` | Focus outline |
| `--danger` | `#B42318` | `#F0645A` | Destructive button fill, error text, error icons |
| `--on-danger` | `#FFF7F6` | `#1F0A08` | Text on `--danger` |
| `--warning` | `#8A5A00` | `#E0A93B` | Warning text and icons, warning badge text |
| `--success` | `#2D7A4E` | `#5BB683` | Success icon in toasts, "Running" state dot |
| `--hover` | `#EAECEF` | `#2A2F35` | Hover fill of ghost buttons, list rows, menu items, nav items; skeleton blocks; progress track |
| `--selected` | `#E1EEF1` | `#1D3338` | Selected list row, active nav item, selected segment |
| `--selection` | `#C4E0E6` | `#2C5560` | Text selection (`::selection`), selected log lines |
| `--highlight` | `#F6E3A1` | `#5C4A12` | Search hits in the log (`<mark>`) |
| `--overlay` | `rgb(16 18 20 / 0.40)` | `rgb(0 0 0 / 0.60)` | Backdrop behind dialogs |
| `--log-error` | `#B42318` | `#F0645A` | Log lines with level ERROR or FATAL (same value as `--danger`) |
| `--log-warn` | `#8A5A00` | `#E0A93B` | Log lines with level WARN (same value as `--warning`) |
| `--log-info` | `#1A1D21` | `#E7E9EC` | Log lines with level INFO and lines without a level (same value as `--text`) |
| `--log-debug` | `#59616D` | `#9AA3AE` | Log lines with level DEBUG or TRACE (same value as `--text-muted`) |

Two values differ from the plan's draft: `--border-strong` was `#B9BEC6` / `#454C55`, which gives 1.7:1 and 2.1:1 against `--bg` and fails the 3:1 rule for control boundaries (WCAG 1.4.11, produkt-regelwerk A4) and the Task 3 token test. The sharpened values pass (table 2.3).

Tokens added beyond the plan's list, because the screens need them: `--text-disabled`, `--hover`, `--selected`, `--selection`, `--highlight`, `--overlay`.

Rules for colour on text inside tinted backgrounds:
- On `--selection` and `--highlight`, text is always `--text`, whatever its level colour was. The log applies `color: var(--text)` to `::selection` and to `mark`.
- Accent text (`--accent`) is allowed on `--bg`, `--surface`, `--surface-raised` and `--selected` only.
- Warning and danger text sit on `--bg`, `--surface` or `--surface-raised` only.

### 2.2 Accent

`--accent` is a petrol teal (light `#1D6F80`, HSL about 191°, 63 %, 31 %; dark `#4FB0C2`, desaturated and lighter for dark surfaces). Reason: a cool, technical colour that reads as "tool", stays calm at the low saturation the rules ask for (below 80 %), and keeps 4.5:1 for text both ways. It is deliberately not the grass or creeper greens associated with the game, not the bright green of Modrinth App, and not the saturated blues, purples or reds that other launchers use as their signature. It is also not the AI-default violet or the terracotta/cream pairing.

Where it appears: the primary button of a screen (at most one per screen), the active nav icon, the check icon of the selected row, the selected segment's outline, the focus ring, links, the progress fill and the "Recommended" badge. It never fills a large area, a header, a sidebar or a background.

### 2.3 Contrast (calculated, WCAG 2.x relative luminance)

Text pairs need 4.5:1, control boundaries and the focus ring 3:1. All values computed from the hex values in 2.1; nothing is estimated.

| Foreground | Background | Light | Dark | Requirement |
|---|---|---|---|---|
| `--text` | `--bg` | 15.37 | 15.17 | 4.5 |
| `--text` | `--surface` | 16.21 | 13.90 | 4.5 |
| `--text` | `--surface-raised` | 16.64 | 12.51 | 4.5 |
| `--text` | `--hover` | 14.29 | 11.09 | 4.5 |
| `--text` | `--selected` | 14.26 | 10.91 | 4.5 |
| `--text` | `--selection` | 12.20 | 6.70 | 4.5 |
| `--text` | `--highlight` | 13.23 | 7.07 | 4.5 |
| `--text-muted` | `--bg` | 5.69 | 7.23 | 4.5 |
| `--text-muted` | `--surface` | 6.00 | 6.62 | 4.5 |
| `--text-muted` | `--surface-raised` | 6.16 | 5.96 | 4.5 |
| `--text-muted` | `--hover` | 5.29 | 5.28 | 4.5 |
| `--text-muted` | `--selected` | 5.28 | 5.20 | 4.5 |
| `--text-disabled` | `--surface` | 3.01 | 3.52 | none (WCAG exempts disabled controls); kept at 3 for legibility |
| `--text-disabled` | `--surface-raised` | 3.09 | 3.17 | as above |
| `--on-accent` | `--accent` | 5.53 | 7.05 | 4.5 |
| `--on-accent` | `--accent-hover` | 7.03 | 8.34 | 4.5 |
| `--on-accent` | `--accent-pressed` | 8.87 | 9.86 | 4.5 |
| `--on-danger` | `--danger` | 6.23 | 6.04 | 4.5 |
| `--accent` (as text) | `--bg` | 5.25 | 7.31 | 4.5 |
| `--accent` (as text) | `--surface` | 5.53 | 6.70 | 4.5 |
| `--accent` (as text) | `--surface-raised` | 5.68 | 6.03 | 4.5 |
| `--accent` (as text) | `--selected` | 4.87 | 5.26 | 4.5 |
| `--danger` | `--bg` | 5.97 | 5.87 | 4.5 |
| `--danger` | `--surface` | 6.30 | 5.38 | 4.5 |
| `--danger` | `--surface-raised` | 6.47 | 4.84 | 4.5 |
| `--warning` | `--bg` | 5.39 | 8.71 | 4.5 |
| `--warning` | `--surface` | 5.68 | 7.98 | 4.5 |
| `--warning` | `--surface-raised` | 5.83 | 7.18 | 4.5 |
| `--success` | `--bg` | 4.76 | 7.44 | 4.5 |
| `--success` | `--surface` | 5.03 | 6.82 | 4.5 |
| `--success` | `--surface-raised` | 5.16 | 6.13 | 4.5 |
| `--border-strong` | `--bg` | 3.08 | 4.01 | 3 |
| `--border-strong` | `--surface` | 3.25 | 3.68 | 3 |
| `--border-strong` | `--surface-raised` | 3.34 | 3.31 | 3 |
| `--focus` | `--bg` | 5.25 | 8.65 | 3 |
| `--focus` | `--surface` | 5.53 | 7.93 | 3 |
| `--focus` | `--surface-raised` | 5.68 | 7.14 | 3 |
| `--focus` | `--selected` | 4.87 | 6.22 | 3 |

The log colours are aliases of the values above, so their contrast on `--surface` (the log background) is the `--danger`, `--warning`, `--text` and `--text-muted` rows. The primary button and the accent icon on `--accent` are covered by the `--on-accent` rows. The focus outline sits 2 px outside the control, so it is measured against the surface around the control, never against the control fill.

### 2.4 Mapping onto shadcn/ui

shadcn/ui (current docs, https://ui.shadcn.com/docs/theming, read 21.11.2026) defines these variables on `:root` and `.dark`: `--radius`, `--background`, `--foreground`, `--card`, `--card-foreground`, `--popover`, `--popover-foreground`, `--primary`, `--primary-foreground`, `--secondary`, `--secondary-foreground`, `--muted`, `--muted-foreground`, `--accent`, `--accent-foreground`, `--destructive`, `--border`, `--input`, `--ring`, `--chart-1` to `--chart-5`, `--sidebar`, `--sidebar-foreground`, `--sidebar-primary`, `--sidebar-primary-foreground`, `--sidebar-accent`, `--sidebar-accent-foreground`, `--sidebar-border`, `--sidebar-ring`, and exposes them to Tailwind with `@theme inline { --color-background: var(--background); ... }`. The docs assume Tailwind v4 (`@import "tailwindcss"`, `@theme inline`, `@custom-variant dark`).

Two names collide with ours: shadcn's `--accent` means "hover fill of menu items and ghost buttons", ours means the brand accent; `--border` means the same in both. Decision: the shadcn variable names are **not** declared on `:root`. Task 3 deletes the `:root` and `.dark` blocks that `shadcn init` writes and maps shadcn's colour utilities directly onto our tokens inside `@theme inline`. Components keep their class names (`bg-primary`, `bg-accent`, `border-input`, ...) and get our values:

| Tailwind theme variable (shadcn name) | Value | So that |
|---|---|---|
| `--color-background` | `var(--bg)` | |
| `--color-foreground` | `var(--text)` | |
| `--color-card` | `var(--surface)` | |
| `--color-card-foreground` | `var(--text)` | |
| `--color-popover` | `var(--surface-raised)` | |
| `--color-popover-foreground` | `var(--text)` | |
| `--color-primary` | `var(--accent)` | `bg-primary` is the Hengst accent |
| `--color-primary-foreground` | `var(--on-accent)` | |
| `--color-secondary` | `var(--surface-raised)` | |
| `--color-secondary-foreground` | `var(--text)` | |
| `--color-muted` | `var(--hover)` | skeleton, tracks |
| `--color-muted-foreground` | `var(--text-muted)` | |
| `--color-accent` | `var(--hover)` | shadcn's hover fill; `bg-accent` never means teal |
| `--color-accent-foreground` | `var(--text)` | |
| `--color-destructive` | `var(--danger)` | |
| `--color-destructive-foreground` | `var(--on-danger)` | replaces the `text-white` that shadcn's destructive button uses |
| `--color-border` | `var(--border)` | |
| `--color-input` | `var(--border-strong)` | control boundaries reach 3:1 |
| `--color-ring` | `var(--focus)` | |

Our own tokens get their own utilities in the same block: `--color-bg`, `--color-surface`, `--color-surface-raised`, `--color-border-strong`, `--color-text`, `--color-text-muted`, `--color-text-disabled`, `--color-accent-hover`, `--color-accent-pressed`, `--color-on-accent`, `--color-focus`, `--color-danger`, `--color-on-danger`, `--color-warning`, `--color-success`, `--color-hover`, `--color-selected`, `--color-selection`, `--color-highlight`, `--color-overlay`, `--color-log-error`, `--color-log-warn`, `--color-log-info`, `--color-log-debug`. In hand-written code, the brand accent is `bg-primary` / `text-primary`; the `accent` utility name belongs to shadcn.

Not mapped: `--chart-*` (no charts in v0.1) and `--sidebar-*` (the sidebar is our own `Sidebar.tsx`, not the shadcn Sidebar block).

Radius: shadcn derives `--radius-sm` to `--radius-xl` from `--radius`. Task 3 sets them explicitly in `@theme inline`: `--radius-sm: var(--radius-control)`, `--radius-md: var(--radius-control)`, `--radius-lg: var(--radius-panel)`, `--radius-xl: var(--radius-panel)`. Generated components that put `rounded-md` on a floating layer (popover, dropdown content, select content, command list container) are edited to `rounded-lg` after `shadcn add`, because floating layers are panels (section 5).

Dark mode: class-based, `.dark` on `<html>`, Tailwind variant `@custom-variant dark (&:where(.dark, .dark *));` (https://tailwindcss.com/docs/dark-mode, as verified in the plan). The theme comes from the backend settings; no next-themes.

Theme variables follow Tailwind v4's namespaces (`--color-*`, `--radius-*`, `--font-*`, `--ease-*`, `--breakpoint-*`; https://tailwindcss.com/docs/theme).

## 3. Typography

### 3.1 Typefaces

System fonts only. The app runs on Windows only and should feel like a Windows tool; no font file ships in the bundle and nothing is fetched at runtime (the CSP allows no font hosts anyway).

| Role | Stack | Tailwind variable |
|---|---|---|
| UI text | `"Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif` | `--font-sans` |
| Headings 24 px and up | `"Segoe UI Variable Display", "Segoe UI", system-ui, sans-serif` | `--font-display` |
| Log, device code, error details | `"Cascadia Mono", Consolas, monospace` | `--font-mono` |

Licence: Segoe UI Variable, Segoe UI, Cascadia Mono and Consolas are part of Windows (Cascadia Mono ships with Windows 11 and with Windows Terminal; on Windows 10 without it, Consolas applies). They are used from the system, not redistributed, so no licence file is needed. Windows 10 has no Segoe UI Variable; the stack falls back to Segoe UI with the same metrics class, and all sizes below work with both.

Weights: 400 and 600 only. Hierarchy comes from size and colour. Letter spacing 0 everywhere, except the device code (`0.04em`).

### 3.2 Scale

Sizes are set in `rem` (root 16 px) so WebView zoom scales them; the px values are at 100 %.

| Role | Size / line height | Weight | Font | Colour | Where |
|---|---|---|---|---|---|
| Start title | 32/40 (2rem/2.5rem) | 600 | display | `--text` | Instance name on the start page (4b), at most two lines, then ellipsis |
| Page title (h1) | 24/32 (1.5rem/2rem) | 600 | display | `--text` | Header of every view |
| Section title (h2), dialog title | 18/26 (1.125rem/1.625rem) | 600 | sans | `--text` | Settings sections, dialog titles, empty-state titles |
| Body | 16/24 (1rem/1.5rem) | 400 | sans | `--text` | Paragraphs, list row name, input text (16 px in fields, FO15), dialog text |
| Label | 14/20 (0.875rem/1.25rem) | 600 | sans | `--text` | Field labels, button text (default size) |
| Secondary | 14/20 | 400 | sans | `--text-muted` | Row meta columns, help lines, inline errors (in `--danger`), nav labels, menu items (in `--text`) |
| Caption | 13/18 (0.8125rem/1.125rem) | 400 | sans | `--text-muted` | Column labels, timestamps, tooltips, toast description |
| Badge | 13/18 | 600 | sans | per badge | Badges |
| Play button | 16/24 | 600 | sans | `--on-accent` | Play button only |
| Log | 13/20 | 400 | mono | `--log-*` | Log lines, error details |
| Device code | 28/36 (1.75rem/2.25rem) | 600 | mono | `--text` | Sign-in code view |

The window title is the native Windows title bar ("Hengst Client", plus " (Development build)" in dev builds); the app does not draw its own.

Paragraph text is capped at 72 characters (`max-width: 72ch`). Headings use `text-wrap: balance`, paragraphs `text-wrap: pretty`. `lang` on `<html>` follows the UI language so German compounds hyphenate (`hyphens: auto` on headings and narrow cells).

### 3.3 Numbers, dates, units

- `font-variant-numeric: tabular-nums` on every number that changes or lines up: counts, file sizes, progress, memory, remaining time, version columns, timestamps.
- Numbers through `Intl.NumberFormat` in the UI language: "3.402 Dateien" / "3,402 files". Never raw `toString()`.
- Units with a non-breaking space: "2048 MB", "1,5 GB" / "1.5 GB". Memory is entered and stored in MB; the hint beside the field shows GB with one decimal.
- Plurals through i18next plural keys ("1 Datei" / "2 Dateien").
- Relative time through `Intl.RelativeTimeFormat` up to 6 days ("vor 2 Tagen", "2 days ago"), from 7 days the date via `Intl.DateTimeFormat` with `dateStyle: "medium"`. Never-played instances say "Noch nie gespielt" / "Never played".
- Version ids and loader versions are shown exactly as the source gives them ("1.14 Pre-Release 1", "47.4.28"), never reformatted.

## 4. Spacing

4 px grid. Tokens and the Tailwind steps they equal (Tailwind's `--spacing` stays at `0.25rem`):

| Token | Value | Tailwind step | Use |
|---|---|---|---|
| `--space-1` | 4 px | `1` | Icon to badge, gap between nav items, menu padding |
| `--space-2` | 8 px | `2` | Label to field, icon to label inside buttons, gap between buttons in a row |
| `--space-3` | 12 px | `3` | Row and nav item horizontal padding, gap between icon and label in nav |
| `--space-4` | 16 px | `4` | Button horizontal padding, field to field, gap between header items, between list columns |
| `--space-5` | 24 px | `6` | Dialog padding, content padding below 1100 px, section title to its content |
| `--space-6` | 32 px | `8` | Content padding from 1100 px, between groups on the start page |
| `--space-7` | 48 px | `12` | Between settings sections, top padding of the start page block |

Only these steps are used for layout. Half steps (`0.5`, `1.5`) only inside small controls (badge padding, checkbox). Inside a group the gap is smaller than between groups.

## 5. Shape, borders, elevation

Radii (two steps on purpose, plus pills):

| Token | Value | Applies to |
|---|---|---|
| `--radius-control` | 6 px | Buttons, inputs, select triggers, segments, list rows, nav items, menu items, tooltips, the device-code box, skeleton blocks |
| `--radius-panel` | 10 px | Dialogs, popovers, menus, select and combobox content, toasts, error panels, the log area |
| `--radius-pill` | 999 px | Account chip, badges, progress bar and track, the initial circle |

Nesting: a panel (10) with 4 px padding contains items with 6; a dialog (10) with 24 px padding contains controls with 6.

Borders are 1 px. The focus outline is 2 px. The selected segment carries a 1 px inset outline in `--accent`.

Elevation has two levels:

| Level | What | Light | Dark |
|---|---|---|---|
| 0 | Sidebar, header, views, lists, the log area, inline panels | No shadow. Separation by `--bg` / `--surface` and 1 px `--border` | Same |
| 1 | Popover, dropdown menu, select content, combobox, dialog, alert dialog, toast, tooltip | 1 px `--border` plus `0 8px 24px rgb(16 18 20 / 0.14)` | 1 px `--border-strong` plus `0 8px 24px rgb(0 0 0 / 0.45)` |

Dialogs sit on `--overlay`. Tooltips use inverse colours (background `--text`, text `--bg`, contrast 15.37 / 15.17) and the level-1 shadow without border.

## 6. Icons

- Set: lucide (`lucide-react`, ISC licence), the set shadcn uses. No second icon library, no hand-drawn icon paths (the brand mark is the only custom SVG).
- Stroke width 1.5 (`strokeWidth={1.5}`), set once through a wrapper or default props.
- Sizes: 18 px for nav items, icon buttons and the Play button; 16 px inside text buttons, menu items, badges, inline errors and toasts. No other sizes.
- Colour follows the text it sits next to (`currentColor`). Exceptions: the active nav icon and the selected-row check are `--accent`; error icons `--danger`; warning icons `--warning`.
- Decorative icons next to a text label get `aria-hidden="true"`.
- Icon names used: `house` (start page), `boxes` (instances), `scroll-text` (log), `settings` (settings), `play`, `square` (stop), `x` (cancel, close), `plus` (create), `ellipsis` (row actions), `folder-open`, `trash-2`, `check`, `chevron-down`, `chevron-up`, `log-in`, `log-out`, `user-plus`, `copy`, `external-link`, `circle-alert` (error), `triangle-alert` (warning), `info`, `search`, `arrow-down-to-line` (jump to end), `loader-circle` (spinner), `refresh-cw` (retry).

When an icon needs a text label: always, except in these four places, where the icon button has an `aria-label` and a tooltip with the same text:
1. Nav items in the collapsed 64 px sidebar.
2. The row action button ("Aktionen für {{name}}" / "Actions for {{name}}").
3. The close button in dialog headers ("Schließen" / "Close").
4. The search hit navigation in the log ("Vorheriger Treffer", "Nächster Treffer").

## 7. Window and layout

### 7.1 Window

- Native Windows title bar, one window, label `main`.
- Minimum 960 × 600, default 1120 × 720 (logical px). `window-state` restores size and position.
- The window is created with the background colour `--bg` of the resolved theme, so no white flash.
- Sizes are in device-independent pixels; Windows display scaling (125 %, 150 %) scales the whole WebView. At 150 % the default window is 1680 × 1080 physical pixels.

### 7.2 Frame

```
+--------------------------------------------------------------------------------+
| Hengst Client                                                    _  []  X      |  native title bar
+------------------+-------------------------------------------------------------+
| [H] Hengst Client|  Instanzen                         [+ Instanz anlegen] (S Steve v) |  header 56
|                  |                                                             |
|  (house) Spielen |  Name                    Version     Loader         Zuletzt gespielt |
|  (boxes) Instanz.|  [v] Überleben 1.21      1.21.11     Fabric 0.17.3  vor 2 Tagen  (...) |
|  (scroll) Log    |      1.20.1 Forge        1.20.1      Forge 47.4.28  Noch nie gespielt (...) |
|                  |                                                             |
|                  |                                                             |
|                  |                                                             |
|                  |                                                             |
| (gear) Einstell. |                                                             |
+------------------+-------------------------------------------------------------+
  sidebar 216, --bg   content column, --surface, padding 32, content max 1120, left-aligned
```

1120 × 720 (wide). At 960 × 600 (compact):

```
+----+---------------------------------------------------------------------------+
| [H]|  Instanzen                               [+ Instanz anlegen] (S Steve v)   |  header 56
|    |                                                                           |
| (h)|  Name                  Version    Loader          Zuletzt gespielt         |
| (b)|  [v] Überleben 1.21    1.21.11    Fabric 0.17.3   vor 2 Tagen         (...) |
| (s)|      1.20.1 Forge      1.20.1     Forge 47.4.28   Noch nie gespielt   (...) |
|    |                                                                           |
| (g)|                                                                           |
+----+---------------------------------------------------------------------------+
 64    content padding 24
```

Breakpoint: one, at 1100 px window width, as Tailwind `--breakpoint-wide: 1100px` (variant `wide:`). Below it the sidebar collapses to 64 px and content padding drops to 24 px. Nothing else changes; there is no mobile layout.

Sidebar:
- Background `--bg`, 1 px `--border` on its right edge, full window height, does not scroll.
- Top brand row 56 px high (aligned with the header): mark 24 px, 8 px gap, wordmark "Hengst Client" 16/24 weight 600 `--text`, padded 12 px left. Collapsed: mark only, centred. The brand row is not a button (there is no "home" beyond the first nav item).
- `<nav aria-label="Hauptnavigation" / "Main">` below with 8 px padding: nav items as `<button>` with `aria-current="page"` on the active one; 40 px high, full width minus 16 px, radius 6, padding 0 12, icon 18 plus 12 px gap plus label 14/20. 4 px gap between items.
- Order: in 4a "Instanzen", then (pinned to the bottom of the sidebar, 8 px from the edge) "Einstellungen". From 4b: "Spielen" (start page, first, the start view), "Instanzen", "Log", bottom "Einstellungen".
- States: default label `--text-muted`, icon `--text-muted`; hover `--hover` fill, label `--text`; active `--selected` fill, label `--text` weight 600, icon `--accent`; focus outline. The active state is fill plus weight plus icon colour, not colour alone.
- Collapsed: items 40 × 40 centred, icon only, `aria-label` and a tooltip to the right with the name.
- In dev builds a badge "Entwicklungs-Build" / "Development build" sits above the settings item (wide: full badge; collapsed: hidden, the window title says it).

Header (top of the content column):
- 56 px, background `--surface`, horizontal padding equal to the content padding. h1 (24/32) on the left, vertically centred, truncated with ellipsis.
- On the right, in this order: the view's primary action (if any), 16 px gap, the account chip. The header is sticky; when the content below is scrolled, a 1 px `--border` line appears at its bottom (no shadow, no animation).

Content:
- Background `--surface`, padding 32 px (24 below 1100) on left, right and bottom, 8 px from the header to the first content. Content width at most 1120 px, left-aligned (not centred), so wide windows grow empty space on the right instead of stretching rows to unreadable widths.
- The content column scrolls vertically (`overflow-y: auto`, `overscroll-behavior: contain`); sidebar and header do not. The log view is the exception: the log area itself scrolls and the page does not.
- Density: comfortable desktop density, mouse first. Buttons 40, rows 48, nav items 40, Play 56.

## 8. Motion

Product UI after `emil-design-eng` and produkt-regelwerk M1 to M8. One curve, few durations, motion only where it shows what changed.

| Token | Value | For |
|---|---|---|
| `--ease-out` | `cubic-bezier(0.23, 1, 0.32, 1)` | Everything that animates, except progress |
| `--dur-press` | 120 ms | Colour change on hover and release, Play button scale |
| `--dur-popover-in` | 160 ms | Popover, dropdown menu, select, combobox, tooltip enter |
| `--dur-popover-out` | 120 ms | The same, exit (25 % shorter) |
| `--dur-dialog-in` | 220 ms | Dialog and alert dialog enter, overlay fade in |
| `--dur-dialog-out` | 160 ms | The same, exit (27 % shorter) |
| `--dur-progress` | 200 ms, `linear` | Progress fill |

The curve differs from the plan's draft `cubic-bezier(0.2, 0, 0, 1)`: that curve starts with a flat tangent (dy/dx = 0 at t = 0), so it behaves like an ease-in at the start and delays the first visible movement, which M3 and `emil-design-eng` rule out for UI. `cubic-bezier(0.23, 1, 0.32, 1)` is the strong ease-out produkt-regelwerk M3 names.

What animates and how:
- Pressed state: on `:active` the fill switches to the pressed colour immediately (0 ms, well inside the 100 ms of Z1); the return transitions `background-color, color, border-color` over `--dur-press`. Hover colour changes use the same transition. Never `transition: all`.
- Play button only: `transform: scale(0.98)` while pressed, `--dur-press`, `--ease-out`.
- Popover, menu, select, combobox, tooltip: `opacity 0 → 1` and `scale 0.97 → 1` from the trigger (`transform-origin: var(--transform-origin)`, which Base UI provides), `--dur-popover-in`; exit reverses with `--dur-popover-out`. Tooltips open after 500 ms of hover; once one tooltip has been shown, the next opens with no delay and no animation for 300 ms (skip-delay).
- Dialog: overlay opacity, panel `opacity 0 → 1` and `scale 0.97 → 1` from the centre, `--dur-dialog-in`; exit `--dur-dialog-out`.
- Progress fill: `transform: scaleX(fraction)` with `transform-origin: left`, `--dur-progress` linear (the plan's "width 200 ms linear", done with transform so only transform animates).
- Toasts: Sonner's own enter, exit and swipe motion, unchanged (occasional, swipe needs its physics).
- Button spinner: `loader-circle` rotating, 800 ms per turn, linear, only while a request is actually running.

What never animates:
- View switches (sidebar navigation), the header border, list loads, list updates, sorting, selection of a row, the active nav item, theme and language switches, settings changes.
- Anything opened by keyboard: a menu, popover or select opened with Enter, Space or arrow keys appears without transition. Implementation: `pointerdown` sets `data-input="pointer"` on `<html>`, `keydown` sets `data-input="keyboard"`; under `[data-input="keyboard"]` the popover and menu transitions are `none`. Dialogs keep their animation (they open rarely).
- Skeletons: static blocks in `--hover`, no pulse, no shimmer. They appear only if loading takes longer than 200 ms (L2).
- No loops other than the button spinner, no bounce, no spring overshoot, no entrance choreography, no stagger.

`prefers-reduced-motion: reduce`: all transforms are dropped (no scale on press, popovers and dialogs); popovers, menus, dialogs and the overlay only fade, 80 ms; the progress fill jumps without transition; the button spinner stands still (the button text already says what is happening); Sonner toasts fade only (CSS override on `[data-sonner-toast]`: `transition: opacity 80ms`).

Every new animation is checked with `~/.claude/skills/review-animations/SKILL.md` in the task that adds it.

## 9. Focus

- `:focus-visible` shows `outline: 2px solid var(--focus); outline-offset: 2px;`. Never removed, never replaced by a colour change alone. Mouse clicks do not show it.
- The outline sits outside the control, so it is visible on accent and danger fills too (measured against the surrounding surface, table 2.3).
- Inside menus, comboboxes and selects the highlighted item is shown with `--hover` fill (keyboard and pointer alike); the outline is not drawn on items inside a listbox.
- List rows show the outline on the whole row.
- Focus is never hidden under the sticky header: the content column has `scroll-padding-top: 64px`.

## 10. Components

Every element the screens need, its shadcn/ui source and its states. "Custom" means built in this repo from shadcn parts. Disabled controls use `--text-disabled` for text and icons and keep their layout; a disabled control always has a visible reason next to it (Z7) and `aria-disabled` plus `aria-describedby` on the reason.

### 10.1 Button (shadcn `button`)

Variants and sizes. Height is fixed per size; width follows the text (`min-width`, never fixed `width`). Text 14/20 weight 600, icon 16 with 8 px gap, padding 0 16. Loading: the label changes to the verb with ellipsis ("Wird angelegt…"), a 16 px spinner replaces the leading icon, the width does not change (min-width set from the idle label), and a guard blocks the second click before the first `await` (Z2).

| Variant | Default | Hover | Pressed | Disabled |
|---|---|---|---|---|
| `primary` | `--accent` fill, `--on-accent` text | `--accent-hover` | `--accent-pressed` | `--hover` fill, `--text-disabled` text |
| `secondary` | `--surface-raised` fill, 1 px `--border-strong`, `--text` | `--hover` fill | `--selected` fill | `--surface` fill, `--border` line, `--text-disabled` |
| `ghost` | transparent, `--text` | `--hover` fill | `--selected` fill | `--text-disabled` |
| `destructive` | `--danger` fill, `--on-danger` text | `--danger` with `filter: brightness(0.92)` (light) / `brightness(1.08)` (dark) | `brightness(0.84)` / `brightness(1.16)` | as primary disabled |
| `link` | `--accent` text, underline on hover | underline | `--accent-pressed` text | `--text-disabled` |

| Size | Height | Use |
|---|---|---|
| `default` | 40 | All buttons |
| `icon` | 40 × 40 | Icon-only buttons (section 6 list), icon 18 |
| `lg` | 56 | Play button only |

All variants: focus outline (section 9), hover styles only under `@media (hover: hover) and (pointer: fine)`.

At most one `primary` button per screen or dialog. Button rows: secondary actions left of the primary, primary last (right); 8 px gap; rows wrap to two lines rather than truncate German labels (WD29).

### 10.2 Play button (custom on shadcn `button`, size `lg`)

56 px high, min-width 240 px, radius 6, text 16/24 weight 600, icon 18. One button whose label and variant follow the game state; it never moves or changes height. The state comes from `AppEvent::GameState` and the account and instance state.

| State | Label (de / en) | Variant, icon | Action |
|---|---|---|---|
| No instance | "Instanz anlegen" / "Create instance" | primary, `plus` | Opens the create dialog |
| No account | "Anmelden" / "Sign in" | primary, `log-in` | Opens the sign-in dialog |
| Account needs sign-in again | "Erneut anmelden" / "Sign in again" | primary, `log-in` | Starts sign-in for that account |
| Ready | "Spielen" / "Play" | primary, `play` | `play(instance_id)` |
| Starting (click to first event) | "Wird gestartet…" / "Starting…" | primary, spinner, inert (`aria-disabled`) | none (guard) |
| Preparing | "Abbrechen" / "Cancel" | secondary, `x` | `play_cancel(op)`; progress panel shows below (10.12) |
| Running | "Beenden" / "Stop" | secondary, `square` | `game_stop()`; a status line above the button reads "Läuft seit 12:04" / "Running since 12:04" with an 8 px `--success` dot |
| Stopping | "Wird beendet…" / "Stopping…" | secondary, spinner, inert | none |
| Stopping for 10 s or more | "Wird beendet…" plus a second button "Sofort beenden" / "Force stop" | second button `destructive`, default size, to the right | `game_force_stop()` |

Pressed: fill colour change plus scale 0.98 (section 8). Focus: outline. Disabled is never used on the Play button: every state either acts or explains itself in its label.

### 10.3 Account chip and account menu (custom chip on shadcn `dropdown-menu`)

Chip: a button, 40 px high, radius pill, `secondary` colours, padding 4 12 4 4: an initial circle (32 px, `--hover` fill, first letter of the player name, 14/20 weight 600 `--text`), 8 px gap, the player name 14/20 (`max-width: 16ch`, ellipsis; player names have at most 16 characters), 4 px gap, `chevron-down` 16. If the selected account needs sign-in again, a warning badge "Erneut anmelden" follows the name. `aria-label`: "Konto: {{name}}" / "Account: {{name}}"; `aria-haspopup="menu"`.

No account: the chip is a plain `secondary` button "Anmelden" / "Sign in" with `log-in` icon, opening the sign-in dialog. If the build cannot sign in (`can_sign_in` false), the chip still opens the menu, where "Konto hinzufügen" is disabled and a caption below it explains: "In diesem Build ist keine Microsoft-Anmeldung eingerichtet." / "This build has no Microsoft sign-in configured."

Menu (DropdownMenu, `align="end"`, width 280, radius 10, padding 4, items 40 px, radius 6):
1. Radio group of accounts: name, `check` 16 in `--accent` on the selected one, warning badge "Erneut anmelden" where needed. Choosing an account selects it; choosing one that needs sign-in starts sign-in for it. The dev account (dev builds only) carries the badge "Entwicklungs-Build".
2. Separator.
3. "Konto hinzufügen" / "Add account" (`user-plus`).
4. Submenu "Abmelden" / "Sign out" (`log-out`) listing one item per account; choosing one opens the remove dialog. Hidden when there are no accounts.

States: chip default, hover, pressed, focus, open (`--selected` fill while the menu is open). Menu items: default, highlighted (`--hover`), disabled.

### 10.4 Instance list row (custom `<li>` with a button)

48 px high, radius 6, padding 0 12, full content width. The row is a `<button aria-pressed>` that selects the instance; the action button is a sibling, not nested.

Columns (CSS grid, 16 px column gap): selection slot 24 px (`check` 18 `--accent` when selected) | name, `minmax(0, 1fr)`, body 16/24 `--text`, one line, ellipsis, full name in `title` | version 140 px, secondary 14/20 tabular | loader 180 px, secondary ("Fabric 0.17.3", "Kein Loader" / "No loader", Forge version shown as "Forge 47.4.28") | last played 160 px, caption 13/18, right-aligned | action button 40 × 40 (`ghost`, `ellipsis`, `aria-label` "Aktionen für {{name}}").

Above the list a column header row (32 px, caption 13/18 `--text-muted`, same grid): "Name", "Version", "Loader", "Zuletzt gespielt", with a 1 px `--border` below. Plain text, not sortable in v0.1.

States: default; hover `--hover`; selected `--selected` plus check icon (not colour alone) and `aria-pressed="true"`; focus outline on the row button; pressed `--selected`. Rows for `tooNew` and `broken` instances: name in `--text-muted`, the version and loader cells merged and holding a warning badge "Neuere Version" / "Newer format" or "Nicht lesbar" / "Unreadable", row not selectable (`aria-disabled`, the badge is the reason), action menu with only "Ordner öffnen".

Row action menu (DropdownMenu, `align="end"`): "Ordner öffnen" / "Open folder" (`folder-open`), separator, "Löschen" / "Delete" (`trash-2`, text `--danger`).

No instance cards in v0.1: the instance list is the only presentation (rows read faster and avoid the card-grid look).

### 10.5 Version picker (custom combobox on shadcn `popover` + `command`)

Trigger: like an input (40 px, `--surface-raised`, 1 px `--border-strong`, radius 6, padding 0 12), showing the selected version id (16/24) and `chevron-down` 16 at the right; `role="combobox"`, `aria-expanded`. Label above ("Spielversion" / "Game version").

Popover content: width equals the trigger, radius 10, padding 4: a search input (40 px, `search` icon, placeholder "Suchen" / "Search"), then the list (max-height 320 px, scrolls), items 40 px: version id left, and for non-release entries a caption at the right ("Snapshot", "Beta", "Alpha"). Selected item has `check`. Empty search result: "Keine Version gefunden für „{{query}}“" / "No version matches “{{query}}”" with a ghost button "Filter zurücksetzen" / "Reset filters" when filters hide results.

Filters: four checkboxes in one row below the trigger (outside the popover, so the active filter stays visible): "Releases", "Snapshots", "Beta", "Alpha"; default Releases only.

States: trigger default, hover (border `--text-muted`, as inputs), focus, open (border `--text-muted`, chevron turns to `chevron-up`), disabled (while versions load: label text "Versionen werden geladen…" / "Loading versions…" in the trigger), error (inline error below with a link button "Erneut versuchen" / "Try again").

### 10.6 Loader picker (custom segments on shadcn `radio-group`) and loader version (shadcn `select`)

Loader: a radio group rendered as separate segments that wrap: "Kein Loader", "Fabric", "Quilt", "Legacy Fabric", "Forge", "NeoForge" (proper names are not translated). Each segment 40 px high, padding 0 16, radius 6, 8 px gap, wraps to a second line when needed. Unselected: `--surface-raised`, 1 px `--border-strong`, text `--text` 14/20 weight 400. Selected: `--selected` fill, 1 px inset `--accent` outline, text weight 600, `check` 16 before the label. Arrow keys move the selection (radio group behaviour), focus outline on the focused segment.

Loader version: Select, trigger styled like the version picker. Items 40 px: version, then badges "Empfohlen" / "Recommended" or "Instabil" / "Unstable". Preselection: the recommended version. Hidden when "Kein Loader" is selected. Loading: trigger disabled with "Wird geladen…" / "Loading…". No builds for this game version: the select is replaced by the help text from `errors.noLoaderVersions` with its hint in `--warning` (icon `triangle-alert` 16), and the create button is disabled with `aria-describedby` pointing to that text. Fetch error: inline error with "Erneut versuchen".

### 10.7 Form field (shadcn `input` + `label`)

- Label above the field, 14/20 weight 600, 8 px gap. Never a placeholder as label.
- Input 40 px, 16/24 text, padding 0 12, `--surface-raised` fill, 1 px `--border-strong`, radius 6. Hover: border `--text-muted`. Focus: outline. Invalid: border `--danger`, `aria-invalid="true"`. Disabled: `--surface` fill, `--border`, `--text-disabled`.
- Below the field a help line with fixed height 20 px (14/20): help text in `--text-muted`, or the error in `--danger` with `circle-alert` 16 and `id` referenced by `aria-describedby`. Reserved height means nothing moves when an error appears (FO4).
- Validation on blur; after the first error, live on each keystroke until fixed (FO2). Inputs keep their values after any error (FO13).
- Character counter at the right of the help line from 80 % of the limit ("52/64"), tabular.
- Numeric fields (memory, 4b): `type="text"` with `inputmode="numeric"`, never `type="number"`.

### 10.8 Checkbox (shadcn `checkbox`) and switch (shadcn `switch`)

Checkbox: 16 px box with a 4 px radius (the single exception to the radius table: 6 px on a 16 px box reads as a circle), 1 px `--border-strong`, `--surface-raised` fill; checked: `--accent` fill with `check` 12 px in `--on-accent`. The box sits in a 40 px high clickable label row, label 14/20 to the right, 8 px gap. States: hover (border `--text-muted`), focus outline on the box, disabled (`--border`, `--text-disabled` label).

Switch (settings, 4b: "Beim Spielstart in den Infobereich"): 36 × 20 track, pill; off: `--hover` track with 1 px `--border-strong`, thumb `--text-muted`; on: `--accent` track, thumb `--on-accent`. The whole row (label plus switch) is the target, 40 px high. Track colour transitions over `--dur-press`; the thumb moves with `transform` over `--dur-press` and `--ease-out` (pointer only; with reduced motion or keyboard it jumps).

### 10.9 Dialogs (shadcn `dialog`, `alert-dialog`)

- Panel `--surface-raised`, radius 10, padding 24, level-1 elevation, on `--overlay`. Max-height `calc(100dvh - 48px)`; the body scrolls, title and footer stay.
- Title 18/26 weight 600 as the dialog's accessible name; text 16/24, at most 72ch. Close button (`icon`, `x`, "Schließen") top right in `dialog`, not in `alert-dialog`.
- Footer: buttons right-aligned, primary or destructive last, 8 px gap, 24 px above.
- Widths: create instance 560, sign-in 480, confirmations 440.
- Escape closes and cancels a running operation (sign-in), focus returns to the trigger. Initial focus: first field (create), primary action (sign-in idle), the safe button (confirmations).
- Confirmations are a simple confirm with named buttons, no typing the name: deleting moves the folder to the Windows recycle bin and signing out leaves the Microsoft account untouched, so both are reversible and typing a name would only slow down a recoverable action. The dialog title is the concrete question, the text names the consequence, the buttons name both outcomes (Z5).

### 10.10 Toast (shadcn `sonner`)

- Position bottom right, offset 24, width 356, at most 3 visible, newest on top.
- `--surface-raised`, radius 10, level-1 elevation, padding 12 16, title 14/20 weight 600 `--text`, description 13/18 `--text-muted`, icon 16 (success `check` in `--success`, info `info` in `--text-muted`).
- Duration 4 s, paused while hovered or focused. `role="status"` (Sonner's live region).
- Toasts confirm results ("{{name}} angelegt", "Angemeldet als {{name}}", "Einstellungen waren beschädigt und wurden zurückgesetzt"). Errors are never only a toast: an error is shown where it happened (inline or ErrorPanel). No undo action in v0.1 (deletion goes to the recycle bin).

### 10.11 ErrorPanel (custom, Task 4) and inline error

ErrorPanel: block, `--surface-raised`, 1 px `--border`, radius 10, padding 16, `role="alert"`. Row 1: `circle-alert` 18 `--danger`, 12 px gap, title 16/24 weight 600 `--text` (what happened). Row 2 (indented to the title): body 14/20 `--text-muted` (what to do). Row 3, 12 px below: actions: "Erneut versuchen" / "Try again" (`secondary`, `refresh-cw`) when `onRetry` is given, the action from `ErrorDto.action` as a second `secondary` button, and a `ghost` toggle "Details" with `chevron-down` / `chevron-up` (`aria-expanded`). Details: `detail` in mono 13/20 in a `--bg` box, radius 6, padding 12, max-height 160 scrolls, with a `ghost` button "Kopieren" / "Copy". Never shows raw codes as the title.

Inline error: under a field (10.7) or under a setting: `circle-alert` 16 plus one sentence in `--danger` 14/20.

### 10.12 Launch progress panel (custom, 4b, shadcn `progress` for the bar)

Sits 24 px below the Play button, width 480 max, appears only in the preparing state; its space is reserved from the moment preparing starts (fixed 120 px block), so nothing jumps between phases.

- Line 1, phase (body 16/24 `--text`), from `PlayEvent`: "Loader wird aufgelöst" / "Resolving loader", "Installer wird geladen" / "Downloading installer", "Dateien werden geladen" / "Downloading files", "Installation wird geprüft" / "Verifying installation", "Wird installiert" / "Installing", "Spiel wird gestartet" / "Launching game".
- Line 2 (secondary 14/20 tabular): counts "3.402 von 4.118 Dateien" / "3,402 of 4,118 files", or the installer step "Schritt 3 von 6: jarsplitter" / "Step 3 of 6: jarsplitter" (a real sequence, so numbers are right here).
- Bar: 6 px, pill, track `--hover`, fill `--accent`, determinate when counts or steps are known (`aria-valuenow`, `aria-valuemax`); during phases without counts the bar is hidden, not looped.
- Line 3 when an install log exists: link button "Installationsprotokoll öffnen" / "Open install log".
- `Notice` events appear as caption lines below (13/18 `--text-muted`), at most the last two.
- Accessibility: the phase line is `aria-live="polite"`; counts are not announced on every change.
- Failure: the panel is replaced by an ErrorPanel in the same place with "Erneut versuchen".

### 10.13 Log view (custom, 4b, Virtuoso inside the log area)

- Log area: `--surface`, 1 px `--border`, radius 10, fills the remaining height of the content, scrolls internally. Line padding 0 12, mono 13/20.
- Line layout (grid): time `hh:mm:ss` (8ch, `--text-muted`) | level (5ch, the level text as given, in its level colour) | thread (16ch max, ellipsis, `--text-muted`) | message (wraps, `overflow-wrap: anywhere`, in its level colour). Throwable lines are indented by 2ch under their message, same colour. The level is shown as text, so colour is never the only signal.
- Level colours: ERROR, FATAL `--log-error`; WARN `--log-warn`; INFO and lines without level `--log-info`; DEBUG, TRACE `--log-debug`.
- Selection: native text selection across lines, `::selection` background `--selection`, text `--text`.
- Search hits: `<mark>` with `--highlight` background and `--text`; the current hit additionally gets a 1 px `--text` outline.
- When the user scrolls up, following stops and a `secondary` button "Zum Ende springen" / "Jump to end" (`arrow-down-to-line`) appears at the bottom right inside the log area, 12 px from the edges; at the end, following resumes and the button disappears (no animation).

Toolbar above the log area, one row, 40 px controls, 8 px gaps, 16 px below:
- Visible label "Im Log suchen" / "Search log" (14/20 weight 600) left of the search input (width 320), then the hit count "12 Treffer" / "12 matches" (secondary, tabular), then icon buttons "Vorheriger Treffer" / "Nächster Treffer" (`chevron-up` / `chevron-down`). Enter and Shift+Enter in the field step through hits. Ctrl+F focuses the field.
- "Stufen" / "Levels" dropdown (`secondary`, shows "Stufen: 4 von 5" / "Levels: 4 of 5") with checkbox items Fehler, Warnung, Info, Debug, Sonstige.
- Right-aligned: "Kopieren" / "Copy" (`secondary`, `copy`; copies the filtered lines, redacted by the backend) and "Logdatei öffnen" / "Open log file" (`ghost`, `external-link`).

### 10.14 Badge (shadcn `badge`)

Pill, 20 px high, padding 0 8, 13/18 weight 600, no fill colours beyond these:

| Badge | Text | Fill | Border |
|---|---|---|---|
| "Empfohlen" / "Recommended" | `--accent` | `--selected` | none |
| "Instabil" / "Unstable", "Neuere Version", "Nicht lesbar", "Erneut anmelden", "Entwicklungs-Build" | `--warning` | `--surface-raised` | 1 px `--border` |

Badges carry text, never a dot or an emoji.

### 10.15 Others

- Skeleton (shadcn `skeleton`, with `animate-pulse` removed): `--hover` fill, radius 6, the exact size of the content it stands for.
- Tooltip (shadcn `tooltip`): inverse colours, radius 6, padding 4 8, 13/18, max-width 240; only for icon-only buttons and truncated text, never for required information.
- Separator (shadcn `separator`): 1 px `--border`, 4 px margin in menus.
- ScrollArea (shadcn `scroll-area`): only inside popovers and dialogs; the content column uses native scrolling. Scrollbar thumb `--border-strong`, track transparent.
- Progress (shadcn `progress`): see 10.12.
- Not used: Card, Sheet, Tabs, Kbd, Avatar (the initial circle is custom and does not load images), Sidebar block, Accordion, Carousel.

## 11. Screens

Each screen: regions, primary action, states (loading, empty, error, plus specific ones), keyboard path. Tab order on every screen: sidebar nav, then header (primary action, account chip), then content top to bottom, left to right.

### 11.0 App start

- `main.tsx` loads `settings_get`, then i18n and theme, then renders. Until then the window shows only `--bg` (no text, no spinner).
- Loading longer than 1 s: still `--bg`; the skeleton of the shell (sidebar items, header title block) appears.
- Empty: not applicable (missing settings give defaults).
- Error (`settings_get` fails even after the broken-file fallback): an ErrorPanel, 480 px wide, horizontally centred, 96 px from the top, on `--bg`, with "Try again". Its text is English when i18n could not be initialised, otherwise in the UI language.
- Settings were reset: toast once after render.

### 11.1 Instances (4a; start view in 4a)

```
Instanzen                                       [+ Instanz anlegen]  (S Steve v)
-------------------------------------------------------------------------------
Name                       Version      Loader            Zuletzt gespielt
[v] Überleben 1.21          1.21.11      Fabric 0.17.3     vor 2 Tagen       (...)
    1.20.1 Forge            1.20.1       Forge 47.4.28     Noch nie gespielt (...)
    Alte Welt               [Neuere Version]                                 (...)
```

- Header: h1 "Instanzen" / "Instances", primary "Instanz anlegen" / "Create instance" (`plus`) in the header actions slot.
- Content: column header row, then `<ul>` of rows (10.4), 0 px gap between rows (the hover fill separates them), list grows; the content column scrolls.
- Loading: column header plus three skeleton rows (name block 40 %, version 80 px, loader 120 px, date 96 px).
- Empty: the column header is not shown. Left-aligned block at the top of the content, max-width 480: title (h2 18/26) "Noch keine Instanz" / "No instances yet", one line secondary "Eine Instanz ist ein eigener Spielordner mit Version und Loader." / "An instance is its own game folder with a version and a loader.", 16 px, primary button "Instanz anlegen". The header primary is hidden in this state so there is one primary action.
- Error: ErrorPanel in place of the list with "Erneut versuchen".
- Many (40): plain scrolling list, no virtualisation (below the 50-row threshold).
- Keyboard: Tab to the header action, the chip, then the first row; Tab moves row button, its action button, next row. Enter or Space on a row selects it; Enter on the action button opens its menu (arrow keys, Escape closes and focus returns to the action button). Delete key on a focused row opens the delete dialog.

### 11.2 Create instance (dialog, 4a)

Width 560. Fields top to bottom, 16 px apart:
1. "Spielversion" / "Game version": combobox (10.5), filter checkboxes below.
2. "Loader": segments (10.6).
3. "Loader-Version" / "Loader version": select (hidden for "Kein Loader").
4. "Name": input, max 64 characters, prefilled from the selection ("1.20.1 Forge") until the user edits it; help line "So heißt die Instanz in der Liste." / "This is how the instance appears in the list."

Footer: "Abbrechen" / "Cancel" (`secondary`), "Instanz anlegen" / "Create instance" (`primary`, loading "Wird angelegt…" / "Creating…").

- Loading: version trigger disabled with "Versionen werden geladen…"; loader version select disabled with "Wird geladen…".
- Empty: version list empty because of filters: the combobox empty text with "Filter zurücksetzen". Loader has no builds: hint and disabled create button (10.6).
- Error: versions failed to load: inline error under the version field with "Erneut versuchen" (the cached list is used if one exists, with the caption "Gespeicherte Liste, nicht aktuell" / "Saved list, may be outdated"). Create failed: ErrorPanel directly above the footer, all inputs kept, focus on the panel title.
- Validation: name empty or whitespace: "Gib einen Namen ein." / "Enter a name."; longer than 64: "Höchstens 64 Zeichen." / "At most 64 characters."; line break: removed on paste.
- Success: dialog closes, toast "{{name}} angelegt" / "{{name}} created", the new row is selected and scrolled into view.
- Keyboard: focus starts on the version trigger; Tab through filters, loader segments (one tab stop, arrows inside), loader version, name, Cancel, Create. Enter in the name field submits. Escape closes (inputs are discarded; the dialog is short).

### 11.3 Delete instance (alert dialog, 4a)

Width 440. Title "{{name}} löschen?" / "Delete {{name}}?". Text "Der Ordner mit Welten, Einstellungen und Screenshots wird in den Papierkorb verschoben." / "The folder with worlds, settings and screenshots is moved to the recycle bin." Buttons "Behalten" / "Keep" (`secondary`, initial focus) and "In den Papierkorb" / "Move to recycle bin" (`destructive`, loading "Wird verschoben…" / "Moving…").

- Loading: the destructive button shows its loading state, both buttons inert.
- Error: ErrorPanel inside the dialog above the buttons; the dialog stays open.
- Success: dialog closes, row disappears without animation, toast "{{name}} ist im Papierkorb" / "{{name}} is in the recycle bin". If it was selected, nothing is selected.
- Empty: not applicable (opened for one instance).
- Keyboard: Tab between the two buttons, Enter activates the focused one, Escape keeps.

### 11.4 Accounts (chip, menu, remove dialog; 4a)

There is no accounts page; accounts live in the header chip (10.3).

- Loading: the chip shows a skeleton of its shape (40 × 120 pill).
- Empty: "Anmelden" button (10.3); the Play button on the start page also says "Anmelden".
- Error (account list failed): the chip becomes a `secondary` button "Konten nicht verfügbar" / "Accounts unavailable" with `triangle-alert`; its menu holds the ErrorPanel text and "Erneut versuchen". The credential store failing does not block the list (plan, review focus 3).
- Remove dialog (alert dialog, 440): title "{{name}} abmelden?" / "Sign out {{name}}?", text "Die gespeicherte Anmeldung wird von diesem PC entfernt. Dein Microsoft-Konto bleibt unverändert." / "The saved sign-in is removed from this PC. Your Microsoft account stays unchanged.", buttons "Behalten" / "Keep" (initial focus) and "Abmelden" / "Sign out" (`destructive`). Success toast "{{name}} abgemeldet" / "Signed out {{name}}".
- Keyboard: Tab to the chip, Enter opens the menu, arrows move, Enter selects, the submenu opens with the right arrow; Escape closes and returns focus to the chip.

### 11.5 Sign-in (dialog, 4a)

Width 480. Title "Konto hinzufügen" / "Add account" in every phase. The body changes per phase; the dialog height may change between phases (no animation).

| Phase | Body | Buttons (left to right) |
|---|---|---|
| idle | "Die Anmeldung läuft im Browser bei Microsoft. Hengst Client sieht dein Passwort nicht." / "You sign in with Microsoft in your browser. Hengst Client never sees your password." | ghost "Code stattdessen verwenden" / "Use a code instead", secondary "Abbrechen", primary "Mit Microsoft anmelden" / "Sign in with Microsoft" (`external-link`) |
| waitingBrowser | "Melde dich im Browser an. Danach geht es hier automatisch weiter." / "Sign in in your browser. This window continues on its own afterwards." | ghost "Code stattdessen verwenden", secondary "Browser erneut öffnen" / "Open browser again", secondary "Abbrechen". No primary: waiting is the state |
| showingCode | "Öffne microsoft.com/link und gib diesen Code ein." / "Open microsoft.com/link and enter this code."; code box (mono 28/36, `--bg`, radius 6, padding 12 16) with a `secondary` "Code kopieren" / "Copy code" to its right; caption "Noch 14:32 gültig" / "Valid for 14:32" (tabular, updates each second, not announced) | secondary "Abbrechen", primary "microsoft.com/link öffnen" / "Open microsoft.com/link" |
| completing | "Wird angemeldet…" / "Signing in…" | all inert |
| failed | ErrorPanel (with "Erneut versuchen", which restarts the same method) | secondary "Schließen" / "Close" |
| appNotApproved | own block, section 11.6 | secondary "Mehr erfahren" / "Learn more", primary "Schließen" / "Close" |
| done | dialog closes, toast "Angemeldet als {{name}}" / "Signed in as {{name}}" | |
| cancelled | dialog closes, focus to the chip | |

- Loading: idle primary shows "Wird geöffnet…" / "Opening…" between the click and `browserOpened` (guard against double click).
- Empty: not applicable.
- Error: the failed phase.
- Code copied: the copy button label changes to "Kopiert" / "Copied" for 2 s (no toast).
- Keyboard: initial focus on the primary (idle) or on "Code kopieren" (showingCode); Escape cancels the running login and closes, focus returns to the chip.

### 11.6 "Sign-in not available yet" block

Shown inside the sign-in dialog when the error code is `appNotApproved`, instead of the generic ErrorPanel. Not an error style: `info` icon 18 in `--text-muted`, title (18/26) "Anmeldung noch nicht verfügbar" / "Sign-in is not available yet", text (16/24) "Mojang hat die Microsoft-Anmeldung von Hengst Client noch nicht freigegeben. Bis dahin lassen sich keine Konten hinzufügen. Mit deinem Konto ist alles in Ordnung, du musst nichts ändern." / "Mojang has not approved Hengst Client's Microsoft sign-in yet. Until then, accounts cannot be added. Your account is fine, nothing needs to change on your side." In dev builds a caption below: "Entwicklungs-Builds können das Offline-Entwicklungskonto verwenden." / "Development builds can use the offline development account." Buttons as in the table above; "Mehr erfahren" opens `UrlTarget::SignInHelp`.

### 11.7 Settings (4a partial, 4b complete) with About

Single column, max-width 640, sections 48 px apart, each with an h2 and controls 24 px below it. Labels above controls. Changes save on selection (no save button); if saving takes longer than 400 ms the help line shows "Wird gespeichert…" / "Saving…".

Sections in order:
1. "Sprache" / "Language": segments "System", "Deutsch", "English".
2. "Design" / "Theme": segments "System", "Hell" / "Light", "Dunkel" / "Dark".
3. "Arbeitsspeicher" / "Memory" (4b): input with `inputmode="numeric"`, suffix "MB", help line "Empfohlen: 2048 bis 4096 MB. Installiert: 16 GB." / "Recommended: 2048 to 4096 MB. Installed: 16 GB.", step buttons are not used (typing is faster), validation 512 to 65,536 with the error "Wähle einen Wert zwischen 512 und 65.536 MB." / "Choose a value between 512 and 65,536 MB." Behind a disclosure "Erweitert" / "Advanced": "Java" (4b): path text in mono 13/20, buttons "Java auswählen…" / "Choose Java…" and "Zurücksetzen" / "Reset".
4. "Spielstart" / "Launch" (4b): switch "Beim Spielstart in den Infobereich verkleinern" / "Minimise to the notification area when the game starts".
5. "Datenordner" / "Data folder": path in mono 13/20, `secondary` "Ordner öffnen" / "Open folder".
6. "Über Hengst Client" / "About Hengst Client": mark 48 px, name 18/26, version (tabular), dev badge if any, "Not an official Minecraft product." (identical in both languages), "Lizenz: GPL-3.0" / "Licence: GPL-3.0", link "Quellcode" / "Source code" (opens the repository via `open_url`), link "Drittlizenzen" / "Third-party licences" (4b). The AboutPanel facts from Task 2 (number of known game versions, newest release and snapshot) as secondary lines.

- Primary action: none (settings save themselves). This is the one screen without a primary button, on purpose.
- Loading: skeletons per section (title block 120 px, control block 320 × 40).
- Empty: not applicable; About facts without data show "Noch keine Versionsliste geladen" / "No version list loaded yet".
- Error: saving failed: inline error under that control, the control returns to the previous value. Loading About facts failed: inline error with "Erneut versuchen" in that section only.
- Keyboard: Tab from section to section control; segments are one tab stop each with arrow keys inside; the switch toggles with Space.

### 11.8 Start page (4b; start view from 4b)

```
Spielen                                                         (S Steve v)

  Überleben mit Freunden 1.21
  1.21.11        Fabric 0.17.3        Zuletzt gespielt vor 2 Tagen
  [Instanz wechseln v]

  [  > Spielen                     ]

  Dateien werden geladen
  3.402 von 4.118 Dateien
  [=================-------]
  Installationsprotokoll öffnen
```

- Header: h1 "Spielen" / "Play", chip on the right, no header action (the Play button is the action).
- Content: one left-aligned block, 48 px from the top of the content, max-width 640: start title (32/40, two lines max), 8 px, meta line (secondary 14/20: version, loader, last played as three separate spans with 24 px gaps, no separators), 16 px, `ghost` "Instanz wechseln" / "Switch instance" with `chevron-down` (DropdownMenu with the instances as radio items), 32 px, Play button (10.2), 24 px, progress panel or ErrorPanel (10.12), and in the running state a link button "Log anzeigen" / "Show log".
- Loading: skeleton title (320 × 40), meta (240 × 20), button (240 × 56).
- Empty: no instance: title "Noch keine Instanz" / "No instances yet", text "Lege eine Instanz an, um zu spielen." / "Create an instance to play.", Play button in its "Instanz anlegen" state. No account: the instance is shown normally, the button says "Anmelden".
- Error: instance or game state failed to load: ErrorPanel in place of the block with "Erneut versuchen". Launch failed: ErrorPanel below the button, button back to "Spielen".
- At 600 px window height everything fits without scrolling: 56 header + 48 top + 80 title + 8 + 20 meta + 16 + 40 switch button + 32 + 56 Play + 24 + 120 progress = 500 px.
- Keyboard: Tab to "Instanz wechseln", then the Play button. The Play button gets initial focus when the view opens from the sidebar, so Enter plays. No global hotkey.

### 11.9 Live log (4b)

- Header: h1 "Log", chip. Above the log area the toolbar (10.13). Below the toolbar, a secondary line naming the run: "Aktueller Lauf: Überleben 1.21, seit 12:04" / "Current run: ..." with a `ghost` select to switch to "Letzter Lauf" / "Previous run".
- Loading: toolbar visible but inert, log area with eight skeleton lines of varying width (40 to 90 %).
- Empty: no run yet: inside the log area, top-left with 16 px padding, title "Noch kein Log" / "No log yet", text "Starte eine Instanz, dann erscheint ihre Ausgabe hier." / "Start an instance and its output appears here.", primary "Zur Startseite" / "Go to Play". Run without output yet: "Noch keine Ausgabe" / "No output yet" caption. Filter hides everything: "Alle Zeilen sind ausgeblendet." / "All lines are hidden." with `ghost` "Alle Stufen zeigen" / "Show all levels".
- Error: attach failed: ErrorPanel inside the log area with "Erneut versuchen". Copy failed: inline error under the toolbar.
- 10,000 lines: virtualised, no layout change.
- Keyboard: Ctrl+F to search; Tab: search, previous, next, levels, copy, open file, then the log area (focusable, `tabindex="0"`, arrow keys and Page Up/Down scroll, Ctrl+A selects all lines of the current filter, Ctrl+C copies the selection), then "Zum Ende springen" if shown.

### 11.10 Instance detail

There is no instance detail screen in v0.1. Everything about an instance is in its row, its menu and the start page.

## 12. Writing

- Two languages, German and English, complete in both; all strings in `src/i18n/{de,en}.json`. German uses informal "du" throughout, never "Sie". No English leftovers in the German UI.
- Sentence case in both languages for everything: titles, buttons, menu items, labels ("Instanz anlegen", "Create instance"). This overrides WIG's title case for English buttons (produkt-regelwerk W15) because the plan fixes one style for both languages and mixed case between the two languages would look inconsistent.
- Buttons are verbs naming the result ("Instanz anlegen", "In den Papierkorb", "Abmelden"), never "OK", "Ja", "Weiter", "Submit". A flow keeps its verb: "Instanz anlegen" leads to "{{name}} angelegt".
- Loading labels: verb plus a real ellipsis "…" ("Wird angelegt…", "Creating…").
- Errors: two parts, what happened and what to do: "Die Versionsliste konnte nicht geladen werden." plus "Prüfe die Internetverbindung und versuche es erneut." No apology, no blame, no codes in the title, no exclamation marks.
- No exclamation marks anywhere. No filler ("einfach", "schnell", "nahtlos", "simply", "seamless"), no greetings, no marketing words. No full stop at the end of titles, labels, buttons and captions; full sentences in body text end with one.
- No dashes as separators in visible text (no "—", "–"); ranges with "bis" / "to". Quotes: „…“ in German, “…” in English.
- Numbers, units, dates: section 3.3.
- Proper names stay untranslated: Fabric, Quilt, Legacy Fabric, Forge, NeoForge, Microsoft, Mojang, Hengst Client. "Minecraft" appears only in the fixed sentence "Not an official Minecraft product." and where Microsoft's own pages are named, never as part of the product name.

## 13. Accessibility

- Focus: visible on every interactive element (section 9); order sidebar, header, content; modals trap focus and return it to the trigger; Escape closes.
- Real elements only: `<button>` for actions, `<a>`-like link buttons only for opening things outside the app, `<nav>` with `aria-label`, one `<h1>` per view (the header title), headings without gaps (h1, then h2).
- Targets: 40 × 40 px minimum for all controls (buttons, icon buttons, segments, menu items, checkbox rows, switch rows), list rows 48 px, Play 56 px; 8 px between adjacent targets. 40 px is above the WCAG 2.2 AA minimum of 24 × 24 and close to the 44 px guideline; for a mouse-first desktop app it is the deliberate density (produkt-regelwerk W7 allows the smaller size for dense desktop UI with a mouse).
- Names: every icon-only button has an `aria-label` that matches its tooltip (section 6). Visible text is part of the accessible name.
- States are programmatic: `aria-current="page"` (nav), `aria-pressed` (rows), `aria-expanded` (menus, details, combobox), `aria-checked` via the radio and checkbox primitives, `aria-busy` on lists while loading, `aria-disabled` plus `aria-describedby` for explained disabled controls, `aria-invalid` on fields.
- Announcements: toasts `role="status"`, ErrorPanel `role="alert"`, the launch phase `aria-live="polite"`.
- Contrast: section 2.3; state is never shown by colour alone (check icons, weight, text badges, level text in the log).
- Scaling: at 125 % and 150 % Windows display scaling the layout is the same in logical pixels; every screen is checked at both. Text sizes are in `rem` and nothing has a fixed height around text except controls whose text fits by design, so WebView zoom up to 200 % (Ctrl+Plus, enabled on the window with `zoom_hotkeys_enabled(true)`) reflows without clipping: rows grow, button rows wrap, the sidebar collapses earlier because the logical width shrinks. On small screens where 960 × 600 does not fit at 150 % (for example 1366 × 768), Windows limits the window; the content still scrolls and nothing is cut off horizontally.
- Reduced motion: section 8.

## 14. Brand mark (placeholder)

The owner designs the real logo. Until then the app uses a placeholder that is plainly geometric and does not pretend to be finished:

- A rounded square, 64 × 64 viewBox, corner radius 14, filled `#1D6F80` (light `--accent`, used in both themes because the mark is a static file and must also work as the taskbar icon).
- On it a stylised "H" in `#F6FBFC` (`--on-accent`) made of three rectangles with square ends: left bar x 18, y 16, 8 × 32; right bar x 38, y 16, 8 × 32; crossbar x 26, y 28, 12 × 8.
- No gradient, no shadow, no horse, no pixel art, nothing from the game.

Where it appears: the sidebar brand row (24 px), the About section (48 px), the window and taskbar icon and the installer icon (generated by `pnpm tauri icon brand/mark.svg`).

Swap: the folder `apps/desktop/brand/` is the only place to change. Replace `apps/desktop/brand/mark.svg`, run `pnpm tauri icon brand/mark.svg`, done; the frontend imports the mark only from that file (`apps/desktop/brand/README.md`, Task 3).

## 15. Not like this

Concrete template tells this app will not have, and what it does instead:

1. Gradient or glowing Play button, "hero" backgrounds, blurred game screenshots behind content. Instead: a flat accent button on `--surface`.
2. A grid of identical shadowed instance cards with cover images. Instead: a 48 px list with columns.
3. Uppercase tracked labels ("INSTANCES", "VERSION") and eyebrows above headings. Instead: sentence-case h1 and plain column labels.
4. Meta strings like "1.21.11 · Fabric · 2 days ago". Instead: separate columns or separate spans with space.
5. "→" or chevrons on buttons that do not open anything. Chevrons only on menu and select triggers.
6. Status dots in front of every item, pulsing "online" dots. One `--success` dot, only next to "Läuft seit".
7. Emoji, sparkles, "New" or "Pro" badges, version numbers as decoration.
8. Pure black dark theme with a neon accent; cream background with terracotta. Instead: the graphite ramp in 2.1 with one petrol accent.
9. One radius on everything. Instead: 6 for controls, 10 for panels, pill only for chip, badges and progress.
10. Fade-and-slide-in of every section, hover scale on rows, `transition: all`, spinners in the middle of the page. Instead: section 8 and skeletons.
11. Greeting headlines ("Welcome back, Steve"), exclamation marks, filler copy. Instead: the view name as title.
12. A centred empty state with a large illustration. Instead: a left-aligned title, one sentence and one button.
13. Anything that imitates the game's look or another launcher's: blocky fonts, dirt or grass textures, pixel icons, skin heads in the account chip (the chip shows an initial).

## 16. Decisions

| Decision | Reason | Rule that decided |
|---|---|---|
| shadcn/ui on Base UI, tokens remapped, shadcn variables not declared on `:root` | Real design system for app UI; the `--accent` name collides, mapping through `@theme inline` keeps component classes unchanged | design-unslop mode A, plan "UI-Bibliothek", pick-ui-library |
| System fonts (Segoe UI Variable, Cascadia Mono) | Windows-only app that should feel native; no bundled files, nothing fetched at runtime | plan Task 1 brief; design-unslop "Inter only for neutral app briefs" does not apply because no web font is chosen at all |
| Weights 400 and 600 only | Hierarchy through size and colour | design-unslop hard rules |
| 40 px controls, 48 px rows, 56 px Play | Mouse-first desktop density; above the 24 px WCAG minimum | produkt-regelwerk T1 with W7 (desktop with mouse) |
| No cards; list rows and background steps | Avoids the SaaS-card kit and keeps the launcher quiet | frontend-design (pattern 4), design-unslop B7 |
| One accent, petrol teal | Calm, technical, distinct from the game's and other launchers' colours | design-unslop F7, F8, F12, tokens "Akzent" |
| `--border-strong` sharpened to 3:1 | Control boundaries must reach 3:1 | produkt-regelwerk A4, WCAG 1.4.11; the plan allows sharpening values for contrast |
| Ease-out curve `cubic-bezier(0.23, 1, 0.32, 1)` instead of the draft `cubic-bezier(0.2, 0, 0, 1)` | The draft curve starts flat and delays the first movement | produkt-regelwerk M3 and W4, emil-design-eng (more specific for product UI than the draft) |
| No animation on keyboard-opened menus and view switches | Frequent actions must feel instant | emil-design-eng "never animate keyboard-initiated actions", produkt-regelwerk M1 |
| Static skeletons, no pulse | Calm tool; loading is short and local | design-unslop M1 (pulse only for live), produkt-regelwerk L1, L2 |
| Simple confirm, no typed name, for delete and sign-out | Both are reversible (recycle bin, account untouched) | produkt-regelwerk Z5 (named buttons); plan fixes a dialog instead of undo because restoring from the recycle bin is outside the app |
| Sentence case in English too | One style in both languages | plan "Sprache und Ton" over WIG title case (W15) |
| Settings without a primary button | Settings save on change; a "Save" button would be a second, pointless step | produkt-regelwerk N1 is about competing actions, not about requiring one |
| Search field label to the left of the field in the log toolbar | Visible label without a second toolbar row | produkt-regelwerk FO1 (visible label), adapted for a toolbar |
| stitch-design-taste used only for the document shape (named colours with hex and role, explicit anti-patterns) | Its Inter ban, perpetual micro-motion, spring defaults and asymmetric hero rules target marketing pages | design-unslop mode A and produkt-regelwerk M1/M2 are the specific rules for app UI |

## 17. Review notes

Checked against `frontend-design`: none of the five default patterns applies (no cream and terracotta, no near-black with neon, no broadsheet layout, no card kit with uniform radius and grey shadow, no uppercase eyebrows, middle-dot meta strings, mono data labels or "→" chrome; the log uses mono because it is log output, not decoration). "Remove one accessory": the instance cards and a coloured status dot per row were dropped in favour of plain rows and one dot only for the running state.

Checked against produkt-regelwerk: WD9 holds (no pure black or white surface or text, table 2.1); M2 holds (120 to 220 ms, exits 25 to 27 % shorter); M3 holds after replacing the draft curve; M7 is specified (section 8); A1 holds (2 px outline, 3:1 or more on every surface, table 2.3); A4 forced the `--border-strong` change.
