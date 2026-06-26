import { SettingRow } from "@/components/settings/controls/SettingRow";
import { SettingSelect } from "@/components/settings/controls/SettingSelect";
import { SettingsSection } from "@/components/settings/controls/SettingsSection";
import { Switch } from "@/components/ui/switch";
import {
  PROCESS_CPU_OPTIONS,
  PROCESS_MEM_OPTIONS,
  PROJECT_CPU_OPTIONS,
  PROJECT_MEM_OPTIONS,
} from "@/lib/sidebar";
import { useSidebarSettings } from "@/store/sidebarSettingsContext";
import type {
  ProcessCpuThreshold,
  ProcessMemThreshold,
  ProjectCpuThreshold,
  ProjectMemThreshold,
  Sidebar,
} from "@/domain";

// The Sidebar tab: what the always-rendered process tree shows. "Hide empty sections" and "Show
// settings footer" drive the live sidebar projection today; the filter input, header usage
// badges, and project hover actions are later sidebar features, so their settings are saved and
// apply once those land (the note below). Pure presentation over the live Sidebar settings.
export function SidebarPanel() {
  const { sidebar, setSidebar } = useSidebarSettings();
  const set = (patch: Partial<Sidebar>) => setSidebar({ ...sidebar, ...patch });

  return (
    <div className="flex flex-col">
      <p className="mb-5 max-w-[54ch] text-xs text-muted-foreground">
        Filtering, header usage badges, and the project hover actions are coming in a later update —
        your choices below are saved and apply once they land.
      </p>

      <SettingsSection title="Filter">
        <SettingRow
          label="Show filter input"
          description="Show a filter box at the top of the sidebar."
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

      <SettingsSection title="Project headers">
        <SettingRow
          label="CPU usage"
          description="Show the CPU badge once usage reaches this level."
        >
          <SettingSelect
            value={sidebar.project_cpu_threshold}
            options={PROJECT_CPU_OPTIONS}
            onValueChange={(value) => set({ project_cpu_threshold: value as ProjectCpuThreshold })}
            ariaLabel="Project CPU usage threshold"
            className="w-28"
          />
        </SettingRow>
        <SettingRow
          label="Memory usage"
          description="Show the memory badge once usage reaches this level."
        >
          <SettingSelect
            value={sidebar.project_mem_threshold}
            options={PROJECT_MEM_OPTIONS}
            onValueChange={(value) => set({ project_mem_threshold: value as ProjectMemThreshold })}
            ariaLabel="Project memory usage threshold"
            className="w-28"
          />
        </SettingRow>
        <SettingRow label="Open in editor" description="Offer the open-in-editor hover action.">
          <Switch
            checked={sidebar.project_open_in_editor}
            onCheckedChange={(value) => set({ project_open_in_editor: value })}
            aria-label="Open in editor"
          />
        </SettingRow>
        <SettingRow label="Open in terminal" description="Offer the open-in-terminal hover action.">
          <Switch
            checked={sidebar.project_open_in_terminal}
            onCheckedChange={(value) => set({ project_open_in_terminal: value })}
            aria-label="Open in terminal"
          />
        </SettingRow>
        <SettingRow
          label="Show in file manager"
          description="Offer the show-in-file-manager hover action."
        >
          <Switch
            checked={sidebar.project_reveal_in_file_manager}
            onCheckedChange={(value) => set({ project_reveal_in_file_manager: value })}
            aria-label="Show in file manager"
          />
        </SettingRow>
      </SettingsSection>

      <SettingsSection title="Process headers">
        <SettingRow
          label="CPU usage"
          description="Show the CPU badge once usage reaches this level."
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
          description="Show the memory badge once usage reaches this level."
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
