import type { ReactNode } from "react";
import { ScratchpadEditor } from "@/components/orchestration/ScratchpadEditor";
import { ScratchpadList } from "@/components/orchestration/ScratchpadList";
import { useScratchpadEditor } from "@/store/useScratchpadEditor";
import type { ScratchpadSummary } from "@/domain";

// The scratchpad surface: the roster on the left, the open document's structured editor on the
// right. The roster is the live snapshot's summaries (refreshed by the parent on ScratchpadChanged);
// opening one reads its full document through the editor hook, the only place here that reaches IPC.
export function ScratchpadPanel({
  project,
  scratchpads,
}: {
  project: number;
  scratchpads: ScratchpadSummary[];
}) {
  const editor = useScratchpadEditor(project);
  const selectedId = scratchpads.find((pad) => pad.name === editor.name)?.id ?? null;

  return (
    <div className="flex h-full min-h-0">
      <div className="w-60 shrink-0 overflow-auto border-r">
        <ScratchpadList scratchpads={scratchpads} selected={editor.name} onSelect={editor.open} />
      </div>
      <div className="min-w-0 flex-1">
        {editor.name == null ? (
          <Placeholder>Select a scratchpad to read or edit it.</Placeholder>
        ) : editor.form == null ? (
          <Placeholder>{editor.loading ? "Loading…" : (editor.error ?? "Not found.")}</Placeholder>
        ) : (
          <ScratchpadEditor
            name={editor.name}
            form={editor.form}
            revision={editor.baseRevision}
            saving={editor.saving}
            conflict={editor.conflict}
            error={editor.error}
            onChange={editor.setForm}
            onSave={editor.save}
            onReload={editor.reload}
            onCopyLink={() => {
              if (selectedId != null) editor.copyLink(selectedId);
            }}
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
