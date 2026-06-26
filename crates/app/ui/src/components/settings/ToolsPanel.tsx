import { SettingRow } from "@/components/settings/controls/SettingRow";
import { SettingSelect } from "@/components/settings/controls/SettingSelect";
import { SettingsSection } from "@/components/settings/controls/SettingsSection";
import { EDITOR_OPTIONS, TERMINAL_OPTIONS } from "@/lib/tools";
import { useToolSettings } from "@/store/useToolSettings";

// Radix Select forbids a null/empty item value, so "system default" rides a sentinel that maps
// to/from null at this edge only (the same pattern as the Appearance font-family picker).
const SYSTEM_DEFAULT = "__system_default__";

const editorOptions = EDITOR_OPTIONS.map((option) => ({
  value: option.value ?? SYSTEM_DEFAULT,
  label: option.label,
}));
const terminalOptions = TERMINAL_OPTIONS.map((option) => ({
  value: option.value ?? SYSTEM_DEFAULT,
  label: option.label,
}));

// The Tools tab: the default editor and terminal used when opening projects. Both fall back to
// the system default; the project can override the editor (per-project settings, 11a). Pure
// presentation over the projected read model — no policy here.
export function ToolsPanel() {
  const { value, update } = useToolSettings();

  return (
    <SettingsSection title="Defaults">
      <SettingRow
        label="Default editor"
        description="Used when opening a project. Can be overridden per project."
      >
        <SettingSelect
          value={value.default_editor ?? SYSTEM_DEFAULT}
          options={editorOptions}
          onValueChange={(next) =>
            update({ ...value, default_editor: next === SYSTEM_DEFAULT ? null : next })
          }
          ariaLabel="Default editor"
          className="w-48"
        />
      </SettingRow>
      <SettingRow
        label="Default terminal"
        description="Used when opening a project's directory in a terminal."
      >
        <SettingSelect
          value={value.default_terminal ?? SYSTEM_DEFAULT}
          options={terminalOptions}
          onValueChange={(next) =>
            update({ ...value, default_terminal: next === SYSTEM_DEFAULT ? null : next })
          }
          ariaLabel="Default terminal"
          className="w-48"
        />
      </SettingRow>
    </SettingsSection>
  );
}
