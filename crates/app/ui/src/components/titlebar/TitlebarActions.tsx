import { Suspense } from "react";
import { BranchCluster } from "@/components/deferredAppComponents";
import { AttentionControl } from "@/components/titlebar/AttentionControl";
import type { AttentionSnapshot, ProcessView } from "@/domain";

interface TitlebarActionsProps {
  /** The project whose repository the chrome reports on, or null when none is in view. */
  project: number | null;
  snapshot: AttentionSnapshot;
  processes: ProcessView[];
  onSelectProcess: (id: number) => void;
  onClearAttention: () => void;
}

/**
 * What the title bar says about the moment: the branch the window is looking at, then what is
 * waiting on the user.
 *
 * The branch leads, because naming what the window is on is the strip's own job — and this is the
 * one place wide enough to name a branch without truncating it. Each control renders nothing when it
 * has nothing to report, which is what lets the strip and its divider stand down together.
 *
 * The cluster loads on demand: the branch switcher needs the command primitive, and nothing else in
 * the eager shell does.
 */
export function TitlebarActions({
  project,
  snapshot,
  processes,
  onSelectProcess,
  onClearAttention,
}: TitlebarActionsProps) {
  return (
    <>
      {project !== null && (
        <Suspense fallback={null}>
          <BranchCluster />
        </Suspense>
      )}
      <AttentionControl
        snapshot={snapshot}
        processes={processes}
        onSelect={onSelectProcess}
        onClearAll={onClearAttention}
      />
    </>
  );
}
