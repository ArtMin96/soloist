import { useState } from "react";
import { SettingRow } from "@/components/settings/controls/SettingRow";
import { SettingsSection } from "@/components/settings/controls/SettingsSection";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { monogram } from "@/store/projects";
import type { ProjectSettings } from "@/domain";

// The project's startup, editor, and icon preferences. The switch persists on change; text fields
// commit on blur or Enter. An empty editor field clears the override (falls back to the global
// default); the icon field accepts a `solo.yml` icon path and surfaces a rejected format inline.
export function ProjectSettingsSection({
  name,
  icon,
  settings,
  resolvedEditor,
  onAutoStartGate,
  onEditorOverride,
  onSetIcon,
}: {
  name: string;
  icon: string | null;
  settings: ProjectSettings;
  resolvedEditor: string | null;
  onAutoStartGate: (engaged: boolean) => void;
  onEditorOverride: (editor: string | null) => void;
  onSetIcon: (icon: string) => Promise<void>;
}) {
  const [iconError, setIconError] = useState<string | null>(null);

  const commitEditor = (value: string) => {
    const next = value.trim() || null;
    if (next !== (settings.editor_override ?? null)) onEditorOverride(next);
  };

  const commitIcon = (value: string) => {
    const next = value.trim();
    if (!next) return;
    setIconError(null);
    onSetIcon(next).catch((e) => setIconError(String(e)));
  };

  return (
    <div className="flex flex-col">
      <SettingsSection title="Startup">
        <SettingRow
          label="Suppress auto-start"
          description="Keep this project's commands from starting automatically when it opens, whatever each command's own auto-start setting is."
        >
          <Switch
            checked={settings.auto_start_gate}
            onCheckedChange={onAutoStartGate}
            aria-label="Suppress auto-start"
          />
        </SettingRow>
      </SettingsSection>

      <SettingsSection title="Tools">
        <SettingRow
          label="Editor override"
          description="Open this project in this editor instead of the global default. Leave empty to use the default."
        >
          <Input
            key={settings.editor_override ?? ""}
            defaultValue={settings.editor_override ?? ""}
            placeholder={resolvedEditor ?? "System default"}
            onBlur={(e) => commitEditor(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") e.currentTarget.blur();
            }}
            aria-label="Editor override"
            className="w-44"
          />
        </SettingRow>
      </SettingsSection>

      <SettingsSection title="Icon">
        <SettingRow
          label="Project icon"
          description="A solo.yml icon path. Use png, jpg, gif, ico, or webp."
        >
          <div className="flex flex-col items-end gap-1.5">
            <div className="flex items-center gap-2">
              <Avatar className="size-6">
                {icon && <AvatarImage src={icon} alt="" />}
                <AvatarFallback>{monogram(name)}</AvatarFallback>
              </Avatar>
              <Input
                placeholder="assets/project-icon.png"
                onBlur={(e) => commitIcon(e.currentTarget.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") e.currentTarget.blur();
                }}
                aria-label="Project icon path"
                className="w-44 font-mono text-xs"
              />
            </div>
            {iconError && (
              <p className="max-w-[30ch] text-right text-xs text-destructive">{iconError}</p>
            )}
          </div>
        </SettingRow>
      </SettingsSection>
    </div>
  );
}
