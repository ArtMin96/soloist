import { type ReactNode, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Command,
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command";
import { Input } from "@/components/ui/input";
import type { DetectedTool, ProjectView } from "@/domain";
import { tokenizeArgs } from "@/lib/tokenizeArgs";
import { cn } from "@/lib/utils";

interface AgentPickerProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** The configured agent tools with detection status (from the agents read-model). */
  tools: DetectedTool[];
  /** Every open project — the launch target, and the fallback chooser when it's ambiguous. */
  projects: ProjectView[];
  /** The project to launch into by default (the selected process's project), or null. */
  activeProjectId: number | null;
  /** Launch `tool` in `project` with `extraArgs` ([] for a plain launch). */
  onLaunch: (project: number, tool: string, extraArgs: string[]) => void;
}

// The agent launch picker (E4): a Cmd/Ctrl+T command palette over the configured tools.
// Enter launches the highlighted tool instantly; Alt+Enter opens a one-shot flags field for
// it ("agent with flags"). It launches into the active project; when several projects are
// open and none is active, it first asks which — the footer always names the target so the
// launch is never ambiguous. Presentational: tools/projects come in as props and the launch
// routes out through `onLaunch`; no IPC lives here.
export function AgentPicker({
  open,
  onOpenChange,
  tools,
  projects,
  activeProjectId,
  onLaunch,
}: AgentPickerProps) {
  // An explicit project choice (the fallback chooser) overrides the active project; the tool
  // whose flags are being edited (null = the list) drives which step shows.
  const [chosenProjectId, setChosenProjectId] = useState<number | null>(null);
  const [flagsTool, setFlagsTool] = useState<string | null>(null);
  const [flags, setFlags] = useState("");
  const [active, setActive] = useState("");

  const targetProject =
    chosenProjectId ?? activeProjectId ?? (projects.length === 1 ? projects[0].id : null);
  const targetName = projects.find((project) => project.id === targetProject)?.name;
  const step = flagsTool ? "flags" : targetProject === null ? "project" : "tool";

  function handleOpenChange(next: boolean) {
    if (!next) {
      // Reset the transient choices so the next open starts clean.
      setChosenProjectId(null);
      setFlagsTool(null);
      setFlags("");
      setActive("");
    }
    onOpenChange(next);
  }

  function launchWith(tool: string, extraArgs: string[]) {
    if (targetProject === null) return;
    onLaunch(targetProject, tool, extraArgs);
    handleOpenChange(false);
  }

  return (
    <CommandDialog
      open={open}
      onOpenChange={handleOpenChange}
      title="Launch agent"
      description="Pick an agent tool to launch in the current project."
    >
      {step === "project" ? (
        <ProjectStep projects={projects} onPick={setChosenProjectId} />
      ) : step === "flags" && flagsTool ? (
        <FlagsStep
          tool={flagsTool}
          flags={flags}
          onFlagsChange={setFlags}
          onLaunch={() => launchWith(flagsTool, tokenizeArgs(flags))}
          onBack={() => setFlagsTool(null)}
        />
      ) : (
        <ToolStep
          tools={tools}
          targetName={targetName}
          onValueChange={setActive}
          onLaunch={(tool) => launchWith(tool, [])}
          onEditFlags={() => {
            const tool = active || tools[0]?.tool.name;
            if (tool) setFlagsTool(tool);
          }}
        />
      )}
    </CommandDialog>
  );
}

// The default step: the searchable tool list. Alt+Enter on the highlighted tool opens its
// flags field instead of launching; plain Enter / click launches it with no extra flags.
function ToolStep({
  tools,
  targetName,
  onValueChange,
  onLaunch,
  onEditFlags,
}: {
  tools: DetectedTool[];
  targetName: string | undefined;
  onValueChange: (value: string) => void;
  onLaunch: (tool: string) => void;
  onEditFlags: () => void;
}) {
  return (
    <Command onValueChange={onValueChange}>
      <CommandInput
        placeholder="Launch agent…"
        onKeyDown={(event) => {
          if (event.altKey && event.key === "Enter") {
            // Steal Alt+Enter from cmdk's select so it opens flags instead of launching.
            event.preventDefault();
            event.stopPropagation();
            onEditFlags();
          }
        }}
      />
      <CommandList>
        <CommandEmpty>No agent tools configured.</CommandEmpty>
        <CommandGroup>
          {tools.map(({ tool, installed }) => (
            <CommandItem key={tool.name} value={tool.name} onSelect={() => onLaunch(tool.name)}>
              <ToolInitial name={tool.name} />
              <span className="font-medium">{tool.name}</span>
              <code className="min-w-0 flex-1 truncate font-mono text-xs text-muted-foreground">
                {tool.command}
              </code>
              <InstalledMark installed={installed} />
            </CommandItem>
          ))}
        </CommandGroup>
      </CommandList>
      <PickerFooter targetName={targetName}>
        <Hint keys="↵" label="launch" />
        <Hint keys="⌥↵" label="with flags" />
        <Hint keys="esc" label="close" />
      </PickerFooter>
    </Command>
  );
}

// The fallback chooser: which open project to launch into (only shown when several are open
// and none is active).
function ProjectStep({
  projects,
  onPick,
}: {
  projects: ProjectView[];
  onPick: (id: number) => void;
}) {
  return (
    <Command>
      <CommandInput placeholder="Launch agent in which project?" />
      <CommandList>
        <CommandEmpty>Open a project first to launch an agent.</CommandEmpty>
        <CommandGroup>
          {projects.map((project) => (
            <CommandItem
              key={project.id}
              value={`${project.name} ${project.root}`}
              onSelect={() => onPick(project.id)}
            >
              <span className="font-medium">{project.name}</span>
              <code className="min-w-0 flex-1 truncate font-mono text-xs text-muted-foreground">
                {project.root}
              </code>
            </CommandItem>
          ))}
        </CommandGroup>
      </CommandList>
      <PickerFooter>
        <Hint keys="↵" label="choose" />
        <Hint keys="esc" label="close" />
      </PickerFooter>
    </Command>
  );
}

// "Agent with flags": edit the extra flags for this one launch. Enter launches; Esc returns
// to the list.
function FlagsStep({
  tool,
  flags,
  onFlagsChange,
  onLaunch,
  onBack,
}: {
  tool: string;
  flags: string;
  onFlagsChange: (value: string) => void;
  onLaunch: () => void;
  onBack: () => void;
}) {
  return (
    <div className="flex flex-col gap-3 p-3">
      <div className="text-sm font-medium">
        {tool} <span className="text-muted-foreground">· flags for this launch</span>
      </div>
      <Input
        autoFocus
        value={flags}
        placeholder="--model sonnet --permission-mode plan"
        onChange={(event) => onFlagsChange(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            onLaunch();
          } else if (event.key === "Escape") {
            event.preventDefault();
            onBack();
          }
        }}
        className="font-mono"
      />
      <div className="flex items-center gap-2">
        <span className="flex flex-1 items-center gap-3 text-xs text-muted-foreground">
          <Hint keys="↵" label="launch" />
          <Hint keys="esc" label="back" />
        </span>
        <Button variant="ghost" size="sm" onClick={onBack}>
          Back
        </Button>
        <Button size="sm" onClick={onLaunch}>
          Launch
        </Button>
      </div>
    </div>
  );
}

// A monochrome initial chip standing in for a tool's icon (no per-vendor brand marks).
function ToolInitial({ name }: { name: string }) {
  return (
    <span className="flex size-5 shrink-0 items-center justify-center rounded bg-muted text-[0.6875rem] font-medium text-muted-foreground">
      {name.charAt(0)}
    </span>
  );
}

// Whether the tool's CLI was detected, encoded with shape + label (never color alone, and
// never the saturated status palette — installation is not a ProcStatus).
function InstalledMark({ installed }: { installed: boolean }) {
  return (
    <span className="flex shrink-0 items-center gap-1.5 text-xs text-muted-foreground">
      <span
        className={cn(
          "size-1.5 rounded-full",
          installed ? "bg-muted-foreground" : "border border-muted-foreground/50",
        )}
      />
      {installed ? "installed" : "not found"}
    </span>
  );
}

function PickerFooter({ targetName, children }: { targetName?: string; children: ReactNode }) {
  return (
    <div className="flex items-center gap-3 border-t px-3 py-2 text-xs text-muted-foreground">
      {children}
      {targetName && <span className="ml-auto min-w-0 truncate">▸ {targetName}</span>}
    </div>
  );
}

function Hint({ keys, label }: { keys: string; label: string }) {
  return (
    <span className="flex items-center gap-1">
      <kbd className="rounded border bg-muted px-1 font-mono text-[0.6875rem]">{keys}</kbd>
      {label}
    </span>
  );
}
