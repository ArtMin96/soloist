import { DesktopAlertsSection } from "@/components/settings/DesktopAlertsSection";
import { NullableSelect } from "@/components/settings/controls/NullableSelect";
import { SettingRow } from "@/components/settings/controls/SettingRow";
import { SettingsSection } from "@/components/settings/controls/SettingsSection";
import { Switch } from "@/components/ui/switch";
import { ALERT_SOUND_OPTIONS } from "@/lib/notifications";
import { useNotificationSettings } from "@/store/useNotificationSettings";

// The Notifications tab: what Soloist alerts about at all, what it sounds like, and whether those
// alerts can reach the desktop.
//
// The preferences come first and the machine's state second, because they answer different
// questions and only one of them is the user's to change. Pure presentation over the projected read
// model — no policy here; the core applies the master switch before it composes anything.
export function NotificationsPanel() {
  const { value, update } = useNotificationSettings();

  return (
    <div className="flex flex-col">
      <SettingsSection
        title="Notifications"
        description="Soloist alerts you when a command crashes, an agent needs you, or a terminal rings the bell. Each project's own settings choose which of those it sends."
      >
        <SettingRow
          label="Show notifications"
          // Named for everything it silences rather than for the desktop alone: the reactor
          // consults this before it decides where an alert goes, so off is off on every surface.
          description="The master switch. Off silences every alert — on the desktop and in the app alike — whatever a project's own settings say."
        >
          <Switch
            checked={value.enabled}
            onCheckedChange={(enabled) => update({ ...value, enabled })}
            aria-label="Show notifications"
          />
        </SettingRow>
        <SettingRow
          label="Alert sound"
          description="Played with every alert. The name is resolved against your desktop's sound theme, so it sounds like the rest of your system."
        >
          <NullableSelect
            value={value.bell}
            options={ALERT_SOUND_OPTIONS}
            onValueChange={(bell) => update({ ...value, bell })}
            ariaLabel="Alert sound"
            className="w-40"
          />
        </SettingRow>
      </SettingsSection>

      <DesktopAlertsSection />
    </div>
  );
}
