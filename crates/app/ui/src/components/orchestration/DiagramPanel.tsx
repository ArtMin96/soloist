import { useCallback, useState, type ReactNode } from "react";
import { diagramArchive } from "@/api";
import { DiagramEditor } from "@/components/orchestration/DiagramEditor";
import { DiagramRoster } from "@/components/orchestration/DiagramRoster";
import { useDiagramEditor } from "@/store/useDiagramEditor";
import type { DiagramSummary } from "@/domain";

// The diagram surface: the roster on the left, the open diagram's source-editor/preview on the right.
// The roster is the live snapshot's summaries (refreshed by the parent on DiagramChanged); opening one
// reads its full Mermaid source through the editor hook, the only place here that reaches IPC. Archiving
// toggles the open document's listing flag through the core (the header control); the emitted event
// re-lists it, so the editor stays open and flips Archive ⇄ Restore. Mirrors the scratchpad panel; a
// diagram has no `solo://` link and no archive hotkey (there is no diagram hotkey scope in the core).
export function DiagramPanel({
  project,
  diagrams,
}: {
  project: number;
  diagrams: DiagramSummary[];
}) {
  const editor = useDiagramEditor(project);
  const [archiveError, setArchiveError] = useState<string | null>(null);

  const openSummary = diagrams.find((diagram) => diagram.name === editor.name);
  const archived = openSummary?.archived ?? false;

  const archiveOpen = useCallback(() => {
    const target = editor.name;
    if (target == null) return;
    setArchiveError(null);
    diagramArchive(project, target, !archived).catch((reason) => setArchiveError(String(reason)));
  }, [project, editor.name, archived]);

  return (
    <div className="flex h-full min-h-0 tracking-[var(--tracking-body)]">
      <div className="w-60 shrink-0 border-r">
        <DiagramRoster diagrams={diagrams} selected={editor.name} onSelect={editor.open} />
      </div>
      <div className="min-w-0 flex-1">
        {editor.name == null ? (
          <Placeholder>Select a diagram to read or edit it.</Placeholder>
        ) : editor.initialSource == null ? (
          <Placeholder>{editor.loading ? "Loading…" : (editor.error ?? "Not found.")}</Placeholder>
        ) : (
          <DiagramEditor
            key={editor.mountKey}
            name={editor.name}
            initialSource={editor.initialSource}
            revision={editor.baseRevision}
            conflict={editor.conflict}
            error={editor.error ?? archiveError}
            archived={archived}
            onSave={editor.save}
            onReload={editor.reload}
            onArchive={archiveOpen}
            onRename={editor.rename}
          />
        )}
      </div>
    </div>
  );
}

function Placeholder({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-full items-center justify-center p-6 text-center text-[0.8125rem] text-muted-foreground">
      {children}
    </div>
  );
}
