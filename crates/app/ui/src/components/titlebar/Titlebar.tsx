import { Bot, FolderOpen } from "lucide-react";
import type { MouseEvent } from "react";
import { useWindowControls } from "@/components/titlebar/useWindowControls";
import { WindowControls } from "@/components/titlebar/WindowControls";
import { Button } from "@/components/ui/button";

interface TitlebarProps {
  appName: string;
  appVersion?: string;
  onOpenProject: () => void;
  onLaunchAgent: () => void;
}

// Marks an element as a window-drag handle. Tauri starts a drag on mousedown over any
// element carrying this attribute; interactive children (the buttons) omit it and stay
// clickable.
const DRAG = { "data-tauri-drag-region": "" };

// The single window-chrome surface: a draggable titlebar carrying the app identity, the
// global Launch-agent / Open-project actions, and the OS window controls. It stands in
// for the native decorations, which are turned off in tauri.conf.json.
export function Titlebar({ appName, appVersion, onOpenProject, onLaunchAgent }: TitlebarProps) {
  const { isMaximized, minimize, toggleMaximize, close } = useWindowControls();

  // Double-clicking the bare bar (not a button) toggles maximize, matching native
  // titlebar behavior the disabled decorations would otherwise provide.
  const onDoubleClick = (event: MouseEvent<HTMLElement>) => {
    if ((event.target as HTMLElement).hasAttribute("data-tauri-drag-region")) toggleMaximize();
  };

  return (
    <header
      {...DRAG}
      onDoubleClick={onDoubleClick}
      className="flex h-11 shrink-0 items-center gap-2 border-b bg-sidebar px-3"
    >
      <span {...DRAG} className="text-[0.9375rem] font-[550] tracking-[-0.005em]">
        {appName}
      </span>
      {appVersion && (
        <span {...DRAG} className="font-mono text-xs text-muted-foreground">
          v{appVersion}
        </span>
      )}
      <Button variant="ghost" size="sm" className="ml-2" onClick={onLaunchAgent}>
        <Bot />
        Launch agent
      </Button>
      <Button variant="ghost" size="sm" onClick={onOpenProject}>
        <FolderOpen />
        Open project
      </Button>
      <div {...DRAG} className="h-full flex-1" />
      <WindowControls
        isMaximized={isMaximized}
        onMinimize={minimize}
        onToggleMaximize={toggleMaximize}
        onClose={close}
      />
    </header>
  );
}
