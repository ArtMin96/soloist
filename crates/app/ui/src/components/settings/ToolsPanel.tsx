import { NullableSelect } from "@/components/settings/controls/NullableSelect";
import { SettingRow } from "@/components/settings/controls/SettingRow";
import { SettingsSection } from "@/components/settings/controls/SettingsSection";
import { EDITOR_OPTIONS, TERMINAL_OPTIONS } from "@/lib/tools";
import { useToolSettings } from "@/store/useToolSettings";

// The Tools tab: the default editor and terminal used when opening projects. Both fall back to
// the system default (the null option, via NullableSelect); the project can override the editor
// (per-project settings, 11a). Pure presentation over the projected read model — no policy here.
export function ToolsPanel() {
  const { value, update } = useToolSettings();

  return (
    <SettingsSection title="Defaults">
      <SettingRow
        label="Default editor"
        description="Used when opening a project. Can be overridden per project."
      >
        <NullableSelect<string>
          value={value.default_editor}
          options={EDITOR_OPTIONS}
          onValueChange={(default_editor) => update({ ...value, default_editor })}
          ariaLabel="Default editor"
          className="w-48"
        />
      </SettingRow>
      <SettingRow
        label="Default terminal"
        description="Used when opening a project's directory in a terminal."
      >
        <NullableSelect<string>
          value={value.default_terminal}
          options={TERMINAL_OPTIONS}
          onValueChange={(default_terminal) => update({ ...value, default_terminal })}
          ariaLabel="Default terminal"
          className="w-48"
        />
      </SettingRow>
    </SettingsSection>
  );
}
