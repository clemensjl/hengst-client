# Hengst Client design

Design specification for the desktop app (`apps/desktop`, Tauri 2, React, Tailwind CSS v4, shadcn/ui on Base UI). Every UI task builds against this file, and the plan defers to it for every value. Values here are decisions, not suggestions. If a value has to change, change it here in the same commit as the code. Appendix A collects implementation notes that are not part of the design itself.

Scope: v0.1 (phase 4a and 4b). Windows 10/11 x64 only. English is US spelling.

## 1. Direction

Hengst Client is a calm, fast tool that people open several times a day to start a game, often for less than ten seconds. It should feel like a well-made Windows utility: quiet surfaces, readable text, one obvious next action, and an immediate response to every click. It must not feel like a game menu, a store or a marketing site: no promotions, no decorative motion, no artwork from the game, nothing that competes with the Play button.

Five lines of direction (design-unslop, step 4):

- **Product:** a launcher that gets a chosen game version running in under ten seconds and shows what it is doing while it works.
- **Character:** a quiet Windows utility. Native fonts and title bar, mouse-first density, no ornament.
- **World:** the game's data, not its art: version ids, loader builds, file counts, installer steps, log lines. These are shown exactly as they come, in a typography that makes them easy to scan.
- **Moments:** two. The Play button that turns from Play into Cancel into Stop while the panel under it shows the real phase and the real file count ("3,402 of 4,118 files", "Step 3 of 6: jarsplitter"); and the live log, readable in both themes at 10,000 lines.
- **Signature:** the petrol accent marks the single next action on a screen and nothing else. A screen has one accent-filled button at most; the only other accent marks are the active navigation icon and the focus ring.

Rules that follow from this:

- One place may stand out: the start page (4b) with the instance name in large type and the Play button. Everything else stays still.
- Surfaces are separated by background steps and 1 px lines, not by shadows. No grid of shadowed cards, no gradients, no glass, no glow.
- No uppercase labels, no eyebrow labels above headings, no numbered markers (01, 02) unless the content is a real sequence (installer steps are), no "→" on buttons or links, no meta strings joined with middle dots.
- The accent is used on exactly three kinds of element: the primary action, the selected state of the navigation, and the focus ring (section 2.2).
- No emojis anywhere. Icons from one set (lucide) are fine.
- No assets from the game or from other launchers: no Minecraft fonts, textures, blocks, logos, skin renders or color schemes, and nothing modeled on Lunar Client, Feather, Badlion, Prism Launcher or Modrinth App.

Mode: this is app UI (design-unslop mode A). It uses a real design system, shadcn/ui, with its tokens remapped to the values below. No marketing "taste" style.

## 2. Color tokens

All colors are CSS custom properties on `:root` (light) and `.dark` (dark) in `apps/desktop/src/styles/index.css` (Task 3). Hex values are authoritative. Neither theme uses pure black or pure white for a surface or for text. `:root` declares `color-scheme: light` and `.dark` declares `color-scheme: dark`, so native scrollbars, text selection handles and form controls follow the theme.

### 2.1 Token table

| Token | Light | Dark | Use |
|---|---|---|---|
| `--bg` | `#F3F4F6` | `#121417` | Window background, sidebar, native window background at creation |
| `--surface` | `#FAFAFB` | `#1A1D21` | Content column (header and view), log area |
| `--surface-raised` | `#FDFDFE` | `#22262B` | Inputs, dialogs, popovers, menus, toasts, error panels |
| `--border` | `#D9DCE1` | `#2E333A` | Decorative hairlines: sidebar edge, header line when scrolled, list column header, light-theme outline of floating layers, error panel outline |
| `--border-strong` | `#B9BEC6` | `#454C55` | Decorative: dark-theme outline of floating layers, separators inside menus in dark, progress track, scrollbar thumb (`scrollbar-color: var(--border-strong) transparent` on `html`, and the ScrollArea thumb) |
| `--border-control` | `#858C96` | `#6E7680` | The boundary of an interactive control when it is its only boundary: input, select and combobox trigger, checkbox, segment, secondary button, switch track (off), account chip |
| `--text` | `#1A1D21` | `#E7E9EC` | Primary text, icons next to primary text, selected-row check, links, progress fill, checkbox and switch "on" fill |
| `--text-muted` | `#59616D` | `#9AA3AE` | Secondary text, help lines, column labels, row meta, inactive nav, switch thumb (off) |
| `--text-disabled` | `#8B929C` | `#6B737D` | Text and icons of disabled controls only |
| `--accent` | `#1D6F80` | `#4FB0C2` | Primary button fill, active nav icon. Nothing else |
| `--accent-hover` | `#185E6D` | `#69BECD` | Primary button hover |
| `--accent-pressed` | `#134E5B` | `#86CCD8` | Primary button pressed |
| `--on-accent` | `#F6FBFC` | `#0B1A1E` | Text and icons on the three accent fills |
| `--focus` | `#1D6F80` | `#69BECD` | Focus outline |
| `--danger` | `#B42318` | `#F4756B` | Destructive button fill, error text, error icons, "Delete" menu item |
| `--danger-hover` | `#A62016` | `#FA877E` | Destructive button hover |
| `--danger-pressed` | `#971D14` | `#FC9990` | Destructive button pressed |
| `--on-danger` | `#FFF7F6` | `#1F0A08` | Text and icons on the three danger fills |
| `--warning` | `#8A5A00` | `#E0A93B` | Warning text and icons, warning badge text |
| `--success` | `#2D7A4E` | `#5BB683` | Success icon in toasts, the "running" dot |
| `--hover` | `#EAECEF` | `#2A2F35` | Hover fill of ghost buttons, list rows, menu items, nav items; skeleton blocks; disabled button fill |
| `--selected` | `#E1E4E8` | `#2F343B` | Selected list row, active nav item, selected segment, open chip, pressed secondary and ghost buttons |
| `--selection` | `#C4E0E6` | `#2C5560` | Text selection (`::selection`) only. A pale tint of the accent hue, but a system-level affordance for selected text, not one of the three accent uses in 2.2 |
| `--highlight` | `#F6E3A1` | `#5C4A12` | Search hits in the log (`<mark>`) |
| `--overlay` | `rgb(16 18 20 / 0.40)` | `rgb(0 0 0 / 0.60)` | Backdrop behind dialogs |
| `--elevation-1` | `0 8px 24px rgb(16 18 20 / 0.14)` | `0 8px 24px rgb(0 0 0 / 0.45)` | Shadow of floating layers (section 5) |
| `--log-error` | `#B42318` | `#F4756B` | Log lines with level ERROR or FATAL (same as `--danger`) |
| `--log-warn` | `#8A5A00` | `#E0A93B` | Log lines with level WARN (same as `--warning`) |
| `--log-info` | `#1A1D21` | `#E7E9EC` | Log lines with level INFO and lines without a level (same as `--text`) |
| `--log-debug` | `#59616D` | `#9AA3AE` | Log lines with level DEBUG or TRACE (same as `--text-muted`) |

Border roles (plan, Task 1): `--border` and `--border-strong` are decorative. They separate regions, rows, panels and dialogs that are also recognizable by surface step, spacing or labels, carry no information on their own, and have no contrast requirement. Every line that is the only boundary of an interactive control uses `--border-control`, which reaches 3:1 against `--bg`, `--surface` and `--surface-raised` (WCAG 1.4.11). shadcn's `--input` points at `--border-control` and never at `--border` or `--border-strong`.

Rules for text on tinted backgrounds:
- On `--selection` and `--highlight`, text is always `--text`, whatever its level color was (`::selection` and `mark` set `color: var(--text)`).
- Warning and danger text sit on `--bg`, `--surface`, `--surface-raised`, `--hover` or `--selected` only.

### 2.2 Accent

`--accent` is a petrol teal (light `#1D6F80`, HSL about 191°, 63 %, 31 %; dark `#4FB0C2`, lighter and less saturated for dark surfaces). Reason: a cool, technical color that reads as "tool", stays calm at the low saturation the rules ask for (below 80 %), and keeps 4.5:1 for text on it in both themes. It is deliberately not the grass or creeper greens associated with the game, not the bright green of Modrinth App, and not the saturated blues, purples or reds that other launchers use as their signature. It is also not the AI-default violet or the terracotta-and-cream pairing.

The accent appears on exactly three kinds of element (design-unslop F7):
1. The primary action: the one primary button of a screen or dialog, including the Play button.
2. The selected state of the navigation: the icon of the active nav item.
3. The focus ring (`--focus`, the same hue).

Everything that earlier drafts tinted with the accent uses neutral tokens: selected rows and segments (`--selected` fill plus a `--text` check), checkboxes and switches (`--text` fill), links (`--text`, underlined), progress (`--text` fill on a `--border-strong` track), and badges (section 10.14). The placeholder brand mark (section 14) is a static image in the accent color; it is not an interface element.

### 2.3 Contrast (calculated, WCAG 2.x relative luminance)

Text pairs need 4.5:1; control boundaries, focus ring, icons and the progress fill need 3:1. Every value is computed from the hex values in 2.1 with (L1 + 0.05) / (L2 + 0.05).

| Foreground | Background | Light | Dark | Requirement |
|---|---|---|---|---|
| `--text` | `--bg` | 15.37 | 15.17 | 4.5 |
| `--text` | `--surface` | 16.21 | 13.90 | 4.5 |
| `--text` | `--surface-raised` | 16.64 | 12.51 | 4.5 |
| `--text` | `--hover` | 14.29 | 11.09 | 4.5 |
| `--text` | `--selected` | 13.26 | 10.31 | 4.5 |
| `--text` | `--selection` | 12.20 | 6.70 | 4.5 |
| `--text` | `--highlight` | 13.23 | 7.07 | 4.5 |
| `--text-muted` | `--bg` | 5.69 | 7.23 | 4.5 |
| `--text-muted` | `--surface` | 6.00 | 6.62 | 4.5 |
| `--text-muted` | `--surface-raised` | 6.16 | 5.96 | 4.5 |
| `--text-muted` | `--hover` | 5.29 | 5.28 | 4.5 |
| `--text-muted` | `--selected` | 4.91 | 4.91 | 4.5 |
| `--text-disabled` | `--surface` | 3.01 | 3.52 | exempt (WCAG 1.4.3 excludes disabled controls) |
| `--text-disabled` | `--surface-raised` | 3.09 | 3.17 | exempt |
| `--text-disabled` | `--hover` (disabled button) | 2.65 | 2.81 | exempt; legibility comes from the visible reason next to every disabled control (10.0) |
| `--on-accent` | `--accent` | 5.53 | 7.05 | 4.5 |
| `--on-accent` | `--accent-hover` | 7.03 | 8.34 | 4.5 |
| `--on-accent` | `--accent-pressed` | 8.87 | 9.86 | 4.5 |
| `--on-danger` | `--danger` | 6.23 | 6.88 | 4.5 |
| `--on-danger` | `--danger-hover` | 7.00 | 7.99 | 4.5 |
| `--on-danger` | `--danger-pressed` | 7.95 | 9.14 | 4.5 |
| `--accent` (nav icon) | `--selected` | 4.53 | 4.97 | 3 |
| `--accent` (primary fill) | `--bg` | 5.25 | 7.31 | 3 (plan Task 3 test) |
| `--accent` (primary fill) | `--surface` | 5.53 | 6.70 | 3 |
| `--accent` (primary fill) | `--surface-raised` | 5.68 | 6.03 | 3 |
| `--focus` | `--bg` | 5.25 | 8.65 | 3 |
| `--focus` | `--surface` | 5.53 | 7.93 | 3 |
| `--focus` | `--surface-raised` | 5.68 | 7.14 | 3 |
| `--focus` | `--hover` | 4.88 | 6.33 | 3 |
| `--focus` | `--selected` | 4.53 | 5.88 | 3 |
| `--border-control` | `--bg` | 3.08 | 4.01 | 3 |
| `--border-control` | `--surface` | 3.25 | 3.68 | 3 |
| `--border-control` | `--surface-raised` | 3.34 | 3.31 | 3 |
| `--danger` | `--bg` | 5.97 | 6.68 | 4.5 |
| `--danger` | `--surface` | 6.30 | 6.12 | 4.5 |
| `--danger` | `--surface-raised` | 6.47 | 5.51 | 4.5 |
| `--danger` | `--hover` (highlighted "Delete") | 5.55 | 4.89 | 4.5 |
| `--danger` | `--selected` | 5.15 | 4.54 | 4.5 |
| `--warning` | `--bg` | 5.39 | 8.71 | 4.5 |
| `--warning` | `--surface` | 5.68 | 7.98 | 4.5 |
| `--warning` | `--surface-raised` | 5.83 | 7.18 | 4.5 |
| `--warning` | `--hover` (badge in a highlighted menu item) | 5.01 | 6.37 | 4.5 |
| `--warning` | `--selected` | 4.65 | 5.91 | 4.5 |
| `--success` (icon, dot) | `--bg` | 4.76 | 7.44 | 3 |
| `--success` | `--surface` | 5.03 | 6.82 | 3 |
| `--success` | `--surface-raised` | 5.16 | 6.13 | 3 |
| `--bg` (checkbox check, switch thumb on) | `--text` | 15.37 | 15.17 | 3 |
| `--text-muted` (switch thumb off) | `--hover` | 5.29 | 5.28 | 3 |
| `--text` (progress fill) | `--border-strong` (progress track) | 9.06 | 7.14 | 3 |
| `--border-strong` (progress track) | `--surface` | 1.79 | 1.95 | none (decorative; the track shows the remaining length, the fill carries the value) |
| Tooltip text `--bg` | `--text` | 15.37 | 15.17 | 4.5 |
| Brand mark `#1D6F80` | `--bg` | 5.25 | 3.20 | 3 (graphic); the dark margin is small, the mark's own "H" carries it at 5.53 |

Log colors are aliases, so their contrast on `--surface` (the log background) is the `--danger`, `--warning`, `--text` and `--text-muted` rows (dark error 6.12). The focus outline sits 2 px outside the control and is measured against the surface around it, never against the control fill; inside menus it is drawn inset on the item and measured against `--hover` (row "`--focus` on `--hover`"). The checkbox box keeps its `--surface-raised` interior in every context, so its `--border-control` boundary is measured against that interior (3.34 / 3.31); against a `--hover` highlighted menu item outside it, the boundary is 2.87 / 2.93, which is acceptable because the interior side already meets 3:1.

### 2.4 Mapping onto shadcn/ui

shadcn/ui (https://ui.shadcn.com/docs/theming, read 21.11.2026) defines `--radius`, `--background`, `--foreground`, `--card`, `--card-foreground`, `--popover`, `--popover-foreground`, `--primary`, `--primary-foreground`, `--secondary`, `--secondary-foreground`, `--muted`, `--muted-foreground`, `--accent`, `--accent-foreground`, `--destructive`, `--border`, `--input`, `--ring`, `--chart-1` to `--chart-5` and `--sidebar-*` on `:root` and `.dark`, and exposes them to Tailwind with `@theme inline { --color-background: var(--background); ... }`. The docs assume Tailwind v4.

Generated components use both the utilities and, in places, the raw variables. So the shadcn names are declared on `:root` (they reference our tokens and need no `.dark` copy, because the tokens they point at change per theme), with one exception: `--accent` is our brand token, so shadcn's "accent" role (the hover fill of menu items and ghost buttons) exists only as the utility.

| shadcn variable on `:root` | Value | Utility in `@theme inline` |
|---|---|---|
| `--background` | `var(--bg)` | `--color-background: var(--bg)` |
| `--foreground` | `var(--text)` | `--color-foreground: var(--text)` |
| `--card` | `var(--surface)` | `--color-card: var(--surface)` |
| `--card-foreground` | `var(--text)` | `--color-card-foreground: var(--text)` |
| `--popover` | `var(--surface-raised)` | `--color-popover: var(--surface-raised)` |
| `--popover-foreground` | `var(--text)` | `--color-popover-foreground: var(--text)` |
| `--primary` | `var(--accent)` | `--color-primary: var(--accent)` |
| `--primary-foreground` | `var(--on-accent)` | `--color-primary-foreground: var(--on-accent)` |
| `--secondary` | `var(--surface-raised)` | `--color-secondary: var(--surface-raised)` |
| `--secondary-foreground` | `var(--text)` | `--color-secondary-foreground: var(--text)` |
| `--muted` | `var(--hover)` | `--color-muted: var(--hover)` |
| `--muted-foreground` | `var(--text-muted)` | `--color-muted-foreground: var(--text-muted)` |
| (not declared: name taken) | | `--color-accent: var(--hover)` |
| `--accent-foreground` | `var(--text)` | `--color-accent-foreground: var(--text)` |
| `--destructive` | `var(--danger)` | `--color-destructive: var(--danger)` |
| `--destructive-foreground` | `var(--on-danger)` | `--color-destructive-foreground: var(--on-danger)` |
| `--border` | (our token, same name and meaning) | `--color-border: var(--border)` |
| `--input` | `var(--border-control)` | `--color-input: var(--border-control)` |
| `--ring` | `var(--focus)` | `--color-ring: var(--focus)` |
| `--radius` | `var(--radius-panel)` | radius scale below |

Not declared: `--chart-*` (no charts in v0.1) and `--sidebar-*` (the sidebar is our own `Sidebar.tsx`). Our own tokens also get utilities in the same block (`--color-surface`, `--color-surface-raised`, `--color-border-strong`, `--color-border-control`, `--color-text`, `--color-text-muted`, `--color-text-disabled`, `--color-accent-hover`, `--color-accent-pressed`, `--color-on-accent`, `--color-focus`, `--color-danger`, `--color-danger-hover`, `--color-danger-pressed`, `--color-on-danger`, `--color-warning`, `--color-success`, `--color-hover`, `--color-selected`, `--color-selection`, `--color-highlight`, `--color-overlay`, `--color-log-error`, `--color-log-warn`, `--color-log-info`, `--color-log-debug`, `--color-bg`). In hand-written code the brand accent is `bg-primary`; the utility name `accent` belongs to shadcn and means the hover fill.

Radius, shadow and font scale in `@theme inline`:
- `--radius-sm: var(--radius-control)`, `--radius-md: var(--radius-control)`, `--radius-lg: var(--radius-panel)`, `--radius-xl: var(--radius-panel)`, `--radius-2xl: var(--radius-panel)`, `--radius-3xl: var(--radius-panel)`, `--radius-4xl: var(--radius-panel)`.
- `--shadow-xs: none`, `--shadow-sm: none`, `--shadow-md: var(--elevation-1)`, `--shadow-lg: var(--elevation-1)`, `--shadow-xl: var(--elevation-1)`.
- `--font-sans`, `--font-display`, `--font-mono` (section 3), `--ease-out` (section 8), the breakpoints and font sizes of 2.5.

Task 3 duties beyond declaring tokens (the plan lists these generated-component details as unverified, so they are checked when running `shadcn add`):
1. Patch generated components where their defaults contradict this document. Known candidates (verify when running `shadcn add`): `focus-visible:ring-[3px] ring-ring/50` and `focus-visible:border-ring` (replace with the outline in section 9), `outline-ring/50` in the base layer, `hover:bg-primary/90` (use `--accent-hover`, `--accent-pressed`), `hover:bg-destructive/90` and `dark:bg-destructive/60` (use `--danger-hover`, `--danger-pressed`), `dark:bg-input/30` (inputs use `--surface-raised` in both themes), `text-white` on the destructive button (use `text-destructive-foreground`), `rounded-md` on floating layers (popover, dropdown, select and command content use `rounded-lg`), `animate-pulse` on the skeleton (remove), `*-primary` utilities (`bg-primary`, `border-primary`, `text-primary`, `fill-primary`, `ring-primary`) in the checkbox, switch, progress, radio group, badge, tooltip and the button's `link` variant (replace with the neutral tokens of 10.8, 10.12, 10.14, 10.15 and 10.1: `--text` fills, `--bg` check and thumb, `--border-strong` track, badge colors, inverse tooltip colors, `--text` link), and the Sonner wrapper's style variables (`--normal-bg`, `--normal-text`, `--normal-border`, `--border-radius`; verify the names when running `shadcn add`, then point them at `--surface-raised`, `--text`, `--border` / `--border-strong` and `--radius-panel`).
2. A gate in `src/styles/tokens.test.ts` that runs over `src/components/ui/**`: every `var(--x)` resolves to a property declared in `index.css` (or in Base UI's documented runtime variables such as `--transform-origin`), and none of `ring-ring/`, `outline-ring/`, `dark:`, `shadow-xs`, `shadow-sm` or `rounded-2xl`, `rounded-3xl`, `rounded-4xl` remains, and no raw `var(--accent)` appears (shadcn code that meant its hover role must use the utility). `primary` utility classes (`*-primary`, `*-primary-foreground`) are allowed only in the `primary` variant of `button.tsx`; anywhere else in `src/components/ui/**` the gate fails. The active navigation icon is hand-written code (`Sidebar.tsx`) and uses `text-primary` there. The exact class strings the generated files contain are verified when running `shadcn add`.
3. A lint rule (ESLint `no-restricted-syntax` on class strings, or a test) that bans `bg-accent`, `text-accent` and `border-accent` outside `src/components/ui/`, so hand-written code cannot use shadcn's hover role by mistake when it means the brand accent.

Dark mode is class-based: `.dark` on `<html>`, Tailwind variant per https://tailwindcss.com/docs/dark-mode. The theme comes from the backend settings; no next-themes. Theme variables follow Tailwind v4's namespaces (https://tailwindcss.com/docs/theme).

### 2.5 Token file (normative for Task 3)

Where each token is declared in `apps/desktop/src/styles/index.css`:

| Group | Declared on | Notes |
|---|---|---|
| Colors of 2.1 (including `--overlay`, `--elevation-1`, the log colors) | `:root` (light) and `.dark` (dark) | The only tokens with two values |
| shadcn names of 2.4 | `:root` only | They reference color tokens, so they follow the theme without a `.dark` copy |
| `--space-1` to `--space-7` (section 4), `--radius-control`, `--radius-panel`, `--radius-pill` (section 5), `--dur-press`, `--dur-popover-in`, `--dur-popover-out`, `--dur-dialog-in`, `--dur-dialog-out`, `--dur-progress`, `--ease-out` (section 8) | `:root` only | Theme-independent |
| Tailwind theme variables (`--color-*`, `--radius-*`, `--shadow-*`, `--font-*`, `--text-*`, `--breakpoint-*`, `--ease-out`) | `@theme inline` | Point at the tokens above; no hex values in this block |

Breakpoints (inner window width; Tailwind's default `sm` to `2xl` are removed with `--breakpoint-*: initial` so nobody builds against them):

| Variable | Value | Meaning |
|---|---|---|
| `--breakpoint-medium` | `880px` | Below it (`max-medium:`): narrow layout (7.3) |
| `--breakpoint-wide` | `1200px` | From it (`wide:`): 216 px sidebar, 32 px content padding |
| `--breakpoint-full` | `1280px` | From it (`full:`): one-row log toolbar (10.13) |

Font-size utilities (Tailwind v4 `--text-*` namespace with `--line-height` and `--font-weight` companions). The names avoid every color name, so a class such as `text-body` is never ambiguous with a color utility. The color tokens `--text`, `--text-muted` and `--text-disabled` live on `:root` and never inside `@theme`, where Tailwind would read them as font sizes:

| Role (3.2) | Variable | Size / line height / weight |
|---|---|---|
| Start title | `--text-display-lg` | 2rem / 2.5rem / 600 (with `font-display`) |
| Page title (h1) | `--text-display` | 1.5rem / 2rem / 600 (with `font-display`) |
| Section and dialog title (h2) | `--text-heading` | 1.125rem / 1.625rem / 600 |
| Body | `--text-body` | 1rem / 1.5rem / 400 |
| Label, button text | `--text-label` | 0.875rem / 1.25rem / 600 |
| Secondary | `--text-small` | 0.875rem / 1.25rem / 400 |
| Caption | `--text-caption` | 0.8125rem / 1.125rem / 400 |
| Badge | `--text-badge` | 0.8125rem / 1.125rem / 600 |
| Play button | `--text-play` | 1rem / 1.5rem / 600 |
| Log, paths, error details | `--text-mono` | 0.8125rem / 1.25rem / 400 (with `font-mono`) |
| Device code | `--text-code` | 1.75rem / 2.25rem / 600 (with `font-mono`, `tracking-[0.04em]`) |

Button variants: `button.tsx` keeps exactly the design's five variants and three sizes (10.1); shadcn's names map as follows, and the unused ones are deleted:

| shadcn variant | Design variant | Change |
|---|---|---|
| `default` | `primary` | Renamed; colors per 10.1 |
| `outline` | `secondary` | Renamed; `--surface-raised` fill, 1 px `--border-control` border (not `--border`) |
| `secondary` | deleted | shadcn's filled grey button has no place in this design |
| `ghost` | `ghost` | Hover `--hover`, pressed `--selected` |
| `destructive` | `destructive` | Danger tokens of 2.1 |
| `link` | `link` | `--text`, underline per 10.1 |
| sizes `default`, `icon`, `lg` | `default` 40, `icon` 40 × 40, `lg` 56 | Heights changed; `sm`, `xs`, `icon-sm`, `icon-lg` deleted |

The default variant of `button.tsx` is `secondary`, so a button without an explicit variant can never become a second accent button.

## 3. Typography

### 3.1 Typefaces

System fonts only. The app runs on Windows only and should feel like a Windows tool; no font file ships in the bundle and nothing is fetched at runtime (the CSP allows no font hosts).

| Role | Stack | Tailwind variable |
|---|---|---|
| UI text | `"Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif` | `--font-sans` |
| Headings 24 px and up | `"Segoe UI Variable Display", "Segoe UI", system-ui, sans-serif` | `--font-display` |
| Log, device code, paths, error details | `"Cascadia Mono", Consolas, monospace` | `--font-mono` |

License: Segoe UI Variable, Segoe UI, Cascadia Mono and Consolas are part of Windows (Cascadia Mono ships with Windows 11 and with Windows Terminal). They are used from the system, not redistributed, so no license file is needed. Windows 10 has no Segoe UI Variable and may lack Cascadia Mono; the stacks fall back to Segoe UI and Consolas. All sizes below work with both. The log grid uses `ch` columns, which are about 5 % narrower in Consolas than in Cascadia Mono; columns are sized in `ch` so they stay aligned in either font.

Weights: 400 and 600 only. Hierarchy comes from size and color. Letter spacing 0 everywhere, except the device code (`0.04em`).

### 3.2 Scale

Sizes are set in `rem` (root 16 px) so WebView zoom scales them; the px values are at 100 %.

| Role | Size / line height | Weight | Font | Color | Where |
|---|---|---|---|---|---|
| Start title | 32/40 (2rem/2.5rem) | 600 | display | `--text` | Instance name on the start page (4b); at most two lines, then ellipsis; long unbroken names break anywhere |
| Page title (h1) | 24/32 (1.5rem/2rem) | 600 | display | `--text` | Header of every view |
| Section title (h2), dialog title | 18/26 (1.125rem/1.625rem) | 600 | sans | `--text` | Settings sections, dialog titles, empty-state titles |
| Body | 16/24 (1rem/1.5rem) | 400 | sans | `--text` | Paragraphs, list row name, input text (16 px in fields, FO15), dialog text |
| Label | 14/20 (0.875rem/1.25rem) | 600 | sans | `--text` | Field labels, button text (default size), ErrorPanel title is 16/24 600 |
| Secondary | 14/20 | 400 | sans | `--text-muted` | Row meta, help lines, inline errors (in `--danger`), nav labels, menu items (in `--text`), segment labels (in `--text`) |
| Caption | 13/18 (0.8125rem/1.125rem) | 400 | sans | `--text-muted` | Column labels, timestamps, tooltips, toast description |
| Badge | 13/18 | 600 | sans | per badge | Badges |
| Play button | 16/24 | 600 | sans | `--on-accent` | Play button only |
| Log, paths, error details | 13/20 | 400 | mono | `--log-*` or `--text-muted` | Log lines, data folder and Java paths, ErrorPanel details |
| Device code | 28/36 (1.75rem/2.25rem) | 600 | mono | `--text` | Sign-in code view |

The window title is the native Windows title bar ("Hengst Client", plus " (Development build)" in dev builds); the app does not draw its own.

Paragraph text is capped at 72 characters (`max-width: 72ch`). Headings use `text-wrap: balance`, paragraphs `text-wrap: pretty`. `lang` on `<html>` follows the UI language; `hyphens: auto` is set on headings and narrow cells, but the layout must hold without hyphenation, because WebView2 may lack the dictionaries: long compounds and unbroken names are covered by `overflow-wrap: anywhere` wherever text can be longer than its box.

### 3.3 Numbers, dates, units

- `font-variant-numeric: tabular-nums` on every number that changes or lines up: counts, file sizes, progress, memory, remaining time, version columns, timestamps (see Appendix A for the font check).
- Numbers through `Intl.NumberFormat` in the UI language: "3.402 Dateien" / "3,402 files".
- Units with a non-breaking space: "2048 MB", "1,5 GB" / "1.5 GB". Memory is entered and stored in MB; the hint shows GB with one decimal.
- Plurals through i18next plural keys ("1 Datei" / "1 file").
- Relative time through `Intl.RelativeTimeFormat` up to 6 days ("vor 2 Tagen" / "2 days ago"); from 7 days the date via `Intl.DateTimeFormat` with `dateStyle: "medium"`. Never-played instances say "Noch nie gespielt" / "Never played".
- Clock times (game start) via `Intl.DateTimeFormat` with `timeStyle: "short"`: "Läuft seit 14:05" / "Running since 2:05 PM".
- Remaining durations as minutes and seconds with a unit, never as a bare `mm:ss` that reads like a clock: "Noch 14:32 Min. gültig" / "14:32 left".
- Version ids and loader versions exactly as the source gives them ("1.14 Pre-Release 1", "47.4.28").

## 4. Spacing

4 px grid. Tokens and the Tailwind steps they equal (Tailwind's `--spacing` stays at `0.25rem`):

| Token | Value | Tailwind step | Use |
|---|---|---|---|
| `--space-1` | 4 px | `1` | Icon to badge, menu padding |
| `--space-2` | 8 px | `2` | Label to field, icon to label in buttons, gap between buttons, gap between nav items, log column gap |
| `--space-3` | 12 px | `3` | Row and nav item horizontal padding, icon to label in nav |
| `--space-4` | 16 px | `4` | Button horizontal padding, field to field, gap between header items, list column gap |
| `--space-5` | 24 px | `6` | Dialog padding, content padding below 1200 px |
| `--space-6` | 32 px | `8` | Content padding from 1200 px, between groups on the start page |
| `--space-7` | 48 px | `12` | Between settings sections, top padding of the start page block |

Only these steps are used for layout. Half steps (`0.5`, `1.5`) only inside small controls (badge padding). Inside a group the gap is smaller than between groups.

## 5. Shape, borders, elevation

| Token | Value | Applies to |
|---|---|---|
| `--radius-control` | 6 px | Buttons, inputs, select triggers, segments, list rows, nav items, menu items, tooltips, the device-code box, skeleton blocks |
| `--radius-panel` | 10 px | Dialogs, popovers, menus, select and combobox content, toasts, error panels, the log area |
| `--radius-pill` | 999 px | Account chip, badges, progress bar and track, switch, the initial circle |

One exception: the checkbox box uses 4 px (6 px on a 20 px box reads as a circle). Nesting: a panel (10) with 4 px padding contains items with 6; a dialog (10) with 24 px padding contains controls with 6.

Borders are 1 px. The focus outline is 2 px. The selected segment carries a 1 px inset outline in `--text`.

Elevation has two levels:

| Level | What | Light | Dark |
|---|---|---|---|
| 0 | Sidebar, header, views, lists, the log area, inline panels | No shadow. Separation by `--bg` / `--surface` and 1 px `--border` | Same |
| 1 | Popover, dropdown menu, select content, combobox, dialog, alert dialog, toast | 1 px `--border` plus `--elevation-1` | 1 px `--border-strong` plus `--elevation-1` |
| 1, tooltip | Tooltip | No border, `--elevation-1` | No border, `--elevation-1` |

Dialogs sit on `--overlay`. Tooltips use inverse colors (background `--text`, text `--bg`).

## 6. Icons

- Set: lucide (`lucide-react`, ISC license), the set shadcn uses. No second icon library, no hand-drawn icon paths (the brand mark is the only custom SVG).
- Stroke width 1.5.
- Sizes: 18 px for nav items, icon buttons and the Play button; 16 px everywhere else (inside text buttons, menu items, badges, inline messages, toasts, the checkbox check). No other sizes.
- Color follows the text it sits next to (`currentColor`). Exceptions: the active nav icon is `--accent`; error icons `--danger`; warning icons `--warning`; the success icon `--success`.
- Decorative icons next to a text label get `aria-hidden="true"`.
- Icon names used: `house` (start page), `boxes` (instances), `scroll-text` (log), `settings`, `play`, `square` (stop), `x` (cancel, close), `plus` (create), `ellipsis` (row actions), `folder-open`, `trash-2`, `check`, `chevron-down`, `chevron-up`, `chevron-right` (submenu), `log-in`, `log-out`, `user-plus`, `copy`, `external-link`, `circle-alert` (error), `triangle-alert` (warning), `info`, `search`, `arrow-down-to-line` (jump to end), `loader-circle` (spinner), `refresh-cw` (retry), `wifi-off` (offline caption).

An icon needs a text label always, except in these places, where the icon-only button has an `aria-label` and a tooltip with the same text:
1. Nav items in the collapsed 64 px sidebar.
2. The row action button ("Aktionen für {{name}}" / "Actions for {{name}}").
3. The close button in dialog headers ("Schließen" / "Close").
4. The search hit navigation in the log ("Vorheriger Treffer" / "Previous match", "Nächster Treffer" / "Next match").
5. The header primary action in the narrow layout (section 7.3).

## 7. Window and layout

### 7.1 Window

- Native Windows title bar, one window, label `main`. The title bar follows the resolved theme at creation and whenever the theme setting or the system theme changes at runtime (Tauri's window theme setter; check the exact API name in Tauri 2.12).
- Sizes are the inner (client) size in logical pixels.
- Minimum inner size: `min(960 × 600, work area minus window frame)`. Default inner size: the component-wise maximum of the minimum and `min(1120 × 720, 90 % of the work area minus window frame)`, so the default is never smaller than the minimum. Where the minimum does not bind, the outer window including frame and title bar takes at most 90 % of the work area in each dimension. The saved size (window state) is clamped the same way on restore. So the app fits a 1366 × 768 screen at 125 % scaling (work area about 1093 × 580 logical, title bar 31): the minimum becomes 960 × 549, the 90 % formula gives 983 × 494, and the default is 983 × 549 (the height is raised to the minimum); the content column scrolls where needed. The debug-only `--window-size` argument is clamped to the minimum and to the work area.
- The window is created with the background color `--bg` of the resolved theme, so no white flash.
- Windows display scaling (125 %, 150 %) scales the whole WebView: CSS pixels equal logical pixels, so the layout at a given logical width is identical at every scaling.
- The regular layouts hold from 880 CSS px wide; below that the narrow layout applies (section 7.3). Widths below 880 occur through WebView zoom (Ctrl+Plus; 960 at 125 % zoom is 768 CSS px, at 150 % 640) or a work area narrower than 880; zoom up to 150 % is guaranteed to reflow without clipping or horizontal scrolling at the minimum window size.

### 7.2 Frame

```
+---------------------------------------------------------------------------------------+
| Hengst Client                                                           _  []  X      |  native title bar
+------------------+--------------------------------------------------------------------+
| [H] Hengst Client|  Instanzen                         [+ Instanz anlegen] (M Mira_K v) |  header 56
|                  |                                                                    |
|  (house) Spielen |  Name                       Version     Loader         Zuletzt gespielt |
|  (boxes) Instanz.|  [v] Überleben 1.21         1.21.11     Fabric 0.17.3  vor 2 Tagen   (...) |
|  (scroll) Log    |      1.20.1 Forge           1.20.1      Forge 47.4.28  Noch nie gespielt (...) |
|                  |                                                                    |
|                  |                                                                    |
| (gear) Einstell. |                                                                    |
+------------------+--------------------------------------------------------------------+
  sidebar 216, --bg   content column, --surface, padding 32, content max 1120, left-aligned
```

At 1200 px and wider (above). At 960 × 600 (compact):

```
+----+----------------------------------------------------------------------------+
| [H]|  Instanzen                                [+ Instanz anlegen] (M Mira_K v)  |  header 56
|    |                                                                            |
| (h)|  Name                         Version   Loader          Zuletzt gespielt    |
| (b)|  [v] Überleben 1.21           1.21.11   Fabric 0.17.3   vor 2 Tagen    (...) |
| (s)|      1.20.1 Forge             1.20.1    Forge 47.4.28   Noch nie gespielt (...) |
|    |                                                                            |
| (g)|                                                                            |
+----+----------------------------------------------------------------------------+
 64    content padding 24
```

Sidebar:
- Background `--bg`, 1 px `--border` on its right edge, full window height, does not scroll.
- Brand row on top, 56 px (aligned with the header): mark 24 px, 8 px gap, wordmark "Hengst Client" 16/24 weight 600 `--text`, 12 px from the left. Collapsed: mark only, centered. The brand row is not a button.
- `<nav aria-label="Hauptnavigation" / "Main navigation">` below, 8 px padding: nav items are buttons with `aria-current="page"` on the active one; 40 px high, full width minus 16 px, radius 6, padding 0 12, icon 18, 12 px gap, label 14/20. 8 px gap between items (T2).
- Order: in 4a "Instanzen" / "Instances", then, pinned to the bottom 8 px from the edge, "Einstellungen" / "Settings". From 4b: "Spielen" / "Play" (start page, first, the start view), "Instanzen", "Log", bottom "Einstellungen".
- States: default label and icon `--text-muted`; hover `--hover` fill, label `--text`; active `--selected` fill, label `--text` weight 600, icon `--accent` (fill, weight and icon color: not color alone); focus outline; pressed `--selected`.
- Collapsed: items 40 × 40 centered, icon only, `aria-label` and a tooltip to the right with the name.
- Dev builds: a badge "Entwicklungs-Build" / "Development build" above the settings item (wide only; collapsed, the window title says it).

Header (top of the content column):
- 56 px, background `--surface`, horizontal padding equal to the content padding. h1 on the left, vertically centered, one line with ellipsis (tooltip with the full title when truncated).
- On the right: the view's primary action (if any), 16 px gap, the account chip. The h1 always keeps at least 160 px; the right group gives way first (section 7.3).
- The header is sticky; when the content below is scrolled, a 1 px `--border` line appears at its bottom (no shadow, no animation).

Content:
- Background `--surface`, padding 32 px (24 below 1200) on left, right and bottom, 8 px from the header to the first content. Content width at most 1120 px, left-aligned (not centered): wide windows get empty space on the right instead of rows stretched to unreadable widths.
- The content column scrolls vertically; sidebar and header do not. The log view is the exception: the log area itself scrolls and the page does not.
- Density: comfortable desktop density, mouse first. Buttons 40, rows 48, nav items 40, Play 56.

### 7.3 Breakpoints

Thresholds on the inner window width in CSS px (token names in 2.5):

| Width | Layout |
|---|---|
| 1280 and wider (`full:`) | As wide, plus the one-row log toolbar (10.13) |
| 1200 and wider (`wide:`) | Sidebar 216 px, content padding 32 |
| 880 to 1199 | Sidebar 64 px (icon rail), content padding 24 |
| below 880 (`max-medium:`; zoom or a narrow work area) | Narrow: as above, plus the instance list hides "Zuletzt gespielt" and shows version and loader as two separate spans under the name (16 px gap, no separator; the row grows to 64 px: 8 padding, name 24, 4 gap, spans 20, 8 padding); the header primary action becomes icon-only (name in `aria-label` and tooltip); the account chip shows only the initial circle and chevron |

Arithmetic (content width = window minus sidebar minus padding). The regular row grid needs 600 px fixed (12 + 12 padding, slot 24, version 128, loader 168, last played 136, action 40, five 16 px gaps); the narrow grid needs 120 px fixed (12 + 12 padding, slot 24, action 40, two 16 px gaps).

| Window | Sidebar + padding | Content | Grid | Name column |
|---|---|---|---|---|
| 640 (960 at 150 % zoom) | 64 + 48 | 528 | narrow | 408 |
| 768 (960 at 125 % zoom) | 64 + 48 | 656 | narrow | 536 |
| 880 | 64 + 48 | 768 | regular | 168 |
| 960 | 64 + 48 | 848 | regular | 248 |
| 1199 | 64 + 48 | 1087 | regular | 487 |
| 1200 | 216 + 64 | 920 | regular | 320 |
| 1280 | 216 + 64 | 1000 | regular | 400 |

The drop at 1200 (1087 to 920) leaves the name column at 320 px, more than at 960. In the narrow layout the header keeps at least 160 px for the h1: at 640 the right group is an icon button (40) plus 16 gap plus the reduced chip (about 68).

## 8. Motion

Product UI after `emil-design-eng` and produkt-regelwerk M1 to M8. One curve for everything that eases, few durations, motion only where it shows what changed.

| Token | Value | For |
|---|---|---|
| `--ease-out` | `cubic-bezier(0.23, 1, 0.32, 1)` | Everything that animates, except the three linear cases below |
| `--dur-press` | 120 ms | Color change on hover and release, Play button scale, switch thumb |
| `--dur-popover-in` | 160 ms | Popover, dropdown menu, select, combobox, tooltip enter |
| `--dur-popover-out` | 120 ms | The same, exit (25 % shorter) |
| `--dur-dialog-in` | 220 ms | Dialog and alert dialog enter, overlay fade in |
| `--dur-dialog-out` | 160 ms | The same, exit (27 % shorter) |
| `--dur-progress` | 200 ms, linear | Progress fill |

Exceptions to the one curve: the progress fill is linear (constant motion), the button spinner rotates linearly (800 ms per turn), and Sonner toasts keep Sonner's own easing and timing (occasional, its swipe needs its physics).

The curve differs from the plan's draft `cubic-bezier(0.2, 0, 0, 1)`: that curve starts with a flat tangent (its first control point is (0.2, 0), so dy/dx = 0 at t = 0) and delays the first visible movement, which M3 and `emil-design-eng` rule out for UI. `cubic-bezier(0.23, 1, 0.32, 1)` is the strong ease-out produkt-regelwerk M3 names.

What animates:
- Pressed state: the fill switches to the pressed color immediately on pointer-down (0 ms, inside the 100 ms of Z1); the return to rest or hover transitions `background-color, color, border-color` over `--dur-press`. Hover color changes use the same transition. Never `transition: all`.
- Play button only: `scale(0.98)` while pressed, `--dur-press`.
- Popover, menu, select, combobox, tooltip opened by pointer: opacity 0 to 1 and scale 0.97 to 1 from the trigger, `--dur-popover-in`; exit reverses over `--dur-popover-out`. Tooltips open after 500 ms of hover; once one has been shown, the next opens without delay and without animation for 300 ms.
- Dialog opened by pointer: overlay opacity; panel opacity 0 to 1 and scale 0.97 to 1 from the center, `--dur-dialog-in`; exit over `--dur-dialog-out`.
- Progress fill: `scaleX(fraction)` from the left edge over `--dur-progress`.
- Switch thumb: `translateX` over `--dur-press` (pointer only).
- Button spinner: rotates only while a request is actually running.

What never animates:
- View switches, the header border, list loads, list updates, sorting, row selection, the active nav item, theme and language switches, settings changes, dialog height changes between sign-in phases.
- Anything opened or toggled by keyboard: menus, popovers, selects, comboboxes, tooltips and dialogs opened with Enter, Space, arrow keys or the Delete key appear and disappear without transition; the switch thumb jumps.
- Skeletons: static blocks in `--hover`, no pulse, no shimmer, shown only if loading takes longer than 200 ms (L2).
- No loops other than the spinner, no bounce, no spring overshoot, no entrance choreography, no stagger.

`prefers-reduced-motion: reduce`: all transforms are dropped (no press scale, no scale on popovers and dialogs, no thumb travel); popovers, menus, dialogs and the overlay only fade, 80 ms; the progress fill jumps; the button spinner is hidden (the button label already says what is happening); Sonner toasts only fade, 80 ms.

Every new animation is checked with `~/.claude/skills/review-animations/SKILL.md` in the task that adds it.

## 9. Focus

- `:focus-visible` shows a 2 px solid `--focus` outline at 2 px offset. Never removed, never replaced by a color change alone. Pointer clicks do not show it.
- The outline sits outside the control, so it is visible on accent and danger fills (measured against the surrounding surface, table 2.3).
- Items inside menus, selects, comboboxes and command lists: the pointer highlight is a `--hover` fill. When the item is highlighted by keyboard, it additionally shows the 2 px `--focus` outline inset (offset -2 px), because the fill alone is 1.16:1 (light) and 1.13:1 (dark) against `--surface-raised` and fails 3:1.
- List rows show the outline on the whole row.
- A focused element is never hidden under the sticky header (scroll padding equal to the header height plus 8 px).

## 10. Components

### 10.0 Common rules

- Disabled controls use `--text-disabled` for text and icons and keep their layout. Every disabled control has a visible reason next to it (Z7), referenced with `aria-describedby`.
- Hover styles apply only to devices with hover and a fine pointer.
- Click guard: an action that starts a request blocks a second activation until it resolves (Z2). Where a button changes into an opposite action (Play into Cancel, Starting into Cancel), the new action ignores activations for 600 ms after the change, so the second click of a double click never triggers it.

### 10.1 Button (shadcn `button`)

Height is fixed per size; width follows the text (`min-width`, never a fixed `width`). Text 14/20 weight 600, icon 16 with 8 px gap, padding 0 16. Loading: the label changes to the verb with ellipsis ("Wird angelegt…"), a spinner (10.16) replaces the leading icon, the width does not shrink (min-width from the idle label).

| Variant | Default | Hover | Pressed | Disabled |
|---|---|---|---|---|
| `primary` | `--accent` fill, `--on-accent` text | `--accent-hover` | `--accent-pressed` | `--hover` fill, 1 px `--border`, `--text-disabled` text |
| `secondary` | `--surface-raised` fill, 1 px `--border-control`, `--text` | `--hover` fill | `--selected` fill | `--surface` fill, 1 px `--border`, `--text-disabled` |
| `ghost` | transparent, `--text` | `--hover` fill | `--selected` fill | `--text-disabled` |
| `destructive` | `--danger` fill, `--on-danger` text | `--danger-hover` | `--danger-pressed` | as primary disabled |
| `link` | `--text`, underline 1 px at 3 px offset | underline 2 px | `--text-muted` | `--text-disabled`, no underline |

| Size | Height | Use |
|---|---|---|
| `default` | 40 | All buttons (the `link` variant inside running text: section 10.15) |
| `icon` | 40 × 40 | Icon-only buttons (section 6 list), icon 18 |
| `lg` | 56 | Play button only |

All variants show the focus outline. At most one `primary` button per screen or dialog. Button rows: secondary actions left of the primary, primary last (right), 8 px gap; a row that does not fit wraps to two lines rather than truncating a label (WD29).

### 10.2 Play button (custom on shadcn `button`, size `lg`)

56 px high, min-width 240 px, radius 6, text 16/24 weight 600, icon 18. One button whose label and variant follow the game state; it never moves or changes height.

State precedence, first match wins:

| # | State | Label (de / en) | Variant, icon | On click |
|---|---|---|---|---|
| 1 | Stopping | "Wird beendet…" / "Stopping…" | secondary, spinner, inert | nothing; after 10 s a second button "Sofort beenden" / "Force stop" (`destructive`, default size) appears to the right |
| 2 | Running | "Beenden" / "Stop" | secondary, `square` | stop the game gracefully; a status line above the button: 8 px `--success` dot plus "Läuft seit {{time}}" / "Running since {{time}}" |
| 3 | Starting (click until the first progress event) | "Wird gestartet…" / "Starting…" | primary, spinner, inert | nothing |
| 3 | Preparing | "Abbrechen" / "Cancel" | secondary, `x`; 600 ms click guard after the change | cancel the launch; the progress panel shows below (10.12) |
| 4 | No readable instance (none at all, or only unreadable or newer-format ones) | "Instanz anlegen" / "Create instance" | primary, `plus` | open the create dialog; if unreadable instances exist, the caption above the button says "Keine Instanz lässt sich starten." / "No instance can be started." |
| 5 | Selected instance unreadable or newer format, and at least one readable instance exists | "Instanz wählen" / "Choose instance" | primary, `boxes` | open the instance switcher (it is shown, because there are at least two instances); above the button the caption "{{name}} lässt sich nicht starten." / "{{name}} cannot be started." |
| 6 | No account | "Anmelden" / "Sign in" | primary, `log-in` | open the sign-in dialog; if this build has no client id (`can_sign_in` false), the dialog is not opened and the no-client-id block of 11.6 is shown instead |
| 7 | Account needs sign-in again | "Erneut anmelden" / "Sign in again" | primary, `log-in` | start sign-in for that account |
| 8 | Ready | "Spielen" / "Play" | primary, `play` | start the selected instance |

Selection fallback: when no instance is selected (first start, or the selected one was deleted), the most recently played readable instance is selected, else the first readable one in list order.

After the game exits: exit code 0 returns to Ready silently. An exit that follows a stop the user requested in the app (the Stop action on the Play button, the force path of the close-while-running dialog in 11.11, the tray's stop entry) is never reported as unexpected, whatever the exit code: the app returns silently to Ready. Only an exit the user did not request, with a non-zero code, shows an ErrorPanel under the button: title "Das Spiel wurde unerwartet beendet" / "The game closed unexpectedly", body "Im Log steht, was passiert ist." / "The log shows what happened.", action "Log anzeigen" / "Show log". It stays until the next launch or until dismissed with "Schließen" / "Dismiss" (ghost).

Pressed: fill change plus scale 0.98. Focus: outline. The Play button is never `disabled`: every state either acts or explains itself in its label.

### 10.3 Account chip and account menu (custom chip on shadcn `dropdown-menu`)

Chip: a button, 40 px high, radius pill, `--surface-raised` fill, 1 px `--border-control`, padding 4 12 4 4: initial circle (32 px, `--hover` fill, first letter of the player name, 14/20 weight 600 `--text`), 8 px gap, the player name 14/20 (`max-width: 208px`, which fits 16 wide characters; ellipsis plus tooltip beyond), 4 px gap, `chevron-down` 16. If the selected account needs sign-in again, a warning badge "Erneut anmelden" / "Sign in again" follows the name. Accessible name "Konto: {{name}}" / "Account: {{name}}". States: hover `--hover`, pressed and open `--selected`, focus outline.

No account: the chip is a `secondary` button "Anmelden" / "Sign in" (`log-in`) that opens the sign-in dialog. If the build has no client id (`can_sign_in` false), it shows the no-client-id block of 11.6 instead and the sign-in dialog is not opened.

Menu (DropdownMenu, aligned to the chip's right edge, `width: max-content`, `min-width: 280px`, `max-width: 360px`, radius 10, padding 4, items 40 px, radius 6, padding 0 12):
1. Radio group of accounts: a 16 px check slot (check `--text` on the selected one), 8 px gap, name, warning badge "Erneut anmelden" where needed. The name is the only part that shrinks: at the 360 px maximum a wide 16-character name with the badge (about 393 px) truncates with ellipsis and shows the full name in a tooltip; the badge never shrinks or wraps. Choosing an account selects it; choosing one that needs sign-in starts sign-in for it. The dev account (dev builds only) carries the badge "Entwicklungs-Build" / "Development build".
2. Separator.
3. "Konto hinzufügen" / "Add account" (`user-plus`). With `can_sign_in` false it stays enabled and shows the no-client-id block of 11.6 instead of opening the sign-in dialog.
4. Submenu "Abmelden" / "Sign out" (`log-out`, `chevron-right`) with one item per account; choosing one opens the remove dialog. Hidden when there are no accounts.

Menu items are adjacent full-width targets without gaps; T2's 8 px spacing applies between separate controls, not between items of one list (menus, list rows).

### 10.4 Instance list row (custom `<li>` with a button)

48 px high, radius 6, padding 0 12, full content width. The row is a button that selects the instance and carries `aria-current="true"` when selected; the action button is a sibling, not nested.

Columns (CSS grid, 16 px gap): check slot 24 px (`check` 18 in `--text` when selected) | name `minmax(0, 1fr)`, body 16/24 `--text`, one line, ellipsis, `overflow-wrap: anywhere`, the full name in a tooltip when truncated (keyboard focus shows it too) | version 128 px, secondary, tabular | loader 168 px, secondary ("Fabric 0.17.3", "Kein Loader" / "No loader", "Forge 47.4.28") | last played 136 px, caption, right-aligned | action button 40 × 40 (`ghost`, `ellipsis`, "Aktionen für {{name}}" / "Actions for {{name}}"). Narrow layout: section 7.3.

Above the list a column header row (32 px, caption, same grid): "Name", "Version", "Loader", "Zuletzt gespielt" / "Last played", 1 px `--border` below. Not sortable in v0.1.

States: default; hover `--hover`; selected `--selected` plus check (not color alone); focus outline on the row button; pressed `--selected`. Rows for `tooNew` and `broken` instances: name in `--text-muted`, the version and loader cells merged and holding a warning badge "Neuere Version" / "Newer format" or "Nicht lesbar" / "Unreadable", the row is not selectable (`aria-disabled`; the badge is part of the row's name, not also its description), and its menu offers "Ordner öffnen" plus "Löschen": for a `tooNew` row "Löschen" is a disabled item whose second line gives the reason ("Mit einer neueren Version von Hengst Client gespeichert. Löschen kannst du sie nur im Explorer." / "Saved by a newer version of Hengst Client. You can delete it only in Explorer."), referenced as its description; a `broken` row may be moved to the Recycle Bin.

Row action menu (DropdownMenu, aligned right): "Ordner öffnen" / "Open folder" (`folder-open`), separator, "Löschen" / "Delete" (`trash-2`, `--danger`). While this instance is preparing or running, "Löschen" is disabled and a caption under it says "Läuft gerade. Beende zuerst das Spiel." / "Currently running. Stop the game first."

No instance cards in v0.1: the list is the only presentation.

### 10.5 Version picker (custom combobox on shadcn `popover` + `command`)

Label above: "Spielversion" / "Game version". Trigger like an input (40 px, `--surface-raised`, 1 px `--border-control`, radius 6, padding 0 12): selected version id (16/24) and `chevron-down` 16 at the right; combobox role with expanded state.

Popover content: width of the trigger, radius 10, padding 4: search input (40 px, `search` icon, placeholder "Suchen" / "Search", visible label is the field's own "Spielversion" via `aria-labelledby`), then the list (max-height 320 px, scrolls), items 40 px: version id left, a caption at the right for non-release entries ("Snapshot", "Beta", "Alpha"), check slot 16 px with the check on the selected item. Empty search: "Keine Version gefunden für „{{query}}“" / "No version matches “{{query}}”", plus ghost "Filter zurücksetzen" / "Reset filters" when filters hide results.

Filters: four checkboxes in one row below the trigger (outside the popover, so the active filter stays visible): "Releases", "Snapshots", "Beta", "Alpha"; default Releases only.

States:
- Default, hover (border `--text-muted`), focus, open (border `--text-muted`, `chevron-up`).
- Loading: trigger disabled with "Versionen werden geladen…" / "Loading versions…".
- Offline or service unreachable with a cached list: the cached list is used; caption under the field with `wifi-off` 16: "Offline: gespeicherte Liste" / "Offline: saved list". Cached list but the refresh failed for another reason: "Gespeicherte Liste, nicht aktuell" / "Saved list, not current".
- Offline without a cached list: inline error "Keine Verbindung. Die Versionsliste braucht beim ersten Mal Internet." / "No connection. The version list needs the internet the first time." with link button "Erneut versuchen" / "Try again"; the create button is disabled with this text as its reason.
- Other load error: inline error with "Erneut versuchen".

### 10.6 Loader picker (custom segments on shadcn `radio-group`) and loader version (shadcn `select`)

Loader: a radio group shown as separate segments that wrap: "Kein Loader" / "No loader", "Fabric", "Quilt", "Legacy Fabric", "Forge", "NeoForge" (proper names are not translated). Each segment 40 px high, padding 0 16 0 12, radius 6, 8 px gap, label 14/20 weight 400 `--text` in both states, preceded by a 16 px check slot plus 8 px gap that is always reserved (empty when unselected), so a segment never changes width. Unselected: `--surface-raised`, 1 px `--border-control`. Selected: `--selected` fill, 1 px inset `--text` outline, check `--text` in the slot. Hover `--hover`. Arrow keys move the selection; one tab stop for the group; focus outline on the focused segment.

The same segment component serves Language and Theme in Settings.

Loader version: Select with label "Loader-Version" / "Loader version", trigger as the version picker. Items 40 px: check slot, version, then a badge "Empfohlen" / "Recommended" or "Instabil" / "Unstable". Preselection: the recommended version. Hidden when "Kein Loader" is selected.
- Loading: trigger disabled with "Wird geladen…" / "Loading…".
- No builds for this game version: the select is replaced by the hint from `errors.noLoaderVersions` with `triangle-alert` 16 in `--warning`; the create button is disabled and described by that text.
- Offline: "Keine Verbindung. Loader-Versionen können gerade nicht geladen werden." / "No connection. Loader versions cannot be loaded right now." with "Erneut versuchen".
- Other error: inline error with "Erneut versuchen".

### 10.7 Form field (shadcn `input` + `label`)

- Label above the field, 14/20 weight 600, 8 px gap. Never a placeholder as label.
- Input 40 px, 16/24 text, padding 0 12, `--surface-raised` fill, 1 px `--border-control`, radius 6. Hover: border `--text-muted`. Focus: outline. Invalid: border `--danger` and `aria-invalid`. Disabled: `--surface` fill, `--border`, `--text-disabled`.
- Help line below with a fixed height of 20 px: help text in `--text-muted`, or the error in `--danger` with `circle-alert` 16, referenced by `aria-describedby`. Nothing moves when an error appears (FO4).
- Validation on blur; after the first error, live on each keystroke until fixed (FO2). Inputs keep their values after any error (FO13).
- Commit: settings text fields save on blur and on Enter; Escape restores the saved value.
- Character counter at the right of the help line from 80 % of the limit ("52/64"), tabular.
- Required fields: every field is required unless its label ends with "(optional)" (FO10). The only optional field in v0.1 is the Java path (behind "Erweitert").
- Name, search and memory fields: `spellcheck="false"`, `autocomplete="off"`. Numeric fields use `inputmode="numeric"`, never `type="number"`.

### 10.8 Checkbox (shadcn `checkbox`) and switch (shadcn `switch`)

Checkbox: 20 px box, radius 4, 1 px `--border-control`, `--surface-raised` interior in every context (also inside a highlighted menu item, so the boundary keeps 3:1 against the interior, 2.3); checked: `--text` fill with a 16 px `check` in `--bg`. Inside a 40 px high clickable label row, label 14/20 to the right, 8 px gap. States: hover (border `--text-muted`), focus outline on the box, disabled (`--border`, `--text-disabled`).

Switch: 36 × 20 track, pill. Off: `--hover` track, 1 px `--border-control`, thumb 14 px `--text-muted`. On: `--text` track, thumb `--bg`. The whole row (label plus switch) is the 40 px target. Track color over `--dur-press`; thumb movement per section 8.

### 10.9 Dialogs (shadcn `dialog`, `alert-dialog`)

- Panel `--surface-raised`, radius 10, padding 24, level-1 elevation, on `--overlay`. Max-height `calc(100dvh - 48px)`; the body scrolls, title and footer stay. Anything that appears in the body (an error) scrolls into view when it appears.
- Title 18/26 weight 600 as the accessible name; long names in titles wrap (`overflow-wrap: anywhere`), never truncate. Text 16/24, at most 72ch. A close button (`icon`, `x`, "Schließen" / "Close") top right in `dialog`, not in `alert-dialog`.
- Footer: at most two buttons, right-aligned, primary or destructive last, 8 px gap, 24 px above. A footer that only dismisses ("Schließen" / "Close") uses `secondary`: closing is never the next action, so it is never the primary.
- Widths: create instance 560, sign-in 560, third-party licenses 640, confirmations 440.
- Closing: Escape closes; it cancels a running operation (sign-in). A pointer-down outside the panel never closes the create and sign-in dialogs (it closes confirmations). Focus returns to the trigger; if the trigger no longer exists, section 13 decides.
- The create dialog keeps a draft: closing it with entered values and reopening it in the same session restores them (FO13); a successful create clears the draft.
- Initial focus: first field (create), the primary action (sign-in idle), the safe button (confirmations).
- Confirmations are a simple confirm with named buttons, no typing a name. Titles are the concrete question, the text names the consequence, the buttons name both outcomes (Z5).

### 10.10 Toast (shadcn `sonner`)

- Bottom right, 24 px from the edges, width 356, at most 3 visible, newest on top.
- `--surface-raised`, radius 10, level-1 elevation, padding 12 16, title 14/20 weight 600 `--text`, description 13/18 `--text-muted`, icon 16 (success `check` in `--success`, info `info` in `--text-muted`). Names in toasts wrap, never truncate.
- 4 s, paused while hovered or focused; Sonner's live region announces them.
- Toasts confirm results ("{{name}} angelegt", "Angemeldet als {{name}}") and report background events that need no action (settings reset, a version-list refresh that fell back to the saved list). An error that needs the user to act is never only a toast: it appears where it happened. No undo in v0.1.

### 10.11 ErrorPanel (custom, Task 4) and inline error

ErrorPanel: block, `--surface-raised`, 1 px `--border`, radius 10, padding 16, alert role. Row 1: `circle-alert` 18 `--danger`, 12 px gap, title 16/24 weight 600 `--text` (what happened, no full stop). Row 2, indented to the title: body 14/20 `--text-muted` (what to do). Row 3, 12 px below: actions: "Erneut versuchen" / "Try again" (`secondary`, `refresh-cw`) when retry is possible, the action from the error as a second `secondary` button, and a "Details" collapsible (10.15). Details: the `detail` text in mono 13/20 in a `--bg` box, radius 6, padding 12, max-height 160, scrolls, with a `ghost` "Kopieren" / "Copy". The detail text is the English core message in both UI languages: it is diagnostic text for bug reports and is exempt from translation.

Inline error: under a field or setting, `circle-alert` 16 plus one sentence in `--danger` 14/20.

### 10.12 Launch progress panel (custom, 4b, shadcn `progress`)

24 px below the Play button, width at most 480, only in the preparing state. Its height is fixed at 160 px from the moment preparing starts, so nothing jumps between phases.

- Line 1, phase (16/24 `--text`): "Loader wird aufgelöst" / "Resolving loader", "Installer wird geladen" / "Downloading installer", "Dateien werden geladen" / "Downloading files", "Installation wird geprüft" / "Verifying installation", "Wird installiert" / "Installing", "Spiel wird gestartet" / "Launching game". 4 px below:
- Line 2 (secondary, tabular): "3.402 von 4.118 Dateien" / "3,402 of 4,118 files", or "Schritt 3 von 6: jarsplitter" / "Step 3 of 6: jarsplitter" (a real sequence). 8 px below:
- Bar: 6 px, pill, track `--border-strong` (1.79 / 1.95 against `--surface`, enough to read the remaining length), fill `--text` (9.06 / 7.14 against the track), determinate when counts or steps are known (value and max exposed); hidden during phases without counts, never looped. 12 px below:
- When an install log exists: link button "Installationsprotokoll öffnen" / "Open install log" (40 px).
- Notice events as caption lines at the bottom, at most the last two.
- The phase line is a polite live region; counts are not announced on every change.
- Offline: if all files are cached the launch starts normally. Otherwise the panel is replaced by an ErrorPanel "Keine Verbindung" / "No connection", body "Für diese Version fehlen Dateien. Prüfe die Internetverbindung und versuche es erneut." / "Files for this version are missing. Check your internet connection and try again.", with "Erneut versuchen".
- Failure: the panel is replaced in place by an ErrorPanel with "Erneut versuchen". For `installFailed` and `installStepFailed` it has the second action "Installationsprotokoll öffnen" / "Open install log" when an install log was reported.

### 10.13 Log view (custom, 4b, Virtuoso inside the log area)

- Log area: `--surface`, 1 px `--border`, radius 10, fills the remaining height of the content, scrolls internally. Line padding 0 12, mono 13/20.
- Line grid, 8 px column gap: time `hh:mm:ss` (8ch, `--text-muted`) | level (5ch, the level word as given, in its level color) | thread (16ch max, ellipsis, `--text-muted`) | message (wraps, `overflow-wrap: anywhere`, level color). Throwable lines are indented 2ch under their message, same color. The level word means color is never the only signal.
- Level colors: ERROR, FATAL `--log-error`; WARN `--log-warn`; INFO and lines without level `--log-info`; DEBUG, TRACE `--log-debug`.
- Selection: native text selection across lines, `--selection` background, text `--text`.
- Search hits: `<mark>` with `--highlight` and `--text`; the current hit adds a 1 px `--text` outline.
- When the user scrolls up, following stops and a `secondary` "Zum Ende springen" / "Jump to end" (`arrow-down-to-line`) appears at the bottom right inside the log area, 12 px from the edges; back at the end, following resumes and the button disappears (no animation).

Toolbar above the log area, 40 px controls, 8 px gaps, 8 px between rows, 16 px below the last row:
- Row 1: visible label "Im Log suchen" / "Search log" (14/20 600) left of the search input (width 240), then the hit count "12 Treffer" / "12 matches" (secondary, tabular), then the icon buttons "Vorheriger Treffer" (`chevron-up`) and "Nächster Treffer" (`chevron-down`). Enter and Shift+Enter in the field step through hits; Ctrl+F focuses the field.
- Row 2: on the left "Stufen" / "Levels" dropdown (`secondary`, label "Stufen: 4 von 5" / "Levels: 4 of 5") with checkbox items "Fehler" / "Error", "Warnung" / "Warning", "Info", "Debug", "Sonstige" / "Other"; on the right "Kopieren" / "Copy" (`secondary`, `copy`, copies the filtered lines, redacted by the backend) and "Logdatei öffnen" / "Open log file" (`ghost`, `external-link`).
- The two rows join into one (row 1 left, row 2 right) from a content width of 1000 px, that is, a window of 1280 px and wider (`full:`). Sum of the joined row: row 1 is label about 95 + 8 + input 240 + 8 + count about 65 + 8 + icon button 40 + 8 + icon button 40 = 512; row 2 is "Stufen: 4 von 5" about 160 + 8 + "Kopieren" about 114 + 8 + "Logdatei öffnen" about 166 = 456; with an 8 px gap between them, about 976 px. Each row alone fits the narrow layout (row 1 about 512 px, row 2 about 456 px, against 528 px content at 960 with 150 % zoom).
- Under the toolbar, the run switcher: a ghost select (10.15) "Aktueller Lauf: {{instance}}, seit {{time}}" / "Current run: {{instance}}, since {{time}}" with the option "Letzter Lauf" / "Previous run".

### 10.14 Badge (shadcn `badge`)

Pill, 20 px high, padding 0 8, 13/18 weight 600:

| Badge | Text | Fill | Border |
|---|---|---|---|
| "Empfohlen" / "Recommended" | `--text` | `--hover` | none |
| "Instabil" / "Unstable", "Neuere Version" / "Newer format", "Nicht lesbar" / "Unreadable", "Erneut anmelden" / "Sign in again", "Entwicklungs-Build" / "Development build" | `--warning` | `--surface-raised` | 1 px `--border` |

Badges carry text, never a dot or an emoji.

### 10.15 Others

- **Skeleton** (shadcn `skeleton`, pulse removed): `--hover` fill, radius 6, the exact size of the content it stands for, shown after 200 ms.
- **Tooltip** (shadcn `tooltip`): inverse colors, radius 6, padding 4 8, 13/18, max-width 240; for icon-only buttons, truncated names and truncated paths only, never for required information. Opens on hover and on keyboard focus.
- **Collapsible** (shadcn `collapsible`): a `ghost` button with the label and `chevron-down` (open: `chevron-up`) and expanded state; the content appears below without animation. Used for "Erweitert" / "Advanced" in Settings, "Details" in the ErrorPanel and the license texts.
- **Ghost select** (shadcn `select` with a ghost trigger): trigger without border or fill, text 14/20 `--text`, `chevron-down` 16, hover `--hover`, 40 px high. Used only for the log run switcher.
- **In-text link**: inside a paragraph, inherits the paragraph's size, `--text`, underline 1 px at 3 px offset, hover 2 px; the 40 px rule does not apply to links in running text (WCAG 2.5.8 exception). A link that opens a browser ends with `external-link` 16 and the accessible name adds "(öffnet im Browser)" / "(opens in browser)".
- **Spinner**: lucide `loader-circle` 16 (18 in the Play button), `currentColor`, rotating linearly at 800 ms per turn, `aria-hidden` (the label carries the state); hidden under reduced motion.
- **Path display**: mono 13/20, one line, truncated in the middle (start and the last folder stay visible), full path in a tooltip and in the accessible name; selectable text.
- **Separator** (shadcn `separator`): 1 px `--border` (in menus `--border-strong` in dark, `--border` in light), 4 px margin in menus.
- **ScrollArea** (shadcn `scroll-area`): only inside popovers and dialogs; the content column uses native scrolling, themed through `color-scheme` and `scrollbar-color: var(--border-strong) transparent`; the ScrollArea thumb uses `--border-strong` too.
- Not used: Card, Sheet, Tabs, Kbd, Avatar (the initial circle is custom and loads no images), the Sidebar block, Accordion, Carousel.

## 11. Screens

Each screen: regions, primary action, states, keyboard path. Tab order on every screen: sidebar nav, then header (primary action, account chip), then content top to bottom, left to right.

### 11.0 App start and first run

- `main.tsx` loads the settings, then i18n and theme, then renders. Until then the window shows only `--bg`.
- Loading longer than 200 ms: the shell skeleton (sidebar items, header title block) appears.
- Empty: not applicable (missing settings give defaults).
- Error (settings cannot be read even after the broken-file fallback): an ErrorPanel, 480 px wide, horizontally centered, 96 px from the top, on `--bg`, with "Erneut versuchen" / "Try again". Its text is English if i18n could not be initialized, otherwise in the UI language.
- Settings were reset: toast once after render "Einstellungen waren beschädigt und wurden zurückgesetzt" / "Settings were damaged and have been reset".
- First run (no account, no instance): 4a starts on Instances with its empty state and the chip "Anmelden". From 4b the start page shows the no-instance state with the text "Lege eine Instanz an und melde dich danach mit Microsoft an, um zu spielen." / "Create an instance, then sign in with Microsoft to play."; after the first instance exists, the Play button moves to "Anmelden" by its precedence (10.2). Instance first, account second, because creating an instance works offline and without approval, while sign-in may still be unavailable (11.6).

### 11.1 Instances (4a; start view in 4a)

```
Instanzen                                         [+ Instanz anlegen]  (M Mira_K v)
------------------------------------------------------------------------------------
Name                            Version     Loader           Zuletzt gespielt
[v] Überleben 1.21              1.21.11     Fabric 0.17.3    vor 2 Tagen       (...)
    1.20.1 Forge                1.20.1      Forge 47.4.28    Noch nie gespielt (...)
    Alte Welt                   [Neuere Version]                               (...)
```

- Header: h1 "Instanzen" / "Instances"; primary "Instanz anlegen" / "Create instance" (`plus`) in the header.
- Content: column header row, then the list of rows (10.4), no gap between rows; the content column scrolls.
- Loading: column header plus three skeleton rows (name 40 %, version 80 px, loader 120 px, date 96 px).
- Empty: no column header. A left-aligned block at the top of the content, max-width 480: h2 "Noch keine Instanz" / "No instances yet", one line secondary "Eine Instanz ist ein eigener Spielordner mit Version und Loader." / "An instance is its own game folder with a version and a loader.", 16 px, primary "Instanz anlegen". The header primary is hidden in this state (one primary action per screen).
- Error: ErrorPanel in place of the list with "Erneut versuchen".
- Many (40): plain scrolling list, no virtualization (below the 50-row threshold).
- Long names: 64-character names with no spaces truncate in the row with a tooltip; nothing overflows.
- Keyboard: Tab to the header action, the chip, then the first row; Tab moves row button, its action button, next row. Enter or Space on a row selects it; Enter on the action button opens its menu (arrows, Escape closes, focus back to the action button). The Delete key on a focused row opens the delete dialog (same rules as the menu item).

### 11.2 Create instance (dialog, 4a)

Width 560. Title "Instanz anlegen" / "Create instance". Fields top to bottom, 16 px apart:
1. "Spielversion" / "Game version": combobox with filter checkboxes below (10.5).
2. "Loader": segments (10.6).
3. "Loader-Version" / "Loader version": select (hidden for "Kein Loader").
4. "Name": input, at most 64 characters, prefilled from the selection ("1.20.1 Forge") until the user edits it; help line "So heißt die Instanz in der Liste." / "This is how the instance appears in the list."

Footer: "Abbrechen" / "Cancel" (`secondary`), "Instanz anlegen" / "Create instance" (`primary`, loading "Wird angelegt…" / "Creating…").

- Loading: version trigger disabled with "Versionen werden geladen…"; loader version disabled with "Wird geladen…".
- Empty: filters hide every version: the combobox empty text with "Filter zurücksetzen". Loader without builds: hint and disabled create button (10.6).
- Error: version list failed or offline: 10.5. Create failed: ErrorPanel directly above the footer, all inputs kept, focus moves to the panel title (which scrolls it into view at the minimum window height, where the body scrolls).
- Validation: name empty or whitespace: "Gib einen Namen ein." / "Enter a name."; longer than 64: "Höchstens 64 Zeichen." / "At most 64 characters."; line breaks are removed on paste.
- Success: dialog closes, toast "{{name}} angelegt" / "{{name}} created", the new row is selected, scrolled into view and focused.
- Keyboard: focus starts on the version trigger; Tab through filters, loader segments (one tab stop, arrows inside), loader version, name, Cancel, Create. Enter in the name field submits. Escape closes and keeps the draft (10.9).

### 11.3 Delete instance (alert dialog, 4a)

Width 440. Title "{{name}} löschen?" / "Delete {{name}}?". Text "Der Ordner mit Welten, Einstellungen und Screenshots wird in den Papierkorb verschoben." / "The folder with worlds, settings and screenshots is moved to the Recycle Bin." Buttons "Behalten" / "Keep" (`secondary`, initial focus) and "In den Papierkorb" / "Move to Recycle Bin" (`destructive`, loading "Wird verschoben…" / "Moving…").

- Loading: the destructive button shows its loading state; both buttons inert.
- Error: ErrorPanel inside the dialog above the buttons; the dialog stays open. If the Recycle Bin cannot take the folder (too large for the bin, drive without a bin), the app never deletes permanently: the ErrorPanel says "Ordner konnte nicht in den Papierkorb verschoben werden" / "The folder could not be moved to the Recycle Bin", body "Es wurde nichts gelöscht. Öffne den Ordner, um ihn selbst zu verschieben oder zu löschen." / "Nothing was deleted. Open the folder to move or delete it yourself.", with the action "Ordner öffnen" / "Open folder".
- Success: dialog closes, the row disappears, toast "{{name}} ist im Papierkorb" / "{{name}} is in the Recycle Bin". Selection fallback per 10.2.
- Focus after delete: the action button of the next row, else of the previous row, else "Instanz anlegen".
- Empty: not applicable (opened for one instance).
- Keyboard: Tab between the two buttons, Enter activates the focused one, Escape keeps.

### 11.4 Accounts (chip, menu, remove dialog; 4a)

There is no accounts page; accounts live in the header chip (10.3).

- Loading: the chip shows a 40 × 200 pill skeleton.
- Empty: the "Anmelden" chip; on the start page the Play button also says "Anmelden".
- Error (account list failed): the chip becomes a `secondary` button "Konten nicht verfügbar" / "Accounts unavailable" with `triangle-alert`; its menu shows the error text and "Erneut versuchen". A failing credential store does not block the list.
- Remove dialog (alert dialog, 440): title "{{name}} abmelden?" / "Sign out {{name}}?", text "Die gespeicherte Anmeldung wird von diesem PC entfernt. Dein Microsoft-Konto bleibt unverändert." / "The saved sign-in is removed from this PC. Your Microsoft account stays unchanged.", buttons "Behalten" / "Keep" (initial focus) and "Abmelden" / "Sign out" (`destructive`). Success toast "{{name}} abgemeldet" / "Signed out {{name}}". Focus afterward: the chip.
- Keyboard: Tab to the chip, Enter opens the menu, arrows move, Enter selects, the right arrow opens the submenu; Escape closes and returns focus to the chip.

### 11.5 Sign-in (dialog, 4a)

Width 560. Title "Konto hinzufügen" / "Add account"; when signing in an existing account again: "{{name}} erneut anmelden" / "Sign in {{name}} again". The body changes per phase; the height may change between phases (no animation). Footer at most two buttons.

| Phase | Body | Footer |
|---|---|---|
| idle | "Die Anmeldung läuft im Browser bei Microsoft. Hengst Client sieht dein Passwort nicht." / "You sign in with Microsoft in your browser. Hengst Client never sees your password." Below it, the link button "Code stattdessen verwenden" / "Use a code instead" | secondary "Abbrechen" / "Cancel", primary "Mit Microsoft anmelden" / "Sign in with Microsoft" (`external-link`) |
| starting | as idle | the primary shows "Wird geöffnet…" / "Opening…" with spinner, inert; Cancel stays active |
| waitingBrowser | spinner (16) plus "Melde dich im Browser an. Danach geht es hier automatisch weiter." / "Complete the sign-in in your browser. This window continues on its own afterward."; caption "Wartet höchstens 5 Minuten." / "Waits up to 5 minutes."; link button "Code stattdessen verwenden" | secondary "Abbrechen", secondary "Browser erneut öffnen" / "Open browser again". No primary: waiting is the state |
| showingCode | "Öffne microsoft.com/link und gib diesen Code ein." / "Open microsoft.com/link and enter this code."; code box (mono 28/36, `--bg`, radius 6, padding 12 16) with `secondary` "Code kopieren" / "Copy code" to its right; caption "Noch 14:32 Min. gültig" / "14:32 left" (tabular, updates each second, not announced) | secondary "Abbrechen", primary "microsoft.com/link öffnen" / "Open microsoft.com/link" |
| completing | spinner plus "Wird angemeldet…" / "Signing in…" | both inert |
| failed | ErrorPanel (with "Erneut versuchen", which restarts the same method), copy per the table below | secondary "Schließen" / "Close" |
| appNotApproved | block in 11.6 | secondary "Schließen" / "Close", primary "Mehr erfahren" / "Learn more" (the next step: read why and what happens next) |
| done | dialog closes, toast "Angemeldet als {{name}}" / "Signed in as {{name}}" | |
| cancelled | dialog closes, focus to the chip | |

- "Browser erneut öffnen" fails: inline error under the body text "Der Browser ließ sich nicht öffnen. Verwende stattdessen den Code." / "The browser could not be opened. Use a code instead." (the link button below stays).
- Code copied: the copy button reads "Kopiert" / "Copied" for 2 s (no toast).
- Signing in again with a different Microsoft account than the one asked for: that account is added and selected as usual; the dialog closes with the toast "Angemeldet als {{new}}. {{old}} ist weiterhin abgemeldet." / "Signed in as {{new}}. {{old}} is still signed out." and the old account keeps its "Erneut anmelden" badge.
- Loading: the starting and completing phases. Empty: not applicable. Error: the failed phase.
- Keyboard: initial focus on the primary (idle) or on "Code kopieren" (showingCode); Escape cancels the running login and closes; focus returns to the chip (or to the Play button when opened from it).

Error copy in the failed phase (title without full stop, body, action):

| Code | Title (de / en) | Body (de / en) | Action |
|---|---|---|---|
| `network`, `offline` | "Keine Verbindung zu Microsoft" / "Cannot reach Microsoft" | "Prüfe die Internetverbindung und versuche es erneut." / "Check your internet connection and try again." | Try again |
| `loginDeclined` | "Anmeldung abgelehnt" / "Sign-in declined" | "Die Anmeldung wurde im Browser abgebrochen oder nicht erlaubt. Versuche es erneut, wenn du Hengst Client den Zugriff erlauben willst." / "The sign-in was canceled or not allowed in the browser. Try again if you want to give Hengst Client access." | Try again |
| `browserLoginTimedOut` | "Die Anmeldung ist abgelaufen" / "The sign-in timed out" | "Im Browser kam innerhalb von 5 Minuten keine Anmeldung an. Starte sie erneut." / "No sign-in arrived from the browser within 5 minutes. Start it again." | Try again |
| `deviceCodeExpired` | "Der Code ist abgelaufen" / "The code expired" | "Fordere einen neuen Code an und gib ihn auf microsoft.com/link ein." / "Request a new code and enter it at microsoft.com/link." | Try again |
| `loopbackUnavailable` | "Die Browser-Anmeldung ist gerade nicht möglich" / "Browser sign-in is not possible right now" | "Hengst Client konnte keinen lokalen Anschluss für die Rückmeldung öffnen. Verwende stattdessen den Code." / "Hengst Client could not open a local port for the reply. Use a code instead." | "Code stattdessen verwenden" |
| `openFailed` | "Der Browser ließ sich nicht öffnen" / "The browser could not be opened" | "Verwende stattdessen den Code." / "Use a code instead." | "Code stattdessen verwenden" |
| `browserLoginFailed` | "Microsoft hat die Anmeldung nicht abgeschlossen" / "Microsoft did not complete the sign-in" | "Versuche es erneut oder verwende stattdessen den Code." / "Try again or use a code instead." | Try again |
| `appMisconfigured` | "Die Anmeldung ist in diesem Build falsch eingerichtet" / "Sign-in is set up incorrectly in this build" | "Das liegt nicht an deinem Konto. Melde den Fehler mit den Details." / "This is not caused by your account. Report it with the details." | none (Details) |
| `missingClientId` | title and body equal the no-client-id block of 11.6 (fallback only, see below) | | none |
| `noXboxAccount` | "Zu diesem Konto gibt es kein Xbox-Profil" / "This account has no Xbox profile" | "Lege einmalig ein Xbox-Profil an, danach klappt die Anmeldung." / "Create an Xbox profile once, then sign-in works." | "Xbox-Profil anlegen" / "Create Xbox profile" |
| `gameNotOwned` | "Dieses Konto besitzt Minecraft: Java Edition nicht" / "This account does not own Minecraft: Java Edition" | "Melde dich mit dem Konto an, mit dem du das Spiel gekauft hast." / "Sign in with the account you bought the game with." | Try again |
| `noProfile` | "Zu diesem Konto gibt es noch kein Spielerprofil" / "This account has no player profile yet" | "Lege auf minecraft.net einen Spielernamen an und versuche es dann erneut." / "Choose a player name on minecraft.net, then try again." | "Profil anlegen" / "Create profile" |
| `credentialStore` | "Die Anmeldung konnte nicht gespeichert werden" / "The sign-in could not be saved" | "Die Windows-Anmeldeinformationsverwaltung hat den Zugriff verweigert. Versuche es erneut." / "Windows Credential Manager refused access. Try again." | Try again |
| other | from `errorCopy.ts` (Task 4) | | |

`missingClientId`: a build without a client id never opens this dialog (`can_sign_in` false); the Play button, the account chip menu and the add-account entry show the no-client-id block of 11.6 instead. The row above is only the fallback if the backend reports the error although the dialog was reached, and its text equals that block: one text, one source. It is a different condition from `appNotApproved`, which keeps its own block.

### 11.6 "Sign-in not available yet" block

Shown inside the sign-in dialog for `appNotApproved`, instead of the generic ErrorPanel. This block is for one condition only: Mojang has not approved the app yet. Not an error style: `info` 18 in `--text-muted`, title (18/26) "Anmeldung noch nicht verfügbar" / "Sign-in is not available yet", text (16/24) "Mojang hat die Microsoft-Anmeldung von Hengst Client noch nicht freigegeben. Bis dahin lassen sich keine Konten hinzufügen. Mit deinem Konto ist alles in Ordnung, du musst nichts ändern." / "Mojang has not approved Hengst Client's Microsoft sign-in yet. Until then, accounts cannot be added. Your account is fine, nothing needs to change on your side." Dev builds add a caption: "Entwicklungs-Builds können das Offline-Entwicklungskonto verwenden." / "Development builds can use the offline development account." Buttons as in 11.5; "Mehr erfahren" opens the sign-in help page in the browser.

Second block, build without a client id (`missingClientId`, `can_sign_in` false): the build was made without a Microsoft client id and therefore cannot sign in. Defined only here and referenced from the Play button (10.2), the account chip menu and the add-account entry (10.3) and the `missingClientId` row in 11.5. Shown as an alert dialog (440), same style as the block above (`info` 18 in `--text-muted`, title 18/26, text 16/24); the sign-in dialog is not opened. Title "Dieser Build kann sich nicht anmelden" / "This build cannot sign in", text "Er wurde ohne Microsoft-Client-ID gebaut. Verwende ein offizielles Release oder trage beim eigenen Bauen die Client-ID ein." / "It was built without a Microsoft client id. Use an official release, or set the client id when building it yourself." Buttons: `secondary` "Schließen" / "Close" (initial focus) and primary "Anleitung zum Bauen" / "Build instructions", which opens the build section of the README in the browser (`UrlTarget::BuildHelp`).

### 11.7 Settings with About (4a partial, 4b complete)

Single column, max-width 640, sections 48 px apart, each with an h2 and controls 24 px below it. Labels above controls. Choices save on selection, text fields on blur or Enter (no save button); if saving takes longer than 400 ms the help line shows "Wird gespeichert…" / "Saving…".

Sections in order:
1. "Sprache" / "Language": segments "System", "Deutsch", "English".
2. "Design" / "Theme": segments "System", "Hell" / "Light", "Dunkel" / "Dark". The window title bar follows at once (7.1).
3. "Arbeitsspeicher" / "Memory" (4b): input with suffix "MB" (width 160), help line "Empfohlen: 2048 bis 4096 MB. Installiert: 16,0 GB." / "Recommended: 2048 to 4096 MB. Installed: 16.0 GB." Out of range: "Wähle einen Wert zwischen 512 und 65.536 MB." / "Choose a value between 512 and 65,536 MB." Above the installed memory: saved, with a warning in the help line (`triangle-alert`, `--warning`): "Mehr als installiert. Das Spiel startet dann womöglich nicht." / "More than installed. The game may not start." Then the collapsible "Erweitert" / "Advanced" with:
   - "Java (optional)" (4b): path display (10.15), or "Automatisch passend zur Spielversion" / "Automatic, matching the game version" when unset; buttons `secondary` "Java auswählen…" / "Choose Java…" (opens the Windows file dialog) and `ghost` "Zurücksetzen" / "Reset" (only when set). After a pick the help line shows spinner plus "Wird geprüft…" / "Checking…"; success shows the detected version as a caption ("Java 21.0.5"); a file that is not `java.exe` or `javaw.exe`: "Wähle eine java.exe oder javaw.exe aus." / "Choose a java.exe or javaw.exe file."; the check fails or times out: "Diese Java-Installation ließ sich nicht starten." / "This Java installation could not be started." In both error cases the previous value stays. Reset returns to automatic without a confirmation.
4. "Spielstart" / "Launch" (4b): switch "Beim Spielstart in den Infobereich verkleinern" / "Minimize to the notification area when the game starts".
5. "Datenordner" / "Data folder": path display, `secondary` "Ordner öffnen" / "Open folder".
6. "Über Hengst Client" / "About Hengst Client": mark 48 px, name 18/26, version (tabular), dev badge if any, "Not an official Minecraft product." (identical in both languages), "Lizenz: GPL-3.0" / "License: GPL-3.0", link button "Quellcode" / "Source code" (opens the repository in the browser), button `secondary` "Lizenzen von Drittanbietern" / "Third-party licenses" (4b, opens 11.10), and the facts from Task 2 as secondary lines (number of known game versions, newest release and snapshot).

- Primary action: none; settings save themselves (the one screen without a primary button, on purpose).
- Loading: skeletons per section (title 120 × 26, control 320 × 40).
- Empty: About facts without data: "Noch keine Versionsliste geladen" / "No version list loaded yet".
- Error: saving failed: inline error under that control, which returns to its previous value. About facts failed: inline error with "Erneut versuchen" in that section only.
- Long paths: middle truncation with tooltip (10.15).
- Keyboard: Tab from control to control; segments are one tab stop each with arrows inside; the switch toggles with Space; the collapsible opens with Enter or Space.

### 11.8 Start page (4b; start view from 4b)

```
Spielen                                                          (M Mira_K v)

  Überleben mit Freunden 1.21
  1.21.11        Fabric 0.17.3        Zuletzt gespielt vor 2 Tagen
  [Instanz wechseln v]

  [  > Spielen                     ]

  Dateien werden geladen
  3.402 von 4.118 Dateien
  [=================-------]
  Installationsprotokoll öffnen
```

- Header: h1 "Spielen" / "Play", chip; no header action (the Play button is the action).
- Content: one left-aligned block, 48 px from the top, max-width 640: start title (32/40, two lines max), 8 px, meta line (secondary 14/20: version, loader, last played as three spans with 24 px gaps, no separators), 16 px, `ghost` "Instanz wechseln" / "Switch instance" with `chevron-down` (DropdownMenu with the instances as radio items; hidden when the list holds exactly one instance, so it is always shown when state 5 of 10.2 applies), 32 px, the Play button (10.2), 24 px, progress panel (10.12) or ErrorPanel, and in the running state a link button "Log anzeigen" / "Show log".
- Loading: skeleton title (320 × 40), meta (240 × 20), button (240 × 56).
- Empty: no instance: h2-sized title "Noch keine Instanz" / "No instances yet" in place of the start title, the first-run text (11.0), Play button in its "Instanz anlegen" state. No account: the instance shows normally, the button says "Anmelden".
- Error: instance or game state failed to load: ErrorPanel in place of the block with "Erneut versuchen". Launch failure and abnormal exit: 10.2 and 10.12.
- Height at 960 × 600: 56 header + 48 + 80 title + 8 + 20 meta + 16 + 40 switcher + 32 + 56 Play + 24 + 160 panel = 540 px, 572 px with the 32 px bottom padding; it fits.
- Keyboard: Tab to "Instanz wechseln", then the Play button. The Play button gets initial focus when the view opens from the sidebar, so Enter plays. No global hotkey.

### 11.9 Live log (4b)

- Header: h1 "Log", chip. Then the toolbar (10.13), the run switcher, the log area.
- Loading: toolbar visible but inert, eight skeleton lines of varying width (40 to 90 %) in the log area.
- Empty: no run yet: in the log area, top-left with 16 px padding, title "Noch kein Log" / "No log yet", text "Starte eine Instanz, dann erscheint ihre Ausgabe hier." / "Start an instance and its output appears here.", primary "Spielen öffnen" / "Open Play". Run without output yet: caption "Noch keine Ausgabe" / "No output yet". Filters hide everything: "Alle Zeilen sind ausgeblendet." / "All lines are hidden." with `ghost` "Alle Stufen zeigen" / "Show all levels".
- Error: attaching failed: ErrorPanel in the log area with "Erneut versuchen". Copy failed: inline error under the toolbar.
- 10,000 lines: virtualized, no layout change.
- Keyboard: Ctrl+F to search; Tab: search, previous, next, levels, copy, open file, run switcher, then the log area (focusable; arrows and Page Up/Down scroll, Ctrl+A selects all lines of the current filter, Ctrl+C copies), then "Zum Ende springen" if shown.

### 11.10 Third-party licenses (4b, dialog)

Opened from About. Dialog width 640, title "Lizenzen von Drittanbietern" / "Third-party licenses". Body: a text line "Hengst Client nutzt diese Bibliotheken. Die vollständigen Texte stehen unter jedem Eintrag." / "Hengst Client uses these libraries. The full texts are under each entry.", then two groups with h3-sized labels (16/24 600) "Rust" and "JavaScript", each a list of collapsible entries (40 px): name, version (tabular, secondary), license id (caption, e.g. "MIT OR Apache-2.0"); expanded, the license text in mono 13/20 in a `--bg` box, max-height 240, scrolls. Footer: secondary "Schließen" / "Close" only (a dismiss, not a next action, so the dialog has no primary button). The data is a static file in the bundle; the dialog makes no network request.
- Loading: five skeleton rows per group.
- Empty: "Keine Lizenzangaben gefunden. Das ist ein Fehler im Build." / "No license data found. This is a build error." (inline error; the build test keeps this from shipping).
- Error: ErrorPanel "Lizenzangaben konnten nicht geladen werden" / "License data could not be loaded" with "Erneut versuchen".
- Keyboard: Tab through entries, Enter expands, Escape closes, focus back to the About button.

### 11.11 Tray and closing while the game runs (4b)

- Tray icon: the brand mark. Tooltip "Hengst Client" when idle, "Hengst Client: {{instance}} läuft" / "Hengst Client: {{instance}} is running" while a game runs.
- Tray menu (native Windows menu, so native styling): "Hengst Client öffnen" / "Open Hengst Client", "Spiel beenden" / "Stop game" (only while a game runs), separator, "Hengst Client beenden" / "Quit Hengst Client" (the word "Beenden" alone belongs to the Play button, where it stops the game). Left click on the icon opens the window.
- When the game starts and "minimize to the notification area" is on, the window hides to the tray. When the game ends, the window is shown again and restored; it is not forced to the foreground (Windows may flash the taskbar button instead).
- Closing the window while a game runs hides it to the tray (no dialog). Closing the window while no game runs (idle, or preparing, which is canceled first) quits the app; there is no tray icon without a running game.
- "Hengst Client beenden" while a game runs opens an alert dialog (440): title "Spiel beenden und Hengst Client schließen?" / "Stop the game and quit Hengst Client?", text "Das Spiel wird beendet. Fortschritt seit dem letzten Speichern im Spiel kann verloren gehen." / "The game is stopped. Progress since the game last saved may be lost.", buttons "Weiterspielen" / "Keep playing" (`secondary`, initial focus) and "Spiel beenden und schließen" / "Stop game and quit" (`destructive`). The window is shown first if it was hidden.
- Loading: the destructive button shows "Wird beendet…" until the game process has exited, then the app quits. Empty: not applicable. Error: if the game does not stop within 10 s, the dialog shows "Das Spiel reagiert nicht" / "The game is not responding" with the destructive button "Sofort beenden und schließen" / "Force stop and quit".

### 11.12 Instance detail

There is no instance detail screen in v0.1. Everything about an instance is in its row, its menu and the start page.

## 12. Writing

- Two languages, German and English (US spelling), complete in both; all strings in `src/i18n/{de,en}.json`. German uses informal "du" throughout, never "Sie". No English leftovers in the German UI, except the legal sentence and the diagnostic error details (10.11).
- Sentence case in both languages for everything: titles, buttons, menu items, labels ("Instanz anlegen", "Create instance"). This overrides WIG's title case for English buttons (produkt-regelwerk W15) because the plan fixes one style for both languages.
- Buttons are verbs naming the result ("Instanz anlegen", "In den Papierkorb", "Abmelden"), never "OK", "Ja", "Weiter", "Submit". A flow keeps its verb: "Instanz anlegen" leads to "{{name}} angelegt".
- Loading labels: verb plus a real ellipsis "…" ("Wird angelegt…", "Creating…").
- Errors in two parts: a title saying what happened, without a full stop ("Versionsliste konnte nicht geladen werden" / "Version list could not be loaded"), and a body saying what to do, as full sentences ("Prüfe die Internetverbindung und versuche es erneut." / "Check your internet connection and try again."). No apology, no blame, no codes in the title.
- No exclamation marks anywhere. No filler ("einfach", "schnell", "nahtlos", "simply", "seamless"), no greetings, no marketing words. No full stop at the end of titles, labels, buttons, badges and captions that are not sentences; body text, help lines and captions that are full sentences end with one.
- No dashes as separators in visible text; ranges with "bis" / "to". Quotes: „…“ in German, “…” in English.
- Numbers, units, dates, durations: section 3.3.
- Proper names stay untranslated: Fabric, Quilt, Legacy Fabric, Forge, NeoForge, Microsoft, Mojang, Xbox, Hengst Client.
- "Minecraft" appears in factual references to the game ("Minecraft: Java Edition", the legal sentence, Microsoft's and Mojang's page names), never in the product name, the mark, the window title or the navigation (docs/PLAN.md section 9).
- Example names in mockups and tests are neutral player names such as "Mira_K", never the game's default character names.

## 13. Accessibility

- Focus: visible on every interactive element (section 9); order sidebar, header, content; modals trap focus and return it to the trigger; Escape closes.
- When the focused element disappears: after deleting a row, focus goes to the next row's action button, else the previous row's, else "Instanz anlegen"; when an empty-state button unmounts because the first item was created, focus goes to the new row; after signing out, to the chip.
- Real elements only: buttons for actions, link buttons for opening things outside the app, `<nav>` with a label, one `<h1>` per view, headings without gaps.
- Targets: 40 × 40 px minimum for all controls, list rows 48 px, Play 56 px; 8 px between separate adjacent targets (menu items and list rows are adjacent by design and do not overlap). 40 px is above the WCAG 2.2 AA minimum of 24 × 24 and close to 44 px; for a mouse-first desktop app it is the deliberate density (produkt-regelwerk W7).
- Names: every icon-only button has a name matching its tooltip. Visible text is part of the accessible name.
- State is programmatic: `aria-current="page"` (nav), `aria-current="true"` (selected row), expanded state (menus, collapsibles, combobox), checked state via the radio and checkbox primitives, busy state on lists while loading, `aria-disabled` plus `aria-describedby` for explained disabled controls, `aria-invalid` on fields.
- Announcements: toasts via Sonner's live region, ErrorPanel as alert, the launch phase as a polite live region.
- Contrast: section 2.3. State is never shown by color alone (check icons, weight, text badges, level words).
- Scaling: at 125 % and 150 % Windows display scaling the layout is identical in logical pixels; every screen is checked at both, plus window placement on a 1366 × 768 screen at 125 % (7.1). Text is in `rem` and nothing around text has a fixed height except controls whose text fits by design, so WebView zoom up to 150 % reflows without clipping: rows grow, button rows wrap, and below 880 CSS px the narrow layout applies (7.3). Windows display scaling never changes the breakpoint; only zoom does.
- Reduced motion: section 8.

## 14. Brand mark (placeholder)

The owner designs the real logo. Until then a placeholder that is plainly geometric and does not pretend to be finished:

- A rounded square, 64 × 64 viewBox, corner radius 14, filled `#1D6F80` (used in both themes, because the mark is a static file that also serves as the taskbar icon).
- On it a stylized "H" in `#F6FBFC` from three rectangles with square ends: left bar x 18, y 16, 8 × 32; right bar x 38, y 16, 8 × 32; crossbar x 26, y 28, 12 × 8.
- No gradient, no shadow, no horse, no pixel art, nothing from the game.

Where it appears: the sidebar brand row (24 px), About (48 px), the window, taskbar and tray icon, and the installer icon.

Swap: the folder `apps/desktop/brand/` is the only place to change: replace `apps/desktop/brand/mark.svg` and regenerate the icons (`apps/desktop/brand/README.md`, Task 3). The frontend imports the mark only from that file.

## 15. Not like this

The tells an app like this could really show, and what Hengst Client does instead:

1. **The stock admin shell.** Logo top left, a sidebar of five to eight equal entries, a header with a page title, a search box and an avatar, a grid of stat cards. Instead: at most four destinations, no global search, no stats, the account shown as a player name chip, and the start page built around one action and the real launch progress.
2. **The default shadcn look.** Zinc grays, `rounded-md` on everything, a soft ring of 50 % opacity for focus, `shadow-sm` under every input, ghost buttons whose hover is the accent. Instead: the remapped tokens (2.4), two radii plus pills, a 2 px focus outline, flat level 0, and the gate test that keeps the defaults out.
3. **Launcher chrome.** Banner art and news feeds, blurred game screenshots behind the Play button, a shop or "featured" tile, a friends list, a gradient or glowing Play button. Instead: a flat Play button on `--surface` and nothing above it but the instance name.
4. **Instance cards with cover images.** Instead: a 48 px list with columns.
5. **Dashboard decoration.** Status dots before every row, pulsing "online" indicators, colored icon tiles, counters in pills. Instead: one `--success` dot next to "Läuft seit", text badges only where a state needs explaining.
6. **Accent everywhere.** Teal checkboxes, teal links, teal progress, teal badges, teal selected rows. Instead: the accent on three kinds of element (2.2), neutral everything else.
7. **Spinner pages and fade-in sections.** A centered spinner while data loads, list items that slide in, hover scale on rows. Instead: static skeletons after 200 ms and motion only on popovers, dialogs and the Play press (section 8).
8. **Template copy.** "Welcome back, Mira", "Let's get you playing", exclamation marks, "Oops, something went wrong". Instead: the view name as the title and errors that say what happened and what to do.
9. **Meta strings and chrome typography.** "1.21.11 · Fabric · 2 days ago", uppercase column labels, "→" on buttons. Instead: columns or separate spans, sentence case, chevrons only on menu and select triggers.
10. **Anything that borrows the game's look or another launcher's**: blocky fonts, dirt or grass textures, pixel icons, skin heads in the account chip. Instead: the system font, line icons and an initial circle.

## 16. Decisions

| Decision | Reason | Rule that decided |
|---|---|---|
| shadcn/ui on Base UI; shadcn variables declared on `:root` pointing at our tokens, except `--accent`; shadcn's hover role only as the `accent` utility; gate test and lint ban | Real design system for app UI; raw `var(--popover)` and `var(--radius)` in generated components must resolve; the `--accent` name collides | design-unslop mode A, plan "UI-Bibliothek" and Task 3 test |
| `--border-control` for control boundaries, `--border` and `--border-strong` decorative | Control boundaries must reach 3:1; separators need not | plan Task 1, WCAG 1.4.11, produkt-regelwerk A4 |
| Accent on three element kinds only | One signature, used for the next action | design-unslop F7, F8 |
| Neutral `--selected` | Selected rows and segments are not accent elements; selection is shown by fill plus check | design-unslop F7, produkt-regelwerk Z6 |
| Dark `--danger` `#F4756B`, explicit hover and pressed danger tokens | `#F0645A` failed 4.5:1 on `--hover` and `--selected` | produkt-regelwerk A4 |
| System fonts (Segoe UI Variable, Cascadia Mono) | Windows-only app that should feel native; nothing bundled or fetched | plan Task 1 |
| Weights 400 and 600 only | Hierarchy through size and color | design-unslop hard rules |
| 40 px controls, 48 px rows, 56 px Play | Mouse-first desktop density; above the 24 px WCAG minimum | produkt-regelwerk T1 with W7 |
| Breakpoint 1200 px, narrow layout below 880 CSS px, one-row log toolbar from 1280 px, zoom guaranteed to 150 % | The row grid needs the room; content width must not jump down at the collapse | produkt-regelwerk A7, T10 |
| Window size clamped to the work area | The app must fit 1366 × 768 at 125 % | produkt-regelwerk T10 |
| No cards; list rows and background steps | Avoids the card kit and keeps the launcher quiet | frontend-design pattern 4, design-unslop B7 |
| Ease-out `cubic-bezier(0.23, 1, 0.32, 1)` instead of the draft `cubic-bezier(0.2, 0, 0, 1)` | The draft curve starts flat and delays the first movement | produkt-regelwerk M3 and W4, emil-design-eng |
| No animation for keyboard-opened layers and view switches | Frequent actions must feel instant | emil-design-eng, produkt-regelwerk M1 |
| Static skeletons, no pulse | Calm tool; loading is short and local | design-unslop M1, produkt-regelwerk L1, L2 |
| Confirmation dialog for delete, no undo | Undo is the rule for reversible deletion (Z5), but the Recycle Bin cannot be guaranteed (size limit, drives without a bin); if moving to the bin fails, the app shows an error and never deletes permanently | produkt-regelwerk Z5 (dialog with named buttons for actions that may not be reversible) |
| Confirmation dialog for sign-out | Removing stored credentials cannot be undone from the app (signing in again is the way back) | produkt-regelwerk Z5 |
| Sentence case in English too | One style in both languages | plan "Sprache und Ton" over WIG title case (W15) |
| Settings without a primary button | Settings save on change | produkt-regelwerk N1 concerns competing actions |
| Log search label left of the field | Visible label without another toolbar row | produkt-regelwerk FO1 |
| Instance first, account second on first run | Creating an instance works offline and without Mojang approval | produkt-regelwerk N7 (first success without detour) |
| stitch-design-taste used only for the document shape | Its Inter ban, perpetual motion, springs and hero rules target marketing pages | design-unslop mode A and produkt-regelwerk M1/M2 are the specific rules for app UI |

## 17. Review notes

Checked against `frontend-design`: none of the five default patterns applies (no cream and terracotta, no near-black with neon, no broadsheet, no card kit with one radius and gray shadows, no uppercase eyebrows, middle-dot meta strings, mono data labels or "→" chrome; mono is used for log output, paths and codes, which are data). "Remove one accessory": instance cards, a status dot per row, and the accent on checkboxes, links, progress and badges were removed.

Checked against produkt-regelwerk: WD9 holds (no pure black or white surface or text); M2 holds (120 to 220 ms, exits 25 to 27 % shorter); M3 holds after replacing the draft curve; M7 is specified; A1 holds with the inset outline for keyboard-highlighted items; A4 holds for every pair in 2.3, after `--border-control` and the dark danger change; E3 is covered by offline states in the version picker, sign-in and launch.

## Appendix A. Notes for implementers

Not normative design; how the above is meant to be built, and what must be verified on the machine.

- **Keyboard versus pointer:** one way to suppress transitions for keyboard-opened layers is a flag on `<html>` (pointer-down sets `data-input="pointer"`, keydown sets `data-input="keyboard"`) with transitions set to `none` under the keyboard flag. The same flag drives the inset outline on highlighted items (section 9).
- **Click guards:** the guard sits before the first `await`, not only on a `disabled` prop set in the next render (produkt-regelwerk Z2).
- **Header border:** detect scrolling with a sentinel element and IntersectionObserver, not a scroll listener.
- **Dark variant:** `@custom-variant dark (&:where(.dark, .dark *));` (as verified in the plan); the shadcn page shows `(&:is(.dark *))`, either matches `.dark` on `<html>`.
- **WebView defaults in release builds (Task 3; check the exact Tauri 2.12 and WebView2 option names):** disable the default context menu everywhere except text inputs and the log area, which keep a native cut, copy, paste and select-all menu; disable the browser accelerator keys F5, Ctrl+R, Ctrl+P, F12 and the built-in find bar (Ctrl+F belongs to the log search); enable the zoom keys (Ctrl+Plus, Ctrl+Minus, Ctrl+0) for section 13.
- **Title bar theme:** set the window theme at creation and again on every resolved-theme change (the plan's `startup.rs` covers creation; the runtime change needs the window theme setter, name to be checked).
- **Window size clamping:** the plan's `window_state.rs` restores size and position; it applies the clamps in 7.1 against the current monitor's work area.
- **Tabular figures:** check once in agentfenster on Windows 10 (Segoe UI) and Windows 11 (Segoe UI Variable) that `tabular-nums` takes effect; Segoe UI's digits are fixed-width by default, so the result should look the same either way.
- **shadcn verification:** the Task 3 duties in 2.4 list every generated-component detail known from memory of the template; confirm each against the files `shadcn add` actually writes.
- **Backend names:** the Play states map to the plan's `play`, `play_cancel`, `game_stop`, `game_force_stop` and `AppEvent::GameState`; "Mehr erfahren" maps to `UrlTarget::SignInHelp`, "Xbox-Profil anlegen" to `XboxSignup`, "Profil anlegen" to `MinecraftProfile`, "microsoft.com/link öffnen" to `DeviceLogin`, "Anleitung zum Bauen" (no-client-id block, 11.6) to `BuildHelp`.
