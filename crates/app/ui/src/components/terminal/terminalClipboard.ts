// Moving text between one xterm instance and the system clipboard, kept beside the terminal hook
// rather than inside it so the emulator's stream lifecycle and its clipboard behavior stay
// separately readable. Nothing here holds state: each entry point is handed the ref the pane keeps
// its emulator in, and reads it at the moment it acts — so work that resolves after the pane is
// torn down finds a null and stops, instead of touching a disposed terminal.

import type { RefObject } from "react";
import type { IDisposable, Terminal } from "@xterm/xterm";
import { readClipboard, writeClipboard } from "@/lib/clipboard";

/** Stable API for moving text between the emulator and the system clipboard. */
export interface TerminalClipboard {
  /** Copy the current selection. With nothing selected the clipboard keeps what it held. */
  copySelection: () => void;
  /** Insert the clipboard through the emulator, so bracketed-paste mode is honored. */
  paste: () => void;
}

/**
 * Copy what is selected, and nothing otherwise — an empty write would replace whatever the user had
 * on the clipboard with a blank, which is worse than the shortcut doing nothing.
 */
export function copySelection(termRef: RefObject<Terminal | null>): void {
  const term = termRef.current;
  if (!term?.hasSelection()) return;
  void writeClipboard(term.getSelection());
}

/**
 * Insert the clipboard through the emulator rather than writing to the PTY directly: `paste`
 * normalizes newlines and applies bracketed-paste markers when the running program asked for them,
 * then emits the result as ordinary input — so this needs no separate write path.
 */
export function pasteClipboard(termRef: RefObject<Terminal | null>): void {
  void readClipboard()
    .then((text) => {
      if (text) termRef.current?.paste(text);
    })
    .catch(() => {});
}

/**
 * Copy each selection as it is made, for as long as `enabled` reports the user has opted in. The
 * event fires as a selection is *cleared* too, so the emptiness guard is what keeps a deselect from
 * wiping the clipboard.
 */
export function copyOnSelect(term: Terminal, enabled: () => boolean): IDisposable {
  return term.onSelectionChange(() => {
    if (!enabled()) return;
    if (!term.hasSelection()) return;
    void writeClipboard(term.getSelection());
  });
}
