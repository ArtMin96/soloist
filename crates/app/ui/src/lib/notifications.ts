import type { NotificationLevel, Notifications } from "@/domain";
import type { Option } from "@/lib/appearance";

// The master switch defaults on (mirrors soloist_core::Notifications::default); the facade's stored
// value supersedes this the moment it loads.
export const DEFAULT_NOTIFICATIONS: Notifications = { enabled: true };

// The three levels, in descending loudness — the one list every level picker renders from.
export const NOTIFICATION_LEVEL_OPTIONS: Option<NotificationLevel>[] = [
  { value: "all", label: "All" },
  { value: "important", label: "Important only" },
  { value: "none", label: "None" },
];

// A command with no level of its own inherits its project's. A dropdown cannot carry `null`, so
// that state travels as a sentinel string; the two mappings across that edge live here only.
const INHERIT = "inherit";

export const COMMAND_LEVEL_OPTIONS: Option<string>[] = [
  { value: INHERIT, label: "Same as project" },
  ...NOTIFICATION_LEVEL_OPTIONS,
];

export function commandLevelValue(level: NotificationLevel | null): string {
  return level ?? INHERIT;
}

export function commandLevelFromValue(value: string): NotificationLevel | null {
  return value === INHERIT ? null : (value as NotificationLevel);
}
