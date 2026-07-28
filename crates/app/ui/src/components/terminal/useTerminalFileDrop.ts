import { useCallback, type RefObject } from "react";
import { quoteShellPaths } from "@/lib/shellQuote";
import { useFileDropTarget } from "@/store/fileDropContext";

/**
 * Accept files dropped onto a terminal pane by inserting their paths at the cursor, quoted for the
 * shell and separated the way a command line separates arguments — what dropping a file onto a
 * terminal does everywhere else on the desktop.
 *
 * Nothing runs as a result: no newline is appended, so a drag produces text the user still has to
 * act on. Dragging is not a decision to execute.
 *
 * Returns whether a drag is currently over the pane. A pane hidden in the keep-alive pool reports
 * false and stops reporting the moment it is hidden, so a drag it was under does not leave a mark
 * waiting to reappear the next time the user selects it.
 */
export function useTerminalFileDrop(
  host: RefObject<HTMLElement | null>,
  insert: (text: string) => void,
  visible: boolean,
): boolean {
  const onDrop = useCallback(
    (paths: string[]) => {
      if (paths.length === 0) return;
      insert(quoteShellPaths(paths));
    },
    [insert],
  );
  return useFileDropTarget(host, onDrop, visible);
}
