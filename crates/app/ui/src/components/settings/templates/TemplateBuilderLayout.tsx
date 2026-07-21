import type { ReactNode } from "react";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
  useDefaultLayout,
} from "@/components/ui/resizable";

const SPLIT_STORAGE_ID = "soloist.templates.builder";
const EDITOR_PANEL_ID = "editor";
const PREVIEW_PANEL_ID = "preview";
const PANEL_IDS = [EDITOR_PANEL_ID, PREVIEW_PANEL_ID];
const EDITOR_DEFAULT_SIZE = "62%";
const EDITOR_MIN_SIZE = "38%";
const PREVIEW_DEFAULT_SIZE = "38%";
const PREVIEW_MIN_SIZE = "24%";

interface TemplateBuilderLayoutProps {
  /** The name/description fields and rich-text body — always shown. */
  editor: ReactNode;
  /** The live preview, or null when this template has nothing to render (a new template, or a
   * non-renderable kind). Its presence is what decides single-column vs. split. */
  preview: ReactNode | null;
}

// The template builder's shape: a full-height editor, alone when there is nothing to preview, or
// paired with a resizable preview pane when there is — so a Prompt template's placeholders and
// rendered output are visible beside the body being edited, never below a scroll. The split ratio
// persists across sessions (keyed by `SPLIT_STORAGE_ID`) the way a real split-pane editor remembers
// where a user left the divider.
export function TemplateBuilderLayout({ editor, preview }: TemplateBuilderLayoutProps) {
  const { defaultLayout, onLayoutChanged } = useDefaultLayout({
    id: SPLIT_STORAGE_ID,
    panelIds: PANEL_IDS,
  });

  if (preview == null) {
    return <div className="min-h-0 flex-1">{editor}</div>;
  }

  return (
    <ResizablePanelGroup
      orientation="horizontal"
      defaultLayout={defaultLayout}
      onLayoutChanged={onLayoutChanged}
      className="min-h-0 flex-1"
    >
      <ResizablePanel
        id={EDITOR_PANEL_ID}
        defaultSize={EDITOR_DEFAULT_SIZE}
        minSize={EDITOR_MIN_SIZE}
      >
        {editor}
      </ResizablePanel>
      <ResizableHandle withHandle />
      <ResizablePanel
        id={PREVIEW_PANEL_ID}
        defaultSize={PREVIEW_DEFAULT_SIZE}
        minSize={PREVIEW_MIN_SIZE}
      >
        {/* Panel forces its own overflow to hidden, so the preview's internal scrolling has to live
            on this child rather than the panel itself — a long rendered prompt scrolls in place
            instead of growing the pane past the available height. */}
        <div className="h-full overflow-y-auto pl-4">{preview}</div>
      </ResizablePanel>
    </ResizablePanelGroup>
  );
}
