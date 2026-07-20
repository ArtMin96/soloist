import { useState, type ComponentType, type ReactNode } from "react";
import { Dialog as DialogPrimitive } from "radix-ui";
import { X } from "lucide-react";
import { AgentsPanel } from "@/components/settings/AgentsPanel";
import { AppearancePanel } from "@/components/settings/AppearancePanel";
import { HotkeysPanel } from "@/components/settings/HotkeysPanel";
import { IntegrationsPanel } from "@/components/settings/IntegrationsPanel";
import { NotificationsPanel } from "@/components/settings/NotificationsPanel";
import { PlaceholderPanel } from "@/components/settings/PlaceholderPanel";
import { SettingsTabRail } from "@/components/settings/SettingsTabRail";
import { SidebarPanel } from "@/components/settings/SidebarPanel";
import { TemplatesPanel } from "@/components/settings/TemplatesPanel";
import { ToolsPanel } from "@/components/settings/ToolsPanel";
import {
  SETTINGS_TABS,
  settingsTabButtonId,
  type SettingsTabId,
  UNDEFINED_TABS,
} from "@/components/settings/tabs";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useScrollEdge } from "@/store/useScrollEdge";

// What every settings panel is handed. Only the panels that address project-scoped state read it;
// the rest declare no props and ignore it, so one map can build them all.
interface SettingsPanelProps {
  /** The project whose settings are in view, or null when none is open. */
  project: number | null;
}

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

function panelFor(id: SettingsTabId, props: SettingsPanelProps): ReactNode {
  const Panel = PANELS[id];
  if (Panel) return <Panel {...props} />;
  const label = SETTINGS_TABS.find((tab) => tab.id === id)?.label ?? "Settings";
  return UNDEFINED_TABS.has(id) ? (
    <PlaceholderPanel title={label} message="These settings have not been defined yet." />
  ) : (
    <PlaceholderPanel title={label} message="Coming in a later update of this build." />
  );
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

  return (
    <DialogPrimitive.Root open={open} onOpenChange={onOpenChange}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Content
          aria-describedby={undefined}
          className="fixed inset-0 z-50 flex flex-col bg-background text-foreground outline-none duration-[var(--dur-sheet)] ease-out-quint data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:slide-in-from-bottom-2 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:slide-out-to-bottom-2 data-[state=closed]:duration-[var(--dur-sheet-out)] motion-reduce:animate-none"
        >
          <header
            data-tauri-drag-region
            className={cn(
              "flex h-11 shrink-0 items-center justify-between border-b px-4 transition-colors duration-[var(--dur-fast)]",
              scrolled ? "border-border" : "border-transparent",
            )}
          >
            <DialogPrimitive.Title className="text-[0.9375rem] font-medium tracking-[-0.005em]">
              Settings
            </DialogPrimitive.Title>
            <DialogPrimitive.Close asChild>
              <Button variant="ghost" size="icon-sm" aria-label="Close settings">
                <X />
              </Button>
            </DialogPrimitive.Close>
          </header>
          <div className="flex min-h-0 flex-1">
            <SettingsTabRail active={active} onSelect={setActive} />
            <div
              ref={panelRef}
              role="tabpanel"
              aria-labelledby={settingsTabButtonId(active)}
              className="min-w-0 flex-1 overflow-y-auto bg-sidebar"
            >
              <div className="mx-auto max-w-2xl px-6 py-6">{panelFor(active, { project })}</div>
            </div>
          </div>
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}
