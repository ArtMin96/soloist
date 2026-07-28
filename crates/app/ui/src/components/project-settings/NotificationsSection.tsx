import { SettingChoice } from "@/components/settings/controls/SettingChoice";
import { SettingsSection } from "@/components/settings/controls/SettingsSection";
import { NOTIFICATION_LEVEL_CHOICES } from "@/lib/notifications";
import type { NotificationLevel, ProjectSettings } from "@/domain";

// How much the project notifies. Persists on change; a single command can still be held quieter
// than the project from the Commands tab.
//
// The levels sit as a choice list rather than a dropdown because their names do not separate them
// — "Important only" is the narrower setting, which reads to some as the louder one — and choosing
// wrong means silently missing a crash. So every description stays on screen to be compared,
// rather than one at a time behind a closed trigger.
export function NotificationsSection({
  settings,
  onNotificationLevel,
}: {
  settings: ProjectSettings;
  onNotificationLevel: (level: NotificationLevel) => void;
}) {
  return (
    <SettingsSection title="Notifications">
      <fieldset className="flex flex-col py-3">
        <legend className="mb-2 text-[0.8125rem] text-foreground">Notify me about</legend>
        <SettingChoice
          value={settings.notification_level}
          choices={NOTIFICATION_LEVEL_CHOICES}
          onChange={onNotificationLevel}
          ariaLabel="Notify me about"
        />
      </fieldset>
    </SettingsSection>
  );
}
