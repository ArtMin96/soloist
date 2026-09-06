import { useDeferredValue, useMemo, useState } from "react";
import { CARD_TRIGGER_ATTRIBUTE } from "@/components/common/CardRow";
import { CollapsibleGroup } from "@/components/common/CollapsibleGroup";
import { SlidingPanels } from "@/components/common/SlidingPanels";
import { TodoCreateForm } from "@/components/orchestration/TodoCreateForm";
import { TodoDetail, type TodoEditState } from "@/components/orchestration/TodoDetail";
import { TodoItem } from "@/components/orchestration/TodoItem";
import { TodoToolbar } from "@/components/orchestration/TodoToolbar";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { distinctTags } from "@/store/boardFilter";
import { createNavigationLedger, useMasterDetail } from "@/store/useMasterDetail";
import { useCollapseState } from "@/store/useCollapseState";
import { useTodoActions } from "@/store/useTodoActions";
import { useTodoEditor } from "@/store/useTodoEditor";
import { groupTodosByScratchpad } from "@/store/todoGrouping";
import { EMPTY_TODO_FILTER, filterTodos, isFiltering, type TodoFilter } from "@/store/todoFilter";
import type { BoardView } from "@/lib/todo";
import type { AgentNode, ScratchpadSummary, TodoView } from "@/domain";

/** Namespaces this board's persisted collapse keys so they cannot collide with the sidebar's. */
const COLLAPSE_PREFIX = "todos.scratchpad";

// Module level, so it outlives the board: the orchestration pane unmounts this component whenever
// the user switches view and re-delivers the same activation to the fresh one — see
// `NavigationLedger` for why that cannot be told apart from a genuine navigation at mount.
const ledger = createNavigationLedger();

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
  const [filter, setFilter] = useState<TodoFilter>(EMPTY_TODO_FILTER);
  const [view, setView] = useState<BoardView>("grouped");
  const [collapsed, setCollapsed] = useCollapseState();

  const tags = useMemo(() => distinctTags(todos), [todos]);
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

  // Ends the open edit session whenever navigation leaves the todo it belongs to, so unsaved edits
  // can neither resurface on a later re-open nor follow the board onto a different todo.
  const endEditUnless = (id: number | null) => {
    if (editor.mode === "edit" && editor.editingId !== id) editor.close();
  };

  const master = useMasterDetail<number>({
    project,
    ledger,
    present: (id) => todos.some((todo) => todo.id === id),
    rowTrigger: (id) => `[data-todo-id="${id}"] [${CARD_TRIGGER_ATTRIBUTE}]`,
    focusKey: focusId,
    focusNonce,
    onOpen: endEditUnless,
    onLeave: () => endEditUnless(null),
  });

  // Resolved against the whole snapshot, never the filtered set: the toolbar's filter belongs to the
  // list panel, and searching there must not slam an open detail shut.
  const detailTodo =
    master.detailKey != null ? todos.find((todo) => todo.id === master.detailKey) : undefined;

  const startCreate = () => {
    master.showList();
    editor.startCreate();
  };

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
        onOpen={() => master.open(todo.id)}
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
              <CollapsibleGroup
                key={group.key}
                label={group.label}
                count={group.todos.length}
                open={!collapsed[`${COLLAPSE_PREFIX}.${group.key}`]}
                onOpenChange={(open) => setCollapsed(`${COLLAPSE_PREFIX}.${group.key}`, !open)}
              >
                <ul className="flex flex-col gap-2 pt-1 pb-2 pl-1">{group.todos.map(card)}</ul>
              </CollapsibleGroup>
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
      <SlidingPanels
        showing={master.showing}
        list={list}
        detail={
          detailTodo && (
            <TodoDetail
              todo={detailTodo}
              onBack={master.back}
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
        onSettled={master.onSettled}
      />
    </div>
  );
}
