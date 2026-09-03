---
name: Soloist
description: A calm, native Linux process-supervisor and agent-coordination workspace — status is the heartbeat.
# Keys below are the app's own semantic role names, and the values are the Soloist Default *light*
# palette. They are a readable reference snapshot, not the source of truth: the normative palette is
# `themes/builtins/catalog.json`, which carries all 57 roles for every built-in theme and is read by
# both Rust and TypeScript. Any value here that disagrees with that file is stale, and that file wins.
colors:
  canvas: "#fbfcfd"
  surface: "#fbfcfd"
  surfaceRaised: "#fbfcfd"
  surfaceOverlay: "#fbfcfd"
  sidebar: "#f4f6f8"
  sidebarRowHover: "#eef0f3"
  sidebarRowSelected: "#e8f2fa"
  toolbarControl: "#eef0f3"
  text: "#14171c"
  textMuted: "#63686e"
  border: "#dcdee1"
  accent: "#1777b8"
  accentForeground: "#fafafa"
  accentSurface: "#eef0f3"
  error: "#cc2827"
  errorSurface: "#fbe9e9"
  warning: "#b77611"
  update: "#1777b8"
  terminalBackground: "#fbfbfd"
  terminalForeground: "#23262c"
  statusRunning: "#1b9247"
  statusTransition: "#b77611"
  statusStopped: "#6e7276"
  statusCrashed: "#cc2827"
  statusExhausted: "#ac0024"
  statusAttention: "#e19100"
  gitBranchSynced: "#007026"
  gitBranchLocal: "#7241a0"
typography:
  headline:
    fontFamily: "SF Pro Text, SF Pro Display, -apple-system, BlinkMacSystemFont, Helvetica Neue, Arial, sans-serif"
    fontSize: "1.125rem"
    fontWeight: 600
    lineHeight: 1.3
    letterSpacing: "-0.01em"
  title:
    fontFamily: "SF Pro Text, SF Pro Display, -apple-system, BlinkMacSystemFont, Helvetica Neue, Arial, sans-serif"
    fontSize: "0.9375rem"
    fontWeight: 550
    lineHeight: 1.35
    letterSpacing: "-0.005em"
  body:
    fontFamily: "SF Pro Text, SF Pro Display, -apple-system, BlinkMacSystemFont, Helvetica Neue, Arial, sans-serif"
    fontSize: "0.8125rem"
    fontWeight: 400
    lineHeight: 1.45
    letterSpacing: "normal"
  label:
    fontFamily: "SF Pro Text, SF Pro Display, -apple-system, BlinkMacSystemFont, Helvetica Neue, Arial, sans-serif"
    fontSize: "0.6875rem"
    fontWeight: 550
    lineHeight: 1.2
    letterSpacing: "0.01em"
  data:
    fontFamily: "Ubuntu Mono, DejaVu Sans Mono, monospace"
    fontSize: "0.8125rem"
    fontWeight: 400
    lineHeight: 1.4
    letterSpacing: "normal"
rounded:
  sm: "4px"
  md: "6px"
  lg: "8px"
spacing:
  xs: "4px"
  sm: "6px"
  md: "8px"
  lg: "12px"
  xl: "16px"
components:
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.accentForeground}"
    rounded: "{rounded.md}"
    padding: "6px 12px"
    typography: "{typography.title}"
  button-outline:
    backgroundColor: "{colors.toolbarControl}"
    textColor: "{colors.text}"
    rounded: "{rounded.md}"
    padding: "6px 10px"
    typography: "{typography.title}"
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.text}"
    rounded: "{rounded.md}"
    padding: "6px 10px"
    typography: "{typography.title}"
  sidebar-row:
    backgroundColor: "transparent"
    textColor: "{colors.text}"
    rounded: "{rounded.md}"
    padding: "4px 8px"
    typography: "{typography.body}"
  sidebar-row-selected:
    backgroundColor: "{colors.sidebarRowSelected}"
    textColor: "{colors.text}"
    rounded: "{rounded.md}"
    padding: "4px 8px"
    typography: "{typography.body}"
---

# Design System: Soloist

## 0. How to use this document

Every rule below has an id (`R3.4` = section 3, rule 4), a bold sentence carrying the testable
number, and at most two lines of rationale or a check. The frontmatter above is normative token
data; a rule may restate a token's *use*, never a different value for it. A number with no source
is marked **(chosen)** — a deliberate, unmeasured default, not a fabricated fact. Section 13 is the
only place this file talks about the current codebase's compliance; every other section states the
standard, not the gap. Never write a phase number, a ticket id, or a "was X, now Y" note into a rule
— this file states what is true now, not a changelog.

**Definition of Done — tick every line for any UI change and cite the check in your report:**

1. **Contrast measured**, not eyeballed: body/label text ≥4.5:1, non-text UI (borders, icons,
   focus rings) ≥3:1 (R9.1–R9.2), against the actual theme(s) touched — at minimum Soloist Default
   light and dark.
2. **Every interaction state exists**: default, hover, focus-visible, active, selected (if
   applicable), disabled, loading (if async), error (if it can fail) — R7.1–R7.9.
3. **Keyboard path verified**: reachable by Tab/Shift+Tab or the composite's roving-tabindex arrow
   keys, operable by Enter/Space, dismissible by Escape where applicable, focus visible throughout
   (R9.3–R9.9).
4. **`prefers-reduced-motion: reduce` checked**: every new animation collapses to instant or is
   removed; nothing added animates a layout property (R8.6–R8.7).
5. **`prefers-reduced-transparency: reduce` checked**: every new translucent surface resolves to an
   opaque theme-role fill with the blur off (R3.16).
6. **Tokens only**: no hex/`rgb()`/`oklch()` literal, no raw Tailwind palette utility, no `dark:`
   variant, no ad-hoc radius/spacing value outside the scales in R3.5 / R5.1 (R2.4).
7. **No banned pattern** from §12 is present in the diff.
8. **Screenshots taken** at the minimum window size (960×480, R5.9) and at a wide window
   (≥1440px, R5.11), light and dark, so both density extremes are seen before merge.
9. **Status, if shown, is redundant**: glyph + color + label, never color alone (R9.2, R11.7).
10. **If icon-only, it has an `aria-label` and a tooltip** (R6.5).

## 1. Principles

1. **Structure over decoration.** Depth, grouping, and hierarchy come from hairlines, tonal layering,
   and the elevation ladder — never from a card, a gradient, or a drop shadow on a resting surface
   (§3).
2. **One accent, spent on meaning.** The azure accent marks focus, selection, and the single primary
   action only; every other saturated hue reports a state the user might need to act on (§2).
3. **Flat where it rests, glass only where it floats.** A surface the user works *on* is opaque and
   shadow-free; a surface that floats *above* the work gets exactly one rung of a closed, budgeted
   ladder (§3).
4. **Read like the desktop it runs on.** Type, icons, and window chrome resolve to what Ubuntu
   actually ships — never a face, a control, or a gesture borrowed from a platform this app isn't
   (§4, §6, §10).
5. **Every control tells the truth about its own state.** Default, hover, focus, active, selected,
   disabled, loading, and error are each defined, never inferred or left to opacity alone (§7).
6. **Motion confirms, it never decorates.** A state change is answered by native spring physics that
   the user feels once and reduced-motion removes entirely — never a default cross-fade, never
   motion with nothing to report (§8).
7. **Nothing is keyboard-inaccessible, and nothing is announced only visually.** Full operability and
   redundant, non-visual status are a floor, not an enhancement (§9).

## 2. Color and theme

**R2.1 — Color is data, never a component's own choice.** `themes/builtins/catalog.json` is the sole
source of every built-in palette; Rust embeds it with `include_str!`, TypeScript imports the same
file, so there is exactly one copy of every value.
*Check: a component that writes a hex/`rgb()`/`oklch()` literal, a raw Tailwind palette utility
(`bg-slate-800`), or a `dark:` variant fails `scripts/check-theme-colors.mjs`.*

**R2.2 — Six themes ship; light and dark are chosen independently.** Soloist Default, Poimandres,
Catppuccin Mocha, Dracula, Tokyo Night, GitHub Light. Appearance mode (light/dark/**system**) decides
which half shows; a separate stored selection decides which theme fills it. Only Soloist Default
publishes both halves; selecting a theme for a half it doesn't publish is **rejected**, not silently
substituted.
*A theme card must show which halves it can serve; a one-appearance theme is normal.*

**R2.3 — A theme is a portable T3-compatible v1 JSON artifact.** Import from file or pasted text,
export to file, copy to clipboard, duplicate a built-in to start editing it (built-ins are
immutable). An ID collision resolves explicitly: **Keep Both** (unique id) or **Update Existing**
(only offered against a custom theme, never a built-in).

**R2.4 — No-Authored-Pigment Rule: a component may not contain a color.** No hex, no
`rgb()`/`hsl()`/`oklch()` literal, no raw Tailwind palette utility, no `dark:` paint variant, no
named CSS color. A surface needing a color the palette doesn't name gets a **new role** in
`catalog.json` (both role enums), never a literal at the call site. Third-party renderers with
literal fallback paint (xterm, diff viewer, Mermaid) get a narrowly scoped adapter that re-points
their variables at theme tokens.

### The Role Vocabulary

A palette answers **57 semantic roles**. `crates/core/src/settings/theme/colors.rs` and
`crates/app/ui/src/theme/roles.ts` hold the authoritative list — one closed enum on each side, so
adding a role is a typed change that cannot silently skip validation or the editor. The editor
groups them as **Main** (surfaces, text, borders, accent, toolbar, sidebar), **Status** (error,
warning, update — each a triple, below), and **Other** (placeholder, labels, muted icon, message
surfaces, code, and the terminal's own chrome).

Roles fall into four kinds, and **the kind is the contract**:

- **Fills** — a background a thing is painted *on*, never text or an icon: `canvas`, `surface`,
  `surfaceRaised`, `surfaceOverlay`, `sidebar`, `sidebarRowHover`, `sidebarRowActive`,
  `sidebarRowSelected`, `toolbar`, `toolbarControl`, `toolbarControlHover`, `accent`, `accentSurface`,
  `secondary`, `muted`, `errorSurface`, `warningSurface`, `updateSurface`, `messageSurface`,
  `messageAction`, `codeBackground`, `terminalBackground`, `terminalSelection`.
- **Inks** — a foreground *for one named fill*, never a background: `text`/`textMuted` on the canvas
  and its panels, `accentForeground` on `accent`, `accentSurfaceForeground` on `accentSurface`,
  `secondaryForeground` on `secondary`, `toolbarForeground`/`toolbarControlForeground` on the toolbar
  pair, `sidebarForeground`/`sidebarMutedForeground` on the rail, `messageForeground` on
  `messageSurface`, `codeForeground` on `codeBackground`, `terminalForeground` on
  `terminalBackground`. `mutedForeground`, `placeholder`, `secondaryLabel`, `iconMuted` are the
  quieter inks for the same surfaces.
- **Lines and marks** — a 1px edge or a small graphical mark, never a text color: `border`, `input`,
  `sidebarBorder`, `toolbarBorder`, `focus`, `terminalCursor`, `terminalScrollbar`.
- **Tones** — a saturated *meaning*, worn by text and edges, **never a fill**: `error`, `warning`,
  `update`. Each has its own fill (`*Surface`) and its own ink.

**R2.5 — Pair-The-Halves Rule: an ink is legible only against the fill it was authored for.** Check
the pairing against the palette, not the role name — a `*Foreground` suffix does not mean "the ink
for the same-named fill" everywhere. `errorForeground` inks `errorSurface` *and* doubles as the
destructive text tone on a neutral surface. `warningForeground` inks `warningSurface` **only**.
`updateForeground` inks the `update` **fill**, not `updateSurface`. This exact mistake has shipped
three times already (a `--destructive` alias fed `errorForeground` instead of `error`; the syntax
theme drew pigment from ink roles; the glass rim-light was mixed from `text`, which paints a dark
line in a light theme) — check the palette every time, not the name.

### The Theme System

- **Rust owns validation.** A palette is normalized and checked in the core, not the UI: hex only
  (`#RGB`/`#RGBA`/`#RRGGBB`/`#RRGGBBAA`, normalized to lowercase long form), bounded id/name/author/
  description, version must be `1`, and `variants` may not restate its own base appearance. A
  **sparse import is completed** against Soloist Default's palette for the matching appearance, so a
  three-role file still installs as a whole theme.
- **Glass is not a theme role.** `--glass-*` values are *derived* at runtime from theme roles plus the
  user's opacity setting (§3). A theme file cannot set them; no glass role may be added to
  `catalog.json`.
- **Two contrast mechanisms, not one.** Derived colors (status hues, git marks, file-language marks)
  are **corrected**: clamped against all four sidebar-rail fills — status/file-language to **≥3:1**,
  version-control marks to **≥4.5:1** — by walking the hue toward the rail's ink. The terminal holds
  its own **4.5:1** floor at render time against the cell behind each glyph. Author-supplied colors
  (the palette itself) are **reported**: `crates/app/ui/src/theme/accessibility.ts` checks seven
  pairs at **≥4.5:1**, surfaced live in the editor, and does **not** block saving. Never state the
  advisory half as a guarantee — "Soloist enforces 4.5:1" is false for an imported theme.

**R2.6 — Status is encoded by glyph + color + label, never color alone.** Each `ProcStatus` maps 1:1
to a permanent, theme-derived hue: Running (`statusRunning`, green, **●**), Transition
(`statusTransition`, amber, **◐**, "Starting"/"Restarting"/"Stopping"), Stopped (`statusStopped`,
grey, **○**), Crashed (`statusCrashed`, red, **✕**), Exhausted (`statusExhausted`, deep red, **⚠**,
"Restart limit reached"), Attention (`statusAttention`, amber, an agent waiting on the user).
`AgentActivity` (Idle/Permission/Thinking/Working/Error) extends this same vocabulary — never a
parallel status system.
*A grayscale screenshot or a color-blind reader must lose nothing; a status word with no glyph beside
it is a bug.*

**R2.7 — Status hue never fills a whole row.** It appears as a small dot (6–8px), a text color, or a
compact badge — never a full-row or full-card tint. A row needing emphasis gets a neutral raise, not
a colored wash.

**R2.8 — One accent per view, ≤10% of the screen.** Azure means exactly one thing:
focused/selected/primary. Two azure elements competing for "primary" in one view is a bug; a second
saturated hue that isn't reporting state is also a bug — desaturate it to slate or make it the
accent.
*Check: count every azure pixel region on screen; if two unrelated controls both read "I am the
primary action," the rule is broken.*

**R2.9 — Color the app does not choose stays unanswerable to R2.6–R2.8.** The terminal's 16 ANSI
slots, the syntax theme, and diff-viewer washes are content, not a report — they answer to
legibility on their own surface, not to the status rule. ANSI slots derive per theme from the
terminal pair and the derived status hues (red←`error`, green←`statusRunning`, yellow←`warning`,
blue←`accent`, cyan←`update`); a theme may override all sixteen explicitly. Program color the palette
never chose gets a 4.5:1 floor against the cell behind it.

## 3. Surfaces, borders and elevation

### Hairline structure (flat surfaces)

**R3.1 — Every structural border is exactly 1px**, sourced from a named theme role
(`border`/`input`/`sidebarBorder`/`toolbarBorder`), never a literal, never 2px, never a sub-pixel
value.

**R3.2 — A divider is `border-t`/`border-b` inheriting the resting surface's `border` role.** Never
`border-x` as a colored accent stripe — that is banned outright (§12).

**R3.3 — Radius scale is 4px (`sm`)/6px (`md`, default)/8px (`lg`)/9999px (pill, badges only).** No
2px, no 12px, no oversized radii on controls. The code's `--radius-xl` (~10px, computed as
`--radius × 1.66`) exists for legacy reasons and is **not** a design token — nothing should reach for
`rounded-xl`.
*Check: `grep -rn "rounded-xl" crates/app/ui/src/components` should return nothing outside §13's
tracked gap.*

**R3.4 — Flat surfaces (canvas, toolbar, sidebar rail, content panes, rows, cards, settings wells,
fields at rest, the terminal) carry no shadow and no blur, ever.** Depth there is a 1px hairline and
tonal layering only. If you cannot name which elevation rung a surface is on, it is rung 0 — flat.

**R3.5 — In the light palette, several fills often share one hex** (`canvas`/`surface`/
`surfaceRaised`/`surfaceOverlay` all `#fbfcfd` in Soloist Default) — structure there is carried by
hairlines and glass, not a tonal ladder; the dark palette does separate them
(`#0c1015`→`#14181d`). Do not assume a visible tonal step exists in every theme — assume the roles,
verify the actual hex in `catalog.json` per theme before asserting a lightness delta **(chosen
guidance; recompute exact OKLCH deltas with a color tool before citing one as fact)**.

### The elevation ladder (glass)

**R3.6 — Four rungs, closed, defined in `crates/app/ui/src/components/ui/glass.ts`.** Rung 0 (Flat):
canvas, toolbar, sidebar, panes, rows, cards, wells, fields at rest, terminal — no shadow, no blur.
Rung 1 (Beveled control): `outline`/`secondary` buttons, select triggers, message composers at rest,
plus `ghost` buttons **only while hovered or open** — fill `--glass-control-surface`, edge
`--glass-border`, shadow `--glass-control-shadow`, `blur-md` (12px). Rung 2 (Floating): popovers,
dropdown/context menus, tooltips, select menus, toasts, theme editor panel — fill `--glass-surface`,
edge `--glass-border`, shadow `--glass-floating-shadow`, `blur-xl` (20px). Rung 3 (Modal): dialogs and
alert dialogs over the scrim — same glass as rung 2 plus the heavier `shadow-dialog` no-glass
fallback.

**R3.7 — A ghost button is rung 0 at rest; the hover/open bevel *is* the affordance.** Stripping it is
a regression, not a simplification.

**R3.8 — The primary button is not glass.** Opaque `accent` fill with `--glass-primary-shadow` (a lit
rim, very short throw) so it reads as the same material without paying a blur repaint for nothing
visible behind an opaque fill.

**R3.9 — A full-viewport surface is never glass.** The fullscreen dialog presentation is opaque
`bg-background`, no shadow — there is nothing behind it to show through.

**R3.10 — Every blurred rung also carries a 1.5× backdrop-saturate**, so color showing through keeps
its hue instead of going milky.

**R3.11 — Shadow geometry is theme-independent; the ink (`shadowInk`) is a theme role.**
`--shadow-overlay: 0 8px 24px -8px shadowInk` (rung 2 no-glass fallback), `--shadow-dialog: 0 16px
48px -12px shadowInk` (rung 3 fallback), `--glass-control-shadow: inset 0 1px 0 highlight, 0 1px 3px
-1px shadowInk` (rung 1), `--glass-primary-shadow` (rung-1 rim over `0 2px 6px -2px`, primary button
only), `--glass-floating-shadow` (rim over a two-layer throw `0 18px 48px -20px` + `0 6px 16px
-10px`, rungs 2–3).

**R3.12 — The rim highlight is mixed from the palette's light end only** — `text` in a dark theme,
`canvas` in a light one — never from `text` in a light theme (that paints a dark smudge above the
control's own border, a mistake already shipped once). A control plate that is already pure white
(GitHub Light: `toolbarControl` = `canvas` = `#ffffff`) legitimately shows **no** rim; that is correct,
not a bug to force.

**R3.13 — Glass fills are derived, closed at ten tokens, never authored per component:** four fills
(`--glass-surface` from `surfaceOverlay` at opacity; `--glass-control-surface` from `toolbarControl`
at **+6**; `--glass-control-hover`/`--glass-control-active` from `toolbarControlHover` at **+10**/
**+14**, each clamped 100%), the edge (`--glass-border` = `border` walked **4%** toward `text`), the
rim (`--glass-highlight`), three shadows, and `--glass-opacity` itself.

**R3.14 — Opacity is the user's, bounded 40–100% in steps of 5, default 80**, enforced in the Rust
core and mirrored in the UI. 40% is a floor: below it the hairline stops separating a floating
surface from what's under it.

**R3.15 — Simultaneously visible blurred surfaces are budgeted at 2**: one rung-2 surface plus the
modal scrim when a modal is open — never more, never both a floating panel and an unrelated second
blurred panel at rest. No full-viewport blur besides the modal scrim's `blur-sm`, no nested blur
(a blurred surface inside another blurred surface), no blur behind an opaque fill "for consistency."
*State the budget, not a paint-time estimate — no fps or millisecond figure in this file is a
measurement until it's recorded in `PROGRESS.md`. Verify with WebKit DevTools' Rendering tab: open
the surface over a chatty terminal pane and watch for dropped frames, don't guess a number.*

**R3.16 — Never blur a surface over animating or high-frequency content.** A popover/menu/tooltip
must overlay static or low-frequency (≤10 updates/sec) content — never an `xterm` pane mid-scroll or
a live log stream.

**R3.17 — Three required fallbacks, all mandatory:** (1) no `backdrop-filter` support → opaque role
fill, 1px border, plain shadow (rung 1's bevel is **not** gated — losing an affordance is worse than
losing translucency); (2) `prefers-reduced-transparency: reduce` → the four glass fill tokens resolve
to opaque roles, `backdrop-filter: none` document-wide, shadows untouched (elevation isn't the
transparency this preference reduces); (3) `prefers-reduced-motion: reduce` → transitions/animations
collapse to instant, modal scrim drops its blur with its fade.

**R3.18 — Platform baseline: WebKitGTK 2.50 is the feature floor for `backdrop-filter`.** Ubuntu
22.04 (jammy) ships 2.36.0 unpatched / 2.50.4 patched; 24.04 (noble) ships 2.44.0 unpatched / 2.52.6
patched. `backdrop-filter` has shipped since 2.29.4 (2020), so it is safe on every supported target.
Newer CSS features (§8) are measured only on 2.52.6 and must degrade cleanly on anything older.

**R3.19 — The Disciplined-Glass test, applied to every diff touching a surface's translucency:** (1)
derived from a `GLASS_*` constant and `--glass-*` tokens, never a hand-rolled `backdrop-filter` or
alpha; (2) bounded by the user's opacity setting, never a hard-coded alpha; (3) on a named rung of
this ladder; (4) legible with the blur removed — the hairline defines the surface, the blur only
relaxes what's behind it.

## 4. Typography

**R4.1 — The UI sans stack is Linux-first: `system-ui, "Adwaita Sans", "Ubuntu Sans", Cantarell,
"Noto Sans", sans-serif`.** The frontmatter above still names the Apple/`SF Pro` stack; that is a
tracked defect, not a design decision — see R13.1. `system-ui` is expected to follow the GTK font
setting inside WebKitGTK; verify the resolved face once in the running app and record it in
`PROGRESS.md`.
*Why it matters: none of the named Apple faces exist on Ubuntu, so fontconfig silently substitutes
Liberation Sans (a print-metric web font), not the desktop's actual UI face.*

**R4.2 — The mono stack is `"Ubuntu Sans Mono", "Ubuntu Mono", "DejaVu Sans Mono", monospace`.** Both
Ubuntu Sans Mono and Ubuntu Mono are present on a stock Ubuntu install (confirmed via `fc-list` on
this machine); the terminal's own font stays the user's setting. The frontmatter's mono entry
(`Ubuntu Mono, DejaVu Sans Mono, monospace`) is compatible and does not need to change, though
`Ubuntu Sans Mono` first is the closer match to the sans-stack correction above **(chosen)**.

**R4.3 — One sans family carries every UI role at multiple weights; mono is reserved for terminal
output and aligned data.** Nothing is bundled — both stacks resolve from what the host has, and both
live once, in `index.css`'s `@theme` block, never a family name in a component.

**R4.4 — Fixed rem scale, ratio ~1.15, never fluid `clamp()`** — this is dense product UI at a
consistent DPI, not a marketing page. Headline 600/1.125rem(18px)/lh 1.3 — dialog titles, empty-state
headings only. Title 550/0.9375rem(15px)/lh 1.35 — panel headers, primary buttons, the terminal's
selected-process name. Body 400/0.8125rem(13px)/lh 1.45 — the default; prose caps at 65–75ch. Label
550/0.6875rem(11px)/lh 1.2/tracking 0.01em/**sentence case** — group headers, captions, status labels.
Data 400/0.8125rem(13px)/mono/lh 1.4 — terminal, PIDs, ports, durations, anything digits must align
in.

**R4.5 — No-Eyebrow Rule: group headers are small sentence-case labels, never tracked
UPPERCASE.** "Agents", not "A G E N T S" or "AGENTS".

**R4.6 — Mono-Means-Data Rule: the mono face appears only in terminal output and aligned values.** A
mono UI label or button is terminal cosplay, not hierarchy.

**R4.7 — Minimum readable size is 11px (Label).** Never render UI text smaller. If space can't fit
11px, truncate with a tooltip rather than shrink the type.

**R4.8 — Numeric data uses `font-variant-numeric: tabular-nums` and right-aligns in columns** (PIDs,
ports, durations, metrics) so digits stack for scanning.

## 5. Layout, density and window

**R5.1 — 4px base unit.** The frontmatter's own spacing scale (`xs 4 / sm 6 / md 8 / lg 12 / xl 16`)
includes a 6px half-step for control padding (e.g. button padding `6px 10–12px`) — that is the real,
existing scale; do not additionally claim a pure 4-multiple grid that would contradict it. Layout
gutters beyond that use 4px multiples: 4, 8, 12, 16, 24, 32px.

**R5.2 — Control height scale:** icon-only 28px (`size-7`/`icon-sm`), default control 32px (`h-8`),
compact 28px (`h-7`/`sm`), dense 24px (`h-6`/`xs`, tooltip-protected contexts only), large 36px
(`h-9`/`lg`) — all confirmed in `button.tsx`'s size variants.

**R5.3 — Sidebar rail is 280px, minimum 200px** before first collapse. Content panel minimum width
300px.

**R5.4 — Toolbar/titlebar height is `h-11` (44px)**, matching the app's `h-11` toolbar tone already
shared by the terminal and orchestration content pane toolbars.

**R5.5 — Prose blocks cap at 65–75ch (~550–600px); dense rows and tables may run denser.**

**R5.6 — Sidebar row height is ~28px**, tight but tappable, no card chrome around a row.

**R5.7 — Split-pane divider: 8px hit target, 1px visual line.**

**R5.8 — Card/well internal padding is 16px; compact panes use 12px.** Sidebar row insets: 8px
left/right, 4px vertical.

**R5.9 — Minimum window size is 960×480**, set in `tauri.conf.json` (`minWidth`/`minHeight`); default
launch is 1100×720, maximized. The app has **no** enforced minimum height beyond 480 today; whether
that floor should rise toward 600px (a common desktop-minimum convention) is an **open product
decision**, not a defect — see R13.2.

**R5.10 — Three responsive breakpoints (chosen):** Narrow (<960px) — sidebar collapses, single
column. Standard (960–1440px) — sidebar + dual-pane, the default target. Wide (>1440px) — sidebar +
multi-pane or user-resizable split.

**R5.11 — Sidebar collapses below 1024px or on explicit toggle, to a 60px icon-only rail (chosen,
not yet implemented — R13.3).** Labels move to tooltip. This is a recommendation grounded in the
GNOME desktop-minimum convention (1024×600), not an existing Soloist behavior.

**R5.12 — Split panes are user-resizable and persist per project** (SQLite-backed, per the app's
durable-state split); a pane position is not lost on relaunch.

## 6. Iconography

**R6.1 — UI icons are lucide/outline only.** No filled icon variants (lucide has none), no mixing
lucide with another icon library for UI chrome. `react-icons` is reserved for file-type icons only.
The one exception is a process/agent status dot — a styled `div`, not an icon glyph.

**R6.2 — One `LucideProvider` at the app root sets `absoluteStrokeWidth` and a single stroke value;
no per-icon `strokeWidth` override.** `strokeWidth` is measured in lucide's 24-unit viewBox, so it
scales with rendered size by default — the same `strokeWidth={2}` paints ~2px at 24px, ~1.33px at
16px, and ~1.0px at 12px, which is why the app's icons currently render inconsistent stroke weight
(observed 1.0–2.0px across `size-3` through `size-6` in the current code — R13.4). `absoluteStrokeWidth`
keeps the painted stroke fixed regardless of render size. Recommended value: **1.5px (chosen)** — near
the app's 1px hairline weight; try 1.75px if 12px icons read too thin, and record the choice made.

**R6.3 — Icon size scale by context:** 12px inline-compact (`size-3`), 14px inline-with-text
(`size-3.5`), 16px toolbar/sidebar/per-row (`size-4`), 20px empty states/section headers (`size-5`),
24px default — buttons, menus, dialogs (`size-6`).

**R6.4 — Icons align to text at the x-height, not the baseline** — `inline-flex items-center`, with a
small optical nudge only if a specific glyph reads low **(chosen; spot-check by eye, not measured)**.

**R6.5 — Every icon-only button has both an `aria-label` and a hover tooltip carrying the same
text.** No exceptions — the glyph alone is not discoverable and is not a screen-reader name.

**R6.6 — No icon soup.** Every icon serves a function; a decorative icon that carries no distinct
meaning from the text beside it is clutter and is removed.

## 7. Controls and interaction states

Every control below defines its complete state set. A variant missing one of these states is
incomplete, not "fine for now."

**R7.1 — Primary button:** `accent` fill, `accentForeground` text, `6px 12px`, `rounded-md`, one per
context. Hover deepens the fill (`bg-primary/85`); active springs a scale-down to **0.97** over
`--dur-press` (90ms), release un-springs; carries `--glass-primary-shadow` (R3.8). Never bordered,
never a second one competing for "primary" in the same view.

**R7.2 — Outline button (the beveled control):** rung-1 glass fill (`GLASS_INTERACTIVE_CONTROL_SURFACE`),
1px `--glass-border`, `--glass-control-shadow`, `6px 10px`. Fill firms up on hover and while its menu
is open (`aria-expanded`/`data-state=open`). This is the control that must read pressable even at
rest.

**R7.3 — Ghost button:** transparent and flat at rest (R3.7); on hover or open it acquires the
hairline, the rung-1 bevel, and the blur — that bevel **is** the affordance. `~28px` square icon
buttons (`icon-sm`), always paired with a tooltip and `aria-label` (R6.5). This is the workhorse for
per-row and toolbar actions.

**R7.4 — Destructive button:** `errorSurface` fill, `errorForeground` ink — the tone's own pair
(R2.5), never bare `error` as a background.

**R7.5 — Focus ring, every control:** 2px solid `focus` role, 2px offset, visible **only** on
`:focus-visible` (keyboard), never on pointer click. Contrast between focused and unfocused state
≥3:1 (R9.1). Never hidden — keyboard operability is a product floor, not a nicety.

**R7.6 — Disabled state:** 40% opacity, `cursor: not-allowed`, no hover effect, `aria-disabled` (kept
in the tab sequence for discoverability in toolbars/menus) rather than `disabled` where discoverability
matters. Prefer omitting an unavailable action entirely over rendering it disabled when the action
simply doesn't exist in the current lifecycle state (e.g. Stop on an already-Stopped process).

**R7.7 — Loading state (controls):** a 1.5s opacity pulse on the affected glyph/label
(`prefers-reduced-motion: reduce` → static), the triggering control disabled (40% opacity) for the
duration.

**R7.7b — Region loading state:** a data region (list, board, roster, document) whose first read has
not landed renders a `Skeleton` stand-in of its own resting layout, at the same row heights and
gaps, so nothing shifts when the data arrives. `LoadableRegion` in `components/common` is the one
component that does this: it reveals the stand-in only after `--skeleton-delay` (150ms), so a read
that lands inside that window never flashes one; it carries `role="status"` and `aria-busy` with an
sr-only label naming what is loading ("Loading todos"), and marks the stand-in itself `aria-hidden`.
A failed first read shows the shared recovery notice with a retry. A re-read while a value is on
screen keeps that value: a region already showing data never falls back to a stand-in. Under reduced
motion the pulse is static and the delay stays, since a delay is not motion.
*A false empty state (an "empty" message rendered before the first read has resolved) or a bare
spinner standing in for a whole region is a bug, not a loading state.*

Rendered Markdown is the leaf case of the same rule. `MarkdownView` is the authoring editor held
read-only, so it costs a frame to start: it paints its prose one pass after the frame it is mounted
in, and until the editor reports its content seeded it holds one stand-in of prose lines (bars at
the body's own line pitch, as many as the text is long) with the editor building itself invisibly
beneath, so there is no blank gap and no second wait between the chunk landing and the words. The
wrapper that marks a wait (`role="status"`, `aria-busy`, the sr-only label, the `--skeleton-delay`
reveal) is `LoadingStandIn` in `components/common`, shared by `LoadableRegion` and `MarkdownView` so
a region and an inline body wait identically. A body announces itself when it is the whole of a
region ("Loading description") and stays silent when the structure around it already reads, as a
comment under its author line does, so a thread of ten bodies does not announce ten waits.

**R7.8 — Error state (fields):** `aria-invalid` shifts the border to `error` and adds a 2px
`error`-tinted ring; the fill stays `input` — never a full-field error tint, which reads as blocked
rather than invalid.

**R7.9 — Selected state (lists, trees, tabs):** the AppKit source-list idiom — an inset, rounded,
azure-tinted fill (`sidebarRowSelected`), never a side-stripe or a solid accent bar. Status hues keep
**full saturation** on the selection. The tint transitions **in place** (~180ms, `--dur-select`), it
never slides between rows. Hover (unselected) is a quiet neutral raise (`sidebarRowHover`); only a
keyboard-focused list wears the azure "emphasized" pair (`[data-selection-scope]:focus-within` in
`index.css`) — a background window desaturates its selection to neutral.

**R7.10 — Inputs:** 1px `border`/`input` role, `input` fill, flat, `rounded-md`, `h-8` (32px),
`px-2.5 py-1`. Focus: border shifts to `focus`, 2px ring eases in over ~120ms (`--dur-fast`), no glow.
Disabled: `muted` fill, muted text.

**R7.11 — Checkboxes, radios, switches:** outline frame at rest; fill appears only on
checked/selected/on. A switch's track stays outline; only the sliding indicator fills with `accent`
when on.

**R7.12 — Segmented control:** outline track, active segment **lifted** to the content surface (tonal
layering, no shadow), slides to the chosen tab as one element over the fixed track (~220ms,
`--ease-spring-settle`) — labels never reflow. One shared component app-wide, never a second
underline-tab style.

**R7.13 — Cursor rules:** `pointer` on every non-disabled button/`role=button`; `not-allowed` on
disabled controls; `default` (not `pointer`) on non-interactive text and rows.

**R7.14 — Every variant is defined via `cva`; `className` overrides layout/spacing only, never color
or border.** Application code (pages, features, orchestration panes) never defines a new color/border
combination inline — it composes `components/ui` primitives.

## 8. Motion

**R8.1 — Motion duration/easing table, sourced from `index.css`:**

| Tier | Token | Value | Easing | Used for |
|---|---|---|---|---|
| Micro | `--dur-press` | 90ms | `--ease-spring` | Button press-in |
| Micro | `--dur-ring` | 150ms | `--ease-spring` | Focus ring grow-in |
| Fast | `--dur-fast` | 120ms | `--ease-spring` | Hover/color crossfade, input focus border |
| Small | `--dur-select` | 180ms | `--ease-spring` | Selection tint (in place), press release |
| Small | `--dur-sheet-out` | 180ms | `--ease-out-quint` | Dialog/sheet dismiss |
| Medium | `--dur-control` | 220ms | `--ease-spring-settle` | Segmented thumb, switch knob, disclosure |
| Medium | `--dur-sheet` | 300ms | `--ease-spring-settle` | Dialog/sheet present |
| Loop | `--dur-shimmer` | 2200ms | linear, infinite | Working-label shimmer sweep |
| Delay | `--skeleton-delay` | 150ms | none | Wait before a loading stand-in is revealed (R7.7b) |

**R8.2 — `--ease-spring` (critically damped, no overshoot) is the default for all utilitarian
motion.** `--ease-spring-settle` (bounce 0.12, ~0.3% peak) is reserved for mechanical metaphors —
disclosure unfold, segmented thumb, switch knob. Bounce/elastic beyond that is banned on utilitarian
UI.

**R8.3 — No single state-change animation exceeds 500ms (chosen).** GNOME/Apple desktop convention;
if a duration needs to exceed it, that's a sign the interaction should be instant instead.

**R8.4 — Distance limits: ≤16–24px translate for a small enter (popovers currently use 8px,
`slide-in-from-top-2`), ≤32–48px for a medium one (chosen).** Beyond that reads as "falling from the
sky," not calm.

**R8.5 — Stagger, if ever used for a list, is ≤50ms/item with a ≤1500ms total (chosen; not currently
used anywhere in the app — process/todo lists update independently, and should continue to).**

**R8.6 — Never animate high-frequency state (process status ticks, terminal output, list reordering).**
These already update instantly; keep them that way — animating a ≥10/sec update means 60fps of motion
competing with the frame budget terminal rendering already spends.

**R8.7 — `prefers-reduced-motion: reduce` collapses every transition/animation to ~0ms**
(`index.css`'s global rule already does this); a component may add a tailored `motion-reduce:`
treatment on top but must never rely on the global rule alone for something that also needs to change
layout.

**R8.8 — Only `transform`, `opacity`, and a container's own `height` animate.** A layout property that
would shove a neighbouring element never does.

**R8.9 — WebKitGTK CSS motion feature support, measured on 2.52.6 (this machine, `CSS.supports`
inside a real WebView) — safe on 24.04, unmeasured on 22.04's 2.50.4 floor, so gate every use with
`@supports`/`data-state` fallback rather than assuming:**

| Feature | 2.52.6 | 22.04 (2.50.4) |
|---|---|---|
| `backdrop-filter` | supported | supported since 2.29.4 — safe unconditionally |
| `@starting-style` | supported | unmeasured — gate, don't rely on |
| `linear()` easing | supported | unmeasured — Soloist already avoids the native syntax and inlines sampled curves as CSS custom properties, which works regardless |
| `transition-behavior: allow-discrete` | supported | unmeasured — don't rely on it |
| `animation-timeline: scroll()` | supported | unmeasured — don't rely on it |
| `document.startViewTransition` | supported | unmeasured — CSS transitions are sufficient today |
| `overlay` property | **not supported** | **not supported** |

**R8.10 — The `overlay` CSS property is never used, on any target.** It has no support on any
measured or floor version.

## 9. Keyboard, focus and accessibility

**R9.1 — Text contrast ≥4.5:1 (WCAG 2.2 1.4.3 AA); large text (18px+ or 14px bold) ≥3:1.** Non-text
UI (focus rings, control borders, status glyphs, graphical marks ≥3px) ≥3:1 (WCAG 2.2 1.4.11 AA).
Derived colors are corrected to clear this floor mechanically; author-supplied palette colors are
reported, not enforced (R2's Theme System note) — never conflate the two when stating a guarantee.

**R9.2 — Color is never the only means of conveying information (WCAG 2.2 1.4.1).** Status pairs a
glyph, a color, and a word (R2.6); a graphical difference (shape/position) backs up anything color
alone would otherwise carry.

**R9.3 — Focus order follows DOM order, which follows visual/reading order.** Never use `tabindex >
0` to reorder — fix the DOM order instead. If truly unavoidable, document why and verify with both
keyboard and a screen reader.

**R9.4 — Composite widgets (trees, toolbars, menus, listboxes) use roving tabindex.** One child
`tabindex="0"` (the active one), the rest `tabindex="-1"`; arrow keys move focus and update which
child is active. `crates/app/ui/src/components/ui/tree.tsx` already implements this via
`@headless-tree`'s `getContainerProps`/`getProps`.

**R9.5 — Standard key bindings apply everywhere:** Tab/Shift+Tab moves between top-level components;
arrow keys navigate within a composite (vertical lists/trees: Up/Down; horizontal menus/toolbars:
Left/Right; grids: all four); Enter activates/toggles; Space toggles selection or activates a button;
Escape closes a popup/modal or cancels; Home/End jump to first/last item.

**R9.6 — Keyboard shortcuts are displayed in the UI (menu, tooltip, or a help overlay), documented,
non-conflicting with OS/AT shortcuts (avoid `Meta+*`, `Alt+F*`, Caps Lock, Insert, Scroll Lock as
modifiers), and consistent app-wide** — the same action always uses the same shortcut.

**R9.7 — Modal focus trap and restore:** on open, focus moves to a reasonable first control inside
the dialog; Tab/Shift+Tab cycle within it and never escape to the page beneath; on close, focus
returns to the element that opened it.

**R9.8 — Menus (dropdown, context) open on click or Down-arrow from a focused trigger; the first item
receives focus; arrow keys navigate (vertical or horizontal per orientation), Home/End jump to
ends, Enter/Space activates, Escape closes and returns focus to the trigger.**

**R9.9 — `:focus-visible` gates the ring to keyboard focus only** — a pointer click does not draw the
2px ring (R7.5); this reduces visual noise for mouse users while keeping keyboard users always
oriented.

**R9.10 — Process/agent status changes are announced via an ARIA live region**
(`aria-live="polite"` for routine transitions, `"assertive"` for urgent ones like restart-limit
exhaustion; `aria-atomic="true"` so the whole region, not a diff, is read). This does **not** exist
in the app today outside `components/ui` — it is a tracked gap, R13.5.

**R9.11 — Semantic HTML first:** `<button>`, `<input>`, `<select>`, `<textarea>` over `<div
role="...">` wherever a native element does the job; every interactive element carries an accessible
name via `aria-label`, `aria-labelledby`, or a visible label; correct ARIA roles (`tree`, `listbox`,
`menu`, `toolbar`, `dialog`) signal widget type. Orca is the deployment reality on Linux/WebKitGTK;
verify keyboard + Orca on any new composite widget.

**R9.12 — Tree pattern (process sidebar, file/changes trees):** `role="tree"` on the container,
`role="treeitem"` + `aria-expanded` + `aria-selected` on rows; Up/Down between siblings, Left
collapses (or moves to parent), Right expands, Enter/Space selects, Home/End jump to
first/last-visible. `components/ui/tree.tsx` implements this for the git/files trees via
`@headless-tree`; the process sidebar's own tree markup does **not** yet declare this outside
`components/ui` — tracked gap, R13.6.

**R9.13 — Listbox/menu/toolbar/tabs/combobox each follow the ARIA Authoring Practices pattern for
that role** — roving tabindex, the arrow-key orientation for that widget, and the open/close/typeahead
behavior the pattern defines. Radix primitives (already in use for popover, dropdown-menu,
context-menu, tabs, command) implement these by default; do not re-derive keyboard handling by hand
when a Radix primitive already owns it.

**R9.14 — `forced-colors` (Windows High Contrast / equivalent Linux modes) is not actively tested
today.** Where it matters, prefer `border`/`outline` over background-color-only affordances so a
forced-colors mode that strips backgrounds still shows structure.

## 10. Native window feel

**R10.1 — The titlebar strip (excluding window controls) is a drag region; double-click toggles
maximize/restore.** Window controls are explicitly **not** part of the drag region.

**R10.2 — Window controls sit top-right (restyled), where a Linux/GNOME user expects them** — never
faked macOS traffic lights on the left. Sized to the app's existing `icon-sm` control scale (28px
square, R5.2), quiet hover highlight, no red-for-close convention (that's Windows/Ubuntu-default, not
a Linux standard this app commits to).

**R10.3 — The whole toolbar strip is borderless at rest; a content pane that scrolls reveals a 1px
hairline under its toolbar only once content slides beneath it** (the macOS scroll-edge effect).

**R10.4 — A control, badge, or divider that has nothing to report is absent, never rendered
empty.** A strip with nothing in it draws no divider beside the window controls either.

**R10.5 — When the window loses focus, the emphasized (azure) selection desaturates to neutral**
(R7.9) and returns when it regains focus — the same AppKit unemphasized-selection behavior already
implemented for lists. Extending the same desaturation to the titlebar/toolbar chrome itself is a
reasonable follow-up but is not yet implemented **(chosen; not a current behavior)**.

**R10.6 — The window supports standard Linux/X11 edge-snapping and edge/corner resize (chosen,
relies on Tauri's window APIs and the window manager)**; the app does not need to reimplement this,
only avoid capturing input that would block it.

## 11. Signature components

**R11.1 — Status Indicator:** glyph + dot color + text label, reading the `ProcStatus`→token map
(R2.6). In the dense sidebar the label may collapse to glyph+dot with the full label in a tooltip and
on the selected-process header — the **glyph is never dropped**. A `Transition` state may pulse the
glyph's opacity over 1.5s (`prefers-reduced-motion`: static).

**R11.2 — Sidebar / Process Tree:** an inset, rounded-selection macOS source list. Project header:
disclosure + icon + name + running count, name always fully visible, every project action in one
hover-revealed `•••` menu and the row's right-click menu (one source, can't drift). Groups (Agents /
Terminals / Commands): sentence-case Label header, muted count, disclosure chevron that rotates and
springs the group open by height (~220ms). Rows: ~28px (R5.6), left = lineage disclosure + status
indicator, center = truncating name + metadata, right = one state-correct quick action that slides in
on hover/focus, trust/crash-recovery actions stay visible at rest. Selection follows R7.9. Scrollbars
are thin, overlay-style — never heavy browser chrome.

**R11.3 — Terminal Pane:** xterm.js on `terminalBackground`, mono type, generous padding, full-bleed
scrollback. Header strip: selected process name (Title type) + Status Indicator + one primary action
+ overflow, plus a "Terminal | Logs" segmented control (R7.12). Terminal surface, cursor, selection,
scrollbar, and all 16 ANSI slots are theme roles or derived from them, projected as hex (xterm can't
parse `oklch()`). Every slot clears 4.5:1 against the background bar whose ANSI role *is* that
theme's surface tone. The terminal is **flat** — never blurred, never translucent (R3.4).

**R11.4 — Theme Studio:** three tiers of commitment. Choose — a three-up System/Light/Dark card row
showing the actual palettes, plus a two-column theme-library grid where each preview panel is itself
the select control for that half. Edit — a non-modal floating panel (draggable, resizable,
minimizable) on the rung-2 glass surface (R3.6); Basic mode takes two colors and derives all 57
roles; Advanced exposes every role grouped Main/Status/Other with hover-to-outline and an Inspect
click-to-jump mode; contrast warnings are inline, advisory, non-blocking (R2's Theme System note).
Exchange — import/export/copy-JSON, an ID collision escalates to Keep Both / Update Existing.

**R11.5 — Glass opacity slider (in Theme Studio):** bounded 40–100% in steps of 5, default 80,
monospace readout, reset control disables itself at default. The thumb/readout follow the pointer
locally; only the released value is committed.

**R11.6 — Dialogs (trust review, orphan resolution):** centered on the theme's scrim
(`overlayScrim`, lightly blurred while open), rung-3 glass (R3.6), `rounded-lg`. Present with a
spring pop (scale + fade, ~300ms, `--dur-sheet`), dismiss faster (~180ms, `--dur-sheet-out`) — a
centered modal never slides, a translate would fight its centering. Actions right-aligned: one
Primary + Ghost alternatives. Reserved for genuine decisions (trust, orphan) — not for flow.

**R11.7 — Every signature component that shows status uses the same resolver** — no component
reinterprets `ProcStatus` on its own; the sidebar, the terminal header, and any future surface read
the one status→token map (R2.6).

## 12. Banned patterns

- **Gradient text** (`background-clip: text`), **gradient backgrounds**, **gradient hero-metric
  cards** — breaks readability and the near-monochrome discipline (§1, §2).
- **Undisciplined glass** — a `backdrop-filter`, blur radius, or translucent color authored in a
  component; a hard-coded alpha instead of the bounded opacity setting; a blurred surface that
  reports nothing (a hero panel, a frosted card grid, a resting pane); full-viewport or nested blur;
  blur behind an opaque fill (R3.19).
- **`border-left`/`border-right` > 1px as a colored accent stripe** on a row or card. Selection is the
  inset azure fill (R7.9), never a side-stripe.
- **Shadow or blur on a resting surface** — pane, row, card, settings well, sidebar, toolbar, a field
  at rest (R3.4). Equally, **stripping** a bevel the ladder claims (a ghost button's hover bevel, an
  outline button's resting bevel, the primary button's rim, a tooltip's/menu's lift) is a regression,
  not a cleanup.
- **A card-in-card nesting** — cards layered inside other cards. Keep hierarchy flat: surface, then
  content.
- **More than one primary button, or more than one accent-colored "primary" signal, per view**
  (R2.8).
- **Uppercase tracked eyebrow labels** — tiny tracked-UPPERCASE section headers (R4.5).
- **Icon soup** — icons that don't carry distinct meaning from adjacent text (R6.6).
- **A region that lies about its first read** — its empty state or a bare spinner rendered before
  that read has resolved, or a stand-in that does not mirror the region's resting layout (a generic
  placeholder box grid). The stand-in is `LoadableRegion`'s, drawn at the row heights and gaps the
  real content uses (R7.7b).
- **A cross-fade as the default transition**, or `transition-opacity` where a thing should move — a
  fade-everywhere reads as web, not native (§8, Spring-Not-Fade).
- **Bounce/elastic easing on utilitarian controls**, or a selection "pill" that travels between
  rows — selection transitions in place (R7.9, R8.2).
- **A tinted/colored background used purely to group content** where a hairline would do — grouping
  is borders, not background washes (R3.1–R3.2).
- **A modal for a decision that isn't genuine** — an inline or progressive affordance exists for
  anything short of trust/orphan-class decisions (R11.6).
- **`overlay` (CSS property)** — unsupported on every measured and floor WebKitGTK version (R8.10).
- **The generic SaaS dashboard, the cream/beige "AI default," web-app-in-a-window chrome, and toy/
  skeuomorphic styling** — the app's four confirmed anti-references (PRODUCT.md); no purple gradient,
  no warm paper background, no browser chrome, no oversized radii, no heavy drop shadows.

## 13. Known gaps in the current code

One line each, ready to become tickets. This is the only section describing current-state
compliance rather than the standard itself.

- **R4.1 violated:** `index.css` and this file's own frontmatter still declare the `SF Pro`/Apple sans
  stack; none of those faces exist on Ubuntu, so fontconfig substitutes Liberation Sans app-wide. Fix
  per R4.1; note the frontmatter block above is frozen for tooling compatibility and will keep
  showing the old stack until a deliberate follow-up updates it in lockstep with `index.css`.
- **R5.9 open decision:** window minimum height is 480px; whether it should rise toward a 600px
  desktop-minimum convention is unresolved.
- **R5.11 not implemented:** no sidebar collapse behavior exists below any viewport width today.
- **R6.2 not implemented:** no `LucideProvider` exists at the app root; no component sets
  `absoluteStrokeWidth`; icon stroke weight currently varies ~1.0–2.0px across the sizes in use
  (`size-3` through `size-6`, observed 18/27/36/7/5 uses respectively).
- **R9.10 not implemented:** no `aria-live`/`role="status"` region exists anywhere outside
  `components/ui`; process and agent status changes are not announced to screen readers.
- **R9.12 partial:** `components/ui/tree.tsx` (the git/files trees, via `@headless-tree`) implements
  tree ARIA and roving focus correctly; the process/agent sidebar's own tree markup does not declare
  `role="tree"`/`role="treeitem"` and has not been audited.
- **R3.3 violated:** `card.tsx` uses `rounded-xl` (~10px via `--radius-xl`), over the 8px radius cap.
- **R3.1 violated:** `card.tsx` uses `ring-1 ring-foreground/10` instead of a 1px `border-border`
  hairline.
- **R3.4-adjacent violated:** `card.tsx`'s `CardFooter` carries `bg-muted/50`, an extra tinted fill
  where the existing `border-t` divider already separates the section.
- **Vendored-component audit incomplete:** only `button.tsx`, `input.tsx`, `card.tsx`, `dialog.tsx`,
  `popover.tsx`, `command.tsx`, and `tree.tsx` have been checked against §3/§7 in writing this
  document; the other ~29 files under `components/ui/` have not been audited against this rule set
  and should not be assumed compliant.
- **`.impeccable/design.json` sidecar is now stale** relative to this rewrite (regenerating it is
  outside this document's scope; run `/impeccable document` to refresh it deliberately, not as a side
  effect of reading this file).
