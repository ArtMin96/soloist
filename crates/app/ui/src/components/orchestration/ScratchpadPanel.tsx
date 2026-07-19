import { useCallback, useRef, useState, type ReactNode } from "react";
import { scratchpadArchive } from "@/api";
import { ScratchpadEditor } from "@/components/orchestration/ScratchpadEditor";
import { ScratchpadRoster } from "@/components/orchestration/ScratchpadRoster";
import { useScratchpadEditor } from "@/store/useScratchpadEditor";
import { useScratchpadHotkeys } from "@/store/useScratchpadHotkeys";
import type { ScratchpadSummary } from "@/domain";

// The scratchpad surface: the roster on the left, the open document's rich-text editor on the right.
// The roster is the live snapshot's summaries (refreshed by the parent on ScratchpadChanged);
// opening one reads its full body through the editor hook, the only place here that reaches IPC.
// Archiving toggles the open document's listing flag through the core (Ctrl+Shift+W, or the header
// control); the emitted event re-lists it, so the editor stays open and flips Archive ⇄ Restore.
export function ScratchpadPanel({
  project,
  scratchpads,
}: {
  project: number;
  scratchpads: ScratchpadSummary[];
}) {
  const editor = useScratchpadEditor(project);
  const panelRef = useRef<HTMLDivElement>(null);
  const [archiveError, setArchiveError] = useState<string | null>(null);

  const openSummary = scratchpads.find((pad) => pad.name === editor.name);
  const selectedId = openSummary?.id ?? null;
  const archived = openSummary?.archived ?? false;

  const archiveOpen = useCallback(() => {
    const target = editor.name;
    if (target == null) return;
    setArchiveError(null);
    scratchpadArchive(project, target, !archived).catch((reason) =>
      setArchiveError(String(reason)),
    );
  }, [project, editor.name, archived]);

  useScratchpadHotkeys(panelRef, editor.name != null ? archiveOpen : undefined);

  return (
    <div ref={panelRef} className="flex h-full min-h-0">
      <div className="w-60 shrink-0 border-r">
        <ScratchpadRoster scratchpads={scratchpads} selected={editor.name} onSelect={editor.open} />
      </div>
      <div className="min-w-0 flex-1">
        {editor.name == null ? (
          <Placeholder>Select a scratchpad to read or edit it.</Placeholder>
        ) : editor.initialBody == null ? (
          <Placeholder>{editor.loading ? "Loading…" : (editor.error ?? "Not found.")}</Placeholder>
        ) : (
          <ScratchpadEditor
            name={editor.name}
            initialBody={editor.initialBody}
            revision={editor.baseRevision}
            mountKey={editor.mountKey}
            conflict={editor.conflict}
            error={editor.error ?? archiveError}
            archived={archived}
            onSave={editor.save}
            onReload={editor.reload}
            onCopyLink={() => {
              if (selectedId != null) editor.copyLink(selectedId);
            }}
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
