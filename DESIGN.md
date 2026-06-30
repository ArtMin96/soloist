---
name: Soloist
description: A calm, native Linux process-supervisor and agent-coordination workspace — status is the heartbeat.
colors:
  azure-accent: "oklch(0.55 0.13 245)"
  azure-accent-foreground: "oklch(0.985 0 0)"
  cool-white-bg: "oklch(0.99 0.002 250)"
  cool-surface: "oklch(0.972 0.004 250)"
  cool-surface-raised: "oklch(0.955 0.005 250)"
  slate-ink: "oklch(0.205 0.01 255)"
  slate-muted: "oklch(0.515 0.012 255)"
  hairline-border: "oklch(0.90 0.005 255)"
  signal-running: "oklch(0.58 0.15 150)"
  signal-transition: "oklch(0.62 0.13 70)"
  signal-stopped: "oklch(0.55 0.008 255)"
  signal-crashed: "oklch(0.55 0.20 27)"
  signal-exhausted: "oklch(0.47 0.19 22)"
typography:
  headline:
    fontFamily: "Geist Variable, system-ui, sans-serif"
    fontSize: "1.125rem"
    fontWeight: 600
    lineHeight: 1.3
    letterSpacing: "-0.01em"
  title:
    fontFamily: "Geist Variable, system-ui, sans-serif"
    fontSize: "0.9375rem"
    fontWeight: 550
    lineHeight: 1.35
    letterSpacing: "-0.005em"
  body:
    fontFamily: "Geist Variable, system-ui, sans-serif"
    fontSize: "0.8125rem"
    fontWeight: 400
    lineHeight: 1.45
    letterSpacing: "normal"
  label:
    fontFamily: "Geist Variable, system-ui, sans-serif"
    fontSize: "0.6875rem"
    fontWeight: 550
    lineHeight: 1.2
    letterSpacing: "0.01em"
  data:
    fontFamily: "Geist Mono Variable, ui-monospace, monospace"
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
    backgroundColor: "{colors.azure-accent}"
    textColor: "{colors.azure-accent-foreground}"
    rounded: "{rounded.md}"
    padding: "6px 12px"
    typography: "{typography.title}"
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.slate-ink}"
    rounded: "{rounded.md}"
    padding: "6px 10px"
    typography: "{typography.title}"
  sidebar-row:
    backgroundColor: "transparent"
    textColor: "{colors.slate-ink}"
    rounded: "{rounded.md}"
    padding: "4px 8px"
    typography: "{typography.body}"
  sidebar-row-selected:
    backgroundColor: "{colors.azure-accent} @ 15% (macOS source-list tinted selection)"
    textColor: "{colors.slate-ink}"
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
primary action; and saturated color is *spent only on status* — running, transitioning,
stopped, crashed, exhausted. Color, here, is not decoration: a saturated hue on screen
means a process is in a state you might need to act on. Density is earned through
hierarchy, hairline dividers, and a compact type scale — never through cards-everywhere.

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
app. Two pragmatic departures keep it honest on Ubuntu: **no liquid-glass / vibrancy** (the
window isn't translucent over the desktop), and the **window controls stay top-right**
(restyled), where a Linux/GNOME user expects them — not faked traffic lights on the left.

**Motion answers interaction the AppKit way — spring, not fade.** Every state change is carried
by native-feeling spring physics: a selection settles, a segmented thumb glides to its tab, a
disclosure unfolds by height, a sheet pops in. It is crisp (~180–240 ms) and effectively
overshoot-free — felt, never waited on, never decorative — and always degrades to instant under
`prefers-reduced-motion`. A cross-fade is reserved for the rare appear/disappear of incidental
chrome; it is never the default transition.

**Key Characteristics:**
- Near-monochrome cool-slate surface; saturated color reserved for process status.
- One azure accent, ≤10% of any screen, for focus / selection / the primary action only.
- Compact fixed type scale (13px body), single family + a mono companion for terminal/data.
- Flat by default; depth from tonal layering and hairline borders, not shadows.
- Status is encoded redundantly — **shape + color + label** — never hue alone.
- Motion is native spring physics — purposeful and state-conveying, crisp and reduced-motion-safe;
  never a default cross-fade.

## 2. Colors

A faintly cool slate neutral with a single azure accent; the only saturated hues on screen
belong to process status.

### Primary
- **Azure Accent** (`oklch(0.55 0.13 245)`): The one accent. Focus rings, the current
  selection in the process tree, the single primary action in a context (Start all), and
  the active tab underline. Desaturated and calm — a Linux-desktop blue, deliberately *not*
  the shadcn-default violet (hue corrected from ~264 to 245 to kill the purple tell). In
  dark theme it lifts to `oklch(0.66 0.13 245)` for legibility on the deep surface.

### Neutral
- **Cool White** (`oklch(0.99 0.002 250)`): The content background — a true near-white with
  a whisper of cool tint, never warm paper.
- **Cool Surface** (`oklch(0.972 0.004 250)`): The second neutral layer — sidebar, toolbar,
  terminal chrome. Sits a hair below the content so panels read as structure, not cards.
- **Cool Surface Raised** (`oklch(0.955 0.005 250)`): Selected/hovered rows and inset wells.
- **Slate Ink** (`oklch(0.205 0.01 255)`): Primary text and icons. Hits ≥ 12:1 on Cool White.
- **Slate Muted** (`oklch(0.515 0.012 255)`): Secondary text — metadata, group counts,
  placeholder. Verified ≥ 4.5:1 on Cool White; never lighter, no "elegant" pale gray.
- **Hairline** (`oklch(0.90 0.005 255)`): 1px dividers and rests-state borders. Structure is
  drawn with hairlines, not boxes.

### Status (the saturated vocabulary — used nowhere else)
One token per meaningful `ProcStatus`. Each is paired with a **distinct glyph and a text
label** so status survives color blindness and a grayscale screenshot. These map 1:1 to the
closed `ProcStatus` enum so the UI can never invent a state the core didn't emit.

- **Signal Running** (`oklch(0.58 0.15 150)`) — green, glyph **● filled disc**, label
  "Running". The process is up.
- **Signal Transition** (`oklch(0.62 0.13 70)`) — amber, glyph **◐ half disc**, labels
  "Starting" / "Restarting" / "Stopping". A reversible in-flight state; controls disable
  while it holds.
- **Signal Stopped** (`oklch(0.55 0.008 255)`) — grey, glyph **○ hollow ring**, label
  "Stopped". At rest, no attention owed.
- **Signal Crashed** (`oklch(0.55 0.20 27)`) — red, glyph **✕ cross**, label "Crashed". Exited
  unexpectedly; needs a decision.
- **Signal Exhausted** (`oklch(0.47 0.19 22)`) — deep red, glyph **⚠ triangle**, label
  "Exhausted". Auto-restart gave up (10/60s). Distinct from Crashed by glyph *and* a deeper,
  more alarming red — the most severe resting state.

Dark theme lifts each status hue ~0.10–0.12 in lightness (e.g. running `oklch(0.70 0.16 150)`)
so dots clear the 3:1 graphical-contrast floor on the deep surface. *(Agent activity —
IDLE/PERMISSION/THINKING/WORKING/ERROR — extends this same shape+color+label system in Phase 7;
do not introduce a parallel status vocabulary.)*

### Named Rules
**The Spent-on-Status Rule.** Saturated color is forbidden except on a status indicator. If a
border, button, icon, or background is saturated and it is not reporting `ProcStatus`, it is
wrong — desaturate it to slate or make it the azure accent.

**The One-Accent Rule.** Azure covers ≤10% of any screen and means exactly one thing:
"focused / selected / primary." Two azure things competing for "primary" on one screen is a bug.

## 3. Typography

**Body / UI Font:** Geist Variable (with `system-ui, sans-serif`)
**Terminal / Data Font:** Geist Mono Variable (with `ui-monospace, monospace`)

**Character:** One technical, neutral grotesque carries every UI role — headings, labels,
body, controls — at multiple weights; its monospace sibling carries the terminal pane and all
tabular data (PIDs, ports, CPU/RSS, durations). Sans + mono is a *functional* pairing, not a
decorative one: mono appears only where character alignment matters.

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
- **Data** (400, 0.8125rem/13px, Geist Mono): Terminal output, PIDs, ports, metrics, durations,
  any value where digits must align.

### Named Rules
**The No-Eyebrow Rule.** Group headers and captions are small sentence-case labels, never
tiny UPPERCASE letter-spaced eyebrows. "Agents", not "A G E N T S".

**The Mono-Means-Data Rule.** The monospace face is reserved for terminal output and aligned
values. A mono UI label or button is wrong — that's terminal cosplay, not hierarchy.

## 4. Elevation

Flat by default. This is a native desktop tool, not a stack of floating web cards. Depth comes
from **tonal layering** (content → cool-surface panels → raised rows) and **1px hairline
borders**, not from shadows. Surfaces are flush and quiet at rest.

The single exception is genuinely floating UI — popovers, dialogs, the command palette, the
orphan/trust dialogs — which lift off the surface with one soft, low shadow to signal
"temporary, above everything." Nothing else casts a shadow.

### Shadow Vocabulary
- **Overlay** (`box-shadow: 0 8px 24px -8px oklch(0.2 0.02 255 / 0.18)`): Popovers, dropdowns,
  command palette. Soft, short-throw, cool-tinted.
- **Dialog** (`box-shadow: 0 16px 48px -12px oklch(0.2 0.02 255 / 0.28)`): Modal dialogs only.

### Named Rules
**The Flat-By-Default Rule.** A resting surface has no shadow. If it floats over other content
*temporarily*, it gets exactly one Overlay or Dialog shadow. A shadow on a sidebar row, a
panel, or a button is a 2014 tell — remove it.

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
- **Primary:** Azure Accent background, near-white text, `6px 12px`. One per context (the bulk
  "Start all"). Hover deepens the azure ~6% lightness; `:active` springs a subtle scale-down
  (~0.97 — a fast press-in, a smooth release), a press you feel rather than a 1px translate or a fade.
- **Ghost (default control):** Transparent, slate-ink text/icon, `6px 10px`. Hover fills with
  Cool Surface Raised. This is the workhorse — per-row ▶ / ⟳ / ■ and toolbar actions are ghost
  icon buttons, ~28px square, with a tooltip and an `aria-label`.
- **Focus:** A 2px Azure Accent ring (`outline`, 2px offset). Always visible on keyboard focus —
  keyboard operability is a product principle, not a nicety.
- **Disabled:** 40% opacity, no hover. Controls disable during a `Transition` status, never
  vanish — the row must not reflow when a process is starting.

### Toolbar / Window chrome
The unified macOS toolbar stands in for the native decorations (turned off in `tauri.conf.json`).
Leading: the **app logo + "Soloist" wordmark** as a quiet identity anchor. Trailing: the global
actions as calm bezeled/ghost toolbar buttons, a short divider, then the **window controls** —
deliberately kept **top-right** (restyled), where a Linux/GNOME user expects them, not faked
traffic lights on the left. The whole strip is a drag region except the controls; double-click
toggles maximize. The terminal and orchestration content panes wear the same `h-11` toolbar tone.
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
- **Rows:** body type, `rounded.md`, inset from the sidebar edge. Left: status indicator. Center:
  process name. Right: per-row ghost controls that **slide in** on hover/focus (never a bare fade),
  always present for the selected row, over reserved space so the name never reflows.
- **Selected:** the macOS source-list selection — an **azure-tinted rounded fill** (`primary` at
  ~15% over the sidebar), inset, not a side-stripe or a full-saturation bar. Status hues keep
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
- The interactive PTY (xterm.js) on Cool Surface chrome, Geist Mono, generous internal padding,
  full-bleed scrollback. A compact header strip names the selected process (Title type) with its
  Status Indicator and the per-process ▶/⟳/■ controls. A "Terminal | Logs" segmented control
  switches the rendered-logs view. The terminal background follows theme; output color is the
  PTY's own ANSI, untouched.

### Inputs / Fields
- **Style:** 1px Hairline border, Cool White fill, `rounded.md`, `6px 10px`, body type.
- **Focus:** Border shifts to Azure Accent + a 2px ring that eases in (~120 ms); no glow.
- **Disabled:** Cool Surface fill, muted text.

### Settings & grouped lists
Settings follow the **macOS System-Settings idiom**: a section is a quiet sentence-case label above
an **inset rounded card** whose rows are split by inset hairline dividers (label left, control
right). The global Settings overlay floats its cards on the sidebar tone so they read as cards;
inline panes (project settings) border-define them. A list of reviewable items inside a dialog uses
the **same grouped well** — one rounded, hairline-divided container, not a stack of separately
bordered cards.

### Dialogs (trust review, orphan resolution)
- Centered modal on a dim cool backdrop, Dialog shadow, `rounded.lg`. They **present** with a
  spring pop (scale + fade, ~300 ms) and dismiss faster; a centered modal never slides (a translate
  would fight its centering). Headline + body type; the diff/command detail in Data (mono);
  reviewable items in a grouped well (above). Actions right-aligned: one Primary + Ghost
  alternatives. Modals are reserved for genuine decisions (trust, orphan) — not for flow.

## 6. Do's and Don'ts

### Do:
- **Do** spend saturated color *only* on a Status Indicator; everything else is slate or the one
  azure accent (The Spent-on-Status Rule).
- **Do** encode every status with **glyph + color + label** so it survives color blindness and a
  grayscale screenshot — the color-blind-safe encoding confirmed for Phase 5.
- **Do** keep the azure accent to ≤10% of a screen and to one meaning: focused / selected / primary.
- **Do** draw structure with 1px hairlines and tonal layering; keep resting surfaces flat.
- **Do** use Geist Mono *only* for terminal output and aligned data (PIDs, ports, metrics).
- **Do** disable (40% opacity) controls during a Transition status; never let a row reflow.
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
- **Don't** use `background-clip: text` gradient text, decorative glassmorphism, or a shadow on any
  resting surface.
- **Don't** encode status by hue alone, ever — drop the glyph and the design has failed its a11y bar.
- **Don't** reach for a modal when an inline/progressive affordance works; modals are for genuine
  decisions (trust, orphan) only.
- **Don't** use a cross-fade as the default transition, or `transition-opacity` where a thing
  should move — a fade-everywhere reads as web, not AppKit.
- **Don't** add bounce/elastic to utilitarian motion, or a selection "pill" that travels between
  source-list rows — macOS selects **in place** (the tint transitions, the row doesn't slide).
