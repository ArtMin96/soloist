import { useEffect, type ReactNode } from "react";
import { Maximize2Icon, Minimize2Icon, XIcon } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { PaneDivider } from "@/components/PaneDivider";
import { Empty, EmptyDescription, EmptyHeader } from "@/components/ui/empty";
import { ScrollArea } from "@/components/ui/scroll-area";
import { isEditableTarget } from "@/lib/hotkeys";
import { cn } from "@/lib/utils";
import {
  SPLIT_MAX_HEIGHT,
  SPLIT_MIN_HEIGHT,
  SPLIT_RESIZE_STEP,
  useSplitLayout,
} from "@/store/git/useSplitLayout";

const RESIZE_LABEL = "Resize the split";
const CLOSE_LABEL = "Close the split";
const MAXIMIZE_LABEL = "Fill the area";
const RESTORE_LABEL = "Share the area with the terminal";

/**
 * The resizable split at the foot of the main area, and the chrome every view inside it wears:
 * the divider, the header strip, and the two controls that shape it.
 *
 * One surface rather than one per view, so a diff and a pull request open at the same height, fill
 * the area the same way, and close on the same key — and so the split's remembered shape belongs to
 * the split rather than to whichever view happened to set it.
 *
 * Presentational: props in, callbacks out. It reaches nothing and knows nothing about what it is
 * showing.
 */
export function SplitSurface({
  label,
  title,
  controls,
  notices,
  children,
  onClose,
}: {
  /** What the region is called — the one thing distinguishing the views to a screen reader. */
  label: string;
  /** The header's leading content: what is being shown. */
  title: ReactNode;
  /** The view's own header controls, ahead of the two that shape the split itself. */
  controls?: ReactNode;
  /** Quiet strips stating something about what is below them, above the scrolling body. */
  notices?: ReactNode;
  children: ReactNode;
  onClose: () => void;
}) {
  const [layout, setLayout] = useSplitLayout();

  useEscapeToClose(onClose);

  return (
    // Filling the area *covers* what is above rather than collapsing it, so the terminal keeps
    // its size and its scrollback — restoring the split puts the reader back on the same frame
    // instead of on one the emulator had to lay out again.
    <div
      className={cn("flex flex-col", layout.maximized ? "absolute inset-0 z-10" : "shrink-0")}
      style={layout.maximized ? undefined : { height: layout.height, maxHeight: "75%" }}
    >
      {!layout.maximized && (
        <PaneDivider
          orientation="horizontal"
          label={RESIZE_LABEL}
          size={layout.height}
          min={SPLIT_MIN_HEIGHT}
          max={SPLIT_MAX_HEIGHT}
          step={SPLIT_RESIZE_STEP}
          onResize={(height) => setLayout({ height })}
        />
      )}
      <section aria-label={label} className="flex min-h-0 flex-1 flex-col bg-background">
        <div className="flex h-11 shrink-0 items-center gap-2 border-b border-border px-3">
          {title}
          {controls}
          <IconButton
            label={layout.maximized ? RESTORE_LABEL : MAXIMIZE_LABEL}
            icon={layout.maximized ? <Minimize2Icon /> : <Maximize2Icon />}
            onClick={() => setLayout({ maximized: !layout.maximized })}
          />
          <IconButton label={CLOSE_LABEL} icon={<XIcon />} onClick={onClose} />
        </div>
        {notices}
        <ScrollArea className="min-h-0 flex-1">{children}</ScrollArea>
      </section>
    </div>
  );
}

/**
 * Closes the split on Escape, but only while the key is not being typed into something — a
 * terminal owns its own Escape, and so does a field.
 */
function useEscapeToClose(onClose: () => void): void {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      if (isEditableTarget(event.target)) return;
      onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);
}

/** A quiet strip stating something about what is below it, with the action that answers it. */
export function SplitNotice({ children, action }: { children: ReactNode; action?: ReactNode }) {
  return (
    <div className="flex shrink-0 items-center gap-3 border-b border-border bg-muted px-3 py-2">
      <p className="type-body min-w-0 flex-1 text-muted-foreground">{children}</p>
      {action}
    </div>
  );
}

/** The quiet line a view shows when it has nothing to render. */
export function SplitMessage({ children }: { children: ReactNode }) {
  return (
    <Empty className="h-full py-10">
      <EmptyHeader>
        <EmptyDescription>{children}</EmptyDescription>
      </EmptyHeader>
    </Empty>
  );
}
