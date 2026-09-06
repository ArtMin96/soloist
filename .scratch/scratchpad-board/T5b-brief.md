# T5b — Scratchpad board, toolbar, skeleton, filter, pane wiring, deletions

Wave 1 (shared kit + data layer), T4 (todo board on the kit) and T5a (scratchpad card/meta/detail/editor/create form) are all DONE and green. This is the last UI task before the e2e reconciliation.

## Settled contracts you consume (do not edit these files)

Kit — `components/common/`:
- `SlidingPanels.tsx`: `type SlidingPanel = "list" | "detail"`, `PANEL_ROUTE_ATTRIBUTE = "data-panel-route"`, `PANEL_ATTRIBUTE = "data-panel"`, `SlidingPanels({ showing, list, detail, onSettled, className? })`.
- `CollapsibleGroup.tsx`: `GROUP_ATTRIBUTE = "data-group"`, `CollapsibleGroup({ label, count, open, onOpenChange, children })`.
- `BoardToolbar.tsx`: `BOARD_TOOLBAR_ATTRIBUTE = "data-board-toolbar"`, `BOARD_COUNT_ATTRIBUTE = "data-board-count"`, `BoardToolbar({ subject, search, onSearchChange, facets?, shown, total, primary?, tags, tag, onTagChange })`. `subject` is the lowercase plural noun: placeholder `Search ${subject}…`, aria-label `Search ${subject}`.
- `BoardSkeleton.tsx`: `skeletonTitleWidth(index)`, `BoardSkeleton({ facets, rows, row })`.
- `CardRow.tsx`: `CARD_ROW_ATTRIBUTE = "data-card-row"`, `CARD_TRIGGER_ATTRIBUTE = "data-card-trigger"`, `CardRow({ onOpen, children, aside? })`, `CardRowStandIn({ children })`.
- `TagList.tsx`, `TagFilterChips.tsx` (moved here in wave 1).

Hooks/helpers:
- `store/useMasterDetail.ts`: `createNavigationLedger()`, `useMasterDetail<Key>({ project, ledger, present, rowTrigger, focusKey?, focusNonce?, onOpen?, onLeave?, onDrop? })` → `{ detailKey, showing, open, back, showList, onSettled }`.
- `store/boardFilter.ts`: `SearchTagFilter { search, tag }`, `matchesSearchAndTag(item, filter, fields)`, `isSearchingOrTagging(filter)`, `distinctTags(items)`.
- `store/useScratchpadEditor.ts`: `{ name, document: Loadable<ScratchpadView>, baseRevision, mountKey, conflict, error, open, close, save, reload, rename, copyLink }`. `open` sets `loading()`; `reload` keeps the held value; a landed `save` replaces `document` with `ready(view)`.
- `store/useScratchpadActions.ts`: `{ error, createError, create(name, body), archive(name, archived), exportMarkdown(name, body), copyMarkdown(name, body), clearError }`.
- `store/useCollapseState.ts`, `store/useScratchpadHotkeys.ts`, `store/scratchpadSort.ts` (`ScratchpadSort`, `SCRATCHPAD_SORT_ORDER`, `SCRATCHPAD_SORT_LABELS`, `sortScratchpads`), `lib/humanize.ts` (`humanizeName`, `distinctHandle`), `lib/format.ts` (`formatUpdatedAt`).

T5a components (settled, exact props):
```ts
ScratchpadCard({ pad: ScratchpadSummary; now: number; onOpen: () => void })
ScratchpadMeta({ pad: ScratchpadSummary; now: number })       // returns a fragment; wrap it
ScratchpadCreateForm({ onCreate: (name, body) => Promise<SaveOutcome>; onCancel: () => void; error: string | null })
interface ScratchpadEditState {
  mountKey: number; conflict: ScratchpadConflict | null; error: string | null;
  onSave: (markdown: string) => Promise<SaveOutcome>; onReload: () => void;
  onRename: (to: string) => Promise<void>; onDone: () => void; onBodyChange: (markdown: string) => void;
}
ScratchpadDetail({
  pad, document, onRetry, now, onBack, onStartEdit, onArchive, onCopyLink, onExport, onCopyMarkdown,
  error: string | null, edit: ScratchpadEditState | null,
})
```

## Your exclusive write set
Create: `components/orchestration/{ScratchpadBoard,ScratchpadBoard.test,ScratchpadToolbar,ScratchpadToolbar.test,ScratchpadBoardSkeleton}.tsx`, `store/scratchpadFilter.ts`, `store/scratchpadFilter.test.ts`.
Edit: `components/orchestration/OrchestrationPane.tsx`, `OrchestrationPane.test.tsx`, `components/orchestration/DocumentList.tsx` (line ~18 comment only: drop the `ScratchpadPanel` reference).
Delete: `components/orchestration/{ScratchpadPanel,ScratchpadPanel.test,ScratchpadRoster,ScratchpadBody}.tsx`.
Must not touch: any Todo* file, any other Scratchpad* file, `components/common/*`, `components/editor/*`, `store/*` other than `scratchpadFilter*`, `lib/*`, `App.tsx` (it needs no change), `DocumentTitle/Roster/Panel`, `Diagram*`, `e2e/`.

## What to build

### `store/scratchpadFilter.ts`
```ts
export type ArchivedFilter = "all" | "active" | "archived";
export interface ScratchpadFilter extends SearchTagFilter { archived: ArchivedFilter }
export const EMPTY_SCRATCHPAD_FILTER: ScratchpadFilter;            // { search: "", archived: "all", tag: null }
export const ARCHIVED_FILTER_ORDER: ArchivedFilter[];              // ["all","active","archived"]
export const ARCHIVED_FILTER_LABELS: Record<ArchivedFilter, string>; // All / Active / Archived
export function filterScratchpads(pads: ScratchpadSummary[], filter: ScratchpadFilter): ScratchpadSummary[];
export function isFilteringScratchpads(filter: ScratchpadFilter): boolean;
```
Search fields: `name`, `humanizeName(name)`, `gist` — through `matchesSearchAndTag`, never a second search body.

### `ScratchpadToolbar.tsx`
`ScratchpadToolbar({ filter, tags, onChange, sort, onSortChange, shown, total, onCreate? })` composing `BoardToolbar subject="scratchpads"` with the archived `Select` (aria-label "Filter by archived state") and the sort `Select` (aria-label "Sort scratchpads", options from `SCRATCHPAD_SORT_LABELS`) as `facets`, and a "New scratchpad" `Button` as `primary` when `onCreate` is given.

### `ScratchpadBoard.tsx` (target under 260 lines)
`ScratchpadBoard({ project, scratchpads, focusName?, focusNonce? })`.
Module level: `const ledger = createNavigationLedger()`, `COLLAPSE_PREFIX = "scratchpads"`, `ARCHIVED_GROUP = "archived"`, `ARCHIVED_GROUP_LABEL = "Archived"`.
State: `filter: ScratchpadFilter`, `sort: ScratchpadSort` (initial `"updated"`), `creating: boolean`, `mode: "read" | "edit"`, a ref holding the editor's latest Markdown so export/copy act on unsaved text.
```ts
const master = useMasterDetail<number>({
  project, ledger,
  present: id => scratchpads.some(p => p.id === id),
  rowTrigger: id => `[data-scratchpad-id="${id}"] [${CARD_TRIGGER_ATTRIBUTE}]`,
  focusKey: scratchpads.find(p => p.name === focusName)?.id,
  focusNonce,
  onOpen: id => { setMode("read"); editor.open(nameOf(id)); },
  onLeave: () => setMode("read"),
  onDrop: () => editor.close(),
});
```
Read-mode re-read: an effect that calls `editor.reload()` when `mode === "read"`, a detail is open, the document is Ready, and the live summary's `revision` is greater than the loaded document's revision.
Hotkey: `useScratchpadHotkeys(rootRef, detailPad ? () => actions.archive(detailPad.name, !detailPad.archived) : undefined)`.
Rows: `<li key={pad.id} data-scratchpad-id={pad.id} data-scratchpad-name={pad.name}><ScratchpadCard pad now onOpen/></li>`.
Grouping: when `!isFilteringScratchpads(deferredFilter)`, active pads render in a flat `<ul>` first, then, only when some are archived, a `CollapsibleGroup label="Archived"` with the archived count, open state from `useCollapseState` under key `` `${COLLAPSE_PREFIX}.${ARCHIVED_GROUP}` `` (so `scratchpads.archived`). While filtering, one flat sorted `<ul>` and no group.
Filtering uses a `useDeferredValue` copy of the filter so the searchbox stays on the live keystroke.
`now = Date.now()` once per board render, passed to the cards and the detail.

### `ScratchpadBoardSkeleton.tsx`
`BoardSkeleton` with two facet stand-ins (matching the archived and sort selects' widths), `rows` a named const, and a `CardRowStandIn` row drawing a title bar (`skeletonTitleWidth(i)`), three short meta pills and a gist bar.

### `OrchestrationPane.tsx`
Replace the `ScratchpadPanel` import and JSX with `ScratchpadBoard` (same four props: `project`, `scratchpads`, `focusName`, `focusNonce`); `VIEW_SKELETON.scratchpads` becomes `<ScratchpadBoardSkeleton />`. In `OrchestrationPane.test.tsx`, add typed stubs for `useScratchpadEditor` and `useScratchpadActions` alongside the existing todo stubs (the `@/api` factory mock throws on an undefined export, so stub the hooks, do not extend the api mock) and one new case: the scratchpads view shows the stand-in while the first snapshot is in flight and "No scratchpads yet" only after it lands empty.

## Copy (verbatim)
Empty board: "No scratchpads yet" + "Agents create them to share a plan or research as they work. They appear here live."
No results: "No scratchpads match your search."
Archived facet aria-label "Filter by archived state"; sort aria-label "Sort scratchpads"; primary "New scratchpad".

## Tests, each observed red first
`scratchpadFilter.test.ts`: search matches name, humanized name ("rich editor" finds `rich-editor-design`) and gist; the archived facet narrows; a tag ANDs with the search; `isFilteringScratchpads` is false only for `EMPTY_SCRATCHPAD_FILTER`.
`ScratchpadToolbar.test.tsx`: emits the filter with only `search` / only `archived` / only `tag` changed; sort emits `onSortChange("name")`; the count renders under the count handle; "New scratchpad" is absent without `onCreate`.
`ScratchpadBoard.test.tsx` (stub `useScratchpadEditor` and `useScratchpadActions` as typed stores the way `TodoBoard.test` stubs the todo hooks; provide a `localStorage` stand-in and a `TooltipProvider`):
- active pads then an "Archived" group carrying its count; no group when nothing is archived.
- collapsing "Archived" persists across an unmount/remount, stored under the key `scratchpads.archived` (assert the stored key; red if the prefix is wrong or absent).
- a search flattens the board (no group) and updates the count; clearing restores the grouping.
- a 3000-row board keeps the typed value synchronous while the rows settle from the deferred filter.
- both empty states: "No scratchpads yet" with nothing, "No scratchpads match your search." with rows but no match.
- sort Name vs Recent changes the order of `[data-scratchpad-name]`.
- one create action at a time: opening the form hides the toolbar's primary; a refused create keeps the form open; a successful one closes it and the route stays "list".
- opening a card → route "detail", the list panel `inert`, focus on `[data-detail-back]`; Back → focus back on that card's trigger.
- cross-surface focus: a new nonce with a name in the snapshot opens that detail even when a filter or a collapsed group hides the row; a nonce landing before the snapshot carries the name retries when it appears; a nonce already acted on is not replayed after a remount.
- in read mode, a rerender whose summary `revision` moved past the loaded document calls the stubbed `reload` once; in edit mode it does not.
- the archive hotkey archives the open scratchpad and does nothing with no detail open. Provide `HotkeysContext` with one binding: `{ action: "archive_scratchpad", scope: "scratchpad", binding: { ctrl: true, alt: false, shift: true, super: false, key: "W" }, is_default: true, conflict: false }`.

## Discipline
No `@/api` import in any component — route through the hooks. Doc comments on exports; comments only for non-obvious decisions; never a phase number, plan reference, rule id, or a restatement of the code. No magic strings: import the kit's handle constants; name every copy string and threshold. Files under 400 lines. Delete the four old files LAST, once `OrchestrationPane` compiles against `ScratchpadBoard`, then confirm with `rg "ScratchpadPanel|ScratchpadRoster|ScratchpadBody|DocumentPlaceholder" crates/app/ui/src` → only `DiagramPanel.tsx` and `DocumentPanel.tsx` (diagrams) should remain.

## Verify
`pnpm -C crates/app/ui exec vitest run src/components/orchestration/Scratchpad src/store/scratchpadFilter.test.ts src/components/orchestration/OrchestrationPane.test.tsx src/components/orchestration/DocumentList.test.tsx` green; `pnpm -C crates/app/ui typecheck` clean (the one pre-existing error in `ScratchpadPanel.tsx` disappears when you delete it); `wc -l` of every new file under 400; `rg "from \"@/api\"" crates/app/ui/src/components/orchestration/Scratchpad*.tsx` → nothing.
