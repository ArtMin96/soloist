# Detail-pane perf diagnosis: "skeleton only the first time" and "slow and laggy after that"

Date: 2026-09-06 · Branch: `feat/todo-workspace-ux` (working tree uncommitted: 45 files, +804/−1772,
and being edited concurrently by another session while this was measured) · Read-and-report only; no
source file was changed.

## TL;DR

Both symptoms have one root cause, and it is not the one the leads pointed at. Every open of a
scratchpad or todo detail mounts a full TipTap editor **synchronously on the main thread, starting on
the very next frame after the stand-in is committed**, and the app path makes that bring-up cost
2–3× what the editor intrinsically needs by re-applying ProseMirror view options 8–11 times per open,
each of which forces a full style+layout of a large, dirty page in WebKit.

- Measured block per open (dev build, WebKitGTK): **0.67–2.3 s for scratchpads of 16k–98k chars, and
  0.43–0.9 s for 2.5k-char todo bodies**. The block starts 50–135 ms after the click and ends when the
  editor is revealed.
- The skeleton is committed to the DOM on every open, but its reveal is a CSS animation with a 150 ms
  delay; the main thread blocks before that delay elapses and the ready flip removes the stand-in in
  the same commit that ends the block, so **after the first open it is never painted**. The first open
  is different only because the lazy editor chunk is fetched asynchronously, leaving the main thread
  free while the stand-in is on screen.
- **Nothing accumulates.** Editor count is 1 while open and 0 after Back in all 18 measured cycles,
  DOM node count is flat (1945–1950) across 10 opens, and per-document block times are the same on a
  second pass. "Gets worse" was not reproduced; what is true is that every open after the first is
  slow, bigger documents are slower, and the first open after a board mount is 1–2 s slower still.

Killed: lead 1 (`onSettled` never fires), lead 2 (TipTap instances leak), lead 3 (re-read loop),
lead 4 as a loop (but the layout effect *is* a per-open cost). Lead 5 is half right: the read stand-in
correctly never shows (IPC lands in <150 ms); the prose stand-in *should* show and cannot.

## 1. Root causes

### RC1 — synchronous editor bring-up per open, multiplied by repeated ProseMirror re-application

**What happens on an open** (`ScratchpadBoard.tsx:252-275`, `TodoBoard.tsx:217`): the click sets the
route; `SlidingPanels` starts a 300 ms `translate` transition; `useMasterDetail`'s no-dependency
`useLayoutEffect` (`useMasterDetail.ts:138-143`) runs `focusPanel` → `scrollIntoView` + `focus`
(`useMasterDetail.ts:79-81`) in the commit; the detail renders a `LoadingStandIn`; the body read lands
(scratchpad) or is already in the snapshot (todo); `MarkdownView` renders its own stand-in and, one
deferred pass later (`MarkdownView.tsx:33`), mounts `LazyRichTextEditor` (`MarkdownView.tsx:44-55`);
`RichTextEditor` creates the editor in an effect (`RichTextEditor.tsx:65-72`, `immediatelyRender:
false`), then seeds it with `setContent(markdown)` and calls `onReady` (`RichTextEditor.tsx:106-107`);
`setReady(true)` removes the stand-in and the `invisible absolute` wrapper.

**Where the time goes** (in-app attribution by patching the app's own module prototypes; see §2):

| Function (warm open, todo #37, 2.5k chars, 94 DOM nodes; block 892 ms) | calls | ms | max |
|---|---|---|---|
| `EditorView.setProps` | 11 | 732 | 285 |
| `Editor.setOptions` (→ `setProps` + `updateState`) | 8 | 287 | 286 |
| `Editor.createNodeViews` (→ `setProps({nodeViews})`, full redraw) | 2 | 185 | 171 |
| `Element.scrollIntoView` (from `focusPanel`) | 2 | 185 | 122 |
| `HTMLElement.focus` (from `focusPanel`) | 2 | 52 | 37 |
| `Editor.dispatchTransaction` (the `setContent` transaction) | 1 | 67 | 67 |
| `EditorView.updateState` | 9 | 57 | 56 |
| `MarkdownManager.parse` | 1 | 14 | 14 |

Parsing the Markdown is 14 ms. Re-applying view props is 732 ms. The chain:

1. **`RichTextEditor.tsx:65-73` hands `useEditor` unstable options every render.** `extensions:
   buildEditorExtensions({ placeholder, slash })` builds fresh extension instances each call
   (`editorExtensions.ts:33-52`; `Extension.configure()` returns a new instance — `@tiptap/core`
   3.27.3 dist, `configure(options = {}) { const extension = this.extend({...}) ... return extension }`),
   and `editorProps` is a fresh object literal. `@tiptap/react` 3.27.3 `useEditor` runs
   `onRender` after **every** render and calls `editor.setOptions(...)` whenever
   `compareOptions` is false; `compareOptions` compares `extensions` element-by-element by identity
   (`node_modules/@tiptap/react/dist/index.js`, `static compareOptions`: `if (extension !==
   b.extensions[index]) return false`). So every React render of `RichTextEditor` costs a
   `view.setProps(editorProps)` plus `view.updateState(state)` (`@tiptap/core` `setOptions`).
   Measured: 8 `setOptions` per open.
2. **In WebKit, each ProseMirror view update that touches the selection/doc forces two full layouts.**
   `prosemirror-view` 1.42.1 `updateStateInner` (dist line 5552):
   `let oldScrollPos = scroll == "preserve" && updateSel && this.dom.style.overflowAnchor == null &&
   storeScrollPos(this);` — `storeScrollPos` (dist 260-275) does `getBoundingClientRect()` and then
   `elementFromPoint` hit-tests every 5 px down the visible editor height; `resetScrollPos` (dist 287)
   reads layout again afterwards. WebKit does not implement `overflow-anchor`, so
   `dom.style.overflowAnchor` is `undefined` there (Chrome returns `""`), and the manual anchoring
   runs on every redraw. Each of those layouts is of the *whole dirty page*: the inert list panel with
   57–60 cards stays mounted and laid out (`SlidingPanels.tsx:85-88`), and the editor's DOM.
3. **`EditorContent`'s mount dance redraws the document.** `PureEditorContent`'s constructor calls
   `editor.view.setProps({ nodeViews: {} })` and `init()` calls `editor.createNodeViews()`
   (`@tiptap/react` dist ~100-118, 93). `createNodeViews` passes `extensionManager.nodeViews`, a getter
   that builds fresh closures on every call (`@tiptap/core` dist 4628), so `changedNodeViews` is true
   and ProseMirror rebuilds the entire `docView` (`prosemirror-view` dist 5535-5540). In the dev build
   `React.StrictMode` (`main.tsx:30-32`) unmounts and remounts `EditorContent` **after** the
   `setContent` effect has run, so the second `createNodeViews` redraws the populated document
   (measured 449 ms on the 98k-char doc). That part is dev-only.
4. **`focusPanel` forces layout before first paint.** `useMasterDetail.ts:79-81` calls
   `scrollIntoView({block:"nearest"})` on the detail's back button (already at the top of a fresh
   panel) and then `focus`, inside a layout effect in the opening commit: 122 + 37 ms per call on this
   page, doubled in dev by StrictMode.
5. **Layout is expensive to begin with.** Forced style+layout of the rendered 98k-char document
   measured 205–243 ms; with `letter-spacing: 0` inline on the editor it is 104–150 ms
   (`editor.css:71` sets `letter-spacing: var(--tracking-body)` = −0.078 px on `.tiptap-body`).
   `text-rendering: optimizeLegibility` (`index.css:330`) and `font-feature-settings`
   (`index.css:328`) made no measurable difference. Long documents get fully laid out (36,637 px tall
   for the 104k-char doc) with no `content-visibility`.
6. **The intrinsic cost is itself over budget.** Headless, with the app's exact extension set, parse +
   DOM build is 141–669 ms for 17k–104k chars (§2). Even with all waste removed, a 20k-char note costs
   ~150 ms plus one layout on the main thread — above the 150 ms skeleton threshold, and mid-slide.

Why the todo and scratchpad boards behave the same: they share `SlidingPanels`, `useMasterDetail`,
`MarkdownView`, and `RichTextEditor`; none of the cost is in scratchpad-specific code.

### RC2 — the prose stand-in is committed but never painted (except on the first open)

Timeline from the MutationObserver + rAF gap meter (warm todo open): stand-in committed at **98 ms**
(second run: 134 ms) → next frame starts the **621–892 ms block** → `editor|visible` at 733/965 ms in
the *same* commit that removes the stand-in. Every scratchpad run shows the same shape: two short gaps
(click → detail commit, read landing → `MarkdownView` commit) and then one block ending at
`tEditorDom`.

`LoadingStandIn.tsx:29` reveals through `animate-skeleton-reveal`, defined at `index.css:101-102` with
`var(--skeleton-delay)` = 150 ms (`index.css:227`) and `backwards` fill (opacity 0 until the delay
elapses). The delay is pure CSS: it cannot know the main thread is about to block. The stand-in has
15–50 ms of free main thread before the block starts, is never painted at opacity > 0, and is removed
by the ready flip (`MarkdownView.tsx:34, 38-42, 54`).

The first open differs because `LazyRichTextEditor.tsx:9` imports the editor chunk asynchronously;
while the chunk loads the main thread is idle, frames are painted, and the stand-in is visible. Measured:
first open after a reload took 1963–2304 ms with painted frames between (gaps 320, 396 then the block;
stand-in in the DOM from 270 to 1936 ms), later opens 430–996 ms with no frame during the wait. In the
dev build the chunk is ~100 Vite modules; in a production bundle it is one local file, so the first-open
skeleton may be shorter or absent there (not measured).

Two stand-ins exist on the scratchpad path and they behave differently:

- The **read stand-in** (`ScratchpadDetail.tsx:167`, `LoadableRegion` around `scratchpadRead`) is
  correctly fast-pathed: `useScratchpadEditor.open()` always resets to `loading()`
  (`useScratchpadEditor.ts:108-118`), so there is no stale-`ready` state; the IPC read lands inside the
  first two short gaps (<150 ms), so it is deliberately not shown. Working as designed.
- The **prose stand-in** (`MarkdownView.tsx:38-42`) is the one the user should see after the first open
  and cannot, for the reason above. For todos there is no read at all, so this is the only stand-in.

So: "no skeleton after the first time" is not stale state; it is RC1's synchronous block starving
paint. It gets fixed by fixing RC1 (shrink the block) and/or by yielding a frame or two before the
editor mounts so the stand-in is painted first.

### Leads, verdicts

| Lead | Verdict | Evidence |
|---|---|---|
| 1. `onSettled` never fires → retained `detailKey`, no `close()` | **Killed.** | Back → `[data-scratchpad-detail]` removed after 313–1483 ms in all 18 cycles; `editorsAfterBack: 0`; `SlidingPanels.tsx:19, 78-82` accept `translate`/`transform`; reduced motion keeps `transition-duration: 0.01ms` (`index.css:374-382`) so `transitionend` still fires; `useMasterDetail.ts:128-132` → `onDrop` → `editor.close()` (`ScratchpadBoard.tsx:116`). Gap: no board-level test dispatches `transitionEnd` (only `SlidingPanels.test.tsx:45,52` and `useMasterDetail.test.ts:119`). |
| 2. TipTap instances leak | **Killed.** | `[data-editor="rich-text"]` = 1 while open, 0 after Back, 18/18; DOM nodes 1945–1950 across 10 opens; second-pass block times equal first-pass. `useEditor` cleanup schedules `destroy()` after 1 ms (`@tiptap/react` dist 458-475) and `EditorContent.componentWillUnmount` does **not** unmount the view (dist 98-118), so `isDestroyed` is still false when the timer fires and `destroy()` runs. TipTap's `<style>` tag is deduplicated (`createStyleTag` queries `style[data-tiptap-style]`). Heap was not measured. |
| 3. Re-read loop | **Killed.** | `ScratchpadBoard.tsx:139-142`: effect deps `[outdated, reload]`; `reload` is `useCallback([name, load])`, `load` is `[project, setDocument]`; a re-render with `outdated` still `true` does not re-fire. `ScratchpadSummary.revision` and `ScratchpadView.revision` are the same `u64` (`crates/core/src/coordination/scratchpad.rs:101,129`; `domain.ts:785,798`). Measured idle window after open: 0 mutations / no gaps for 8 of 10 opens; the mermaid-heavy doc (#61, 10 diagrams) shows 12–36 mutations and gaps of 121/267/424 ms, which is its diagrams rendering asynchronously through the bounded queue, then quiet. |
| 4. No-dep `useLayoutEffect` loops | **Killed as a loop; confirmed as a cost.** | `useMasterDetail.ts:138-143` returns unless `pendingFocusRef` is set, clears it, writes no state. But its `scrollIntoView` + `focus` cost 185 + 52 ms per open here (2 calls each in dev due to StrictMode). |
| 5. Skeleton timing correct-by-design | **Half right.** | Read stand-in: correct fast path. Prose stand-in: should show and is never painted (RC2). |
| "Gets worse over time" | **Not reproduced.** | Same-doc block on pass 1 vs 2: 1516→790, 2069→1813, 691→737, 2323→2268, 669→732 ms. |

## 2. What was measured

**Setup.** The already-running dev instance (`cargo tauri dev --features agent-bridge`, pid 90419;
Vite on :1420; bridge WebSocket on :9223) driven directly over the bridge's `execute_js` command from
Node 26 (`/tmp/claude-1000/.../scratchpad/bridge.mjs`, `nav.mjs`, `prof.mjs`; injected scripts
`harness.js`, `inspect.js`, `profile2.js`, `experiment.js`). React 19.2.7 development build inside
`<React.StrictMode>` (`main.tsx:30-32`), `@tiptap/react` 3.27.3, `prosemirror-view` 1.42.1, WebKitGTK
(UA `AppleWebKit/605.1.15 … Version/60.5`). The Soloist project's own 57 scratchpads and 60 todos. Only
read-only IPC (`scratchpad_read`, `project_list`) and UI clicks were used; edit mode was never entered.

**Caveats.** (1) This is the dev build: StrictMode double-invokes mounts/effects and React itself runs
unminified. Dev-only portions are called out; a production estimate is given below. (2) The window was
mostly unfocused, but rAF ran at ~63 fps regardless (38 frames in 600 ms; window visible), so gap
measurements are valid. (3) Another session was editing `OrchestrationPane.tsx` during the run; Vite
full-reloaded the page several times and wiped injected globals. Runs that straddled a reload were
discarded; the final scratchpad profile attempts hit a state where the body read never landed and were
abandoned. (4) IPC counting by wrapping `__TAURI_INTERNALS__.invoke` is impossible (non-writable,
non-configurable); the re-read verdict rests on code plus the idle-mutation/gap measurement.

**Cycle pass (open → 400 ms → Back, five different scratchpads):**

| id | chars | open total ms | rAF gaps during open (ms) | editors open/after | drop after Back ms | DOM nodes before→after |
|---|---|---|---|---|---|---|
| 62 | 19,498 | 1871 | 180, 74, 40, 34, 151, **1322** | 1 / 0 | 332 | 1945→1948 |
| 61 | 50,095 | 2426 | 80, 44, **2314** | 1 / 0 | 1004 | 1947→1950 |
| 24 | 18,289 | 2432 | 1533, 1593, 773 (reload noise) | 1 / 0 | 313 | 1950→1948 |
| 9 | 98,325 | 2917 | 81, 56, **2791** | 1 / 0 | 592 | 1947→1948 |
| 57 | 16,543 | 795 | 69, 47, **686** | 1 / 0 | 330 | 1947→1948 |

**Inspect passes (two passes, same five docs; block = largest gap during open):**

| id | name | chars | top-level blocks | code blocks (React node views) | tables | mermaid | block pass 1 / 2 (ms) | idle mutations |
|---|---|---|---|---|---|---|---|---|
| 62 | mcp-read-auto-chunking-design | 19,498 | 51 | 4 | 3 | 0 | 1516 / 790 | 0 / 0 |
| 61 | agent-interop-design | 50,095 | 194 | 12 | 14 | 10 | 2069 / 1813 | 12 / 36 |
| 24 | dsa-audit-S20 | 18,289 | 65 | 3 | 0 | 0 | 691 / 737 | 0 / 0 |
| 9 | dsa-audit-contract | 98,325 | 140 | 0 | 4 | 0 | 2323 / 2268 | 0 / 0 |
| 57 | dsa-audit-S34 | 16,543 | 35 | 0 | 1 | 0 | 669 / 732 | 0 / 0 |

The block tracks document length, not code-block count (doc 9 has zero React node views and the
longest block), which kills the "flushSync per node view" sub-hypothesis as the main driver.

**Todo pass:** #36 (1,606 chars, first open after a reload) 2304 ms with gaps 1189 + 876; #37 (2,552
chars) 430 ms; #38 (2,337 chars) 447 ms; `editors` 1/0, idle mutations 0. Warm profile of #37: block
892 ms with the attribution table in RC1; stand-in committed at 98 ms, editor visible at 965 ms.

**Profile of the 98k-char scratchpad (#9):** block 1910 ms; `Editor.createNodeViews` 2 calls, 455 ms
(max 449 — the StrictMode remount redraw); `Editor.dispatchTransaction` 249 ms; `Editor.setOptions`
7 calls, 101 ms; `Editor.mount` 5 ms. Forced style+layout A/B on the rendered document (3 runs each):
as-is 243/213/205 ms; `letter-spacing: 0` 150/107/104; `text-rendering: optimizeSpeed` 203/199/196;
`font-feature-settings: normal` 216/190/191; all three 129/104/118. Computed on the editor:
`letter-spacing: -0.078px`, `text-rendering: optimizelegibility`, `font-feature-settings: "liga" 0`.

**Headless, same page, app's own `buildEditorExtensions`** (editor into a detached element; "parse" =
`element: null`, state only; "layout" = attach to body at 640 px and force layout):

| name | chars | fences | parse ms | parse+DOM ms | layout ms | plain `CodeBlock` instead of React node view: parse+DOM / layout |
|---|---|---|---|---|---|---|
| dsa-audit-S34 | 17,698 | 0 | 80 | 152 | 320 (first run outlier) | 186 / 16 |
| dsa-audit-S20 | 19,309 | 3 | 69 | 141 | 18 | 158 / 15 |
| mcp-read-auto-chunking-design | 21,005 | 4 | 48 | 141 | 29 | 148 / 20 |
| agent-interop-design | 53,556 | 12 | 497 | 584 | 75 | 605 / 46 |
| dsa-audit-contract | 104,214 | 0 | 589 | 669 | 221 | 669 / 80 |

In-app block ÷ headless (parse+DOM+layout): 16.5k chars 700 ÷ ~170 ≈ 4×; 98k chars 2300 ÷ ~890 ≈ 2.6×.
That ratio is RC1 items 1–5.

**Production estimate** (not measured — no production build was run): remove StrictMode's second
redraw and doubled effects, and React's dev overhead, and the block is roughly parse+DOM (140–670 ms)
plus 1–2 forced layouts (100–250 ms each) plus the `setOptions` re-applications: about **0.35–0.5 s for a
17–21k-char note and ~1–1.2 s for the 98k-char one**. Still far above 150 ms, still mid-slide, still
starving the stand-in. Verify with `just bundle` + the same harness before trusting these numbers.

## 3. Fix plan

Ordered by expected win ÷ risk. F1, F3, F4 and F7 are in the shared kit or the shared editor and fix
both boards (and every future surface) at once.

**F1. Stable `useEditor` options — `components/editor/RichTextEditor.tsx:65-73`.** Memoise
`extensions` on `[placeholder, slash]` and the `editorProps` object on `[editable, ariaLabel]` (the
handlers already read refs), so `compareOptions` is true on re-render and `setOptions` is never called
for a plain React re-render. Expected: `Editor.setOptions` per open 8 → ≤2 (the two TipTap-internal
`{element}` calls), removing ~250–300 ms of the warm-open block and every snapshot-driven re-application
while a document is open. Risk: low; the editor is keyed per document, and `placeholder`/`slash` do not
change during an editor's life. Verify with the `Editor.setOptions` spy (T1).

**F2. Opt out of ProseMirror's manual scroll anchoring on read-only surfaces —
`RichTextEditor.tsx`, effect on `[editor]`.** ProseMirror's escape hatch is `overflow-anchor: none` on
the editor DOM, but the check reads the JS property (`dom.style.overflowAnchor == null`), which WebKit
never sets from CSS because it does not implement the property. Setting `editor.view.dom.style.overflowAnchor = "none"` from JS makes the check false (an expando on the style declaration) and skips
`storeScrollPos`/`resetScrollPos`. For `editable={false}` this loses nothing (there is no caret to
anchor). Expected: removes the two forced layouts per redraw (the bulk of the 285 ms `setProps` max).
Risk: medium — relies on WebKit accepting the expando; guard it to read-only surfaces and re-measure;
if WebKitGTK ever implements `overflow-anchor`, the CSS value takes over correctly.

**F3. Do not bring the editor up mid-slide; let the stand-in paint — `MarkdownView.tsx:33` with a
"panel settled" signal from the kit (`SlidingPanels` → `DetailPane`/context).** Today the editor
mounts one deferred pass after the stand-in, i.e. on the next frame. Mount it only after the panel
reports settled (`onSettled`, ~300 ms) or, when no transition ran, after a bounded fallback (two rAFs
+ `--dur-sheet`), so the slide finishes at full frame rate and the stand-in is painted before the
synchronous work starts. This is what makes the skeleton appear on every open, independent of F1/F2.
Risk: medium — the signal must be robust (fallback timer) and the `focusPanel` order must still land
focus after the panel arrives. Verify with T2 and the rAF meter (no gap > 50 ms during the slide).

**F4. Take the off-screen panel out of layout — `components/common/SlidingPanels.tsx:85-88,
110`.** Set `content-visibility: hidden` on the inert `Panel` (DOM, state and scroll position are
kept; layout and paint are skipped). Every forced layout during an open then no longer pays for 60
cards. Risk: low–medium — confirm WebKitGTK support on the target (Safari 15.4+ semantics), and that
`transitionend` still fires on the track (it is the track that transitions, not the panel).

**F5. Drop `letter-spacing` from the prose body — `components/editor/editor.css:71`** (and decide in
DESIGN.md whether −0.078 px tracking on 13 px prose is worth a 2× layout cost; it is sub-pixel).
Measured: forced layout 243 → 104 ms on the 98k-char doc. Risk: design sign-off only.

**F6. Lazy layout for long read-only documents — `editor.css`, scoped to `.tiptap-shell--plain
.tiptap-body > *`:** `content-visibility: auto; contain-intrinsic-size: auto 1.5em`. Only visible
blocks are laid out, so the cost stops scaling with document height (36,637 px for the 104k doc).
Risk: medium — read-only only (find-in-note decorations and outline positions must be re-measured;
do not apply to the editable surface).

**F7. Cheaper focus restoration — `store/useMasterDetail.ts:77-82, 138-143`.** On open, do not
`scrollIntoView` the detail's back button (it is at the top of a fresh panel); `focus({ preventScroll:
true })` is enough. Keep `scrollIntoView` for the Back path (the list row). Consider moving the whole
`focusPanel` call to after settle (it only needs to land before the user's next keystroke). Saves
120–185 ms before the first paint of every open. Risk: low; `useMasterDetail.test.ts` pins focus
behaviour and must be kept green.

**F8. Reduce the intrinsic cost (later, structural).** Either cache the parsed ProseMirror document per
`${id}:${revision}` (a bounded LRU; the headless parse is 48–589 ms of the floor), or render read-only
bodies with a lighter path than a full editor instance (the "one renderer" decision in
`MarkdownView.tsx` trades ~150–700 ms per open for dialect consistency). Not needed to fix the reported
symptoms if F1–F7 land; needed to get 50k+ char notes under 150 ms.

Not a fix, but record it: the dev build's StrictMode remount adds a full document redraw per open
(449 ms on the 98k doc) plus doubled layout effects. Measure production before deciding any of this is
"good enough".

## 4. Regression tests that would have caught it

The suite is jsdom, which cannot measure layout, so the tests pin the *mechanism* counts; the frame
budget itself is guarded by the live harness.

**T1 — `RichTextEditor` does not re-apply options on re-render** (`components/editor/RichTextEditor.test.tsx`,
real `@tiptap/react`): render `<RichTextEditor initialMarkdown="# a" editable={false} onChange={() => {}} />`;
`vi.spyOn(Editor.prototype, "setOptions")` (import `Editor` from `@tiptap/react`); wait for the
`[data-editor="rich-text"]` element; record the call count; `rerender` three times with a *new*
`onChange` closure each time; assert the count did not increase. Before F1 it increases by one per
re-render (`compareOptions` fails on fresh `extensions`); after F1 it stays flat.

**T2 — the prose stand-in gets a painted frame** (`components/editor/MarkdownView.test.tsx`, fake
timers, `LazyRichTextEditor` mocked to call `onReady` synchronously on mount): render `MarkdownView`
with a long body inside a provider that has *not* reported settled; assert the stand-in (`[aria-busy=
"true"]`) is in the DOM and the editor is **not**; report settled (or advance the fallback timer);
assert the editor mounts and then the stand-in is removed. Before F3 the editor mounts on the very
next pass regardless of the panel; after F3 it waits. (Phrase the exact trigger to match F3's design.)

**T3 — Back settles, drops and closes at board level** (`components/orchestration/ScratchpadBoard.test.tsx`):
render the board with a mocked `scratchpadRead`; open a card; wait for the read; click Back; assert
the detail is still rendered (retained through the slide); `fireEvent.transitionEnd(track, {
propertyName: "translate" })`; assert the detail is gone, `scratchpadRead` was not called again, and
opening another card shows the stand-in (document back to `loading()`). This pins the settle → `onDrop`
→ `editor.close()` chain no board test exercises today. (Also add the `TodoBoard` twin: after settle the
`TodoDetail` unmounts.)

**T4 — mount-time redraw budget** (`RichTextEditor.test.tsx`): `vi.spyOn(EditorView.prototype,
"setProps")` (from `@tiptap/pm/view`); mount + seed one read-only editor; assert `setProps` ≤ 3 calls.
Guards F1/F2 against regressions that re-introduce per-render re-application.

**T5 — frame-budget guard (live, not vitest):** with `just agent-bridge`, run the rAF gap meter from
this session's harness across five opens of a 20k-char note in a production bundle and assert no gap >
200 ms during the open and none > 50 ms during the slide. Script: `harness.js` + `nav.mjs` in the
session scratchpad (copy into `scripts/perf/` if it is to be kept).

## 5. Other findings (not the two symptoms — do not bundle silently)

1. **N+1 editors per todo.** `CommentList.tsx:52` mounts a full `MarkdownView` (full extension set:
   `TableKit`, search, task lists, mermaid node view) per comment; a todo with N comments mounts N+1
   TipTap instances. The sampled todos had no comments, so this was not measured; expect the block to
   scale with N.
2. **Every snapshot event re-applies the open editor's options.** `useOrchestration.ts:21-26` re-reads
   the snapshot on `ProcessStatusChanged`, `AgentActivityChanged`, timers, etc.; each new snapshot
   re-renders the board, `ScratchpadDetail`/`TodoDetail`, and `RichTextEditor`, which (until F1) calls
   `setOptions` → `view.updateState` on the open document. Inference from code + the `setOptions` call
   counts; not measured in isolation.
3. **State writes during render in `useMasterDetail.ts:151-155`.** The vanished-target branch calls
   `onLeave`/`onDrop` during render; `editor.close()` (`useScratchpadEditor.ts:119-127`) then sets state
   on sibling hooks and writes refs (`loadRequestRef`, `documentRef`) from render. Works today, but it is
   not replay-safe and may make the React Compiler bail on `ScratchpadBoard`.
4. **`LoadingStandIn`'s delay is CSS-only** (`LoadingStandIn.tsx:29`, `index.css:101-102, 227`). The
   design note "a read that lands inside 150 ms swaps without a stand-in" is right for IPC reads and
   wrong for synchronous rendering; any future consumer that renders heavy content synchronously will
   hit RC2 again unless it yields first (F3 belongs in the kit, not in `MarkdownView` alone).
5. **Dev-only StrictMode cost** (`main.tsx:30-32`): a second full `EditorContent` mount dance per open
   (449 ms redraw on the 98k doc) and doubled `focusPanel` layout reads. Not a bug; explains why
   `just dev` feels worse than the installed build. Any perf number taken from the dev build must say
   so.
6. **Working tree in flux.** The refactor under diagnosis is uncommitted (45 files, +804/−1772) and was
   being edited during measurement (`OrchestrationPane.tsx`, its test); all line numbers above refer
   to the working tree as read on 2026-09-06.

## Coverage ledger

| # | Sub-question | Status | Evidence |
|---|---|---|---|
| 1 | Does `onSettled` fire on every leave path; what if not? | ANSWERED (fires) | 18 live cycles, `SlidingPanels.tsx:19,78-82`, `index.css:374-382`, `useMasterDetail.ts:128-132` |
| 2 | Are TipTap instances destroyed; does the invisible trick leak? | ANSWERED (no leak) | 18/18 editor counts, flat DOM, `@tiptap/react` dist 98-118, 458-475 |
| 3 | Can the revision reload effect loop? | ANSWERED (no) | `ScratchpadBoard.tsx:139-142`, `scratchpad.rs:101,129`, idle mutations 0 |
| 4 | Is the no-dep layout effect a no-op? | ANSWERED (no-op unless pending; costs 185+52 ms when pending) | `useMasterDetail.ts:138-143`, profile |
| 5 | Which symptom is fast-path vs never-loading? | ANSWERED | RC2 timeline; `useScratchpadEditor.ts:108-118` |
| 6 | Live measurement? | ANSWERED (dev build; production estimated only) | §2 |
| 7 | Where does the block's time go? | ANSWERED (dev); production PARTIAL | attribution table; headless split; StrictMode caveat |

---

## 2026-09-06, later session: did F1–F7 work on the running app?

Same dev build, same driver (bridge on :9223, pid 181486, `cargo tauri dev --features agent-bridge`),
same WebKitGTK (UA `AppleWebKit/605.1.15 … Version/60.5`), React 19.2.7 dev + StrictMode,
`@tiptap/core` 3.27.3, `prosemirror-view` 1.42.1. Working tree settled (45 files, +1071/−1505;
`git diff crates/` sha256 `7511d2ed…b3532` identical before and after this session; no source file
was touched, every probe was injected over the bridge). The before-numbers below are the earlier
session's (`before-todo.json`, `before-pad.json`, §2 above); the after-numbers are this session's.
All numbers are dev-build; production (no StrictMode, minified React) will be somewhat faster on the
React stretches and no faster on parse or style+layout.

**Method.** The earlier `meter.js` (a 4 ms `setInterval` gap meter; rAF measured at 37 frames /
600 ms unfocused, so the window's focus did not matter this time) re-run unchanged for the
like-for-like rows, plus `meter2.js`, which also samples the stand-in's computed opacity on every
rAF frame (a frame in which the `animate-skeleton-reveal` element has opacity > 0 is a frame the
engine produced with the stand-in visible), counts DOM nodes per cycle, and snapshots per-open call
counters installed on the app's own module instances (`Editor.setOptions`, `EditorView.setProps`
/`updateState`, `Document.elementFromPoint`, `Element.getBoundingClientRect`, `scrollIntoView`,
`HTMLElement.focus`). A phase timeline (`p_timeline.js`) then timestamped TipTap's construction
methods, `MarkdownManager.parse`, DOM milestones and a forced style+layout at the ready flip.
Scripts: `/tmp/claude-1000/-home-arthur-Downloads-soloist/247ac9bb-…/scratchpad/{run2.mjs, meter2.js,
wrap.js, open.mjs, poll.mjs, timeline.mjs, p_timeline.js, p_layout.js, p_scroll*.js, p_f4.js,
p_anchor_*.js}`; raw results in the `*.json` files beside them.

### Verdict per fix

| Fix | Mechanism engaged? | Measured effect | Verdict |
|---|---|---|---|
| F1 memoised `useEditor` options | Yes: `Editor.setOptions` 4 calls/open (was 8), `EditorView.setProps` 6 (was 11), all six together 3–61 ms/open | Block unchanged against the clean before-numbers (below) | engaged, no measurable effect on the open block |
| F2 `overflowAnchor = "none"` | Yes: `CSS.supports("overflow-anchor","none")` is **false**; a fresh `div.style.overflowAnchor` reads `undefined` before and `"none"` after assignment (own property on the same style object; `cssText` stays empty, so it is a JS expando, not a declaration); the open editor reads `"none"` on all 36 sampled opens; `Document.elementFromPoint` **0 calls/open**, `getBoundingClientRect` 1 (was a hit-test loop) | Block unchanged | engaged, not decoration, no measurable effect on the open block |
| F3 editor after panel settled | Yes: editor construction begins 20–60 ms after the list panel gets `content-visibility: hidden` (368–454 ms after click) | Stand-in reaches opacity ≥ 0.90 on **36 / 36** warm opens; time-to-content +~250 ms | **helped** (the reported symptom is gone); costs time-to-content |
| F4 `content-visibility: hidden` on the resting panel | Yes: computed `hidden` on the 60-card list while a todo is open | Forced page style+layout as-is 115/83/81 → undone 87/82/79 ms | engaged, no measurable effect |
| F5 `letter-spacing: normal` | Yes: computed `normal` | On #9 with F6 on: 146/131/114 ms vs 108/105/123 with −0.078 px re-added; with F6 off: 306/297/325 vs 310/294/309 | no measurable effect (the earlier 243 → 104 ms did not reproduce) |
| F6 `content-visibility: auto` on prose blocks | Yes: computed `auto`, `contain-intrinsic-size: auto 19.5px` | On #9: forced style+layout 306/297/325 → **146/131/114 ms**; laid-out height 31,497 → 11,474 px | **helped**; introduces scrollbar drift, see Q4 |
| F7 no `scrollIntoView` on open | Yes: `scrollIntoView` 0 calls/open | First block 192/112/141 → 100–147 ms (todo), 52–87 (pad); the remaining cost is one `focus` at 32–90 ms | helped a little |

### Symptom 1: does the stand-in paint on every open?

| | Before (theirs) | After (mine) |
|---|---|---|
| Free main thread while stand-in in DOM, todo #36/#37/#38 | 7 / 34 / 17 ms | 190 / 140 / 171 ms |
| Same, scratchpad #62/#61/#24 | 26 / 27 / 7 ms | 216 / 250 / 226 ms |
| Stand-in in DOM for | removed in the commit that ends the block | ~450–600 ms (todos, 20k pads), ~1.1–1.5 s (#9) |
| Frames rendered with stand-in opacity > 0 | not sampled (inferred none) | 7–11 per todo open, 4–9 per #62 open, 3–7 per #9 open; max opacity 0.90–1.0 on 36/36 |
| First frame with stand-in visible | — | 154–330 ms after click |

Yes. Cold open after a reload (todo #39): 1525 ms total with the chunk-load blocks as before;
not opacity-sampled.

### Symptom 2: is the block shorter? (warm opens, same rows, dev build)

| Doc | Before block (theirs, `meter.js`) | After block (mine, `meter.js`/`meter2.js`, uninstrumented) | Total click → content, before → after |
|---|---|---|---|
| todo #36 (1.6k) | 223 + 33 | 200 | 479 → 672 |
| todo #37 (2.5k) | 218 + 41 | 307; 12-cycle drift pass 175–297 (median 196) | 428 → 595–768 |
| todo #38 (2.3k) | 247 + 54 | 285 | 460 → 737 |
| pad #62 (19.5k) | 254 + 101 | 356; 12-cycle pass 347–465 (as one or two blocks) | 503 → 713–821 |
| pad #61 (50k, 10 mermaid) | 1027 + 54 | 1184 + 52 | 1209 → 1627 |
| pad #24 (18k) | 271 + 97 | 219+171, 216+158, 203+159 | 526 → 754–787 |
| pad #9 (98k) | 2791 (cycle) / 2323 / 2268 (inspect) / 1910 (profile) | 1109+642, 1216, 1401, 1333 | 2917 → 1578–1832 |

No for todos and 20–50k notes: the block is the same length, and because it now waits for the
slide it ends ~250 ms later. Yes for the 98k note, by roughly 40–50 %. Where the earlier profile
attributed 732 ms to `setProps` on a todo, the clean before-meter had already shown the same todo
at 218 ms; that 732 ms existed only under the earlier instrumentation, which is why removing it
(F1/F2) did not move the clean number.

**Where the block goes now** (phase timeline, one warm open each):

| Phase | todo #37 | pad #62 | pad #9 |
|---|---|---|---|
| `focus` in the opening commit (first style+layout of the detail) | 70–90 | 36–47 | 32–35 |
| TipTap construction (`createExtensionManager` … `createNodeViews`) | 9 | 9 | 8 |
| `MarkdownManager.parse` (inside `setContent`, before dispatch) | 4 | 92 | **670** |
| `dispatchTransaction` | 4 | 11 | 15 |
| StrictMode second `createNodeViews` | 1 | 8 | 16 |
| Style+layout after the ready commit (forced, so it lands in a mark) | **123** | **101** | **132** |
| Paint after that until the thread is free | 1 | 3 | 57 |

Two costs the earlier report did not name: the Markdown → ProseMirror parse (the earlier
`dispatchTransaction` figure did not include it) is the whole story for long notes, and a
size-independent ~100–130 ms style+layout of the page at the reveal is the whole story for todos.
The dev-only StrictMode redraw that cost 449 ms on #9 is now 16 ms (no `storeScrollPos`, no forced
layout inside it).

### Does it drift? (Q2)

No. Todo #37 × 12: max block 204, 297, 183, 187, 280, 175, 183, 195, 202, 244, 196, 200 ms; editors
0/1/0 and DOM nodes 2728 / 2900 / 2729 on every cycle. Scratchpad #62 × 12: total 713–821 ms, DOM
1928 / 2775 / 1929 on every cycle, editors 0/1/0. #9 × 3: DOM 1928 / 4580 / 1929. One outlier: the
first #24 open after #61 (10 mermaid diagrams) showed a 1203 ms block *before* the detail appeared;
three isolated re-runs of #24 were normal (754–787 ms), so it is most likely #61's diagram queue
still draining, as §2 noted for that document.

### F2 in the real engine (Q3)

`CSS.supports("overflow-anchor","none")` → `false`. Fresh element: `style.overflowAnchor` is
`undefined` (`== null` true, `"overflowAnchor" in style` false, not on
`CSSStyleDeclaration.prototype`); after `= "none"` it reads `"none"`, is an own property of the
style object (`Object.getOwnPropertyDescriptor` present), `cssText`/`getPropertyValue` stay empty,
and `el.style === el.style` so it persists. On the open editor it reads `"none"` with `data-editable
= "false"`. Behaviourally, `elementFromPoint` is called 0 times per open (the `storeScrollPos` loop
is skipped) and `getBoundingClientRect` once. F2 is real, not decoration; it simply had no block to
save in the clean measurement.

### Scroll stability under `content-visibility: auto` (Q4)

#9 (98,325 chars, 140 blocks) in the detail's `overflow-auto` container (877 px tall):

- Page-sized steps (0.8 × viewport, fresh instance): 44 steps down. The scroll range grew
  **11,454 → 31,567 px** over the descent (12 steps changed `scrollHeight` after the scroll), and
  on 4 of them the block under the viewport centre moved **181–589 px** within 120 ms of the scroll
  (`scrollTop` itself never drifted). The ascent was stable (0 changes), because `auto` keeps the
  rendered sizes.
- Wheel-sized steps (120 px, fresh instance): 256 steps down: `scrollHeight` changed after 45 of
  them (the thumb visibly shrinks as you read), content under the centre moved on **0**; 256 up: 0
  and 0.

So a reader scrolling with the wheel sees the scrollbar drift but no jump; a reader who drags the
thumb or pages down can land on content that then shifts by up to a screen. The cause is the
placeholder: `contain-intrinsic-size: auto 1.5em` = 19.5 px per unrendered block against a real
average of ~225 px (31,497 / 140), a 10× under-estimate. A larger estimate would shrink both effects.

### F6 after-number (Q5)

On the rendered #9, three forced style+layout passes each: as-is (F5+F6) **146 / 131 / 114 ms**,
repeated 122 / 109 / 132; with `content-visibility: visible` forced in-page 306 / 297 / 325 ms;
laid-out height 11,474 px vs 31,497 px without F6 (the report's 36,637 px was the headless 640 px
column). F5 made no difference in either state.

### Still slow enough to notice

1. **A ~100–130 ms page style+layout at the reveal, on every open, independent of document size.**
   It is the largest single cost on a todo now and is not the list panel (F4 A/B). Next target:
   attribute it (style resolution vs layout; the detail pane's own subtree) before touching CSS.
2. **The parse.** 92 ms at 20k chars, 670 ms at 98k: only F8 (cache the parsed document per
   `id:revision`, or a lighter read-only renderer) makes long notes fast.
3. **F3's price.** Content appears ~250 ms later than before; the freeze moved from mid-slide to
   after the slide, and on a todo it is ~200 ms of frozen skeleton at the end of a smooth slide. Fine
   for long notes; on a 2.5k-char todo, with parse at 4 ms, a build at settle minus a frame or two
   would recover some of it.
4. The opening commit's 50–150 ms (`focus` forcing the detail's first layout) still delays the
   slide's first frame.

Coverage: Q1–Q5 answered with observed numbers; nothing in this section is estimated.
