import { useState, type ComponentType, type ReactNode } from "react";
import { Dialog as DialogPrimitive } from "radix-ui";
import { X } from "lucide-react";
import { AgentsPanel } from "@/components/settings/AgentsPanel";
import { AppearancePanel } from "@/components/settings/AppearancePanel";
import { HotkeysPanel } from "@/components/settings/HotkeysPanel";
import { IntegrationsPanel } from "@/components/settings/IntegrationsPanel";
import { PlaceholderPanel } from "@/components/settings/PlaceholderPanel";
import { SettingsTabRail } from "@/components/settings/SettingsTabRail";
import { SidebarPanel } from "@/components/settings/SidebarPanel";
import { ToolsPanel } from "@/components/settings/ToolsPanel";
import {
  SETTINGS_TABS,
  settingsTabButtonId,
  type SettingsTabId,
  UNDEFINED_TABS,
} from "@/components/settings/tabs";
import { Button } from "@/components/ui/button";

// The built panel for each tab — the single place a tab maps to its component. Tabs absent here
// fall through to a placeholder (undefined-in-source vs. still-to-come, decided below), so the
// rail can list every source tab without each one needing a panel yet.
const PANELS: Partial<Record<SettingsTabId, ComponentType>> = {
  appearance: AppearancePanel,
  agents: AgentsPanel,
  hotkeys: HotkeysPanel,
  integrations: IntegrationsPanel,
  sidebar: SidebarPanel,
  tools: ToolsPanel,
};

function panelFor(id: SettingsTabId): ReactNode {
  const Panel = PANELS[id];
  if (Panel) return <Panel />;
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
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [active, setActive] = useState<SettingsTabId>("appearance");

  return (
    <DialogPrimitive.Root open={open} onOpenChange={onOpenChange}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Content
          aria-describedby={undefined}
          className="fixed inset-0 z-50 flex flex-col bg-background text-foreground outline-none data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 motion-reduce:animate-none"
        >
          <header
            data-tauri-drag-region
            className="flex h-11 shrink-0 items-center justify-between border-b border-border px-4"
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
              role="tabpanel"
              aria-labelledby={settingsTabButtonId(active)}
              className="min-w-0 flex-1 overflow-y-auto"
            >
              <div className="mx-auto max-w-2xl px-6 py-6">{panelFor(active)}</div>
            </div>
          </div>
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}
