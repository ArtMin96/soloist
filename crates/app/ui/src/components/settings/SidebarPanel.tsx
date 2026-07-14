import { SettingRow } from "@/components/settings/controls/SettingRow";
import { SettingSelect } from "@/components/settings/controls/SettingSelect";
import { SettingsSection } from "@/components/settings/controls/SettingsSection";
import { Switch } from "@/components/ui/switch";
import { PROCESS_CPU_OPTIONS, PROCESS_MEM_OPTIONS } from "@/lib/sidebar";
import { useSidebarSettings } from "@/store/sidebarSettingsContext";
import type { ProcessCpuThreshold, ProcessMemThreshold, Sidebar } from "@/domain";

// The Sidebar tab: what the always-rendered process tree shows. Every control here drives the live
// sidebar — the filter input, empty-section hiding, the per-process usage thresholds, and the
// settings footer. Pure presentation over the live Sidebar settings.
export function SidebarPanel() {
  const { sidebar, setSidebar } = useSidebarSettings();
  const set = (patch: Partial<Sidebar>) => setSidebar({ ...sidebar, ...patch });

  return (
    <div className="flex flex-col">
      <SettingsSection title="Filter">
        <SettingRow
          label="Show filter input"
          description="Show a box at the top of the sidebar for filtering processes by name."
        >
          <Switch
            checked={sidebar.show_filter_input}
            onCheckedChange={(value) => set({ show_filter_input: value })}
            aria-label="Show filter input"
          />
        </SettingRow>
      </SettingsSection>

      <SettingsSection title="Sections">
        <SettingRow
          label="Hide empty sections"
          description="Hide subtype sections with no processes (e.g. Agents, Terminals)."
        >
          <Switch
            checked={sidebar.hide_empty_sections}
            onCheckedChange={(value) => set({ hide_empty_sections: value })}
            aria-label="Hide empty sections"
          />
        </SettingRow>
      </SettingsSection>

      <SettingsSection title="Process rows">
        <SettingRow
          label="CPU usage"
          description="Show a row's CPU read-out once usage reaches this level."
        >
          <SettingSelect
            value={sidebar.process_cpu_threshold}
            options={PROCESS_CPU_OPTIONS}
            onValueChange={(value) => set({ process_cpu_threshold: value as ProcessCpuThreshold })}
            ariaLabel="Process CPU usage threshold"
            className="w-28"
          />
        </SettingRow>
        <SettingRow
          label="Memory usage"
          description="Show a row's memory read-out once usage reaches this level."
        >
          <SettingSelect
            value={sidebar.process_mem_threshold}
            options={PROCESS_MEM_OPTIONS}
            onValueChange={(value) => set({ process_mem_threshold: value as ProcessMemThreshold })}
            ariaLabel="Process memory usage threshold"
            className="w-28"
          />
        </SettingRow>
      </SettingsSection>

      <SettingsSection title="Footer">
        <SettingRow
          label="Show settings footer"
          description="Show the Settings button at the bottom of the sidebar. Still reachable via the command palette and hotkey."
        >
          <Switch
            checked={sidebar.show_settings_footer}
            onCheckedChange={(value) => set({ show_settings_footer: value })}
            aria-label="Show settings footer"
          />
        </SettingRow>
      </SettingsSection>
    </div>
  );
}
