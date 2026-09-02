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

## 1. Overview

**Creative North Star: "The Instrument Panel"**

Soloist is the panel you glance at, not the screen you stare at. Like the gauges in a
cockpit, every reading is honest, immediate, and quiet until it isn't. A developer keeps
it open for days while a dozen processes and agents run; its job is to make their live
state legible in a half-second glance and to pull the eye *only* when something actually
changed — a crash, an agent waiting for permission, a worker going idle. The interface is
the dark glass around the instruments: it recedes, and the signal stands out.

The system is **near-monochrome by discipline**. A faintly cool slate neutral carries
almost the entire surface; one calm azure accent marks focus, selection, and the single
primary action; and saturated color is *spent on state* — a process's (running,
transitioning, stopped, crashed, exhausted), an agent waiting on you, or the repository's.
Color, here, is not decoration: a saturated hue on screen means something is in a state you
might need to act on. Density is earned through hierarchy, hairline dividers, and a compact
type scale — never through cards-everywhere.

**The palette is swappable; the discipline is not.** Those hues are **Soloist Default**, one
theme among several the app ships and any number the user can author or import (§2, *The Theme
System*). What the design system fixes is not the pigment but the **structure**: which named role
a surface reads, how little of the screen the accent may cover, that saturation is spent on state,
and that a hue is always redundant with a glyph and a word. A Dracula or Catppuccin palette is a
legitimate Soloist; a surface that authors its own color is not, in any palette. So every rule in
this document is written against a **role**, never against a hex value — and the values quoted
here describe Soloist Default rather than constraining the theme in force.

This system explicitly rejects the **generic SaaS dashboard** (no gradient hero-metric
cards, no identical icon+heading card grids, no purple gradients), the **cream/beige "AI
default"** (no warm paper background, no tiny tracked-uppercase eyebrows, no `01/02/03`
section scaffolding), the **web-app-in-a-window** (no browser chrome, no Electron bloat),
and the **toy/skeuomorphic** (no oversized radii, no heavy drop shadows). It must read as a
first-class native Linux desktop tool.

**Native-macOS chrome (on Linux).** The instrument panel wears a **macOS-faithful AppKit
shell**: a unified toolbar carrying the app identity (logo + wordmark), a **source-list
sidebar** with inset rounded selection, segmented controls, system-settings-style grouped
panels, and thin overlay scrollbars — the calm, dense, keyboard-first *feel* of a native mac
app. Two pragmatic departures keep it honest on Ubuntu: **no vibrancy** — the window is opaque,
never translucent over the desktop or the wallpaper behind it — and the **window controls stay
top-right** (restyled), where a Linux/GNOME user expects them, not faked traffic lights on the
left. Glass exists *inside* that opaque window, on the surfaces that genuinely float above the
work, and nowhere else; §4 draws the line and names the ladder.

**Motion answers interaction the AppKit way — spring, not fade.** Every state change is carried
by native-feeling spring physics: a selection settles, a segmented thumb glides to its tab, a
disclosure unfolds by height, a sheet pops in. It is crisp (~180–240 ms) and effectively
overshoot-free — felt, never waited on, never decorative — and always degrades to instant under
`prefers-reduced-motion`. A cross-fade is reserved for the rare appear/disappear of incidental
chrome; it is never the default transition.

**Key Characteristics:**
- Near-monochrome cool-slate surface; saturated color reserved for state — a process's, an agent's,
  the repository's.
- One azure accent, ≤10% of any screen, for focus / selection / the primary action only.
- Compact fixed type scale (13px body), single family + a mono companion for terminal/data.
- Every pigment comes from a **named semantic role** in the active theme; nothing is authored in a
  component. Palettes are user-swappable, user-authorable, and portable.
- Flat where it rests, glass where it floats: the working surfaces carry no shadow, and the small,
  named set of surfaces that sit *above* them get one translucent, blurred, hairlined lift.
- Status is encoded redundantly — **shape + color + label** — never hue alone.
- Motion is native spring physics — purposeful and state-conveying, crisp and reduced-motion-safe;
  never a default cross-fade.

## 2. Colors

A faintly cool slate neutral with a single azure accent; the saturated hues on screen report
state — a process's, an agent's, or the repository's — under the rule at the end of this section.
That description is **Soloist Default**. Every value below is one theme's answer to a **role**, and
the role is the part a component is allowed to know about.

### The Theme System

Color is not authored in this codebase; it is **data**. `themes/builtins/catalog.json` is the single
source of truth for every built-in palette, and it is read by *both* sides of the app — Rust embeds
it with `include_str!` at compile time, TypeScript imports the same file — so there is exactly one
copy of every value and no way for the two to drift. Six themes ship in it: **Soloist Default**,
**Poimandres**, **Catppuccin Mocha**, **Dracula**, **Tokyo Night**, and **GitHub Light**.

- **Light and dark are chosen independently.** The appearance mode (light / dark / **system**, which
  follows the OS) decides *which half* is showing; a separate stored selection decides *which theme*
  fills that half. So a user can run GitHub Light by day and Tokyo Night by night. Only Soloist
  Default publishes both halves — it carries a light base palette plus a complete `dark` variant.
  The other five publish one appearance each (four dark, one light), and selecting a theme for a half
  it does not publish is **rejected**, not silently substituted. A theme card must therefore show
  which halves it can serve, and a one-appearance theme is normal, not broken.
- **A file is a portable artifact.** Themes are **T3-compatible v1 JSON**: import from a file or
  pasted text, export to a file, copy to the clipboard, duplicate a built-in as a starting point,
  edit a custom theme in place. Built-ins are immutable — the path to changing one is *duplicate,
  then edit*. On an ID collision the user resolves it explicitly (**Keep Both** installs under a
  deterministic unique ID; **Update Existing** replaces, and is offered only when the conflict is
  with a custom theme, never a built-in).
- **Rust owns validation.** A palette is normalized and checked in the core, not the UI: hex only
  (`#RGB`, `#RGBA`, `#RRGGBB`, `#RRGGBBAA`, normalized to lowercase long form), a bounded ID / name /
  author / description, a version that must be `1`, and a rule that a theme's `variants` may not
  restate its own base appearance. A **sparse import is completed** against Soloist Default's palette
  for the matching appearance, so a file naming three roles still installs as a whole theme. What
  reaches the UI is always a complete palette, which is why no component needs a fallback.
- **Glass is not a theme role.** The `--glass-*` values are *derived* at runtime from theme roles
  plus the user's opacity setting (§4). A theme file cannot set them, and no glass role may be added
  to `catalog.json`.

### The Role Vocabulary

A palette answers **57 semantic roles**. `crates/core/src/settings/theme/colors.rs` and
`crates/app/ui/src/theme/roles.ts` hold the authoritative list — one closed enum on each side, so
adding a role is a typed change that cannot silently skip validation or the editor. The editor
groups them as **Main** (surfaces, text, borders, accent, toolbar, sidebar), **Status** (error,
warning, update — each a triple, below), and **Other** (placeholder, labels, muted icon, message
surfaces, code, and the terminal's own chrome).

Roles fall into four kinds, and **the kind is the contract**. Using a role as the wrong kind is the
single most productive source of invisible-text bugs in this app, so it is worth being blunt:

- **Fills** — a background a thing is painted *on*, and never text or an icon: `canvas`, `surface`,
  `surfaceRaised`, `surfaceOverlay`, `sidebar`, `sidebarRowHover`, `sidebarRowActive`,
  `sidebarRowSelected`, `toolbar`, `toolbarControl`, `toolbarControlHover`, `accent`, `accentSurface`,
  `secondary`, `muted`, `errorSurface`, `warningSurface`, `updateSurface`, `messageSurface`,
  `messageAction`, `codeBackground`, `terminalBackground`, `terminalSelection`.
- **Inks** — a foreground *for one named fill*, and never a background: `text` and `textMuted` on the
  canvas and its panels, `accentForeground` on `accent`, `accentSurfaceForeground` on `accentSurface`,
  `secondaryForeground` on `secondary`, `toolbarForeground`/`toolbarControlForeground` on the toolbar
  pair, `sidebarForeground`/`sidebarMutedForeground` on the rail, `messageForeground` on
  `messageSurface`, `codeForeground` on `codeBackground`, `terminalForeground` on
  `terminalBackground`. `mutedForeground`, `placeholder`, `secondaryLabel`, and `iconMuted` are the
  quieter inks for the same surfaces.
- **Lines and marks** — a 1px edge or a small graphical mark, never a text color: `border`, `input`,
  `sidebarBorder`, `toolbarBorder`, `focus`, `terminalCursor`, `terminalScrollbar`.
- **Tones** — a saturated *meaning*, worn by text and edges, **never a fill**: `error`, `warning`,
  `update`. Each has its own fill (`*Surface`) and its own ink; see the rule below for which is which.

**The Pair-The-Halves Rule.** An ink is only legible against the fill it was authored for, so it
travels with that fill and nothing else. Wire a role to the wrong half of a pair — an ink used as a
background, or a fill's ink placed on a different fill — and the text disappears in whichever theme
happens to have the opposite polarity.

This is the most expensive mistake available in this system, and it has already shipped three times in
three different places: the `--destructive` alias was fed `errorForeground` instead of the `error`
tone; the syntax theme drew its pigment from ink roles; and the glass rim-light was mixed from `text`,
which paints a dark line in a light theme. Each was invisible to whoever wrote it, because each looks
correct in exactly one of the two appearances. **Check the pairing against the palette, not against the
role name** — a `*Foreground` suffix does **not** mean "the ink for the same-named fill" everywhere.
Two consequences worth memorizing:

- `errorForeground` is the ink for `errorSurface`, and it doubles as the destructive text tone on a
  neutral surface — in Soloist Default it *is* the error red, in both themes. `warningForeground` is
  the ink for `warningSurface` **only**; it is a near-black in the light palette and would say
  nothing on the canvas.
- `updateForeground` goes the other way: it is the ink for the `update` **fill**, not for
  `updateSurface`. Check the pairing in the palette before reaching for one of these three.

**Soloist extension roles.** Beyond the 57 T3 roles, the app needs colors T3 does not name: the six
process-status hues, seven version-control marks, nine file-language marks, the modal scrim, and the
shadow ink — plus the terminal's 16 ANSI slots, its search decorations, and its inactive-selection
and scrollbar states. Those are **derived** for every theme from roles the palette already supplies
(the status hues from `accent`, `warning`, `error` and `textMuted`; the ANSI slots from the terminal
pair and the derived status hues), so a three-line imported theme gets a coherent status vocabulary
and a coherent terminal for free. A theme *may* override any of them explicitly under
`extensions.soloist`, and Soloist Default does so for its light palette. A `variants` block carries
colors only — a theme cannot ship per-appearance extension overrides, so its opposite half's
extensions are always derived.

### Contrast: what is corrected, and what is only reported

Two different mechanisms, and they must not be described as one.

- **Derived colors are corrected.** Every derived status hue, git mark, and file-language mark is
  clamped against all four sidebar-rail fills before it is used — the status and file-language marks
  to **≥3:1**, the version-control marks to **≥4.5:1** — by walking the hue toward the rail's ink
  until it clears. The terminal holds its own **4.5:1** floor at render time, against the cell
  actually behind each glyph, which covers program color the palette never chose. This half is
  mechanical: an imported palette cannot produce an unreadable status dot.
- **Author-supplied colors are reported.** `crates/app/ui/src/theme/accessibility.ts` is the
  enforcement seam for the palette itself: seven pairs (body text, muted text, accent controls, code,
  sidebar text, error messages, warning messages), each at **≥4.5:1**, surfaced live in the editor
  and computable over a whole set of themes. These are **advisory** — the editor names the failing
  pair and its measured ratio, and still lets the theme be saved. Extending the checked set means
  adding a pair there, and it is the right place for the non-text **≥3:1** floor the product commits
  to.

Never state the advisory half as a guarantee. "Soloist enforces 4.5:1" is false for an imported
theme; "Soloist corrects the colors it derives and reports on the ones it is given" is true.

### How a surface gets a color

One way, no exceptions: a **semantic token**. `theme/runtime.ts` projects the active palette onto the
document root as `--theme-*` custom properties, aliases them to the shadcn/Tailwind layer
(`--background`, `--primary`, `--border`, `--sidebar-*`…) and to the app's own
`--status-*` / `--git-*` / `--file-language-*` families, and writes only the properties whose value
actually changed. A component reads `var(--theme-…)` or the Tailwind utility bound to it —
`bg-canvas`, `text-muted-foreground`, `border-border`.

**The No-Authored-Pigment Rule.** A component may not contain a color. No hex, no `rgb()`/`hsl()`/
`oklch()` literal, no raw Tailwind palette utility (`bg-slate-800`, `text-red-500`), no `dark:`
paint variant, no named CSS color. `scripts/check-theme-colors.mjs` fails the build on all four, over
every `.ts`/`.tsx`/`.css`/`.html` file under the UI source. If a surface needs a color the palette
doesn't name, the answer is a **new role in `catalog.json`** (and both role enums), never a literal
at the call site. Third-party renderers that ship literal fallback paint (xterm, the diff viewer,
Mermaid) get a narrowly scoped adapter that re-points their variables at theme tokens.

Values below are Soloist Default's, quoted so the intent behind each role is legible. The role is
the contract; the hex is one theme's answer to it.

### Primary
- **Azure Accent** — role `accent` (`#1777b8` light, `#4299dc` dark), with `focus` set to the same
  value: the one accent. Focus rings, the current selection in the process tree, the single primary
  action in a context (Start all), and the active tab underline. Desaturated and calm — a
  Linux-desktop blue, deliberately *not* the shadcn-default violet, and never a purple. `accent` is
  a **fill**; its ink is `accentForeground` (`#fafafa` light). The softer tint a selected row wears
  is a separate fill, `accentSurface`, inked with `accentSurfaceForeground`.

### Neutral
- **Cool White** — role `canvas` (`#fbfcfd`): the content background — a true near-white with a
  whisper of cool tint, never warm paper. `surface`, `surfaceRaised`, and `surfaceOverlay` are the
  panel, raised-row, and floating-surface fills above it. In the light palette these four sit at the
  same value, so **structure is carried by hairlines and by glass, not by tonal steps**; the dark
  palette does separate them (`#0c1015` canvas → `#14181d` raised/overlay). Do not assume a visible
  tonal ladder in every theme — assume the roles.
- **Cool Sidebar** — role `sidebar` (`#f4f6f8` light, `#12161b` dark): the source-list rail, a hair
  off the content so it reads as structure rather than a card. `sidebarRowHover` is the quiet neutral
  hover fill, `sidebarRowSelected` the azure-tinted selection fill.
- **Slate Ink** — role `text` (`#14171c` light, `#f0f2f4` dark): primary text and icons. Clears 12:1
  on the light canvas.
- **Slate Muted** — role `textMuted` (`#63686e` light, `#999fa6` dark): secondary text — metadata,
  group counts. Held at ≥4.5:1 on the canvas by the editor's contrast pair; never lighter, no
  "elegant" pale gray. `placeholder`, `secondaryLabel`, `iconMuted`, and `mutedForeground` are its
  siblings for the quieter jobs.
- **Hairline** — role `border` (`#dcdee1` light, `#2b3037` dark): 1px dividers and resting borders,
  with `input` for field edges and `sidebarBorder` / `toolbarBorder` for the shell's own seams.
  Structure is drawn with hairlines, not boxes.

### Status (the app chrome's saturated vocabulary)
One extension role per meaningful `ProcStatus`. Each is paired with a **distinct glyph and a text
label** so status survives color blindness and a grayscale screenshot. These map 1:1 to the closed
`ProcStatus` enum so the UI can never invent a state the core didn't emit. Every value is
theme-derived and contrast-clamped against the rail (above); Soloist Default's light palette pins
them explicitly, and the hexes here are those.

- **Status Running** (`statusRunning`, `#1b9247`) — green, glyph **● filled disc**, label
  "Running". The process is up. Derived from the theme's `accent` rotated to a green hue.
- **Status Transition** (`statusTransition`, `#b77611`) — amber, glyph **◐ half disc**, labels
  "Starting" / "Restarting" / "Stopping". Derived from `warning`. Starting and Restarting may expose
  one Stop cancellation while the owning actor can receive it; Stopping exposes no action.
- **Status Stopped** (`statusStopped`, `#6e7276`) — grey, glyph **○ hollow ring**, label
  "Stopped". Derived from `textMuted`. At rest, no attention owed.
- **Status Crashed** (`statusCrashed`, `#cc2827`) — red, glyph **✕ cross**, label "Crashed". Exited
  unexpectedly; needs a decision. Derived from `error`, which is why the destructive tone and the
  crashed red are the same color by construction rather than by coincidence.
- **Status Exhausted** (`statusExhausted`, `#ac0024`) — deep red, glyph **⚠ triangle**, label
  "Restart limit reached". Auto-restart gave up (10/60s). Distinct from Crashed by glyph *and* a
  deeper, more alarming red — the most severe resting state.
- **Status Attention** (`statusAttention`, `#e19100`) — amber, for an agent waiting on the user.

A theme's dark palette does not restate these; they are derived from its own roles, and each is
lifted until it clears the **3:1** graphical floor against all four rail fills. *(Agent activity —
IDLE/PERMISSION/THINKING/WORKING/ERROR — extends this same shape+color+label system; do not
introduce a parallel status vocabulary.)*

### Named Rules
**The Spent-on-Status Rule.** Saturated color **reports**; it never decorates. It reports state:
`ProcStatus` and the attention an agent is waiting on, in the app chrome; version-control state — the
change a path is in, which branch is checked out, how it stands against its upstream — in the
repository surfaces. And it reports the app's own **advisories**: something failed, something needs
care, something is worth reading. If a border, button, icon, or background is saturated and reports
none of that, it is wrong: desaturate it to slate, or make it the azure accent. The test is *reports
something*, not *is a `ProcStatus`* — a saturated edge on a real advisory is the rule working, and
desaturating it loses information.

Color is spent deliberately on two jobs beside state, and they are the whole list.

The first job is the **advisory tones** — the three Status-group roles, each a
`<tone>` / `<tone>Surface` / `<tone>Foreground` triple (*The Role Vocabulary*, above). `error` is the
**destructive tone**: it marks the action that destroys and the words that say something failed, and
it is the crashed red itself, in both themes, because a failure is a failure and a sixth hue would
only invite a sixth meaning. In tokens that is `errorForeground` for the words and `errorSurface` for
the fill beneath them — the tone's ink and the tone's fill — and `statusCrashed` is derived from the
same `error` role, so the identity holds in every theme without anyone maintaining it. `warning`
carries a caution the app is raising for the user to read: the theme editor's contrast well is exactly
this, and its saturated border is the rule working, not a violation of it. `update` carries
informational chrome that is not a caution, and in practice that is the diff viewer's — hunk headers,
hunk gutter hover, the widget tooltip. In all three the bare tone is a **line or an ink and never a
fill**: wire a destructive control's background to `error` and you have painted a red button whose
label is also red. The pairing, not the hue, is what makes it read.

The second job is the **file-language marks**, which key a language in a dense tree rather than report
anything about it, and are scoped to that mark alone — a document, a control, or a panel never wears
one. Every value in all four families is a **named role in the active theme**, projected onto the
document root by the theme runtime; `index.css` binds those tokens to utilities and holds the
theme-independent tokens (radii, motion, shadow geometry, type ramp) — it holds no color of its own.

Whatever the job, the hue is redundant: a glyph, a letter, or a word carries the same fact beside it,
so a grayscale screenshot and a color-blind reader lose nothing. The branch badge is the whole rule in
one control — a branch glyph, the name, and the standing in words ("Up to date", "Local only", "Not
fetched", "2 ahead") — and a tone with no word beside it is a bug, not a shorthand.

*Color the app does not choose.* Some color on screen is content rather than report: the terminal's
16 ANSI slots (and the Settings swatch row whose whole subject is that palette), the syntax theme over
a diff or a file preview, and the low-chroma washes the diff viewer paints behind an added or removed
row. None of it is the app saying anything about a state, so none of it answers to the rule above —
what each answers to is legibility on its own surface. The ANSI slots are **derived per theme** from
that theme's terminal pair and its own derived status hues — red from `error`, green from
`statusRunning`, yellow from `warning`, blue from `accent`, magenta from the violet file-language
mark, cyan from `update`, the bright ramp emphasized toward the terminal's foreground — so the
emulator reads as one of the instruments rather than a foreign surface, in any palette. A theme may
override all sixteen explicitly. Program color the terminal cannot author gets a 4.5:1 floor against
the cell actually behind it. The **syntax theme** is projected the same way — TextMate scopes mapped
onto the active palette's own tones (`error` for keywords, `warning` for constants, `statusRunning` for
strings, `textMuted` for comments), each **clamped against `codeBackground`** before it is handed to
the highlighter, so a palette that never considered syntax still produces readable code. Its washes
stay low-chroma because the syntax colors are painted over them.

**The One-Accent Rule.** Azure covers ≤10% of any screen and means exactly one thing:
"focused / selected / primary." Two azure things competing for "primary" on one screen is a bug.

## 3. Typography

**Body / UI Font:** the AppKit UI stack — `"SF Pro Text", "SF Pro Display", -apple-system,
BlinkMacSystemFont, "Helvetica Neue", Arial, sans-serif`
**Terminal / Data Font:** Ubuntu Mono (with `"DejaVu Sans Mono", monospace`)

**Character:** One technical, neutral grotesque carries every UI role — headings, labels,
body, controls — at multiple weights; one monospace face carries the terminal pane and all
tabular data (PIDs, ports, CPU/RSS, durations). Sans + mono is a *functional* pairing, not a
decorative one: mono appears only where character alignment matters. Nothing is bundled: the sans
stack names Apple's UI families first, after the AppKit idiom the interface is drawn to, and falls
through to what the host has; the mono stack may name only families Ubuntu's own packaging installs,
because the terminal and the app shell's `--font-mono` are one requirement and may not answer it
differently. Both stacks live once, in `index.css` — never a family name in a component.

### Hierarchy
A compact, **fixed rem scale** (ratio ~1.15) — never fluid `clamp()`; this is dense product
UI viewed at a consistent DPI, not a hero page.
- **Headline** (600, 1.125rem/18px, lh 1.3): The only large text — a dialog title or empty-state
  heading. There is no hero type in this app.
- **Title** (550, 0.9375rem/15px, lh 1.35): Panel headers, the selected process name in the
  terminal header, primary buttons.
- **Body** (400, 0.8125rem/13px, lh 1.45): The default — process rows, descriptions, dialog prose.
  Prose blocks cap at 65–75ch; dense rows and tables may run denser.
- **Label** (550, 0.6875rem/11px, tracking 0.01em, **sentence case**): Sidebar group headers
  ("Agents", "Terminals", "Commands"), metadata captions, status labels. Small and quiet —
  **not** an all-caps tracked eyebrow.
- **Data** (400, 0.8125rem/13px, Ubuntu Mono): Terminal output, PIDs, ports, metrics, durations,
  any value where digits must align.

### Named Rules
**The No-Eyebrow Rule.** Group headers and captions are small sentence-case labels, never
tiny UPPERCASE letter-spaced eyebrows. "Agents", not "A G E N T S".

**The Mono-Means-Data Rule.** The monospace face is reserved for terminal output and aligned
values. A mono UI label or button is wrong — that's terminal cosplay, not hierarchy.

## 4. Elevation

**The window is opaque; the depth is all inside it.** Nothing shows through Soloist from the desktop
behind it — no vibrancy, no transparent window (§1). Within that opaque window the app maintains a
short, closed **elevation ladder**: the surfaces you work *on* are flat, and the small named set of
surfaces that sit *above* the work say so with a translucent, blurred, hairlined lift. Depth is
information — "this is temporary and above everything" — not texture.

### The Elevation Ladder

Four rungs, and a surface belongs to exactly one. The treatments live in
`crates/app/ui/src/components/ui/glass.ts` as five named constants; the values they apply are derived
in `theme/runtime.ts`. Nothing is authored per component.

- **Rung 0 — Flat.** The canvas, the toolbar, the sidebar rail, content panes, rows, cards, settings
  wells, fields at rest, and the terminal. An opaque theme role, a 1px hairline or nothing, **no
  shadow and no blur**. This is most of the app.
- **Rung 1 — Beveled control.** `outline` and `secondary` buttons, select triggers, and the
  commit/comment composers at rest — plus `ghost` buttons **only while hovered or open**. Fill
  `--glass-control-surface`, edge `--glass-border`, shadow `--glass-control-shadow`, blur `blur-md`.
  `secondary` differs from `outline` by **fill, not elevation**: it tints from its own `secondary`
  role rather than `toolbarControl`, and sits on the same rung with the same bevel.
- **Rung 2 — Floating.** Popovers, dropdown menus, context menus, tooltips (and the tooltip arrow),
  select menus, toasts, and the theme editor panel. Fill `--glass-surface`, edge `--glass-border`,
  shadow `--glass-floating-shadow`, blur `blur-xl`.
- **Rung 3 — Modal.** Modal dialogs and alert dialogs, over the theme's scrim. The same glass as
  rung 2, including the same floating shadow; what it adds is the scrim beneath it and the heavier
  `shadow-dialog` as its no-glass fallback.

Every blurred rung also carries a **1.5× backdrop saturate**, so what shows through keeps its color
rather than going milky. Blur without it reads as frosted plastic.

Three points need spelling out, because they are where the ladder is most often misread:

- **A ghost button is rung 0 at rest.** It is transparent and flat until it is hovered or its menu is
  open; only then does it acquire the hairline, the bevel, and the blur. The bevel *is* the hover
  affordance — remove it and the workhorse control of the whole app loses its feedback. This is not a
  resting shadow.
- **The primary button is not glass.** It is an opaque accent fill, and it carries
  `--glass-primary-shadow` — a lit top rim over a very short throw — so it reads as the same material
  as the beveled controls beside it without being translucent. A blur behind an opaque fill would
  cost a repaint and show nothing.
- **A full-viewport surface is never glass.** The fullscreen dialog presentation is an opaque
  `bg-background` with no shadow at all, precisely because it covers everything: there is nothing
  behind it to see through and nothing to float above.

### Shadow Vocabulary
The geometry is theme-independent and defined once; the **ink** is a theme role (`shadowInk`), so a
shadow tints with the palette instead of assuming a light one.

- **Overlay** (`--shadow-overlay: 0 8px 24px -8px var(--shadow-ink)`): the fallback lift for a rung-2
  surface where glass is unavailable. Soft, short-throw.
- **Dialog** (`--shadow-dialog: 0 16px 48px -12px var(--shadow-ink)`): the same, one weight heavier,
  for rung 3.
- **Glass control** (`--glass-control-shadow`): `inset 0 1px 0 <highlight>` over
  `0 1px 3px -1px <shadowInk>`. A lit top rim and a 3px throw — a bevel, not a drop shadow. Rung 1.
- **Glass primary** (`--glass-primary-shadow`): the same rim over `0 2px 6px -2px`. The primary
  button only.
- **Glass floating** (`--glass-floating-shadow`): the same rim over a two-layer throw
  (`0 18px 48px -20px` plus `0 6px 16px -10px`). Rungs 2 and 3.

**The highlight is mixed from the palette's light end** — the `text` ink in a dark theme, the `canvas`
in a light one — because a glass edge catches light. Mixing it from the ink in *both* directions is the
mistake that has already been made here once: in a light theme it lays a dark line along the top of
every control, above that control's own border, which reads as a smudge rather than a rim. **A rim is
light or it is absent; it is never dark.**

That rule has an honest floor. Where a theme's control plate is already pure white — GitHub Light sets
both `toolbarControl` and `canvas` to `#ffffff` — nothing mixed from the light end can be lighter than
the plate, so the rim simply does not appear. This is correct behavior, not a bug to work around: in
those themes the control is defined by its glass border and its outer shadow, and forcing a visible
rim there would mean darkening it, which is the mistake above. The edge weight is likewise capped on
purpose — one `--glass-border` recipe, the palette's `border` walked **4%** toward `text`, so a glass
edge sits just above the plain border rather than escalating past it.

### Glass Derivation

`--glass-*` are **derived tokens, not theme roles** (§2). A theme file cannot set them; the runtime
computes them per palette from three inputs — the palette, the appearance, and the user's opacity
setting. The set is **closed at ten**: four fills (`--glass-surface`, `--glass-control-surface`,
`--glass-control-hover`, `--glass-control-active`), the edge (`--glass-border`), the rim
(`--glass-highlight`), three shadows (`--glass-control-shadow`, `--glass-primary-shadow`,
`--glass-floating-shadow`), and the raw setting (`--glass-opacity`). If a treatment needs an eleventh,
that is a change to this section, not a new value at a call site.

- **Fills** are the corresponding opaque role mixed down to the opacity setting: `--glass-surface`
  from `surfaceOverlay` at the set opacity; `--glass-control-surface` from `toolbarControl` at
  **+6**; `--glass-control-hover` and `--glass-control-active` from `toolbarControlHover` at **+10**
  and **+14**, each clamped at 100%. A control is therefore always a little more solid than the panel
  it sits on, and gets more solid as it is engaged.
- **The edge** is the theme's `border` walked 4% toward its `text` — the palette's own hairline, one
  step firmer, so it survives being drawn over a blurred, tinted background.
- **Opacity is the user's**, bounded to **40–100% in steps of 5, default 80**. The bound is enforced
  in the Rust core, not the UI, and the UI mirrors the same numbers. Adjusting it rewrites only the
  `--glass-*` properties, which is why a dragged slider does not trigger the document-wide transition
  freeze a palette swap needs.

### Platform Budget

Soloist targets **WebKitGTK on Ubuntu x86_64 and nothing else** (D2), so the blur cost is a real,
single-platform number rather than an abstraction. `backdrop-filter` forces the compositor to repaint
the region beneath the blurred element every frame that region changes. Consequences, all binding:

- **Simultaneously visible blurred surfaces are a budget.** Rung 2 and 3 surfaces are transient and
  usually singular — one menu, one tooltip, one dialog. A design that leaves several blurred surfaces
  on screen at rest is over budget regardless of how it looks.
- **No full-viewport blur, no nested blur.** The one viewport-sized blur in the app is the modal
  scrim's `blur-sm`, and it exists only while a modal is open. A blurred surface inside another
  blurred surface stacks the repaint and is out.
- **A blur behind an opaque fill is pure cost.** It buys nothing and must not be added "for
  consistency."
- **Chatty processes are the real load.** The app's frame budget is already spent on terminal output
  and status fan-out; glass must not compete with it.

### Required Fallbacks

Glass is **additive by construction** — every translucent tint and every blur sits behind a
`supports-backdrop-filter:` gate, over an opaque base that is already complete on its own. The bevel
is deliberately *not* gated: the rung-1 control bevel, the `ghost` hover and open bevel, the
`secondary` bevel, and the primary button's rim all apply **unconditionally**, because a control's
edge is an affordance rather than an enhancement — losing it entirely on an engine without real blur
is a harder failure than losing translucency. Only rungs 2 and 3 gate their shadow, and they do it
safely: the gated `--glass-floating-shadow` replaces an **ungated** `shadow-overlay` /
`shadow-dialog`, so a floating surface keeps its elevation either way. Three degradations, all of
which must keep working:

- **No `backdrop-filter`:** the surface keeps its opaque role fill (`bg-popover`,
  `bg-toolbar-control`), its 1px border, and a plain shadow — rungs 2 and 3 fall back to Overlay and
  Dialog respectively, and rung 1 keeps its bevel unchanged. Nothing disappears.
- **`prefers-reduced-transparency: reduce`:** the four glass fill tokens resolve to their fully
  opaque roles and `backdrop-filter` is switched off document-wide. Shadows are deliberately
  **left alone** — elevation is not the transparency this preference is asking to reduce.
- **`prefers-reduced-motion: reduce`:** transitions and animations collapse to instant, and the modal
  scrim drops its blur along with its fade-in.

### Named Rules

**The Flat-Where-It-Rests Rule.** A surface the user works *on* — a pane, a row, a card, a settings
well, the sidebar, the toolbar, a field at rest — has no shadow and no blur. Depth there comes from a
1px hairline and the tonal roles. A surface that floats *above* the work, or a control being
engaged, gets exactly one rung of the ladder above. If you cannot name the rung, the surface is rung
0.

**The Disciplined-Glass Rule.** This app has an intentional glass system; "glassmorphism" is what
happens when it isn't a system. The four tests below separate them, and they are meant to be applied
to a diff:

1. **Derived, not authored.** The treatment comes from a `GLASS_*` constant and `--glass-*` tokens
   computed from theme roles. A component that writes its own `backdrop-filter`, its own blur radius,
   or its own translucent color literal has failed, whatever it looks like.
2. **Bounded and user-controlled.** Opacity is the user's setting inside a validated 40–100% range —
   never a hard-coded alpha.
3. **On a named rung.** It is applied to a surface on the ladder above. Expanding the set is a
   deliberate change to that table, not a per-component decision.
4. **Paired with a hairline, and legible without the blur.** The edge defines the surface; the blur
   only relaxes what's behind it. Remove the blur and the surface must still read.

A treatment that passes all four is this design system. A treatment that fails any of them is the
trope, and the fix is to route it through the system — **not** to delete glass from a surface the
system already claims. Removing a shipped `GLASS_*` treatment is a change to this document first.

**The No-Gradient-Decoration Rule.** Still absolutely out, and unchanged by any of the above:
`background-clip: text` gradient text, decorative gradient fills, a gradient *as* a surface, and
glass used where it reports nothing — a blurred hero panel, a frosted card grid, a translucent
resting pane. Glass in Soloist means "temporarily above the work" or "this control is engaged." If it
means neither, it is decoration.

## 5. Components

Earned familiarity is the bar: every control behaves like its equivalent in Linear/Raycast,
with the full state set (default, hover, focus-visible, active, disabled, selected). shadcn/ui
+ Radix primitives supply the mechanics; this section sets their dress.

Motion is one shared system, not per-screen flourish: a small set of spring easings and a
duration scale (defined once in `index.css` — the spring curves are the sampled step-response of
a critically-damped spring, so deceleration reads native) flow through these primitives, so every
surface inherits the same feel. Only `transform`, `opacity`, and a container's own `height` move;
a layout property that would shove a neighbour never does.

**The Spring-Not-Fade Rule.** Interaction is answered by movement with native spring physics — a
thing slides, settles, scales, or unfolds — never by a generic cross-fade. Fade is allowed only
for the genuine appear/disappear of incidental chrome. Bounce/elastic is forbidden on utilitarian UI.

### Buttons
- **Shape:** Crisp, lightly softened corners (6px / `rounded.md`). Never pill, never sharp.
- **Primary:** `accent` fill, `accentForeground` text, `6px 12px`. One per context (the bulk
  "Start all"). Opaque, and beveled with the glass primary shadow (§4) so it reads as the same
  material as the controls beside it. Hover deepens the fill; `:active` springs a subtle scale-down
  (~0.97 — a fast press-in, a smooth release), a press you feel rather than a 1px translate or a fade.
- **Outline (the bezeled control):** the beveled-control rung of the ladder — `toolbarControl` glass
  fill, glass hairline, glass control bevel, `6px 10px`. The fill firms up on hover and again while
  its menu is open. This is what a mac toolbar's bezeled button is, and it is the right variant for a
  control that must look pressable when nothing is hovering it.
- **Ghost (default control):** Transparent and flat at rest, `text` ink, `6px 10px`. On hover or while
  its menu is open it takes the glass hairline, the control bevel, and the blur — that bevel *is* the
  affordance (§4), so it is never a "resting shadow" to remove. This is the workhorse — per-row
  ▶ / ⟳ / ■ and toolbar actions are ghost icon buttons, ~28px square, with a tooltip and an
  `aria-label`.
- **Destructive:** `errorSurface` fill with `errorForeground` ink — the tone's own pair (§2). Never
  the bare `error` role as a background.
- **Focus:** A 2px Azure Accent ring (`outline`, 2px offset). Always visible on keyboard focus —
  keyboard operability is a product principle, not a nicety.
- **Unavailable actions are absent:** never render disabled or irrelevant controls to preserve a
  button pattern. Disabled styling (40% opacity, no hover) is reserved for a real action that is
  temporarily pending for reasons other than the process lifecycle.

### Toolbar / Window chrome
The unified macOS toolbar stands in for the native decorations (turned off in `tauri.conf.json`).
Leading: the **app logo + "Soloist" wordmark** as a quiet identity anchor. Trailing: the
**contextual strip** as calm bezeled/ghost toolbar buttons, a short divider, then the **window
controls** — deliberately kept **top-right** (restyled), where a Linux/GNOME user expects them, not
faked traffic lights on the left. The whole strip is a drag region except the controls; double-click
toggles maximize.

The contextual strip carries what the window is currently looking at and what is waiting on the
user: the **checked-out branch** with its standing against its upstream and the controls that settle
it, then the **attention count**. The branch is scoped to the project in view, because naming what the
window is on is a title bar's own job; the count is every project's, because an alert must not hide
behind whichever project happens to be selected. The strip has width the 280px rail did not — there
the branch name was the only thing left that could shrink, and it shrank to nothing — but the badge is
still the only item here that gives width up, so at the window's 960px minimum a long name truncates,
and the badge's tooltip carries it in full when it does. Each control is **absent, never empty**, when
it has nothing to report, and the divider goes with them: a strip with nothing in it draws no line
beside the window controls. The terminal and orchestration content panes wear the same `h-11` toolbar tone.
A content surface that scrolls reveals a 1px hairline under its toolbar only once content slides
beneath it (the macOS **scroll-edge** effect); the toolbar is borderless at rest.

### Segmented Control
The app's one **view-switch** vocabulary (the orchestration views, the Appearance theme switch): a
recessed muted track with the active segment **lifted to the content surface** (tonal layering, no
shadow). The active segment is a single lifted thumb that **slides** to the chosen tab — one
element translated over a fixed track (~220 ms spring-settle), so the labels never reflow. One
shared component — never a second underline-tab style competing with it. An optional count rides a
segment as a quiet **monochrome** badge (saturated hue stays on status).

### Status Indicator (signature component)
The heartbeat of the app. A small inline cluster: **glyph + dot color + text label**, reading
the `ProcStatus`→token map from §2. The glyph (●/◐/○/✕/⚠) carries state without color; the hue
reinforces it; the label names it. In the dense sidebar the label may collapse to glyph+dot
with the full label in a tooltip and on the selected-process header — but the **glyph is never
dropped**. A `Transition` state may use a slow 1.5s opacity pulse on the glyph (reduced-motion:
static). Never encode status by color alone, anywhere.

### Sidebar / Process Tree (signature component)
A macOS **source list**: an inset, rounded-selection tree the user scans at a glance. Reads
unmistakably mac-native while keeping the status vocabulary and density rules above.

- **Project header:** disclosure + project icon + **name + running count**. The name is the
  header's job and **always stays fully visible** — every project action (Start all / Restart
  running / Stop all / Orchestration / Project settings) lives in a single hover-revealed `•••`
  menu **and** the row's right-click context menu, both driven by one source so they can't
  drift. Never a row of inline buttons competing with the name for width.
- **Groups:** Three collapsible sections — Agents / Terminals / Commands — each a sentence-case
  Label header with a muted count and a disclosure chevron. Collapse state persists per project.
  The chevron rotates and the group **springs open by height** (~220 ms), rather than snapping.
- **Rows:** body type, `rounded.md`, inset from the sidebar edge. Left: lineage disclosure and the
  status indicator. Center: a truncating process name and priority-ordered metadata. Right: one
  state-correct quick action that **slides in** on hover/focus (never a bare fade), stays present
  for the selected row, and is followed by an overflow menu only when a secondary action exists.
  Trust and crash-recovery actions remain visible at rest because they require attention. All
  projections read the same action resolver; no component reinterprets `ProcStatus`.
- **Selected:** the macOS source-list selection — an **azure-tinted rounded fill** (the
  `sidebarRowSelected` role, a tint the palette authors rather than a computed alpha over the rail),
  inset, not a side-stripe or a full-saturation bar. Status hues keep
  their **full saturation** on the selection (the heartbeat must not lose contrast to it), so the
  fill stays a *tint*, never a solid accent bar with inverted text. Hover is a quiet neutral
  raised fill; selected goes blue — the macOS hover-vs-selected distinction. The tint **transitions
  in place** (~180 ms) — it does not slide between rows; macOS selects in place. When the window is
  not the key window, the tint **desaturates to neutral** (AppKit's unemphasized selection), the
  azure returning when the window regains focus.
- **Density:** ~28px row height. Tight but tappable; no card chrome around rows.
- **Scrollbars:** thin, overlay-style (a transparency of the ink, inset to a hairline rail) — a
  native-desktop signal, never heavy browser chrome.

### Terminal Pane (signature component)
- The interactive PTY (xterm.js) on the `terminalBackground` role, Ubuntu Mono, generous internal
  padding, full-bleed scrollback. A compact header strip names the selected process (Title type) with
  its Status Indicator and the same one-primary-action plus overflow controls used by the sidebar. A
  "Terminal | Logs" segmented control switches the rendered-logs view. The terminal surface, its
  cursor, selection, and scrollbar, and the full 16-slot ANSI palette are all theme roles or derived
  from them, projected as **hex** because xterm cannot parse `oklch()`. Every slot clears 4.5:1 on its
  own background bar the one whose ANSI role *is* that theme's surface tone — light
  `white`/`brightWhite`, dark `black`. Colour the palette does not choose (256-colour and truecolor
  output) is left as the PTY sent it, with a 4.5:1 readability floor the renderer applies against the
  cell actually behind it. The terminal is a **flat** surface: never blurred, never translucent.

### Inputs / Fields
- **Style:** 1px `border`/`input` hairline, `input` fill, `rounded.md`, `6px 10px`, body type. Flat at
  rest. The exception is the message composers (commit, comment), which sit on the beveled-control
  rung so they read as a control you type into rather than a well cut into the pane.
- **Focus:** Border shifts to `focus` + a 2px ring that eases in (~120 ms); no glow.
- **Disabled:** `muted` fill, muted text.

### Settings & grouped lists
Settings follow the **macOS System-Settings idiom**: a section is a quiet sentence-case label above
an **inset rounded card** whose rows are split by inset hairline dividers (label left, control
right). The global Settings overlay floats its cards on the sidebar tone so they read as cards;
inline panes (project settings) border-define them. A list of reviewable items inside a dialog uses
the **same grouped well** — one rounded, hairline-divided container, not a stack of separately
bordered cards.

### Theme Studio (signature component)
The Appearance panel is where the palette stops being a preference and becomes a thing the user
makes. It has three tiers, and each one is a deliberately different weight of commitment.

- **Choose.** A three-up card row — System / Light / Dark — where each card previews the *actual*
  palettes selected for the two halves, so the choice is shown rather than named. Below it, the
  **theme library** as a two-column grid of cards. A card previews one panel per appearance the theme
  publishes (two side by side for a paired theme, one full-width for a single-appearance theme), and
  each preview is itself the **select** control for that half: pressing the light panel makes it the
  light theme, pressing the dark panel the dark one. That is why a card is honest about scope — a
  dark-only theme simply offers no light panel to press. Name and author sit under the previews, and
  every non-primary action (Edit for a custom theme, Duplicate, Copy JSON, Export, Remove) lives in
  one `•••` menu, the same source-of-truth pattern the sidebar uses. Built-ins offer no Edit and no
  Remove, because they are immutable; Duplicate is the path in.
- **Edit.** A **non-modal floating panel**, bottom-right, on the floating glass surface (§4) —
  draggable by its header, resizable, minimizable to its title bar. Non-modal is the point: the app stays
  live behind it and every keystroke repaints the real UI, so the user is editing the thing itself
  rather than a swatch grid. Two levels: **Basic** takes exactly two colors, a background and an
  accent, and derives all 57 roles from them with one shared recipe (so the editor, the preview, the
  saved file, and the live app can never disagree); **Advanced** exposes every role, grouped
  Main/Status/Other with a filter field. Two affordances close the loop between a pixel and a role —
  hovering a role **outlines every element currently using it**, and an **Inspect** mode lets the user
  click any element in the running app to jump straight to the role that painted it. Contrast warnings
  appear inline as a warning-toned well naming each failing pair and its measured ratio, and they do
  **not** block saving (§2, *Contrast*).
- **Exchange.** Import accepts a dropped `.json` file, a chosen file, or pasted text, and reports the
  core's own rejection reason verbatim rather than a generic failure. An ID collision escalates to a
  distinct step — a plain statement of which two themes collide, then **Keep Both** or **Update
  Existing** (absent when the existing theme is a built-in). Export writes a file, Copy JSON puts the
  same bytes on the clipboard.

**Glass opacity** rides in this panel as a bounded slider: **40–100% in steps of 5, default 80**,
with a monospace readout and a reset control that disables itself at the default. The thumb and the
readout follow the pointer locally; only a released value is committed, so dragging never waits on a
durable write. 40% is a floor, not a suggestion — below it the hairline stops separating a floating
surface from what's under it.

### Dialogs (trust review, orphan resolution)
- Centered on the theme's own scrim (`overlayScrim`, itself lightly blurred while open), on the
  **modal** rung of the elevation ladder — glass fill, glass hairline, glass floating shadow, and the
  heavier Dialog shadow as its opaque fallback — `rounded.lg`. They **present** with a
  spring pop (scale + fade, ~300 ms) and dismiss faster; a centered modal never slides (a translate
  would fight its centering). Headline + body type; the diff/command detail in Data (mono);
  reviewable items in a grouped well (above). Actions right-aligned: one Primary + Ghost
  alternatives. Modals are reserved for genuine decisions (trust, orphan) — not for flow.

## 6. Do's and Don'ts

### Do:
- **Do** spend saturated color on state — a process's, an agent's, or version control's — and
  otherwise only on the destructive tone and the file-language marks; everything else is slate or the
  one azure accent (The Spent-on-Status Rule).
- **Do** encode every status with **glyph + color + label** so it survives color blindness and a
  grayscale screenshot — the color-blind-safe encoding confirmed for Phase 5.
- **Do** keep the azure accent to ≤10% of a screen and to one meaning: focused / selected / primary.
- **Do** take every color from a **named theme role**, through a semantic token — never a hex, an
  `oklch()`/`rgb()` literal, a raw Tailwind palette utility, or a `dark:` paint variant. A surface
  needing a color the palette doesn't name needs a **new role**, not a literal (The
  No-Authored-Pigment Rule).
- **Do** pair a fill with its own ink and check the pairing before reusing a `*Foreground` role;
  `errorForeground` inks `errorSurface`, `warningForeground` inks `warningSurface`, and
  `updateForeground` inks the `update` fill (The Pair-The-Halves Rule).
- **Do** draw structure with 1px hairlines and tonal layering, and keep the surfaces the user works
  *on* flat — pane, row, card, well, sidebar, toolbar, field at rest (The Flat-Where-It-Rests Rule).
- **Do** reach for the shared `GLASS_*` treatment when a surface genuinely floats above the work or a
  control is being engaged, and keep it on a named rung of the §4 ladder.
- **Do** give every translucent surface a `prefers-reduced-transparency: reduce` fallback that
  resolves to an opaque role, and keep it legible with the blur removed.
- **Do** use Ubuntu Mono *only* for terminal output and aligned data (PIDs, ports, metrics).
- **Do** omit unavailable lifecycle controls; Starting/Restarting may show only Stop cancellation,
  and Stopping shows none. Keep the trailing intent zone stable so a row never reflows.
- **Do** give every control a visible 2px Azure focus ring and full keyboard operability.
- **Do** answer interaction with native spring motion on the shared tokens — selection settles,
  segments glide, disclosures unfold, sheets pop — kept crisp (~180–240 ms) and overshoot-free
  (The Spring-Not-Fade Rule).
- **Do** give every animation a `prefers-reduced-motion: reduce` fallback (instant), and animate
  only `transform` / `opacity` / a container's own `height` so an interaction never reflows a neighbour.

### Don't:
- **Don't** build the **generic SaaS dashboard** — no gradient hero-metric cards, no identical
  icon+heading card grids, no purple gradients (PRODUCT.md anti-reference).
- **Don't** use the **cream/beige "AI default"** — no warm paper/sand/parchment background, no tiny
  tracked-UPPERCASE eyebrows, no `01 / 02 / 03` numbered section scaffolding (PRODUCT.md).
- **Don't** look like a **web-app-in-a-window** — no browser chrome, no Electron-y bloat, nothing
  that reads as "obviously a website" (PRODUCT.md).
- **Don't** go **toy / skeuomorphic** — no oversized radii (cap ~8px), no heavy drop shadows, no
  playful mascot energy (PRODUCT.md).
- **Don't** use `border-left`/`border-right` > 1px as a colored accent stripe on rows or cards.
  Selection is the macOS azure-tinted inset fill, never a side-stripe marker.
- **Don't** use `background-clip: text` gradient text, a gradient as a surface, or **undisciplined
  glass**: a `backdrop-filter`, blur radius, or translucent color authored in a component; a hard-coded
  alpha instead of the user's bounded opacity setting; a blurred or frosted surface that reports
  nothing (a hero panel, a card grid, a resting pane); a full-viewport or nested blur; or a blur behind
  an opaque fill. Soloist's glass is a derived system on a named ladder (§4) — route a treatment
  through it rather than hand-rolling one.
- **Don't** put a shadow or a blur on a surface the user works *on* — pane, row, card, settings well,
  sidebar, toolbar, a field at rest. Equally, **don't strip** the bevel from a control the ladder
  claims: a ghost button's hover bevel, an outline button's or select trigger's resting bevel, the
  primary button's rim, and a tooltip's or menu's lift are the system working, not a 2014 tell.
  Deleting one is a regression, and changing the set is an edit to §4 first.
- **Don't** encode status by hue alone, ever — drop the glyph and the design has failed its a11y bar.
- **Don't** reach for a modal when an inline/progressive affordance works; modals are for genuine
  decisions (trust, orphan) only.
- **Don't** use a cross-fade as the default transition, or `transition-opacity` where a thing
  should move — a fade-everywhere reads as web, not AppKit.
- **Don't** add bounce/elastic to utilitarian motion, or a selection "pill" that travels between
  source-list rows — macOS selects **in place** (the tint transitions, the row doesn't slide).
