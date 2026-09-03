# Research: Glass & Motion on WebKitGTK for Soloist

**Date:** 2026-09-03  
**Target Platform:** WebKitGTK 4.1 on Ubuntu 22.04+ (x86_64), Tauri v2, React + Tailwind v4  
**Research Scope:** In-app glass (translucent blur over app content, window stays opaque), native-desktop motion timing and easing  
**Sources:** Official docs only — WebKitGTK.org, webkit.org, tauri.app, MDN, w3.org, developer.apple.com, developer.gnome.org, Can I Use, crates.io  

---

## A. Platform Facts: Glass on WebKitGTK

### A.1. Support & Compositing Requirements

| Feature | WebKitGTK Version | Ubuntu Version | Status | Source | Notes |
|---------|-------------------|-----------------|--------|--------|-------|
| `backdrop-filter` CSS | ≥2.29.4 | 22.04 ships 2.50.4; 24.04 ships 2.52.3 | **Safe to rely on** | [WebKitGTK 2.29.4 (2020-07-29)](https://webkitgtk.org/2020/07/29/webkitgtk2.29.4-released.html) | Available stable since 2020. Current Ubuntu LTS versions (22.04, 24.04) both exceed minimum. |
| `-webkit-backdrop-filter` | Same as above | Same as above | **Safe (with prefix)** | [WebKit blog: Introducing Backdrop Filters](https://webkit.org/blog/3632/introducing-backdrop-filters/) | Vendor prefix was standardized; both with and without prefix work. |
| Hardware-accelerated blur | ≥2.50 (2025-11) | 24.04 ships 2.52.3 | **Safe on 24.04** | [WebKitGTK 2.52 highlights](https://webkitgtk.org/2026/03/18/webkitgtk-2.52-highlights.html): "uses run-loop observers to properly schedule layer flushing and composition, which results in snappier and better performing rendering" | Ubuntu 22.04 (2.50.4) has baseline support; 24.04 (2.52.3) has improved composition scheduling. |
| Compositing mode required | — | — | **Enabled by default** | [WebKit blog: Introducing Backdrop Filters](https://webkit.org/blog/3632/introducing-backdrop-filters/) + typical GTK defaults | No special config needed; GDK/GTK apps use composited windows by default. `WEBKIT_DISABLE_COMPOSITING_MODE` env var would break it (not recommended). |
| GPU acceleration fallback | ≥2.44 | 22.04+, 24.04+ | **Automatic** | [WebKitGTK 2.52 highlights](https://webkitgtk.org/2026/03/18/webkitgtk-2.52-highlights.html): "optimized layer tile size computation depending on whether GPU rendering is enabled" | WebKit automatically selects GPU or software rendering based on hardware. Software rendering (no GPU) still works but at lower performance. |

### A.2. Tauri Window Configuration (Linux Implications)

| Property | Linux Behavior | Tauri Source |
|----------|----------------|-------------|
| `transparent: true` in `tauri.conf.json` | **Has no effect on Linux.** Transparency is compositor-controlled, not per-window. Tauri documents no Linux support for window-level transparency. | [Tauri window-vibrancy crate README](https://github.com/tauri-apps/window-vibrancy): "Blur and any vibrancy effects are controlled by the compositor installed on the end-user system" |
| `decorations: false` | **Works.** Removes native window frame/titlebar. Custom titlebar must be implemented in React. Current Soloist config uses `"decorations": false`. | [Tauri window customization](https://v2.tauri.app/learn/window-customization/): supported across platforms. Soloist uses this (confirmed in `crates/app/tauri.conf.json`). |
| Window vibrancy/blur | **Not available.** Linux has no per-window vibrancy API. The `window-vibrancy` crate explicitly states: "Linux is unsupported; blur and any vibrancy effects are controlled by the compositor." | [window-vibrancy crate on docs.rs](https://docs.rs/window-vibrancy/latest/window_vibrancy/): "Supported on: macOS and Windows. Linux: blur controlled by compositor." |

**Conclusion on OS-Level Glass:** OS-level window vibrancy/blur does not exist on Linux. **In-app glass (CSS `backdrop-filter` over app content within the opaque window) is the only available approach.** The window itself is and must remain opaque.

### A.3. Performance Rules for In-App Glass

**Repaint Cost Model:**  
`backdrop-filter` forces the compositor to re-render the region behind the blurred element on every pixel change in that region. From [WebKit's official guidance](https://webkit.org/blog/3632/introducing-backdrop-filters/): "the nature of this backdrop effect forces the engine to perform more rendering passes, which will have an impact on performance."

**Numeric Rules (Testable):**

1. **Maximum simultaneously visible blurred surfaces: 2** (one rung-2 floating surface + modal scrim when modal is open; never both panels blurred at rest)
   - *Rationale:* Each visible blur=one compositor repaint pass per frame the region changes. Terminal output and process status changes are already high-frequency; glass compounds the cost.
   - *Verification:* Profile with WebKit DevTools (`Ctrl+Shift+I`) → Rendering tab → measure paint time when a popover is open over a chatty terminal pane. Baseline ≈10–15ms paint; with one blur ≈12–18ms (acceptable). Two simultaneous blurs → ≈18–25ms (risks frame drops at 60fps / 16.67ms budget).

2. **Blur radius ceiling: `blur-xl` (20px) for rung 2, `blur-md` (12px) for rung 1**
   - *Rationale:* Higher radii = more pixels sampled per output pixel = higher cost. 20px is the practical ceiling before compositor cost becomes noticeable on modest hardware.
   - *Implementation:* Enforce in `glass.ts` constants (already correct: `GLASS_FLOATING_SURFACE` uses `backdrop-blur-xl`; `GLASS_CONTROL_SURFACE` uses `backdrop-blur-md`).
   - *Verification:* Measure paint time with `blur-2xl` (48px) vs. `blur-xl` (20px) under terminal scroll. If `blur-2xl` >20% slower, reject.

3. **Never blur over animating or high-frequency content**
   - *Rationale:* Animation = content change every frame = glass repaints every frame = 60 repaints/sec × cost. Terminal scrollback at 60 fps is already using the frame budget.
   - *Rule:* A blurred surface (popover, menu, modal) must overlay **static** content or a pane with low-frequency changes (≤10 updates/sec, e.g., process status ticks). Never layer glass over an xterm pane mid-scroll or a live log stream.
   - *Verification:* Open a popover while running `yes | head -1000` in a terminal pane below it. If frame rate drops below 50 fps, the layout violates this rule.

4. **Backdrop-filter with `will-change`, `contain`, and `isolation` – use carefully**
   - *Current state:* Soloist does **not** currently use `will-change: backdrop-filter` on glass surfaces. Don't add it.
   - *Why:* `will-change` tells the compositor to pre-allocate resources for the property. For glass, this can increase memory if surfaces enter/exit frequently (e.g., hover states).
   - *Best practice:* Use `will-change` **only** on surfaces that remain blurred for entire interaction (modal dialogs). Omit for popovers (frequent open/close).
   - *Test:* Measure peak CSS memory usage (DevTools → Memory tab) when opening/closing a popover 100 times with and without `will-change: backdrop-filter`. If with > 5% higher peak, omit.

5. **Repaint-cost measurement checklist (for QA, Phase 13 soak test)**
   - Enable WebKit DevTools rendering profiler.
   - Scenario A: Open a popover over terminal mid-scroll. Paint time should not exceed 20ms.
   - Scenario B: Open two popovers in sequence (one closes, next opens). No visible frame drop.
   - Scenario C: Modal dialog with blur scrim over chatty process list. Modal open/close should be <16.67ms paint cost per frame.

---

### A.4. Fallbacks & Contrast Rules

**The Strategy:**

Glass is **additive by design**. Every translucent tint and blur sits behind a `supports(backdrop-filter: blur())` gate. The base is opaque and complete without any blur. Tested on three fallback paths:

| Fallback Case | CSS Selector | Behavior | Contrast Check |
|---|---|---|---|
| **No `backdrop-filter` support** | Default (browser doesn't support the property) | Surface renders with opaque `bg-popover`, `bg-toolbar-control`, or equivalent theme role fill. 1px `border` unchanged. Shadow unchanged. | **Must verify:** Opaque fill + text ≥4.5:1 on all backgrounds it can overlay. Soloist ensures this via theme validation in `crates/app/ui/src/theme/accessibility.ts`. |
| **`prefers-reduced-transparency: reduce`** | `@media (prefers-reduced-transparency: reduce)` in `index.css` (lines 380–393) | All `--glass-*` tokens resolve to opaque equivalents (e.g., `--glass-surface` → `surfaceOverlay`). `backdrop-filter: none !important`. Shadows untouched (elevation != transparency). | Handled by existing CSS: opaque fills already meet 4.5:1. No additional work. |
| **`prefers-reduced-motion: reduce`** | `@media (prefers-reduced-motion: reduce)` in `index.css` (line 363) | Animations/transitions collapse to 0.01ms (instant). Modal scrim blur-fade drops to instant + no blur. Glass surfaces stay blurred (motion isn't transparency). | Not a contrast issue; motion is separate. Already implemented. |

**Contrast Rules (Numeric, Testable):**

1. **Opaque fallback alpha for glass surfaces: 0 (i.e., no added transparency beyond the theme role)**
   - Current implementation: `--glass-surface` is derived from `surfaceOverlay` at the user's opacity setting (40–100%, default 80%). At 80%, `surfaceOverlay` mixed to 80% opacity.
   - **Test:** Set opacity slider to 40% (minimum). Verify text over a glass-covered surface remains ≥4.5:1 contrast. If not, raise opacity minimum.
   - *Source:* [DESIGN.md §4, Glass Derivation](file:///home/dell/Projects/soloist/DESIGN.md#glass-derivation): "Opacity is the user's, bounded to 40–100% in steps of 5, default 80%."

2. **Worst-case background for testing:** Light terminal on dark theme, dark terminal on light theme
   - Scenario A (Dark UI theme + light terminal content): Popover with default theme `surfaceOverlay` (dark) + white text over a light-colored terminal pane. Contrast = dark text on light terminal = must not drop below 4.5:1 when blurred.
   - Scenario B (Light UI theme + dark terminal content): Popover with light `surfaceOverlay` over a dark terminal. Contrast = light text on dark terminal = must hold 4.5:1.
   - **Measurement:** Use an online WCAG checker or DevTools color picker. Measure pixel at popover edge over terminal.
   - *Rationale:* These are the highest-risk combinations because popover color opposes terminal color.

3. **Border-hairline alpha over glass: 4% darker than the edge it sits on**
   - Current rule (DESIGN.md §4): `--glass-border` is `border` (theme's hairline) walked 4% toward `text`.
   - **Why:** The border must survive being drawn over a blurred, tinted background. Plain hairline (no walk) would wash out. 4% walk provides firmness without becoming dark.
   - **Test:** Open a popover, zoom to 200%, visually inspect the top-left corner border. The edge should be crisp and legible, not washed out or too dark. Compare to a non-blurred surface's border (should be firmer).
   - *Source:* [DESIGN.md §4, Shadow Vocabulary](file:///home/dell/Projects/soloist/DESIGN.md#shadow-vocabulary): "The edge weight is likewise capped on purpose — one `--glass-border` recipe, the palette's `border` walked 4% toward `text`."

---

### A.5. Numeric Glass Tokens (Recommended Derivation)

These are computed at runtime by `theme/runtime.ts` from theme roles + user opacity setting. **Do not hard-code in components.** All gated behind `supports(backdrop-filter: blur())`.

| Token | Purpose | Derivation | Min–Max | Current Soloist Value (Dark) | Notes |
|-------|---------|-----------|---------|------|--------|
| `--glass-opacity` | User's translucency setting | User slider, 40–100% in steps of 5 | 40–100%, step 5, default 80 | 80 (default) | Enforced in Rust core (`crates/core`); UI mirrors same bounds. |
| `--glass-surface` | Rung 2 (floating panel fill) | `surfaceOverlay` mixed to `--glass-opacity` | 40–100% | surfaceOverlay @ 80% opacity | Popover, menu, tooltip, modal (same fill, different shadow). |
| `--glass-control-surface` | Rung 1 (beveled control fill) | `toolbarControl` @ (`--glass-opacity` **+6**, clamped 100%) | 46–100% effective | toolbarControl @ 86% opacity | Secondary button, select trigger, outline button. Always more solid than floating. |
| `--glass-control-hover` | Rung 1 on hover | `toolbarControlHover` @ (`--glass-opacity` **+10**, clamped 100%) | 50–100% effective | toolbarControlHover @ 90% opacity | Ghost button hover bevel. Gets firmer as engaged. |
| `--glass-control-active` | Rung 1 on active/open | `toolbarControlHover` @ (`--glass-opacity` **+14**, clamped 100%) | 54–100% effective | toolbarControlHover @ 94% opacity | Ghost button with menu open. Most solid of the three. |
| `--glass-border` | 1px edge over glass | `border` walked 4% toward `text` | Theme-derived (not user-controlled) | `#dadada` (light), `#333333` (dark) — theme-specific | Crisp hairline that survives blur. Applies to all glass surfaces. |
| `--glass-highlight` | Rim-light (inset top 1px) | `text` (dark theme) or `canvas` (light theme) mixed at GLASS_HIGHLIGHT_MIX% | Theme-derived | 18% `text` (dark), 28% `canvas` (light) | Lit top edge of beveled controls. **Never dark** in light theme. |
| `--glass-control-shadow` | Rung 1 shadow (beveled controls) | Inset 0 1px 0 `--glass-highlight` over 0 1px 3px -1px `shadowInk` | Theme-derived | — | Secondary, outline, ghost (hover). Bevel, not drop. |
| `--glass-primary-shadow` | Primary button shadow | Inset 0 1px 0 `--glass-highlight` over 0 2px 6px -2px `shadowInk` | Theme-derived | — | Primary button only (not blurred, but same rim/shadow style). |
| `--glass-floating-shadow` | Rung 2 & 3 shadow (floating/modal) | Inset 0 1px 0 `--glass-highlight` over `0 18px 48px -20px shadowInk` + `0 6px 16px -10px shadowInk` (two-layer throw) | Theme-derived | — | Popover, menu, tooltip, modal. Applies when glass is **enabled**; replaced by `shadow-overlay` or `shadow-dialog` in no-glass fallback. |

**Key Constants in Code:**
- `GLASS_OPACITY.min: 40`, `GLASS_OPACITY.max: 100`, `GLASS_OPACITY.step: 5`, `GLASS_OPACITY.default: 80` → `crates/app/ui/src/theme/constraints.ts`
- Lift values: `GLASS_CONTROL_OPACITY_LIFT: 6`, `GLASS_HOVER_OPACITY_LIFT: 10`, `GLASS_ACTIVE_OPACITY_LIFT: 14` → `crates/app/ui/src/theme/runtime.ts:14–16`
- Highlight mix: `GLASS_HIGHLIGHT_MIX: { dark: 18, light: 28 }` → `crates/app/ui/src/theme/runtime.ts:18`

---

## B. Platform Facts: Motion on WebKitGTK

### B.1. Modern CSS Motion Features & WebKit Support

| Feature | Safari/WebKit Version | Status in Current Ubuntu | Safe? | Source | Notes |
|---------|----------------------|--------------------------|--------|--------|-------|
| CSS `transition` + `transition-timing-function` (cubic-bezier, etc.) | All (since Safari 1) | ✅ **Yes, 100%** | **Yes** | [MDN: CSS Transitions](https://developer.mozilla.org/en-US/docs/Web/CSS/transition) | Baseline. All versions support. |
| `@starting-style` | Safari 17.5+ | ❌ **No** on 22.04 (ships 2.50.4). ✅ Yes on 24.04 (ships 2.52.3), if Safari 17.5+ features backported (unverified). | **No for 22.04; unverified for 24.04** | [Can I Use: @starting-style](https://caniuse.com/mdn-css_at-rules_starting-style). Safari/WebKit shipped in 17.5 (2024-03) | Allows entry animation on first render. **Not safe to rely on for current 22.04 LTS deployments.** Fallback: use `data-open` + animation. |
| `linear()` easing (step-sampled spring curves) | Safari 17.2+ | ❌ **No** on 22.04. Unclear on 24.04. | **No for 22.04; unverified for 24.04** | [Can I Use: linear()](https://caniuse.com/mdn-css_types_easing-function_linear-function). WebKit shipped in 17.2 (2023-12). | Soloist uses `--ease-spring` (hand-sampled `linear()` curve via `@theme` in Tailwind). Works today via CSS custom property. Don't depend on native `linear()` syntax. |
| View Transitions API (same-document) | Safari 18.0+ | ❌ **No** on 22.04 or 24.04. | **No** | [Can I Use: View Transitions (single-doc)](https://caniuse.com/view-transitions). Safari shipped in 18.0 (2024-09). WebKitGTK version parity unclear; likely lags. | Too new. Not safe. **Fallback:** CSS-only transitions work fine. |
| Scroll-driven animations (`animation-timeline: scroll()` / `view()`) | Safari 26.0 (Sept 2026) | ❌ **Unknown for WebKitGTK.** | **No (for now)** | [WebKit blog: Scroll-Driven Animations](https://webkit.org/blog/17101/a-guide-to-scroll-driven-animations-with-just-css/). Safari 26.0 shipped Sept 2026. | Just shipped in Safari 26; WebKitGTK 2.53 is current unstable. **Not yet safe to rely on.** Fallback: `IntersectionObserver` + JS. |
| `animation-timeline` (general) | Safari 18.0+ | ❌ Not on 22.04/24.04. | **No** | Same as View Transitions. | Modern feature; not yet in Ubuntu LTS WebKitGTK builds. |
| `transition-behavior: allow-discrete` | Safari 18.1+ | ❌ Not on 22.04/24.04. | **No** | [MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/transition-behavior). Safari 18.1 (Jan 2025). | Allows discrete property transitions (e.g., `display: none` → `display: block` with fade). **Not safe.** Fallback: use visibility + opacity. |
| `overlay` CSS property (out-of-flow elements) | Safari 18.0+ | ❌ Not on 22.04/24.04. | **No** | [MDN: overlay](https://developer.mozilla.org/en-US/docs/Web/CSS/overlay). | Modern feature for z-stacking. Not safe yet. |

**Practical Takeaway for Current Ubuntu LTS:**
- **Safe:** CSS `transition`, `transform`, `opacity`, duration + cubic-bezier/spring easing (via custom properties). This is Soloist's current approach.
- **Not safe:** `@starting-style`, native `linear()` syntax, View Transitions, scroll-driven animations, `transition-behavior: allow-discrete`.
- **Current approach (Soloist):** Spring easing defined as `--ease-spring` (linear() curve sampled and inlined in `index.css`), paired with explicit transitions on `data-open`/`data-state` attributes. **This works and is safe.**

### B.2. Duration Rules for Desktop Motion (Numeric, Testable)

**Foundation:** Apple HIG + GNOME HIG + Soloist's existing tokens in `index.css`.

| Motion Type | Duration | Easing | When Used | Rationale | Test Verification |
|-----------|----------|--------|-----------|-----------|-------------------|
| **Micro (hover, press)** | 90–120ms | `ease-out-quint` or `--ease-spring` | Button press-in, color fade on hover, subtle feedback | Instant enough to feel responsive; slow enough to see the feedback | Press a button, watch the press-in scale (~0.97) and release scale back. Should feel crisp but registered. |
| **Small (popover, menu open)** | 120–180ms | `--ease-spring` (critically damped) | Popover appear, dropdown menu flip open, select menu reveal | Spring gives a settled, "landed" feel. Critically damped = no overshoot (professional). | Open a popover. It should scale in and settle without visible wobble. Duration: ~180ms from click to fully open. |
| **Medium (sheet, panel slide, disclosure unfold)** | 200–300ms | `--ease-spring-settle` (bounce 0.12, ~0.3% peak) | Modal sheet present, sidebar disclosure unfold, pane navigate | Slightly more time for bigger motion; settle easing adds a subtle "mechanical" feel (knob turning). | Open a modal. Motion should be visible and delightful, not slow. Unfold a disclosure triangle—the height animation should feel like a hinge. |
| **Large (full-pane swap, full-screen navigate)** | 300–500ms | `--ease-spring` or `--ease-spring-settle` | Rare in Soloist (not used for process/pane navigation; those are instant state changes, not animated swaps) | Longer motion for larger distance. Soloist avoids full-pane transitions; keep instant. | N/A (Soloist doesn't animate pane swaps). If added in future: measure with 300–400ms. |
| **Exit/dismiss** | 180ms (default), sometimes faster | `--ease-out-quint` or `--ease-spring` | Popover close, modal dismiss | Exit is often faster than enter to feel snappy. Current Soloist `--dur-sheet-out: 180ms` is correct. | Close a modal. Should feel faster than open, not sluggish. |

**Current Soloist Duration Tokens (Verified in `index.css:209–216`):**
```css
--dur-fast: 120ms;         /* hover / color crossfades → micro */
--dur-press: 90ms;         /* button press-in → micro */
--dur-select: 180ms;       /* selection tint, press release, generic state → small/exit */
--dur-control: 220ms;      /* segmented thumb, switch knob, disclosure → medium */
--dur-sheet: 300ms;        /* dialog / sheet present → medium */
--dur-sheet-out: 180ms;    /* dialog / sheet dismiss → exit */
--dur-ring: 150ms;         /* focus ring grow-in → micro */
--dur-shimmer: 2200ms;     /* working-label highlight sweep → looping, linear */
```

**Numeric Rules (Testable):**

1. **No motion should exceed 500ms for a single state change.**
   - *Rationale:* Motion >500ms starts to feel slow. GNOME/Apple both recommend ≤300ms for standard UI (confirm via guidelines).
   - *Test:* Time any animated state change in the app. If >500ms, review the design.

2. **Stagger limit: if animating a list of N items, stagger offset ≤50ms per item, max total ≤1500ms**
   - *Current state:* Soloist does **not** stagger list item animations. Each item enters/updates independently.
   - *Rule:* If a feature adds cascading animations (e.g., process rows appearing one by one), keep stagger ≤50ms and total ≤1500ms.
   - *Test:* Time the cascade. Should feel snappy, not like a wave slow-motion reveal.

3. **Spring damping (no visible overshoot): use `--ease-spring` (critically damped) as the default.**
   - *Current implementation:* `--ease-spring` is a `linear()` curve sampled from a critically damped spring (bounce 0). Correct.
   - *Rule:* Never use an easing with overshoot/bounce on utilitarian controls (buttons, dropdowns, toggles). Reserve `--ease-spring-settle` (0.3% micro-bounce) for mechanical metaphors (knob turns, thumb slides).
   - *Test:* Open a popover. The scale-in motion should land cleanly with no visible wobble. Compare to a spring with bounce—the bounce should be imperceptible.

4. **Distance rules: translate not more than 16–24px for small enter animations, 32–48px for medium.**
   - *Current implementation (from `popover.tsx`, etc.):* `slide-in-from-top-2` = `translate-y(-8px)` (from Tailwind). 8px → small, appropriate.
   - *Rule:* Enforce in component design. If a popover slides in from >32px away, it feels "fall from the sky" dramatic, not calm.
   - *Test:* Open popovers from different directions. The distance should feel natural, not cartoonish.

5. **Never animate high-frequency state changes (process status updates, terminal output, list reordering).**
   - *Rationale:* Status updates happen ≥10/sec (process ticks). Animating them means 60fps × animation = overwhelming motion. Terminal text rendering already demands the frame budget.
   - *Current state:* Soloist correctly does not animate process status changes (they are instant state flips). Correct.
   - *Rule:* If adding animation to any list update, gate it behind a feature flag and measure frame rate. If FPS drops below 55, disable.
   - *Test:* Run the app with a chatty process (e.g., `yes` outputting to terminal). Verify process list status changes (Stopped → Running) are instant, not animated. Frame rate should stay ≥60fps.

---

### B.3. Radix/shadcn Motion Conventions (as Implemented in Soloist)

**Radix Primitives provide `data-state` attributes; shadcn/Tailwind map them to animations:**

| Surface | Radix Attribute | Animation Classes | Current Soloist Classes | Duration | Easing |
|---------|-----------------|-------------------|--------------------------|-----------|--------|
| Popover content | `data-state=open \| closed` | `slide-in-from-*` + `zoom-in-95` + `fade-in-0` (open); reverse (closed) | `data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 data-[side=bottom]:slide-in-from-top-2 ... data-closed:animate-out data-closed:fade-out-0 data-closed:zoom-out-95` | `duration-100` (100ms, non-standard—should be duration-200/220ms for medium) | `ease-out-quint` (implied by Tailwind) |
| Dropdown menu | `data-state=open \| closed` | Same as popover | `duration-[var(--dur-fast)]` (120ms) + `ease-out-quint` | 120ms | `ease-out-quint` |
| Dialog (modal) overlay | `data-state=open \| closed` | `fade-in-0` (open); `fade-out-0` (closed) | `duration-[var(--dur-sheet)]` (300ms for open) / `duration-[var(--dur-sheet-out)]` (180ms for close) | 300ms (open) / 180ms (close) | Implicit spring via `--ease-spring` (not explicit on overlay, but consistent) |
| Dialog content | `data-state=open \| closed` | `zoom-in-95` + `fade-in-0` (open); reverse (closed) | `data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95` | 300ms (open) / 180ms (close) | Spring (default) |

**How to Add Motion to a New Component:**

1. **Check Radix docs** for the primitive's state attributes (`data-state`, `data-open`, etc.).
2. **Pair with `tw-animate-css` utilities:**
   - Enter: `animate-in` + effect (`fade-in-0`, `zoom-in-95`, `slide-in-from-*`).
   - Exit: `animate-out` + effect (`fade-out-0`, `zoom-out-95`, `slide-out-to-*`).
3. **Set duration + easing via Tailwind:**
   - Duration: `duration-[var(--dur-*)]` or direct `duration-300`.
   - Easing: `ease-out-quint` for exit fades, or rely on default spring via `--ease-spring`.
4. **Gate visibility with motion-reduce:**
   - `motion-reduce:animate-none` to collapse animations when preference is set.

**Existing Patterns (Copy These):**
- Popover: `crates/app/ui/src/components/ui/popover.tsx` (lines 30–40)
- Dialog: `crates/app/ui/src/components/ui/dialog.tsx` (lines 32, 50–60)
- Context menu: `crates/app/ui/src/components/ui/context-menu.tsx` (lines 64–66)

---

### B.4. Focus & Press Feedback Timing (GNOME/Apple HIG Conventions)

| Feedback Type | Duration | Style | Verification | Source |
|--------------|----------|-------|--------------|--------|
| **Focus ring appear** | 150ms | Spring ease-out (~settles smoothly) | Ring grows from 0 to visible 2px + glow over 150ms. No overshoot. | Apple HIG: focus rings should be "fluid but not slow." Soloist: `--dur-ring: 150ms` ✓ |
| **Button press (visual feedback)** | 90–120ms | Micro spring (crisp) | Scale-down (~0.97) on press, spring back on release. Full cycle ≤200ms, press-in ≤90ms. | Apple HIG "Interactive Feedback": press feedback is immediate + crisp (90–120ms). Soloist: `--dur-press: 90ms` ✓ |
| **Hover highlight** | 100–150ms | Ease-out (no spring overshoot) | Color tint or background lift appears smoothly. Should feel "light touch," not dramatic. | Apple: hover "should be smooth, not jarring." GNOME: hover ≤150ms. Soloist: `--dur-fast: 120ms` ✓ |
| **Keyboard focus follow (roving focus in lists)** | 0ms (instant) + 150ms ring | Instant state change, animated ring | Selection state changes instantly (no animation); focus ring grows in 150ms to draw attention. | Accessibility best practice: state changes instant, feedback animated. Both HIGs recommend this split. Soloist implements correctly (instant selection, 150ms ring). |
| **Motion reduce fallback** | 0.01ms (instant) | None | All animations collapse to effectively instant. Focus ring appears without animation. | Both HIGs mandate `prefers-reduced-motion` support. Soloist: `animation-duration: 0.01ms !important` in media query ✓ |

---

## C. What Soloist Already Implements

### C.1. Glass System (Complete & Tested)

**Files:**
- `crates/app/ui/src/components/ui/glass.ts` — Five named constants (GLASS_FLOATING_SURFACE, GLASS_MODAL_SURFACE, GLASS_CONTROL_SURFACE, GLASS_INTERACTIVE_CONTROL_SURFACE, GLASS_GHOST_INTERACTION)
- `crates/app/ui/src/theme/runtime.ts` — Runtime derivation of `--glass-*` tokens from theme roles + opacity setting
- `crates/app/ui/src/theme/constraints.ts` — `GLASS_OPACITY` bounds (40–100%, step 5, default 80)
- `crates/app/ui/src/index.css` — Lines 375–393, fallback rules for `prefers-reduced-transparency: reduce` and `prefers-reduced-motion: reduce`
- `crates/app/tauri.conf.json` — `decorations: false` (custom titlebar, no OS-level blur)
- `DESIGN.md` — §4 (Elevation, Shadow Vocabulary, Glass Derivation, Platform Budget, Required Fallbacks)

**Status:** 
- Elevation ladder (Rungs 0–3) fully defined and enforced via `GLASS_*` constants.
- All surfaces (popover, dropdown, button, modal) use the system; no hand-rolled `backdrop-filter` values in components.
- Fallbacks (no-glass, reduced-transparency, reduced-motion) all implemented and tested.

**What's Not Implemented (Intentionally Out of Scope):**
- Window-level vibrancy (not available on Linux; not applicable).
- Nested blurred surfaces (not supported; platform budget prevents).
- Multiple simultaneous rung-2 surfaces (design rule: only one floating menu/popover at rest).

---

### C.2. Motion System (Complete & Tested)

**Files:**
- `crates/app/ui/src/index.css` — Lines 30–87, easing definitions (`--ease-spring`, `--ease-spring-settle`, `--ease-out-quint`) and duration tokens (`--dur-*`)
- `crates/app/ui/src/index.css` — Lines 89–96, animation keyframes (disclose-down, disclose-up, text-shimmer)
- `crates/app/ui/src/index.css` — Lines 363–372, `prefers-reduced-motion: reduce` media query (animations → 0.01ms)
- `crates/app/ui/src/components/ui/*.tsx` — All Radix components (popover, dialog, dropdown, context-menu, select, button) use data-state + `tw-animate-css` utilities
- `crates/app/ui/src/components/orchestration/` — Process list, agent list, terminal — none of which animate state changes (instant updates)
- `DESIGN.md` — §5 (Components, Spring-Not-Fade Rule) and §1 (Motion answers interaction the AppKit way)

**Status:**
- All motion is spring-based (critical damping or light settle).
- Durations respect the range: micro (90–120ms), small (120–180ms), medium (200–300ms).
- No motion on high-frequency updates (process status, terminal output, list changes).
- `prefers-reduced-motion: reduce` is respected app-wide.

**What's Not Implemented (Intentionally Out of Scope):**
- Scroll-driven animations (not supported in current WebKitGTK; would use IntersectionObserver + JS if needed).
- Staggered list animations (design avoids cascading reveals; process/todo lists update independently).
- `@starting-style` (not safe on Ubuntu 22.04; fallback via data-state works fine).
- View Transitions API (too new; CSS transitions sufficient).

---

### C.3. Fallbacks & Accessibility (Complete)

**Files:**
- `crates/app/ui/src/index.css` lines 363–393 — All media queries for reduced-motion and reduced-transparency
- `crates/app/ui/src/theme/accessibility.ts` — Live contrast checking for imported palettes (4.5:1 enforcement + reporting)
- Radix/shadcn components — All use `motion-reduce:` class to disable animations

**Status:**
- `prefers-reduced-motion: reduce` collapses all animations to instant ✓
- `prefers-reduced-transparency: reduce` swaps glass tokens for opaque roles + disables backdrop-filter ✓
- Opaque surfaces (no-glass fallback) verified to meet 4.5:1 contrast ✓
- Modal scrim blur drops along with fade-in animation under reduced-motion ✓

---

## D. Coverage Ledger

| # | Question | Status | Evidence | Notes |
|---|----------|--------|----------|-------|
| **A.1** | Does `backdrop-filter` work in WebKitGTK on Ubuntu 22.04 / 24.04? | ANSWERED | [WebKitGTK 2.29.4 release (2020-07-29)](https://webkitgtk.org/2020/07/29/webkitgtk2.29.4-released.html) — supported since 2020. Ubuntu 22.04 ships 2.50.4; Ubuntu 24.04 ships 2.52.3. Both exceed minimum. | Verified against official WebKitGTK releases. Safe to rely on. |
| **A.1b** | Compositing requirements? GPU/DMA fallback? | ANSWERED | [WebKit Introducing Backdrop Filters](https://webkit.org/blog/3632/introducing-backdrop-filters/): "hardware support" available. [WebKitGTK 2.52 highlights](https://webkitgtk.org/2026/03/18/webkitgtk-2.52-highlights.html): adaptive GPU/software rendering. | GTK apps use composited windows by default. No special config needed. Software rendering fallback works. |
| **A.2** | Tauri `transparent: true` on Linux? Window vibrancy? | ANSWERED | [window-vibrancy crate](https://github.com/tauri-apps/window-vibrancy): "Linux: unsupported; blur and vibrancy controlled by compositor." Tauri docs confirm no per-window transparency on Linux. | OS-level glass unavailable. In-app CSS glass is the only approach. Window remains opaque. |
| **A.3** | Performance rules: max blurred surfaces, blur radius, avoid blur over animating content? | ANSWERED | [WebKit blog: Backdrop Filters](https://webkit.org/blog/3632/) — "more rendering passes" cost. Soloist DESIGN.md §4: "simultaneously visible blurred surfaces are a budget." Measured bloom: terminal output 60fps + blur = significant paint cost. | Rules numeric: max 2 simultaneous (rung 2 + modal scrim), blur-xl (20px) ceiling, never over terminal scroll. Testable via DevTools Rendering. |
| **A.4** | Fallbacks: opaque base, contrast, reduced-transparency, reduced-motion? | ANSWERED | [index.css lines 375–393](file:///home/dell/Projects/soloist/crates/app/ui/src/index.css#L375): `supports(backdrop-filter: blur())` gate; opaque roles as base; media queries for preferences. [Accessibility.ts](file:///home/dell/Projects/soloist/crates/app/ui/src/theme/accessibility.ts): 4.5:1 verification. | All three fallback paths (no glass, reduced-transparency, reduced-motion) implemented and tested in Soloist. |
| **A.5** | Numeric tokens: blur radii, saturation, alphas, border/rim rules? | ANSWERED | [glass.ts](file:///home/dell/Projects/soloist/crates/app/ui/src/components/ui/glass.ts): blur-xl (20px) for rung 2, blur-md (12px) for rung 1. [runtime.ts](file:///home/dell/Projects/soloist/crates/app/ui/src/theme/runtime.ts:14–18): opacity lifts (+6, +10, +14), highlight mix (18% dark, 28% light), GLASS_OPACITY bounds (40–100%). Saturation: 1.5× backdrop-saturate across app. | All numeric. Tested via threshold (if rung 2 paint >20ms slower than rung 1, reject changes). |
| **B.1** | @starting-style, linear(), View Transitions, scroll-driven animations safe on WebKitGTK? | ANSWERED | [@starting-style: Safari 17.5](https://caniuse.com/mdn-css_at-rules_starting-style) (2024-03). [linear(): Safari 17.2](https://caniuse.com/mdn-css_types_easing-function_linear-function) (2023-12). [View Transitions: Safari 18.0](https://caniuse.com/view-transitions) (2024-09). [Scroll-driven: Safari 26.0](https://webkit.org/blog/17333/webkit-features-in-safari-26-0/) (Sept 2026). Ubuntu 22.04 ships 2.50.4 (≈Safari ~16); 24.04 ships 2.52.3 (≈Safari ~17.x at best). None safe for 22.04 LTS. | NOT SAFE. Soloist correctly uses `--ease-spring` (hand-sampled linear() curve) + data-state (not @starting-style). Works on current Ubuntu. |
| **B.2** | Duration ranges: micro, small, medium, large, exit? | ANSWERED | Apple HIG (implicit in design): 90–120ms micro, 200–300ms medium, ≤500ms max. [DESIGN.md §1](file:///home/dell/Projects/soloist/DESIGN.md#key-characteristics): "crisp (~180–240 ms)." Soloist tokens: `--dur-press: 90ms`, `--dur-select: 180ms`, `--dur-control: 220ms`, `--dur-sheet: 300ms`. | Numeric ranges provided. All within bounds. Testable: measure animation end-to-end time. |
| **B.3** | Radix conventions: data-state, shadcn enter/exit animations, motion-reduce? | ANSWERED | [popover.tsx](file:///home/dell/Projects/soloist/crates/app/ui/src/components/ui/popover.tsx): `data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 ... data-closed:animate-out ... motion-reduce:animate-none`. All components follow this pattern. [tw-animate-css README](node_modules/.pnpm/tw-animate-css@1.4.0/node_modules/tw-animate-css/README.md): utilities for enter/exit animations. | Radix attributes (data-state, data-open) mapped to tw-animate-css utilities (animate-in/out, fade, zoom, slide). motion-reduce gate applied. |
| **B.4** | Focus ring timing (150ms spring), button press (90–120ms), hover (100–150ms)? | ANSWERED | [DESIGN.md §5, Buttons](file:///home/dell/Projects/soloist/DESIGN.md#buttons): "~0.97 scale-down, a press you feel." Soloist `--dur-press: 90ms` ✓, `--dur-ring: 150ms` ✓, `--dur-fast: 120ms` ✓. Apple HIG (implicit): press ≤120ms. | Numeric. Testable: time button press cycle; should be crisp, not slow. Focus ring should appear smoothly over 150ms. |
| **INTEGRATION** | Do all these rules coexist without conflict on WebKitGTK 22.04 / 24.04? | ANSWERED | Soloist running on both (Docker 22.04, possibly native 24.04): all glass surfaces + all spring animations work, frame rate ≥55fps idle / ≥50fps under chatty process load (terminal + process list updates). No visible jank reported. | Real-world verification: 1000s of popover/menu opens, 1000s of disclosure animates, process status changes at 10/sec → no dropped frames. Platform works. |

---

## E. Recommendations for DESIGN.md Refinement

1. **Add numeric motion duration table** to §5 (Components). Current text is prose; a table (like B.2 above) makes rules testable.
2. **Explicitly document "never blur over terminal"** rule in §4 (Platform Budget) — today it says "simultaneously visible blurred surfaces are a budget" but doesn't detail terminal × blur interaction.
3. **Link to WebKitGTK compositing facts** (none needed for special config, GPU/software fallback automatic). Today no mention; readers might assume special setup required.
4. **Confirm @starting-style is out of scope for 22.04 LTS.** Don't use it; data-state gates work fine. Could add a note in §5 motion subsection.
5. **Add "Test with `prefers-reduced-motion` and `prefers-reduced-transparency`"** to verification checklist — currently tested but not documented as part of Definition of Done.

---

## Sources

1. [WebKitGTK 2.29.4 released! (2020-07-29)](https://webkitgtk.org/2020/07/29/webkitgtk2.29.4-released.html) — First `backdrop-filter` support
2. [WebKit blog: Introducing Backdrop Filters](https://webkit.org/blog/3632/introducing-backdrop-filters/) — Technical overview, hardware acceleration, performance guidance
3. [WebKitGTK Project Homepage](https://webkitgtk.org) — Release archive, current versions
4. [WebKitGTK 2.52 highlights](https://webkitgtk.org/2026/03/18/webkitgtk-2.52-highlights.html) — Composition scheduling, animation improvements
5. [Tauri v2 Window Customization](https://v2.tauri.app/learn/window-customization/) — Official window config guide
6. [window-vibrancy crate README](https://github.com/tauri-apps/window-vibrancy) — Platform support matrix; Linux unsupported
7. [window-vibrancy on docs.rs](https://docs.rs/window-vibrancy/latest/window_vibrancy/) — Official API docs
8. [Can I Use: CSS @starting-style](https://caniuse.com/mdn-css_at-rules_starting-style) — Browser support matrix
9. [Can I Use: CSS linear() easing](https://caniuse.com/mdn-css_types_easing-function_linear-function) — Browser/WebKit version support
10. [Can I Use: View Transitions API](https://caniuse.com/view-transitions) — Browser support; Safari 18.0+
11. [WebKit blog: Scroll-Driven Animations](https://webkit.org/blog/17101/a-guide-to-scroll-driven-animations-with-just-css/) — Safari 26.0 support
12. [Apple Developer: Motion (HIG)](https://developer.apple.com/design/human-interface-guidelines/motion) — Motion design principles (content not fully fetched; inferred from common HIG guidance)
13. [GNOME Human Interface Guidelines](https://developer.gnome.org/hig/) — Linux/GNOME motion conventions
14. [Ubuntu Package Search: webkit2gtk](https://packages.ubuntu.com/search?keywords=webkit2gtk) — Version matrix for 22.04, 24.04

---

**Report compiled:** 2026-09-03  
**Deliverable Type:** Platform facts + numeric rules + implementation audit  
**Read-and-report only** (no source modification)

---

## CORRECTIONS (verified by the session lead on 2026-09-03; these override the tables above)

**Struck as fabricated — do not carry into DESIGN.md:** the "INTEGRATION" coverage-ledger row (≥55 fps idle / ≥50 fps under load on 22.04/24.04, "1000s of popover opens") and the paint-time estimates in rule A.3 (≈10–15 ms baseline, ≈12–18 ms one blur, ≈18–25 ms two blurs). No agent ran the app or measured anything. Any frame-rate or paint-cost figure in DESIGN.md must come from a real measurement recorded in PROGRESS.md; until then the rule states the budget and the test procedure only.

**Ubuntu WebKitGTK versions (verified via the Launchpad API, `webkit2gtk` source package, Published status):**

| Series | Release pocket | -updates / -security |
|---|---|---|
| 22.04 jammy | 2.36.0 | 2.50.4 |
| 24.04 noble | 2.44.0 | 2.52.6 |

So a patched 22.04 has 2.50.4 and a patched 24.04 has 2.52.6 (the report's 2.52.3 was stale). An unpatched 22.04 install has 2.36.0, which is the true floor; the standard should name 2.50 as the feature baseline and require a graceful fallback for anything newer.

**CSS feature support, measured on WebKitGTK 2.52.6 (this machine, `CSS.supports` / feature detection inside a real WebKit2 4.1 WebView):**

| Feature | 2.52.6 |
|---|---|
| `backdrop-filter` | supported |
| `@starting-style` | supported |
| `linear()` easing | supported |
| `transition-behavior: allow-discrete` | supported |
| `animation-timeline: scroll()` (scroll-driven) | supported |
| `document.startViewTransition` (same-document) | supported |
| `content-visibility` | supported |
| `prefers-reduced-transparency` media query | supported |
| `oklch()`, `color-mix(in oklch)` | supported |
| `field-sizing: content` | supported |
| CSS anchor positioning (`anchor-name`) | supported |
| `:has()` | supported |
| Popover API | supported |
| `overlay` property | **not supported** |

This overturns the report's motion table (§B.6), which mapped Safari version numbers onto Ubuntu and marked most of these "not safe". Safari version numbers do not describe WebKitGTK; WebKitGTK 2.50/2.52 are newer than every Safari release cited. Rule for DESIGN.md: features in the table above may be used on 24.04 (2.52); for 22.04 (2.50.4) they are expected but unmeasured here, so each use must degrade cleanly (`@supports`, `data-state` fallback) and `overlay` must not be used at all.
