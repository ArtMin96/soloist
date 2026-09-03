import { useDeferredValue, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { TodoCreateForm } from "@/components/orchestration/TodoCreateForm";
import { TodoDetail, type TodoEditState } from "@/components/orchestration/TodoDetail";
import { TodoGroup } from "@/components/orchestration/TodoGroup";
import { TodoItem } from "@/components/orchestration/TodoItem";
import { TodoPanels, type TodoPanel } from "@/components/orchestration/TodoPanels";
import { TodoToolbar } from "@/components/orchestration/TodoToolbar";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { useCollapseState } from "@/store/useCollapseState";
import { useTodoActions } from "@/store/useTodoActions";
import { useTodoEditor } from "@/store/useTodoEditor";
import { groupTodosByScratchpad } from "@/store/todoGrouping";
import {
  EMPTY_TODO_FILTER,
  filterTodos,
  isFiltering,
  todoTags,
  type TodoFilter,
} from "@/store/todoFilter";
import type { BoardView } from "@/lib/todo";
import type { AgentNode, ScratchpadSummary, TodoView } from "@/domain";

/** Namespaces this board's persisted collapse keys so they cannot collide with the sidebar's. */
const COLLAPSE_PREFIX = "todos.scratchpad";

// The last activation each project's board has already navigated for. A `focusNonce` is one
// navigation, not a standing instruction — but the pane leaves it set on the props after acting on
// it and unmounts the board whenever the user switches to another orchestration view, so a fresh
// board would see the same activation still standing and open a detail nobody asked for. The board
// cannot tell that case from a genuine one: an activation legitimately arrives at mount too, since
// opening a todo from a terminal mounts the board with the nonce already on its props. So the fact
// has to be remembered somewhere that outlives the component, which is here. One entry per project
// the user navigates into, holding one number.
const navigatedNonces = new Map<number, number>();

/**
 * The detail panel's target. `showing` is the route — false while the panel slides back out, which
 * is what keeps the todo rendered for the length of that movement rather than blanking on the way.
 */
interface DetailTarget {
  id: number;
  showing: boolean;
}

// Moves DOM focus into the panel a route change just brought on screen, aiming at `within` when that
// panel offers it and at the panel itself when it does not. Queried by the same `data-todo-*` handles
// the end-to-end walks use, rather than threading refs down through two panels' components. The
// scroll is asked for explicitly and refused to `focus`, so bringing a row back into view stays
// vertical and neither call can drag the panel track sideways.
function focusPanel(panel: TodoPanel, within: string) {
  const root = document.querySelector<HTMLElement>(`[data-todo-panel="${panel}"]`);
  const target = root?.querySelector<HTMLElement>(within) ?? root;
  target?.scrollIntoView({ block: "nearest" });
  target?.focus({ preventScroll: true });
}

// The to-do board: the project's shared work items, filterable and fully editable. The todos come
// from the live snapshot (refreshed on TodoChanged); every write — create, edit, complete, comment —
// routes through the same core commands agents use (the editor and action hooks are the only IPC
// here). Editing is revision-guarded: the board watches the live revision to raise the conflict
// banner when a concurrent write moves a todo out from under an open editor. Blocker titles and a
// lock owner's label are resolved from the same snapshot, so the board names them, not bare ids.
//
// The board is two panels, not one list. Opening a todo hands the whole pane to its detail, where
// the document, its blockers and its discussion each get a width a dense list cannot give them, and
// Back returns to the list exactly as it was left — filter, grouping and scroll position included.
// At most one todo is open, and any edit session belongs to it, so leaving that todo ends the
// session rather than letting an unsaved draft outlive the panel showing it.
//
// Cards are grouped by the scratchpad each todo derives from by default, because that is the shape
// the work actually has — tasks extracted from a plan belong under it. `All` flattens the board for
// triage, when the question is "what is open" rather than "what came from where". Filtering flattens
// it too: a search is already a triage question, and headers over one or two surviving cards each
// would bury the matches they are meant to organise. A card never names its own scratchpad in either
// view — the detail panel is where a todo's provenance is stated, and repeating it on every card
// would spend a line of the densest surface on what the header above already said. Grouping is a
// wrapper: the cards are identical in both views.
export function TodoBoard({
  project,
  todos,
  agents,
  scratchpads,
  onOpenAgent,
  focusId,
  focusNonce,
}: {
  project: number;
  todos: TodoView[];
  agents: AgentNode[];
  scratchpads: ScratchpadSummary[];
  /** Opens the agent a card is locked by — absent when the caller offers no navigation. */
  onOpenAgent?: (process: number) => void;
  /** The todo to open the detail panel on when `focusNonce` changes — cross-surface navigation. */
  focusId?: number;
  /** Bumped to re-trigger the navigation above, even to repeat the same `focusId`. */
  focusNonce?: number;
}) {
  const actions = useTodoActions(project);
  const editor = useTodoEditor(project);
  const [detail, setDetail] = useState<DetailTarget | null>(null);
  const [filter, setFilter] = useState<TodoFilter>(EMPTY_TODO_FILTER);
  const [view, setView] = useState<BoardView>("grouped");
  const [collapsed, setCollapsed] = useCollapseState();

  const tags = useMemo(() => todoTags(todos), [todos]);
  // Filtering is the toolbar's own render — the search box, status select and tag chips must track
  // every keystroke and click exactly, so they stay bound to the live `filter`. Everything downstream
  // of it (the filtered rows, their grouping, and whether to group at all) is what can lag: deferring
  // the filter, not just `visible`, keeps those three in step with each other so a filtered flat list
  // and a "no matches" empty state never render for a beat over rows still keyed to the old filter.
  const deferredFilter = useDeferredValue(filter);
  const visible = useMemo(() => filterTodos(todos, deferredFilter), [todos, deferredFilter]);
  const groups = useMemo(() => groupTodosByScratchpad(visible), [visible]);
  const grouped = view === "grouped" && !isFiltering(deferredFilter);

  const titleOf = (id: number) => todos.find((todo) => todo.id === id)?.doc.title;
  const labelOf = (id: number) => agents.find((agent) => agent.id === id)?.label;

  // Resolved against the whole snapshot, never the filtered set: the toolbar's filter belongs to the
  // list panel, and searching there must not slam an open detail shut.
  const detailTodo = detail != null ? todos.find((todo) => todo.id === detail.id) : undefined;
  const showing: TodoPanel = detail?.showing ? "detail" : "list";

  const pendingFocusRef = useRef<{ panel: TodoPanel; within: string } | null>(null);

  // Ends the open edit session whenever navigation leaves the todo it belongs to, so unsaved edits
  // can neither resurface on a later re-open nor follow the board onto a different todo.
  const endEditUnless = (id: number | null) => {
    if (editor.mode === "edit" && editor.editingId !== id) editor.close();
  };

  const openDetail = (id: number) => {
    endEditUnless(id);
    setDetail({ id, showing: true });
    pendingFocusRef.current = { panel: "detail", within: "[data-todo-back]" };
  };

  const showList = () => setDetail((current) => (current ? { ...current, showing: false } : null));

  const back = () => {
    const from = detail?.id;
    endEditUnless(null);
    showList();
    pendingFocusRef.current = {
      panel: "list",
      within: `[data-todo-id="${from}"] [data-todo-trigger]`,
    };
  };

  const startCreate = () => {
    showList();
    editor.startCreate();
  };

  // A route change is the one moment focus can be lost: the panel leaving goes inert, and focus left
  // inside it would fall to the document body and restart keyboard traversal at the top of the app.
  // Laid out rather than deferred, so there is no painted frame in between; read from a ref rather
  // than from the route, so a repeat navigation to the todo already open still refocuses it.
  useLayoutEffect(() => {
    const pending = pendingFocusRef.current;
    if (pending == null) return;
    pendingFocusRef.current = null;
    focusPanel(pending.panel, pending.within);
  });

  // A todo can vanish from under an open detail panel — deleted, or moved out of this project. There
  // is nothing left to show or edit, so the board drops straight back to the list rather than holding
  // a panel over a todo that no longer exists. Only while the panel is showing: one already sliding
  // out is cleared by `onSettled` a beat later, and cutting it short would blank it mid-movement.
  // Adjusted here during render, keyed off `todos` itself changing, so the drop lands the same
  // render the todo disappears in rather than painting a dead panel for a frame first.
  if (detail != null && detail.showing && detailTodo == null) {
    setDetail(null);
    if (editor.mode === "edit" && editor.editingId === detail.id) editor.close();
  }

  // Whether the navigation target has actually arrived in the live snapshot. Coming from a freshly
  // mounted pane, `focusNonce` can be set before the first snapshot lands — `targetPresent` gates
  // the navigation below on that, and its own presence in the effect's deps is what makes the
  // navigation retry once the todo shows up, rather than silently missing it.
  const targetPresent = focusId != null && todos.some((todo) => todo.id === focusId);

  // Cross-surface navigation's inbound half: a fresh nonce opens that todo's detail panel directly,
  // so the target is on screen whatever the list panel's filter and grouping happen to be. An
  // activation already navigated for is spent, even across a remount — see `navigatedNonces`.
  useEffect(() => {
    if (focusId == null || focusNonce == null || !targetPresent) return;
    if (navigatedNonces.get(project) === focusNonce) return;
    // Marks the nonce spent before acting on it: a persistent module-level Map, not render state —
    // render must stay replayable (Strict Mode, discarded renders), and mutating it there would
    // mark a navigation spent that never actually happened. `openDetail` in turn writes
    // `pendingFocusRef.current`, a ref write that render itself may not make either — so the panel
    // open (and the ref-driven focus move that depends on it) genuinely belongs here, once, in
    // response to this external navigation event.
    navigatedNonces.set(project, focusNonce);
    // eslint-disable-next-line react-hooks/set-state-in-effect -- opens in response to an external navigation event (see above); the ref write it makes cannot happen during render.
    openDetail(focusId);
    // Only the navigation's own trigger conditions belong here — `openDetail` closes over the live
    // editor, a fresh object every render, which would re-fire this on every one.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [focusNonce, focusId, targetPresent, project]);

  // The edit surface for one todo, present only while it is the one being edited. A concurrent write
  // that moves the live todo past the opened revision is the conflict the editor pauses on.
  const editStateFor = (todo: TodoView): TodoEditState | null => {
    if (editor.mode !== "edit" || editor.editingId !== todo.id || editor.initial == null) {
      return null;
    }
    const conflict =
      editor.baseRevision != null && todo.revision > editor.baseRevision
        ? { actual: todo.revision }
        : null;
    return {
      initial: editor.initial,
      initialScratchpad: editor.scratchpad,
      mountKey: editor.mountKey,
      conflict,
      error: editor.error,
      onSave: editor.save,
      onReload: () => editor.reload(todo),
      onDone: editor.close,
    };
  };

  const card = (todo: TodoView) => (
    <li key={todo.id} data-todo-id={todo.id}>
      <TodoItem
        todo={todo}
        onOpen={() => openDetail(todo.id)}
        lockOwnerLabel={todo.locked_by != null ? labelOf(todo.locked_by) : undefined}
        onOpenAgent={onOpenAgent}
      />
    </li>
  );

  // The board's one accent-filled default action — and only while there is no form open, since the
  // form's own Create is then the default and two filled buttons would each claim to be it.
  const creating = editor.mode === "create";

  const list = (
    <>
      <TodoToolbar
        filter={filter}
        tags={tags}
        onChange={setFilter}
        view={view}
        onViewChange={setView}
        shown={visible.length}
        total={todos.length}
        onCreate={creating ? undefined : startCreate}
      />

      {creating && editor.initial && (
        <TodoCreateForm
          onCreate={editor.save}
          scratchpads={scratchpads}
          onCancel={editor.close}
          error={editor.error}
        />
      )}

      <div className="min-h-0 flex-1 overflow-auto">
        {visible.length === 0 ? (
          <Empty>
            <EmptyHeader>
              {isFiltering(deferredFilter) ? (
                <EmptyDescription>No todos match your search.</EmptyDescription>
              ) : (
                <>
                  <EmptyTitle>No todos yet</EmptyTitle>
                  <EmptyDescription>
                    Create one, or let agents create them to hand off and order work — they appear
                    here live, with their blockers, locks, and comments.
                  </EmptyDescription>
                </>
              )}
            </EmptyHeader>
          </Empty>
        ) : grouped ? (
          // Sections are plain containers, not list items: the cards stay the only list entries, so a
          // card is addressed the same way whichever view is showing.
          <div className="flex flex-col gap-2 px-3 pt-2 pb-2">
            {groups.map((group) => (
              <TodoGroup
                key={group.key}
                label={group.label}
                count={group.todos.length}
                open={!collapsed[`${COLLAPSE_PREFIX}.${group.key}`]}
                onOpenChange={(open) => setCollapsed(`${COLLAPSE_PREFIX}.${group.key}`, !open)}
              >
                <ul className="flex flex-col gap-2 pt-1 pb-2 pl-1">{group.todos.map(card)}</ul>
              </TodoGroup>
            ))}
          </div>
        ) : (
          <ul className="flex flex-col gap-2 px-3 py-2">{visible.map(card)}</ul>
        )}
      </div>
    </>
  );

  return (
    <div className="flex h-full min-h-0 flex-col tracking-[var(--tracking-body)]">
      <TodoPanels
        showing={showing}
        list={list}
        detail={
          detailTodo && (
            <TodoDetail
              todo={detailTodo}
              onBack={back}
              titleOf={titleOf}
              lockOwnerLabel={
                detailTodo.locked_by != null ? labelOf(detailTodo.locked_by) : undefined
              }
              onOpenAgent={onOpenAgent}
              busy={actions.busyId === detailTodo.id}
              error={actions.errorById[detailTodo.id]}
              onComplete={() => actions.complete(detailTodo.id)}
              onCopyLink={() => actions.copyLink(detailTodo.id)}
              onComment={(body) => actions.comment(detailTodo.id, body)}
              onStartEdit={() => editor.editTodo(detailTodo)}
              scratchpads={scratchpads}
              edit={editStateFor(detailTodo)}
            />
          )
        }
        // The detail's todo stays rendered until its panel has finished sliding out, so the panel
        // leaving carries the content the reader was looking at rather than blanking on the way.
        onSettled={() => setDetail((current) => (current?.showing === false ? null : current))}
      />
    </div>
  );
}
