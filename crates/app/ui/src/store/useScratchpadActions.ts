import { useCallback, useState } from "react";
import { exportMarkdown as exportMarkdownFile, scratchpadArchive, scratchpadWrite } from "@/api";
import { writeClipboard } from "@/lib/clipboard";
import type { SaveOutcome } from "@/store/saveOutcome";

export interface ScratchpadActionsStore {
  /** The last refusal of an action on the open scratchpad (archive, export), or null. */
  error: string | null;
  /** The core's refusal of the last create (a taken or invalid name), or null. */
  createError: string | null;
  /**
   * Creates the scratchpad, resolving to whether it went through. A refusal never rejects — it is
   * surfaced through `createError`, so the form keeps the name the user typed.
   */
  create: (name: string, body: string) => Promise<SaveOutcome>;
  /** Archives the scratchpad, or restores it — a listing flag, not a delete. */
  archive: (name: string, archived: boolean) => void;
  /** Writes the scratchpad to a `.md` file the user picks. */
  exportMarkdown: (name: string, body: string) => void;
  /** Copies the scratchpad to the clipboard as the same Markdown an export writes. */
  copyMarkdown: (name: string, body: string) => void;
  clearError: () => void;
}

// The scratchpad as canonical Markdown — its name as the leading H1 over the body — so an export and
// a copy hand off byte-identical text.
function asDocument(name: string, body: string): string {
  return `# ${name}\n\n${body}`;
}

// The scratchpad board's write seam for everything that is not the open document's own read/write
// (that is `useScratchpadEditor`): creating one, archiving or restoring it, and handing its Markdown
// to a file or the clipboard. Every refusal is the core's to make and is surfaced verbatim rather
// than pre-empted here — a duplicate name is refused by the core, not guessed at by the form. The
// board's data and its live refresh come from the snapshot hook; a landed write emits the domain
// event that re-reads it.
export function useScratchpadActions(project: number): ScratchpadActionsStore {
  const [error, setError] = useState<string | null>(null);
  const [createError, setCreateError] = useState<string | null>(null);

  const create = useCallback(
    async (name: string, body: string): Promise<SaveOutcome> => {
      setCreateError(null);
      try {
        // No revision guard: this document does not exist yet, and a name already taken is exactly
        // what the core refuses.
        await scratchpadWrite(project, name, body, null);
        return "saved";
      } catch (reason) {
        setCreateError(String(reason));
        return "refused";
      }
    },
    [project],
  );

  const archive = useCallback(
    (name: string, archived: boolean) => {
      setError(null);
      scratchpadArchive(project, name, archived).catch((reason) => setError(String(reason)));
    },
    [project],
  );

  const exportMarkdown = useCallback((name: string, body: string) => {
    setError(null);
    exportMarkdownFile(name, asDocument(name, body)).catch((reason) => setError(String(reason)));
  }, []);

  // `writeClipboard` degrades rather than rejecting, so a refused copy leaves the clipboard as it
  // was and there is nothing to report.
  const copyMarkdown = useCallback((name: string, body: string) => {
    void writeClipboard(asDocument(name, body));
  }, []);

  const clearError = useCallback(() => setError(null), []);

  return { error, createError, create, archive, exportMarkdown, copyMarkdown, clearError };
}
