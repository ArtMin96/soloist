import { useRef, useState, type ComponentType, type ReactNode } from "react";
import { X } from "lucide-react";
import { AgentsPanel } from "@/components/settings/AgentsPanel";
import { AppearancePanel } from "@/components/settings/AppearancePanel";
import { HotkeysPanel } from "@/components/settings/HotkeysPanel";
import { IntegrationsPanel } from "@/components/settings/IntegrationsPanel";
import { NotificationsPanel } from "@/components/settings/NotificationsPanel";
import { PlaceholderPanel } from "@/components/settings/PlaceholderPanel";
import { SettingsColumn } from "@/components/settings/SettingsPanelLayout";
import { SettingsTabRail } from "@/components/settings/SettingsTabRail";
import { SidebarPanel } from "@/components/settings/SidebarPanel";
import { TemplatesPanel } from "@/components/settings/TemplatesPanel";
import { ToolsPanel } from "@/components/settings/ToolsPanel";
import {
  SETTINGS_TABS,
  settingsTabButtonId,
  type SettingsPanelProps,
  type SettingsTabId,
  UNDEFINED_TABS,
} from "@/components/settings/tabs";
import { Button } from "@/components/ui/button";
import { Dialog, DialogClose, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { cn } from "@/lib/utils";
import { useScrollEdge } from "@/store/useScrollEdge";

// The built panel for each tab — the single place a tab maps to its component. Tabs absent here
// fall through to a placeholder (undefined-in-source vs. still-to-come, decided below), so the
// rail can list every source tab without each one needing a panel yet.
const PANELS: Partial<Record<SettingsTabId, ComponentType<SettingsPanelProps>>> = {
  appearance: AppearancePanel,
  agents: AgentsPanel,
  hotkeys: HotkeysPanel,
  integrations: IntegrationsPanel,
  notifications: NotificationsPanel,
  sidebar: SidebarPanel,
  templates: TemplatesPanel,
  tools: ToolsPanel,
};

// Panels whose width follows their own internal state, so they render their own wrapper — Templates
// is a centered column while browsing and a full-width builder once a create form or the editor is
// open. Everything else takes the standard column from here.
const SELF_LAID_OUT_PANELS: ReadonlySet<SettingsTabId> = new Set(["templates"]);

function placeholderFor(id: SettingsTabId): ReactNode {
  const label = SETTINGS_TABS.find((tab) => tab.id === id)?.label ?? "Settings";
  return UNDEFINED_TABS.has(id) ? (
    <PlaceholderPanel title={label} message="These settings have not been defined yet." />
  ) : (
    <PlaceholderPanel title={label} message="Coming in a later update of this build." />
  );
}

function panelFor(id: SettingsTabId, props: SettingsPanelProps): ReactNode {
  const Panel = PANELS[id];
  const content = Panel ? <Panel {...props} /> : placeholderFor(id);
  return SELF_LAID_OUT_PANELS.has(id) ? content : <SettingsColumn>{content}</SettingsColumn>;
}

// The global Settings surface: a full-window overlay with a left section rail and the active
// panel. Built on the Radix Dialog primitive for focus trapping and Escape-to-close (a
// keyboard-first destination), styled as a flat opaque surface rather than a floating card.
export function SettingsOverlay({
  open,
  onOpenChange,
  project,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** The project in view, handed to the panels that address project-scoped state (Templates). */
  project: number | null;
}) {
  const [active, setActive] = useState<SettingsTabId>("appearance");
  const { ref: panelRef, scrolled } = useScrollEdge<HTMLDivElement>();
  const contentRef = useRef<HTMLDivElement>(null);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        ref={contentRef}
        showCloseButton={false}
        presentation="fullscreen"
        aria-describedby={undefined}
        // Radix decides "outside" from a capture handler it skips whenever the pointerdown is
        // already `defaultPrevented`. The resizable split's divider prevents default on the way
        // down, so a press on it read as a click outside the overlay and dismissed Settings
        // mid-drag. The DOM knows better: re-check containment against the real event target.
        onPointerDownOutside={(event) => {
          const target = event.detail.originalEvent.target;
          if (target instanceof Node && contentRef.current?.contains(target)) {
            event.preventDefault();
          }
        }}
      >
        <header
          data-tauri-drag-region
          className={cn(
            "flex h-11 shrink-0 items-center justify-between border-b px-4 transition-colors duration-[var(--dur-fast)]",
            scrolled ? "border-border" : "border-transparent",
          )}
        >
          <DialogTitle className="text-[0.9375rem] font-medium tracking-[-0.005em]">
            Settings
          </DialogTitle>
          <DialogClose asChild>
            <Button variant="ghost" size="icon-sm" aria-label="Close settings">
              <X />
            </Button>
          </DialogClose>
        </header>
        <div className="flex min-h-0 flex-1">
          <SettingsTabRail active={active} onSelect={setActive} />
          <div
            ref={panelRef}
            role="tabpanel"
            aria-labelledby={settingsTabButtonId(active)}
            className="min-w-0 flex-1 overflow-y-auto bg-sidebar"
          >
            {panelFor(active, { project })}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
