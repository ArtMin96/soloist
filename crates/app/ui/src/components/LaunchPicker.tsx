import { useRef, useState } from "react";
import { PaletteFooter, PaletteHint } from "@/components/palette/PaletteFooter";
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
import type { Detection, DetectedTool, ProjectView } from "@/domain";
import { detectionLabel } from "@/lib/agents";
import { HOTKEY_ACTION_LABELS } from "@/lib/hotkeys";
import { tokenizeArgs } from "@/lib/tokenizeArgs";
import { cn } from "@/lib/utils";
import { GROUP_LABELS, KIND_LABELS } from "@/store/grouping";

// cmdk identifies, highlights, and reports every entry by one opaque `value` string, so all the
// entries share a single name-space: an agent tool named "Terminal" would otherwise be
// indistinguishable from the terminal entry, and Alt+Enter could not tell which was highlighted.
// Prefixing the agent entries keeps the two apart. The value is still what search scores, and it
// keeps the tool's own name, so typing a tool still finds it; typing "agent" lists them all.
const AGENT_ENTRY_PREFIX = "agent:";
const TERMINAL_ENTRY = "terminal";

const agentEntry = (tool: string) => `${AGENT_ENTRY_PREFIX}${tool}`;

/** The tool an entry launches, or null if it is not an agent entry (so it takes no flags). */
function agentOf(entry: string): string | null {
  return entry.startsWith(AGENT_ENTRY_PREFIX) ? entry.slice(AGENT_ENTRY_PREFIX.length) : null;
}

interface LaunchPickerProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** The configured agent tools with detection status (from the agents read-model). */
  tools: DetectedTool[];
  /** Every open project — the launch target, and the fallback chooser when it's ambiguous. */
  projects: ProjectView[];
  /** Launch `tool` in `project` with `extraArgs` ([] for a plain launch). */
  onLaunch: (project: number, tool: string, extraArgs: string[]) => void;
  /** Open a plain interactive shell in `project`. */
  onCreateTerminal: (project: number) => void;
}

// The launch picker: a Cmd/Ctrl+T command palette over everything the user can start — the
// configured agent tools, and a plain terminal. Enter launches the highlighted entry instantly;
// Alt+Enter opens a one-shot flags field for an agent ("agent with flags"), which a terminal has
// no use for. When several projects are open it always asks which first — the footer always names
// the target so the launch is never ambiguous. Presentational: tools/projects come in as props and
// both actions route out through callbacks; no IPC lives here.
export function LaunchPicker({
  open,
  onOpenChange,
  tools,
  projects,
  onLaunch,
  onCreateTerminal,
}: LaunchPickerProps) {
  // The user's explicit project choice drives the step; the tool whose flags are being
  // edited (null = the list) drives which of the remaining two steps shows.
  const [chosenProjectId, setChosenProjectId] = useState<number | null>(null);
  const [flagsTool, setFlagsTool] = useState<string | null>(null);
  const [flags, setFlags] = useState("");
  // The cmdk-highlighted entry is only read when the user opens the flags step (Alt+Enter); it is
  // never rendered, so a ref keeps it current without re-rendering the picker on every highlight.
  const activeRef = useRef("");

  const targetProject = chosenProjectId ?? (projects.length === 1 ? projects[0].id : null);
  const targetName = projects.find((project) => project.id === targetProject)?.name;
  const step = flagsTool ? "flags" : targetProject === null ? "project" : "launch";

  function handleOpenChange(next: boolean) {
    if (!next) {
      // Reset the transient choices so the next open starts clean.
      setChosenProjectId(null);
      setFlagsTool(null);
      setFlags("");
      activeRef.current = "";
    }
    onOpenChange(next);
  }

  function launchWith(tool: string, extraArgs: string[]) {
    if (targetProject === null) return;
    onLaunch(targetProject, tool, extraArgs);
    handleOpenChange(false);
  }

  function createTerminal() {
    if (targetProject === null) return;
    onCreateTerminal(targetProject);
    handleOpenChange(false);
  }

  return (
    <CommandDialog
      open={open}
      onOpenChange={handleOpenChange}
      title={HOTKEY_ACTION_LABELS.new_agent_or_terminal}
      description="Pick an agent tool to launch, or open a plain terminal, in the current project."
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
        <LaunchStep
          tools={tools}
          targetName={targetName}
          onValueChange={(value) => (activeRef.current = value)}
          onLaunch={(tool) => launchWith(tool, [])}
          onCreateTerminal={createTerminal}
          onEditFlags={() => {
            const entry = activeRef.current || (tools[0] && agentEntry(tools[0].tool.name));
            // Flags belong to an agent's command line; a terminal takes none, so Alt+Enter on
            // it does nothing rather than opening a field that could not be applied.
            const name = entry ? agentOf(entry) : null;
            if (name && tools.some((candidate) => candidate.tool.name === name)) {
              setFlagsTool(name);
            }
          }}
        />
      )}
    </CommandDialog>
  );
}

// The default step: the searchable list of everything launchable. Alt+Enter on a highlighted
// agent opens its flags field instead of launching; plain Enter / click starts the entry.
function LaunchStep({
  tools,
  targetName,
  onValueChange,
  onLaunch,
  onCreateTerminal,
  onEditFlags,
}: {
  tools: DetectedTool[];
  targetName: string | undefined;
  onValueChange: (value: string) => void;
  onLaunch: (tool: string) => void;
  onCreateTerminal: () => void;
  onEditFlags: () => void;
}) {
  return (
    <Command onValueChange={onValueChange}>
      <CommandInput
        placeholder="Launch an agent or open a terminal…"
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
        <CommandEmpty>Nothing matches.</CommandEmpty>
        <CommandGroup heading={GROUP_LABELS.Agent}>
          {tools.map(({ tool, detection }) => (
            <CommandItem
              key={tool.name}
              value={agentEntry(tool.name)}
              onSelect={() => onLaunch(tool.name)}
            >
              <EntryInitial name={tool.name} />
              <span className="font-medium">{tool.name}</span>
              <code className="min-w-0 flex-1 truncate font-mono text-xs text-muted-foreground">
                {tool.command}
              </code>
              <InstalledMark detection={detection} />
            </CommandItem>
          ))}
        </CommandGroup>
        <CommandGroup heading={GROUP_LABELS.Terminal}>
          <CommandItem value={TERMINAL_ENTRY} onSelect={onCreateTerminal}>
            <EntryInitial name={KIND_LABELS.Terminal} />
            <span className="font-medium">{KIND_LABELS.Terminal}</span>
            {/* No command is shown: a terminal runs the user's own login shell, resolved on the
                machine at spawn time, so naming one here would be a guess that could drift. */}
            <span className="min-w-0 flex-1 truncate text-xs text-muted-foreground">
              your default shell
            </span>
          </CommandItem>
        </CommandGroup>
      </CommandList>
      <PaletteFooter
        hints={[
          { keys: "↵", label: "launch" },
          { keys: "⌥↵", label: "with flags" },
          { keys: "esc", label: "close" },
        ]}
        target={targetName}
      />
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
      <CommandInput placeholder="Launch in which project?" />
      <CommandList>
        <CommandEmpty>Open a project first to launch anything.</CommandEmpty>
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
      <PaletteFooter
        hints={[
          { keys: "↵", label: "choose" },
          { keys: "esc", label: "close" },
        ]}
      />
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
          <PaletteHint keys="↵" label="launch" />
          <PaletteHint keys="esc" label="back" />
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

// A monochrome initial chip standing in for an entry's icon (no per-vendor brand marks).
function EntryInitial({ name }: { name: string }) {
  return (
    <span className="flex size-5 shrink-0 items-center justify-center rounded bg-muted text-[0.6875rem] font-medium text-muted-foreground">
      {name.charAt(0)}
    </span>
  );
}

// What detection found, encoded with shape + label (never color alone, and never the saturated
// status palette — installation is not a ProcStatus). Three shapes for three outcomes: a filled
// dot for a detected CLI, a solid ring for one the probe confirmed absent, and a dashed ring for
// one it never got an answer about.
function InstalledMark({ detection }: { detection: Detection }) {
  return (
    <span className="flex shrink-0 items-center gap-1.5 text-xs text-muted-foreground">
      <span
        className={cn(
          "size-1.5 rounded-full",
          detection === "Installed" && "bg-muted-foreground",
          detection === "Missing" && "border border-muted-foreground/50",
          detection === "Unknown" && "border border-dashed border-muted-foreground/50",
        )}
      />
      {detectionLabel[detection]}
    </span>
  );
}
