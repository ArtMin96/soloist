import type { ReactNode } from "react";
import { Command, CommandDialog, CommandInput, CommandList } from "@/components/ui/command";
import { PaletteFooter, type PaletteHintData } from "@/components/palette/PaletteFooter";

interface CommandPaletteShellProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Accessible dialog title (visually hidden — the input placeholder carries the on-screen cue). */
  title: string;
  /** Accessible dialog description (visually hidden). */
  description: string;
  placeholder: string;
  hints: PaletteHintData[];
  /** The scoped target named on the footer's right (e.g. the active project), when any. */
  target?: ReactNode;
  children: ReactNode;
}

// The standard single-step command palette frame: a titled command dialog over a fuzzy-filtered
// list with a key-hint footer. Palettes pass their grouped command items as children and run each
// action through `useCommandAction` so the dialog closes on select. (AgentPicker is intentionally
// not built on this — it is a multi-step picker — but shares the same footer.)
export function CommandPaletteShell({
  open,
  onOpenChange,
  title,
  description,
  placeholder,
  hints,
  target,
  children,
}: CommandPaletteShellProps) {
  return (
    <CommandDialog open={open} onOpenChange={onOpenChange} title={title} description={description}>
      <Command>
        <CommandInput placeholder={placeholder} autoFocus />
        <CommandList>{children}</CommandList>
        <PaletteFooter hints={hints} target={target} />
      </Command>
    </CommandDialog>
  );
}
