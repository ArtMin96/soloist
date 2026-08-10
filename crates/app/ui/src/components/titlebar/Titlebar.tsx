import type { MouseEvent, ReactNode } from "react";
import { useWindowControls } from "@/components/titlebar/useWindowControls";
import { WindowControls } from "@/components/titlebar/WindowControls";

interface TitlebarProps {
  appName: string;
  appVersion?: string;
  /** Contextual controls for the trailing end of the strip, before the window controls. They carry
   *  no drag attribute of their own, so they stay clickable — only the space around them drags. */
  actions?: ReactNode;
}

// Marks an element as a window-drag handle. Tauri starts a drag on mousedown over any
// element carrying this attribute; interactive children (the buttons) omit it and stay
// clickable.
const DRAG = { "data-tauri-drag-region": "" };

// The single window-chrome surface: a unified toolbar carrying app identity and the OS window
// controls. Workspace and session actions live on the start surface, leaving this strip available
// for contextual repository controls. It stands in for the disabled native decorations.
export function Titlebar({ appName, appVersion, actions }: TitlebarProps) {
  const { isMaximized, minimize, toggleMaximize, close } = useWindowControls();

  // Double-clicking the bare bar (not a button) toggles maximize, matching native
  // titlebar behavior the disabled decorations would otherwise provide.
  const onDoubleClick = (event: MouseEvent<HTMLElement>) => {
    if ((event.target as HTMLElement).hasAttribute("data-tauri-drag-region")) toggleMaximize();
  };

  return (
    // `translateZ(0)` promotes the strip to its own compositing layer so a theme switch repaints it
    // on the compositor thread alongside the terminal and sidebar (both already composited), instead
    // of the deferred main-thread root-layer flush that made it recolor seconds after the body on
    // WebKitGTK. A no-op transform: it doesn't move the strip or affect drag-region hit-testing.
    <header
      {...DRAG}
      onDoubleClick={onDoubleClick}
      className="flex h-11 shrink-0 items-center gap-2.5 border-b bg-sidebar pr-2 pl-3 [transform:translateZ(0)]"
    >
      <img
        src="/logo.png"
        alt=""
        width={18}
        height={18}
        draggable={false}
        {...DRAG}
        className="size-[18px] shrink-0 rounded-[5px]"
      />
      <span {...DRAG} className="text-[0.9375rem] font-[550] tracking-[-0.005em] text-foreground">
        {appName}
      </span>
      {appVersion && (
        <span {...DRAG} className="font-mono text-[0.6875rem] text-muted-foreground">
          v{appVersion}
        </span>
      )}
      <div {...DRAG} className="h-full flex-1" />
      {/* The contextual strip and the short divider that separates it from the window controls.
          Both stand down together when nothing contextual is showing: the strip is `:empty` only
          when every control in it rendered nothing, which is what makes the divider disappear
          rather than divide one side from nothing.

          The strip and the divider carry the drag attribute so the space between controls stays a
          window handle, the way the rest of the bar is. It does not reach the controls inside them:
          the attribute applies only to the element it is on, never to children. */}
      <div {...DRAG} className="peer flex min-w-0 items-center gap-2.5 empty:hidden">
        {actions}
      </div>
      <div {...DRAG} aria-hidden className="h-4 w-px shrink-0 bg-border peer-empty:hidden" />
      <WindowControls
        isMaximized={isMaximized}
        onMinimize={minimize}
        onToggleMaximize={toggleMaximize}
        onClose={close}
      />
    </header>
  );
}
