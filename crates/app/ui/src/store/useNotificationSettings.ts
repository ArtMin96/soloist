import { notificationSettings, setNotificationSettings } from "@/api";
import { DEFAULT_NOTIFICATIONS } from "@/lib/notifications";
import { useSettingsResource } from "@/store/useSettingsResource";

// The Notifications tab's read model: the global master switch for every desktop toast, auto-saved
// on change. The single place the Notifications document is bound to its facade getter/setter and
// pre-load default.
export function useNotificationSettings() {
  return useSettingsResource(notificationSettings, setNotificationSettings, DEFAULT_NOTIFICATIONS);
}
