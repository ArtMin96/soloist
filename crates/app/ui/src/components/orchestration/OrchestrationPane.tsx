import { useDeferredValue, useState, type ReactNode } from "react";
import { LoadableRegion } from "@/components/common/LoadableRegion";
import { SkeletonList } from "@/components/common/SkeletonList";
import { DiagramPanel } from "@/components/orchestration/DiagramPanel";
import { MessagesPanel } from "@/components/orchestration/MessagesPanel";
import { OrchestrationTree } from "@/components/orchestration/OrchestrationTree";
import { ScratchpadPanel } from "@/components/orchestration/ScratchpadPanel";
import { TimersPanel } from "@/components/orchestration/TimersPanel";
import { TodoBoard } from "@/components/orchestration/TodoBoard";
import { TodoBoardSkeleton } from "@/components/orchestration/TodoBoardSkeleton";
import { SegmentedControl } from "@/components/SegmentedControl";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { LoadStatus, loading } from "@/store/loadable";
import { monogram } from "@/store/projects";
import { useOrchestration, type OrchestrationReadModel } from "@/store/useOrchestration";
import type { OrchestrationFocus } from "@/components/orchestration/orchestrationFocus";
import type { Option } from "@/lib/appearance";
import type { ProjectView } from "@/domain";

type View = "agents" | "todos" | "scratchpads" | "diagrams" | "timers" | "messages";

// The one name each view has: the switcher offers it, and the body says what it is waiting for by
// lowercasing it ("Loading to-dos"). Key order is the order the switcher offers them in, and the
// record refuses to compile until a new view is named, where a hand-written list could omit one.
const VIEW_LABEL: Record<View, string> = {
  agents: "Agents",
  todos: "To-dos",
  scratchpads: "Scratchpads",
  diagrams: "Diagrams",
  timers: "Timers",
  messages: "Messages",
};

const VIEW_OPTIONS: Option<View>[] = (Object.keys(VIEW_LABEL) as View[]).map((value) => ({
  value,
  label: VIEW_LABEL[value],
}));

/** Stand-in rows a view without a skeleton of its own draws: enough to fill a pane at rest. */
const VIEW_SKELETON_ROWS = 8;

const GENERIC_SKELETON = <SkeletonList count={VIEW_SKELETON_ROWS} className="p-3" />;

// What each view puts on screen while it has nothing to show. A view earns a stand-in of its own
// when its layout is distinctive enough that a plain list would shift under the data landing.
const VIEW_SKELETON: Record<View, ReactNode> = {
  agents: GENERIC_SKELETON,
  todos: <TodoBoardSkeleton />,
  scratchpads: GENERIC_SKELETON,
  diagrams: GENERIC_SKELETON,
  timers: GENERIC_SKELETON,
  messages: GENERIC_SKELETON,
};

// The orchestration surface for one project: a live view of the lead→worker agent tree and the
// shared coordination documents (todos, scratchpads, timers) and the traffic between the agents. Owns the read-model hook — the only
// place here that reaches IPC — and switches the body between views. Each view is presentational
// over the one snapshot the hook keeps live (snapshot-then-deltas).
export function OrchestrationPane({
  project,
  focus,
  onOpenAgent,
}: {
  project: ProjectView;
  /** A session-work item to switch to and expand/select, set by the caller on each activation. */
  focus?: OrchestrationFocus | null;
  /** Opens the agent a todo row is locked by — forwarded to the todo board. */
  onOpenAgent?: (process: number) => void;
}) {
  const { snapshot, error, refresh } = useOrchestration(project.id);
  const [view, setView] = useState<View>("agents");

  // The switcher is bound to the live view so it moves the instant it is clicked, while the body
  // renders the deferred one — a heavy view mounting can then never hold up the click that asked
  // for it. Until that render commits, the region stands in for the view being switched to.
  const deferredView = useDeferredValue(view);
  const settling = view !== deferredView;

  // Cross-surface navigation's inbound half: a focus target switches the pane to its view. The
  // target row's own expand-and-focus is the board/panel's job, keyed off the same nonce. Tracked
  // against `focus`'s own identity rather than an effect so the switch lands the same render the
  // navigation arrives in.
  const [syncedFocus, setSyncedFocus] = useState(focus);
  if (syncedFocus !== focus) {
    setSyncedFocus(focus);
    if (focus != null) setView(focus.view);
  }

  return (
    <section className="flex h-full min-w-0 flex-col bg-background">
      <header className="flex h-11 shrink-0 items-center gap-2.5 border-b bg-sidebar px-3">
        <Avatar className="size-5">
          {project.icon && <AvatarImage src={project.icon} alt="" />}
          <AvatarFallback>{monogram(project.name)}</AvatarFallback>
        </Avatar>
        <span className="type-title min-w-0 shrink truncate font-[550] tracking-[var(--tracking-title)]">
          {project.name}
        </span>
        <div className="ml-auto shrink-0">
          <SegmentedControl<View>
            value={view}
            options={VIEW_OPTIONS}
            onChange={setView}
            ariaLabel="Orchestration views"
            counts={
              snapshot.status === LoadStatus.Ready
                ? { timers: snapshot.value.timers.length }
                : undefined
            }
          />
        </div>
      </header>
      {error && <p className="px-3 pt-2 text-xs text-destructive">{error}</p>}
      <div className="min-h-0 flex-1 overflow-hidden">
        <LoadableRegion
          state={settling ? loading<OrchestrationReadModel>() : snapshot}
          label={VIEW_LABEL[view].toLowerCase()}
          skeleton={VIEW_SKELETON[view]}
          onRetry={refresh}
          className="h-full"
        >
          {(model) => (
            <>
              {deferredView === "agents" && (
                <div className="h-full overflow-auto p-3">
                  <OrchestrationTree tree={model.tree} />
                </div>
              )}
              {deferredView === "todos" && (
                <TodoBoard
                  project={project.id}
                  todos={model.todos}
                  agents={model.agents}
                  scratchpads={model.scratchpads}
                  onOpenAgent={onOpenAgent}
                  focusId={focus?.view === "todos" ? focus.id : undefined}
                  focusNonce={focus?.view === "todos" ? focus.nonce : undefined}
                />
              )}
              {deferredView === "scratchpads" && (
                <ScratchpadPanel
                  project={project.id}
                  scratchpads={model.scratchpads}
                  focusName={focus?.view === "scratchpads" ? focus.name : undefined}
                  focusNonce={focus?.view === "scratchpads" ? focus.nonce : undefined}
                />
              )}
              {deferredView === "diagrams" && (
                <DiagramPanel project={project.id} diagrams={model.diagrams} />
              )}
              {deferredView === "timers" && (
                <TimersPanel timers={model.timers} agents={model.agents} project={project.id} />
              )}
              {deferredView === "messages" && (
                <MessagesPanel messages={model.messages} agents={model.agents} />
              )}
            </>
          )}
        </LoadableRegion>
      </div>
    </section>
  );
}
