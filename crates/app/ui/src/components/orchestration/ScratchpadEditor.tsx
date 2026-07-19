import { Archive, ArchiveRestore, Link2 } from "lucide-react";
import { AdvisoryNotice } from "@/components/AdvisoryNotice";
import { Button } from "@/components/ui/button";
import { ScratchpadBody } from "@/components/orchestration/ScratchpadBody";
import { ScratchpadTitle } from "@/components/orchestration/ScratchpadTitle";
import { humanizeName } from "@/lib/humanize";
import type { ScratchpadConflict } from "@/store/useScratchpadEditor";

interface ScratchpadEditorProps {
  name: string;
  initialBody: string;
  revision: number | null;
  /** Bumped on open/reload so the editor body remounts with fresh content and undo history. */
  mountKey: number;
  conflict: ScratchpadConflict | null;
  error: string | null;
  /** Whether the open scratchpad is archived — flips the header control between Archive and Restore. */
  archived: boolean;
  onSave: (markdown: string) => Promise<void>;
  onReload: () => void;
  onCopyLink: () => void;
  /** Archives the open scratchpad, or restores it when already archived (also bound to Ctrl+Shift+W). */
  onArchive: () => void;
  /** Renames the open scratchpad; rejects with the core's refusal so the field can surface it. */
  onRename: (to: string) => Promise<void>;
}

// The scratchpad's editing surface: a persistent header (the renamable title, the raw handle when it
// reads differently, revision, actions), the conflict banner, and the remounting editor body.
// Presentational — the body, the revision guard, and every callback arrive as props; the parent owns
// the read/write. A stale save surfaces the conflict banner
// (the core already refused it, so nothing was clobbered) with a Reload to the other edit; while it
// shows, autosave is paused (`paused`) so the rejected edit is never retried behind the user's back.
// Validity is the core's call, surfaced as the error line.
export function ScratchpadEditor({
  name,
  initialBody,
  revision,
  mountKey,
  conflict,
  error,
  archived,
  onSave,
  onReload,
  onCopyLink,
  onArchive,
  onRename,
}: ScratchpadEditorProps) {
  // The handle earns its place only when the title no longer reads as it — a name the user already
  // wrote is its own handle, and printing it twice would be noise.
  const handle = humanizeName(name) === name ? null : name;
  return (
    <div className="flex h-full min-w-0 flex-col">
      <header className="flex h-9 shrink-0 items-center gap-2 border-b px-3">
        <ScratchpadTitle name={name} onRename={onRename} />
        {handle && (
          <span
            className="type-label max-w-[12rem] shrink-0 truncate font-mono text-muted-foreground"
            title={`Handle: ${handle}`}
          >
            {handle}
          </span>
        )}
        {revision != null && (
          <span className="type-label shrink-0 font-mono tabular-nums text-muted-foreground">
            revision {revision}
          </span>
        )}
        <Button variant="ghost" size="sm" onClick={onArchive}>
          {archived ? (
            <>
              <ArchiveRestore aria-hidden /> Restore
            </>
          ) : (
            <>
              <Archive aria-hidden /> Archive
            </>
          )}
        </Button>
        <Button variant="ghost" size="sm" onClick={onCopyLink}>
          <Link2 aria-hidden /> Copy link
        </Button>
      </header>

      {conflict && (
        <AdvisoryNotice
          className="mx-3 mt-3"
          action={
            <Button variant="outline" size="sm" onClick={onReload}>
              Reload
            </Button>
          }
        >
          This scratchpad changed elsewhere (now at revision {conflict.actual}). Your edits were not
          saved and nothing was overwritten.
        </AdvisoryNotice>
      )}

      {error && (
        <p className="mx-3 mt-3 text-[0.8125rem] text-destructive" aria-live="polite">
          {error}
        </p>
      )}

      <ScratchpadBody
        key={`${name}:${mountKey}`}
        initialBody={initialBody}
        name={name}
        onSave={onSave}
        paused={conflict != null}
      />
    </div>
  );
}
