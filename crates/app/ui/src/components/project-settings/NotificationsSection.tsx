import { SettingRow } from "@/components/settings/controls/SettingRow";
import { SettingSelect } from "@/components/settings/controls/SettingSelect";
import { SettingsSection } from "@/components/settings/controls/SettingsSection";
import { NOTIFICATION_LEVEL_OPTIONS } from "@/lib/notifications";
import type { NotificationLevel, ProjectSettings } from "@/domain";

// How much the project notifies. Persists on change; a single command can still be held quieter
// than the project from the Commands tab.
export function NotificationsSection({
  settings,
  onNotificationLevel,
}: {
  settings: ProjectSettings;
  onNotificationLevel: (level: NotificationLevel) => void;
}) {
  return (
    <SettingsSection title="Notifications">
      <SettingRow
        label="Notify me about"
        description="All: terminal bells as well as crashes and agents waiting on you. Important only: crashes and agents waiting on you. None: nothing."
      >
        <SettingSelect
          value={settings.notification_level}
          options={NOTIFICATION_LEVEL_OPTIONS}
          onValueChange={(value) => onNotificationLevel(value as NotificationLevel)}
          ariaLabel="Notify me about"
        />
      </SettingRow>
    </SettingsSection>
  );
}
