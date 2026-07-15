import { SettingRow } from "@/components/settings/controls/SettingRow";
import { SettingsSection } from "@/components/settings/controls/SettingsSection";
import { Switch } from "@/components/ui/switch";
import { useNotificationSettings } from "@/store/useNotificationSettings";

// The Notifications tab: the global master switch for desktop toasts. Off silences every
// notification; on defers to each project's crash/exit and terminal-alert switches (per-project
// settings). Pure presentation over the projected read model — no policy here.
export function NotificationsPanel() {
  const { value, update } = useNotificationSettings();

  return (
    <SettingsSection
      title="Notifications"
      description="Soloist shows desktop notifications when a command crashes, an agent needs you, or the terminal rings the bell. Each project's Notifications settings choose which of those it sends."
    >
      <SettingRow
        label="Desktop notifications"
        description="The master switch. Off silences every notification, whatever a project's own switches say."
      >
        <Switch
          checked={value.enabled}
          onCheckedChange={(enabled) => update({ ...value, enabled })}
          aria-label="Desktop notifications"
        />
      </SettingRow>
    </SettingsSection>
  );
}
