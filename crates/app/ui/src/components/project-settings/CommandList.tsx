import { useState } from "react";
import { ChevronRight, Plus } from "lucide-react";
import { AddCommandModal } from "@/components/project-settings/AddCommandModal";
import { CommandEditor } from "@/components/project-settings/CommandEditor";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { STATUS } from "@/lib/status";
import { cn } from "@/lib/utils";
import type { CommandOps } from "@/components/project-settings/commands";
import type { ProcStatus, ProjectCommandView } from "@/domain";

// The project's commands: each row shows its name, command line, an AUTO badge when it starts on
// open, a storage badge (the shared solo.yml vs an app-local command), and a live status dot. A row
// expands to its editor; new commands are added through the modal.
export function CommandList({
  commands,
  ops,
}: {
  commands: ProjectCommandView[];
  ops: CommandOps;
}) {
  const [expanded, setExpanded] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h3 className="text-[0.6875rem] font-medium tracking-[0.01em] text-muted-foreground">
          {commands.length} {commands.length === 1 ? "command" : "commands"}
        </h3>
        <Button variant="outline" size="sm" onClick={() => setAdding(true)}>
          <Plus />
          Add command
        </Button>
      </div>

      {commands.length === 0 ? (
        <p className="rounded-lg border border-dashed border-border px-3 py-6 text-center text-xs text-muted-foreground">
          No commands yet. Add one to run it under Soloist.
        </p>
      ) : (
        <ul className="divide-y divide-border overflow-hidden rounded-lg border border-border bg-card">
          {commands.map((command) => {
            const isOpen = expanded === command.name;
            return (
              <li key={command.name}>
                <button
                  type="button"
                  aria-expanded={isOpen}
                  onClick={() => setExpanded(isOpen ? null : command.name)}
                  className="flex w-full items-center gap-2.5 px-3 py-2 text-left outline-none transition-colors hover:bg-muted/50 focus-visible:bg-muted/50"
                >
                  <ChevronRight
                    aria-hidden
                    className={cn(
                      "size-3.5 shrink-0 text-muted-foreground transition-transform",
                      isOpen && "rotate-90",
                    )}
                  />
                  <StatusDot status={command.status} />
                  <span className="shrink-0 text-[0.8125rem] font-medium text-foreground">
                    {command.name}
                  </span>
                  <code className="min-w-0 flex-1 truncate font-mono text-xs text-muted-foreground">
                    {command.command}
                  </code>
                  {command.auto_start && <Badge variant="muted">AUTO</Badge>}
                  <Badge variant={command.visibility === "shared" ? "outline" : "muted"}>
                    {command.visibility === "shared" ? "solo.yml" : "Local"}
                  </Badge>
                </button>
                {isOpen && <CommandEditor command={command} ops={ops} />}
              </li>
            );
          })}
        </ul>
      )}

      <AddCommandModal open={adding} onOpenChange={setAdding} onAdd={ops.add} />
    </div>
  );
}

// A command's live state as a colored glyph (DESIGN.md: glyph + hue + label, never hue alone), or a
// muted ring when no process of that name is currently registered.
function StatusDot({ status }: { status: ProcStatus | null }) {
  if (!status) {
    return (
      <span
        className="inline-flex items-center leading-none text-muted-foreground/60"
        title="No process"
      >
        <span aria-hidden className="text-[0.7rem] leading-none">
          &#9675;
        </span>
        <span className="sr-only">No process</span>
      </span>
    );
  }
  const display = STATUS[status];
  return (
    <span className="inline-flex items-center leading-none" title={display.label}>
      <span
        aria-hidden
        className={cn(
          "text-[0.7rem] leading-none",
          display.toneClass,
          display.transitional && "motion-safe:animate-pulse",
        )}
      >
        {display.glyph}
      </span>
      <span className="sr-only">{display.label}</span>
    </span>
  );
}
