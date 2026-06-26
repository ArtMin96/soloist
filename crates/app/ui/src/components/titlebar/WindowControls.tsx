import { Copy, Minus, Square, X } from "lucide-react";
import type { ComponentProps, ReactNode } from "react";
import { cn } from "@/lib/utils";

interface WindowControlButtonProps extends ComponentProps<"button"> {
  label: string;
  danger?: boolean;
  children: ReactNode;
}

// A single window-chrome control. Deliberately not the app's shadcn Button: window
// controls are a specialized OS affordance — a uniform square hit target, a hover-only
// surface, and a red close — rather than an application action.
function WindowControlButton({ label, danger, className, children, ...props }: WindowControlButtonProps) {
  return (
    <button
      type="button"
      aria-label={label}
      className={cn(
        "inline-flex size-7 items-center justify-center rounded-md text-muted-foreground outline-none",
        "transition-colors duration-150 motion-reduce:transition-none",
        "hover:bg-foreground/10 hover:text-foreground",
        "focus-visible:ring-2 focus-visible:ring-ring",
        "[&_svg]:size-3.5 [&_svg]:shrink-0",
        danger && "hover:bg-destructive/15 hover:text-destructive",
        className,
      )}
      {...props}
    >
      {children}
    </button>
  );
}

interface WindowControlsProps {
  isMaximized: boolean;
  onMinimize: () => void;
  onToggleMaximize: () => void;
  onClose: () => void;
}

// The minimize / maximize-restore / close cluster, right-aligned in the titlebar. The
// middle control toggles and reflects the live maximized state.
export function WindowControls({ isMaximized, onMinimize, onToggleMaximize, onClose }: WindowControlsProps) {
  return (
    <div className="flex items-center gap-0.5">
      <WindowControlButton label="Minimize" onClick={onMinimize}>
        <Minus />
      </WindowControlButton>
      <WindowControlButton label={isMaximized ? "Restore" : "Maximize"} onClick={onToggleMaximize}>
        {isMaximized ? <Copy /> : <Square />}
      </WindowControlButton>
      <WindowControlButton label="Close" danger onClick={onClose}>
        <X />
      </WindowControlButton>
    </div>
  );
}
