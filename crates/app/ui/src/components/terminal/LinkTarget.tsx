// Where the hovered link actually goes, read out in the corner of the pane the way a browser's
// status readout does.
//
// It sits in the app's own chrome layer at a fixed corner rather than following the pointer,
// because the thing it defends against is a link whose displayed text disagrees with its
// destination: a program can print anything it likes into the terminal's cells, but it cannot paint
// over this. Non-interactive by construction — it must never take a click meant for the terminal.

interface LinkTargetProps {
  /** The destination URI of the link under the pointer. */
  uri: string;
}

export function LinkTarget({ uri }: LinkTargetProps) {
  return (
    <div
      data-testid="terminal-link-target"
      className="pointer-events-none absolute bottom-0 left-0 max-w-full animate-in truncate rounded-tr-md border-t border-r bg-sidebar px-2 py-1 font-mono text-xs text-foreground fade-in-0 duration-[var(--dur-fast)]"
    >
      {uri}
    </div>
  );
}
