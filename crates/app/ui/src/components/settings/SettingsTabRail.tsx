import type { KeyboardEvent } from "react";
import { SETTINGS_TABS, settingsTabButtonId, type SettingsTabId } from "@/components/settings/tabs";
import { cn } from "@/lib/utils";

// The left rail of settings sections. The active tab carries a full-height azure selection
// marker (the same affordance as a selected sidebar row, not a decorative stripe).
export function SettingsTabRail({
  active,
  onSelect,
}: {
  active: SettingsTabId;
  onSelect: (id: SettingsTabId) => void;
}) {
  // Arrow / Home / End move the selection and the focus together (the WAI-ARIA tablist pattern,
  // with automatic activation since each panel is cheap), so the rail is fully keyboard-operable
  // and not just reachable on its one already-active tab.
  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const current = SETTINGS_TABS.findIndex((tab) => tab.id === active);
    const last = SETTINGS_TABS.length - 1;
    let next: number;
    switch (event.key) {
      case "ArrowDown":
      case "ArrowRight":
        next = current >= last ? 0 : current + 1;
        break;
      case "ArrowUp":
      case "ArrowLeft":
        next = current <= 0 ? last : current - 1;
        break;
      case "Home":
        next = 0;
        break;
      case "End":
        next = last;
        break;
      default:
        return;
    }
    event.preventDefault();
    const nextId = SETTINGS_TABS[next].id;
    onSelect(nextId);
    document.getElementById(settingsTabButtonId(nextId))?.focus();
  }

  return (
    <div
      role="tablist"
      aria-orientation="vertical"
      aria-label="Settings sections"
      onKeyDown={onKeyDown}
      className="flex w-48 shrink-0 flex-col gap-0.5 overflow-y-auto border-r border-border bg-sidebar p-2"
    >
      {SETTINGS_TABS.map((tab) => {
        const isActive = tab.id === active;
        return (
          <button
            key={tab.id}
            id={settingsTabButtonId(tab.id)}
            type="button"
            role="tab"
            aria-selected={isActive}
            tabIndex={isActive ? 0 : -1}
            onClick={() => onSelect(tab.id)}
            className={cn(
              "relative rounded-sm px-2.5 py-1.5 text-left text-[0.8125rem] transition-colors",
              "focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-ring",
              isActive
                ? "bg-sidebar-accent text-foreground"
                : "text-muted-foreground hover:bg-sidebar-accent hover:text-foreground",
            )}
          >
            {isActive && (
              <span
                aria-hidden
                className="absolute inset-y-1 left-0 w-0.5 rounded-full bg-primary"
              />
            )}
            {tab.label}
          </button>
        );
      })}
    </div>
  );
}
