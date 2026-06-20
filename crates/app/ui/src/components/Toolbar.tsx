import { FolderOpen } from "lucide-react";
import { Button } from "@/components/ui/button";

interface ToolbarProps {
  appName: string;
  appVersion?: string;
  onOpenProject: () => void;
}

// The top bar: the app identity and the open-project action. Stack-wide controls live in
// each project's sidebar header, scoped to that project — so the toolbar stays a header,
// not a place that acts on an ambiguous "current" project.
export function Toolbar({ appName, appVersion, onOpenProject }: ToolbarProps) {
  return (
    <header className="flex h-11 shrink-0 items-center gap-2 border-b bg-sidebar px-3">
      <span className="text-[0.9375rem] font-[550] tracking-[-0.005em]">{appName}</span>
      {appVersion && <span className="font-mono text-xs text-muted-foreground">v{appVersion}</span>}
      <Button variant="ghost" size="sm" className="ml-auto" onClick={onOpenProject}>
        <FolderOpen />
        Open project
      </Button>
    </header>
  );
}
