import { useCallback, useEffect, useRef, useState } from "react";
import { scratchpadArchive } from "@/api";
import { DocumentPanel, DocumentPlaceholder } from "@/components/orchestration/DocumentPanel";
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
  focusName,
  focusNonce,
}: {
  project: number;
  scratchpads: ScratchpadSummary[];
  /** The scratchpad to open and focus when `focusNonce` changes — cross-surface navigation, inbound. */
  focusName?: string;
  /** Bumped to re-trigger the focus above, even to repeat the same `focusName`. */
  focusNonce?: number;
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

  // Cross-surface navigation's inbound half: a fresh `focusNonce` opens the named scratchpad
  // through the existing `editor.open` path and moves DOM focus to its roster row, even on a
  // repeat of the same `focusName`. Coming from a terminal, this pane mounts fresh and its first
  // snapshot can still be in flight when the focus props land — `targetPresent` re-fires the
  // effect once the row actually appears in `scratchpads`, and `focusedNonceRef` remembers which
  // nonce was last acted on so that retry stops there, while a genuinely new nonce (even for the
  // same name) still refocuses. `open` is `useCallback`-stable on `project`, so depending on it
  // (not the fresh `editor` object every render returns) still fires only on a real focus change.
  const { open } = editor;
  const focusedNonceRef = useRef<number | null>(null);
  const targetPresent = focusName != null && scratchpads.some((pad) => pad.name === focusName);
  useEffect(() => {
    if (focusName == null || focusNonce == null || !targetPresent) return;
    if (focusedNonceRef.current === focusNonce) return;
    focusedNonceRef.current = focusNonce;
    open(focusName);
    const row = document.querySelector<HTMLElement>(`[data-scratchpad-name="${focusName}"]`);
    row?.scrollIntoView({ block: "nearest" });
    row?.focus();
  }, [focusNonce, focusName, open, targetPresent]);

  return (
    <DocumentPanel
      panelRef={panelRef}
      roster={
        <ScratchpadRoster scratchpads={scratchpads} selected={editor.name} onSelect={editor.open} />
      }
      content={
        editor.name == null ? (
          <DocumentPlaceholder>Select a scratchpad to read or edit it.</DocumentPlaceholder>
        ) : editor.initialBody == null ? (
          <DocumentPlaceholder>
            {editor.loading ? "Loading…" : (editor.error ?? "Not found.")}
          </DocumentPlaceholder>
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
        )
      }
    />
  );
}
