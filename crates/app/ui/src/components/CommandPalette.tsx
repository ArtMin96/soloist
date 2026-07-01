import { CommandEmpty, CommandGroup, CommandItem, CommandSeparator } from "@/components/ui/command";
import { CommandPaletteShell } from "@/components/palette/CommandPaletteShell";
import type { PaletteHintData } from "@/components/palette/PaletteFooter";
import { useCommandAction } from "@/components/palette/useCommandAction";
import { buildCommands, type CommandContext } from "@/lib/commands";
import { useAppearance } from "@/store/appearanceContext";

interface CommandPaletteProps extends Omit<CommandContext, "theme" | "setTheme"> {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

const HINTS: PaletteHintData[] = [
  { keys: "↵", label: "run" },
  { keys: "esc", label: "close" },
];

// The command palette (Ctrl+K): a fuzzy-filtered registry of every app-wide action — new
// agent/terminal, open settings/project, theme, per-project bulk + navigation, and per-process
// focus + actions. The registry is the pure `buildCommands`, so the palette stays presentational
// and a new command appears here automatically. Theme is read from the appearance context (the
// palette renders inside its provider); everything else is wired in from the app shell.
export function CommandPalette({ open, onOpenChange, ...wiring }: CommandPaletteProps) {
  const { appearance, setAppearance } = useAppearance();
  const run = useCommandAction(onOpenChange);
  const groups = buildCommands({
    ...wiring,
    theme: appearance.theme,
    setTheme: (theme) => setAppearance({ ...appearance, theme }),
  });

  return (
    <CommandPaletteShell
      open={open}
      onOpenChange={onOpenChange}
      title="Command Palette"
      description="Run any command"
      placeholder="Type a command…"
      hints={HINTS}
    >
      <CommandEmpty>No commands found.</CommandEmpty>
      {groups.map((group, idx) => (
        <div key={group.heading}>
          {idx > 0 && <CommandSeparator />}
          <CommandGroup heading={group.heading}>
            {group.commands.map((command) => (
              <CommandItem
                key={command.id}
                value={`${command.label} ${command.keywords.join(" ")}`}
                onSelect={run(command.run)}
              >
                {command.label}
              </CommandItem>
            ))}
          </CommandGroup>
        </div>
      ))}
    </CommandPaletteShell>
  );
}
