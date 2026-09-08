import { useDeferredValue, useEffect, useMemo, useRef, useState } from "react";
import { BOARD_CREATE_ATTRIBUTE } from "@/components/common/BoardToolbar";
import { CARD_TRIGGER_ATTRIBUTE } from "@/components/common/CardRow";
import { CollapsibleGroup } from "@/components/common/CollapsibleGroup";
import { SlidingPanels } from "@/components/common/SlidingPanels";
import { ScratchpadCard } from "@/components/orchestration/ScratchpadCard";
import { ScratchpadCreateForm } from "@/components/orchestration/ScratchpadCreateForm";
import {
  ScratchpadDetail,
  type ScratchpadEditState,
} from "@/components/orchestration/ScratchpadDetail";
import { ScratchpadToolbar } from "@/components/orchestration/ScratchpadToolbar";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { distinctTags } from "@/store/boardFilter";
import { LoadStatus } from "@/store/loadable";
import {
  EMPTY_SCRATCHPAD_FILTER,
  filterScratchpads,
  isFilteringScratchpads,
  type ScratchpadFilter,
} from "@/store/scratchpadFilter";
import { sortScratchpads, type ScratchpadSort } from "@/store/scratchpadSort";
import { useCollapseState } from "@/store/useCollapseState";
import { createNavigationLedger, useMasterDetail } from "@/store/useMasterDetail";
import { useScratchpadActions } from "@/store/useScratchpadActions";
import { useScratchpadEditor } from "@/store/useScratchpadEditor";
import { useScratchpadHotkeys } from "@/store/useScratchpadHotkeys";
import type { ScratchpadSummary } from "@/domain";

/** The handle a row carries, so the route can aim focus back at the card a detail was opened from. */
export const SCRATCHPAD_ROW_ID_ATTRIBUTE = "data-scratchpad-id";
/** The handle naming which scratchpad a row is — how a reader addresses one by name. */
export const SCRATCHPAD_ROW_NAME_ATTRIBUTE = "data-scratchpad-name";

/** Namespaces this board's persisted collapse key so it cannot collide with another list's. */
const COLLAPSE_PREFIX = "scratchpads";
/** The one section the board draws, and the header it wears. */
const ARCHIVED_GROUP = "archived";
const ARCHIVED_GROUP_LABEL = "Archived";
const ARCHIVED_COLLAPSE_KEY = `${COLLAPSE_PREFIX}.${ARCHIVED_GROUP}`;

/** The order a board opens on: what was written most recently is what a reader is looking for. */
const DEFAULT_SORT: ScratchpadSort = "updated";

// Which of the two surfaces the open scratchpad's pane is showing. At most one scratchpad is open,
// so this belongs to the board rather than to the document: leaving it ends the session.
type PaneMode = "read" | "edit";

// Module level, so it outlives the board: the orchestration pane unmounts this component whenever
// the user switches view and re-delivers the same activation to the fresh one — see
// `NavigationLedger` for why that cannot be told apart from a genuine navigation at mount.
const ledger = createNavigationLedger();

// The scratchpad board: the project's shared notes, filterable and fully editable. The summaries
// come from the live snapshot (refreshed on ScratchpadChanged); the open document's body and every
// write route through the editor and action hooks, the only IPC here. Editing is revision-guarded,
// and while a document is merely being read the board follows the snapshot's revision, so a note an
// agent is writing into stays current under the reader without ever moving text under an editor.
//
// The board is two panels, not one list. Opening a scratchpad hands the whole pane to its document,
// where prose gets a width a roster beside it cannot give, and Back returns to the list exactly as
// it was left — filter, sort, grouping and scroll position included.
//
// Archived scratchpads are kept, not hidden: they sit in one collapsible section under the active
// ones, so a superseded plan stays reachable without competing with live work. Filtering flattens
// the board — a search is already a triage question, and a header over the surviving matches would
// bury them.
export function ScratchpadBoard({
  project,
  scratchpads,
  focusName,
  focusNonce,
}: {
  project: number;
  scratchpads: ScratchpadSummary[];
  /** The scratchpad to open the detail panel on when `focusNonce` changes — cross-surface navigation. */
  focusName?: string;
  /** Bumped to re-trigger the navigation above, even to repeat the same `focusName`. */
  focusNonce?: number;
}) {
  const actions = useScratchpadActions(project);
  const editor = useScratchpadEditor(project);
  const rootRef = useRef<HTMLDivElement>(null);
  const [filter, setFilter] = useState<ScratchpadFilter>(EMPTY_SCRATCHPAD_FILTER);
  const [sort, setSort] = useState<ScratchpadSort>(DEFAULT_SORT);
  const [mode, setMode] = useState<PaneMode>("read");
  const [collapsed, setCollapsed] = useCollapseState();

  // One clock reading for the whole board, shared by every card and the open document, so a column
  // of recency stamps cannot disagree with itself by the milliseconds between two of them. Taken
  // once per visit rather than per render: the clock may not be read while rendering, and a reading
  // that moved under a re-render would make two stamps drift apart anyway.
  const [now] = useState(() => Date.now());

  const tags = useMemo(() => distinctTags(scratchpads), [scratchpads]);
  // Filtering is the toolbar's own render — the search box, the facets and the tag chips must track
  // every keystroke and click exactly, so they stay bound to the live `filter`. Everything
  // downstream of it (the rows, their grouping, and whether to group at all) is what can lag.
  const deferredFilter = useDeferredValue(filter);
  const visible = useMemo(
    () => sortScratchpads(filterScratchpads(scratchpads, deferredFilter), sort),
    [scratchpads, deferredFilter, sort],
  );
  const grouped = !isFilteringScratchpads(deferredFilter);
  const active = useMemo(() => visible.filter((pad) => !pad.archived), [visible]);
  const retired = useMemo(() => visible.filter((pad) => pad.archived), [visible]);

  const master = useMasterDetail<number>({
    project,
    ledger,
    present: (id) => scratchpads.some((pad) => pad.id === id),
    rowTrigger: (id) => `[${SCRATCHPAD_ROW_ID_ATTRIBUTE}="${id}"] [${CARD_TRIGGER_ATTRIBUTE}]`,
    createTrigger: `[${BOARD_CREATE_ATTRIBUTE}="scratchpad"]`,
    focusKey: scratchpads.find((pad) => pad.name === focusName)?.id,
    focusNonce,
    onOpen: (id) => {
      setMode("read");
      const name = scratchpads.find((pad) => pad.id === id)?.name;
      if (name != null) editor.open(name);
    },
    onLeave: () => setMode("read"),
    onDrop: () => editor.close(),
  });

  // Resolved against the whole snapshot, never the filtered set: the toolbar's filter belongs to the
  // list panel, and searching there must not slam an open document shut.
  const detailPad =
    master.detailKey != null ? scratchpads.find((pad) => pad.id === master.detailKey) : undefined;

  const documentRead = editor.document;
  const loaded = documentRead.status === LoadStatus.Ready ? documentRead.value : null;

  // The text an export or a copy hands off: seeded from the read and advanced by every keystroke, so
  // what leaves the app is what is on screen rather than what was last saved.
  const bodyRef = useRef("");
  const loadedBody = loaded?.body ?? "";
  useEffect(() => {
    bodyRef.current = loadedBody;
  }, [loadedBody]);

  // A concurrent write moves the summary's revision past the body on screen. While the document is
  // only being read, the reader should be looking at what it now says, so the board re-reads it —
  // never while editing, where a reload would replace the text under the caret.
  const { reload } = editor;
  const outdated =
    mode === "read" && detailPad != null && loaded != null && detailPad.revision > loaded.revision;
  useEffect(() => {
    if (outdated) reload();
  }, [outdated, reload]);

  const archiveDetail = detailPad
    ? () => actions.archive(detailPad.name, !detailPad.archived)
    : undefined;
  useScratchpadHotkeys(rootRef, archiveDetail);

  const startCreate = () => {
    master.startCreate();
  };

  // The form stays open on a refusal — a name already taken is the core's to reject, and the draft
  // the user typed is not thrown away over it.
  const create = async (name: string, body: string) => {
    const outcome = await actions.create(name, body);
    if (outcome === "saved") master.back();
    return outcome;
  };

  // The edit surface for the open scratchpad, present only while it is being written to.
  const editState: ScratchpadEditState | null =
    mode === "edit" && detailPad != null
      ? {
          mountKey: editor.mountKey,
          conflict: editor.conflict,
          error: editor.error,
          onSave: editor.save,
          onReload: editor.reload,
          onRename: editor.rename,
          onDone: () => setMode("read"),
          onBodyChange: (markdown) => {
            bodyRef.current = markdown;
          },
        }
      : null;

  const card = (pad: ScratchpadSummary) => (
    <li
      key={pad.id}
      {...{ [SCRATCHPAD_ROW_ID_ATTRIBUTE]: pad.id, [SCRATCHPAD_ROW_NAME_ATTRIBUTE]: pad.name }}
    >
      <ScratchpadCard pad={pad} now={now} onOpen={() => master.open(pad.id)} />
    </li>
  );

  const list = (
    <>
      <ScratchpadToolbar
        filter={filter}
        tags={tags}
        onChange={setFilter}
        sort={sort}
        onSortChange={setSort}
        shown={visible.length}
        total={scratchpads.length}
        onCreate={startCreate}
      />

      <div className="min-h-0 flex-1 overflow-auto">
        {visible.length === 0 ? (
          <Empty>
            <EmptyHeader>
              {isFilteringScratchpads(deferredFilter) ? (
                <EmptyDescription>No scratchpads match your search.</EmptyDescription>
              ) : (
                <>
                  <EmptyTitle>No scratchpads yet</EmptyTitle>
                  <EmptyDescription>
                    Agents create them to share a plan or research as they work. They appear here
                    live.
                  </EmptyDescription>
                </>
              )}
            </EmptyHeader>
          </Empty>
        ) : grouped ? (
          // The section is a plain container, not a list item: the cards stay the only list entries,
          // so a card is addressed the same way whichever arrangement is showing.
          <div className="flex flex-col gap-2 px-3 py-2">
            {active.length > 0 && <ul className="flex flex-col gap-2">{active.map(card)}</ul>}
            {retired.length > 0 && (
              <CollapsibleGroup
                label={ARCHIVED_GROUP_LABEL}
                count={retired.length}
                open={!collapsed[ARCHIVED_COLLAPSE_KEY]}
                onOpenChange={(open) => setCollapsed(ARCHIVED_COLLAPSE_KEY, !open)}
              >
                <ul className="flex flex-col gap-2 pt-1 pb-2 pl-1">{retired.map(card)}</ul>
              </CollapsibleGroup>
            )}
          </div>
        ) : (
          <ul className="flex flex-col gap-2 px-3 py-2">{visible.map(card)}</ul>
        )}
      </div>
    </>
  );

  return (
    <div ref={rootRef} className="flex h-full min-h-0 flex-col tracking-[var(--tracking-body)]">
      <SlidingPanels
        showing={master.showing}
        list={list}
        detail={
          master.creating ? (
            <ScratchpadCreateForm
              onCreate={create}
              onCancel={master.back}
              error={actions.createError}
            />
          ) : (
            detailPad && (
              <ScratchpadDetail
                pad={detailPad}
                document={documentRead}
                onRetry={editor.reload}
                now={now}
                onBack={master.back}
                onStartEdit={() => setMode("edit")}
                onArchive={() => actions.archive(detailPad.name, !detailPad.archived)}
                onCopyLink={() => editor.copyLink(detailPad.id)}
                onExport={() => actions.exportMarkdown(detailPad.name, bodyRef.current)}
                onCopyMarkdown={() => actions.copyMarkdown(detailPad.name, bodyRef.current)}
                // While editing, the editor states its own refusals beside the text; the pane's line
                // then carries only what the header's own actions were refused.
                error={actions.error ?? (mode === "read" ? editor.error : null)}
                edit={editState}
              />
            )
          )
        }
        onSettled={master.onSettled}
      />
    </div>
  );
}
