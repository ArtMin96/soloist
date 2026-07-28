import { SettingChoice } from "@/components/settings/controls/SettingChoice";
import { NOTIFICATION_LEVEL_CHOICES } from "@/lib/notifications";
import type { NotificationLevel, ProjectSettings } from "@/domain";

// How much the project notifies. Persists on change; a single command can still be held quieter
// than the project from the Commands tab.
//
// The levels sit as a choice list rather than a dropdown because their names do not separate them
// — "Important only" is the narrower setting, which reads to some as the louder one — and choosing
// wrong means silently missing a crash. So every description stays on screen to be compared,
// rather than one at a time behind a closed trigger.
//
// The choices carry their own borders, so they are not wrapped in the grouped settings card: that
// card exists to draw a frame around hairline-divided rows, and around a single fieldset whose rows
// are already framed it would only nest one box inside another.
export function NotificationsSection({
  settings,
  onNotificationLevel,
}: {
  settings: ProjectSettings;
  onNotificationLevel: (level: NotificationLevel) => void;
}) {
  return (
    <section className="mb-6">
      <fieldset className="flex flex-col">
        <legend className="mb-1.5 px-1 text-[0.6875rem] font-medium tracking-[0.01em] text-muted-foreground">
          Notify me about
        </legend>
        <SettingChoice
          value={settings.notification_level}
          choices={NOTIFICATION_LEVEL_CHOICES}
          onChange={onNotificationLevel}
          ariaLabel="Notify me about"
        />
      </fieldset>
    </section>
  );
}
