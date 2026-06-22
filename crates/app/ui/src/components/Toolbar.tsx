import { Bot, FolderOpen } from "lucide-react";
import { Button } from "@/components/ui/button";

interface ToolbarProps {
  appName: string;
  appVersion?: string;
  onOpenProject: () => void;
  onLaunchAgent: () => void;
}

// The top bar: the app identity, the launch-agent action (also Cmd/Ctrl+T), and open-project.
// Stack-wide controls live in each project's sidebar header, scoped to that project — so the
// toolbar stays a header, not a place that acts on an ambiguous "current" project. Launching
// an agent is a global new-process action, so it belongs here; the picker resolves the target
// project itself.
export function Toolbar({ appName, appVersion, onOpenProject, onLaunchAgent }: ToolbarProps) {
  return (
    <header className="flex h-11 shrink-0 items-center gap-2 border-b bg-sidebar px-3">
      <span className="text-[0.9375rem] font-[550] tracking-[-0.005em]">{appName}</span>
      {appVersion && <span className="font-mono text-xs text-muted-foreground">v{appVersion}</span>}
      <Button variant="ghost" size="sm" className="ml-auto" onClick={onLaunchAgent}>
        <Bot />
        Launch agent
      </Button>
      <Button variant="ghost" size="sm" onClick={onOpenProject}>
        <FolderOpen />
        Open project
      </Button>
    </header>
  );
}
