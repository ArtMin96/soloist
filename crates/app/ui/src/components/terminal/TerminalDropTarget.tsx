// Marks the pane a dropped file would land in, for as long as the drag is over it.
//
// The tint and the inset ring are the whole affordance on purpose. Which pane receives the drop is
// the only thing in doubt while the drag is in flight, and the result — a quoted path arriving at
// the cursor — explains itself the moment it lands; a label would sit over the output the file is
// being dropped into, to say something the next instant says better.
//
// Non-interactive by construction: the drop is handled by the OS rather than by DOM pointer events,
// so this never needs to receive one and must never take a click meant for the terminal.

export function TerminalDropTarget() {
  return (
    <div
      aria-hidden
      data-testid="terminal-drop-target"
      className="pointer-events-none absolute inset-0 animate-in bg-primary/10 fade-in-0 duration-[var(--dur-fast)] ring-2 ring-primary/70 ring-inset"
    />
  );
}
