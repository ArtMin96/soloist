import { SettingRow } from "@/components/settings/controls/SettingRow";
import { SettingsSection } from "@/components/settings/controls/SettingsSection";
import { Switch } from "@/components/ui/switch";
import type { ProjectSettings } from "@/domain";

// The project's notification toggles: crash/exit alerts and terminal-bell alerts. Both persist on
// change; a single command can still be silenced from the Commands tab.
export function NotificationsSection({
  settings,
  onCrashExitAlerts,
  onTerminalAlerts,
}: {
  settings: ProjectSettings;
  onCrashExitAlerts: (enabled: boolean) => void;
  onTerminalAlerts: (enabled: boolean) => void;
}) {
  return (
    <SettingsSection title="Notifications">
      <SettingRow
        label="Crash & exit alerts"
        description="Notify when a command crashes or exits unexpectedly."
      >
        <Switch
          checked={settings.crash_exit_alerts}
          onCheckedChange={onCrashExitAlerts}
          aria-label="Crash and exit alerts"
        />
      </SettingRow>
      <SettingRow
        label="Terminal alerts"
        description="Notify when a command rings the terminal bell or asks for attention."
      >
        <Switch
          checked={settings.terminal_alerts}
          onCheckedChange={onTerminalAlerts}
          aria-label="Terminal alerts"
        />
      </SettingRow>
    </SettingsSection>
  );
}
