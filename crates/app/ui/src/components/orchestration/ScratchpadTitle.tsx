import { DocumentTitle } from "@/components/orchestration/DocumentTitle";

interface ScratchpadTitleProps {
  /** The scratchpad's raw name handle — what a rename edits, and what the title is humanized from. */
  name: string;
  /**
   * Commits a rename. Resolves once the core accepted it; rejects with the refusal (a taken name,
   * an invalid one) so the field can stay open showing the user's text.
   */
  onRename: (to: string) => Promise<void>;
}

// The scratchpad instantiation of the shared rename-in-place document title (`DocumentTitle`).
export function ScratchpadTitle({ name, onRename }: ScratchpadTitleProps) {
  return <DocumentTitle kind="scratchpad" name={name} onRename={onRename} />;
}
