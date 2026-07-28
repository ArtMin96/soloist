import type { Choice } from "@/components/settings/controls/SettingChoice";
import type { NotificationLevel, Notifications } from "@/domain";

// The master switch defaults on (mirrors soloist_core::Notifications::default); the facade's stored
// value supersedes this the moment it loads.
export const DEFAULT_NOTIFICATIONS: Notifications = { enabled: true };

// The three levels, in descending loudness — the one list every level picker renders from.
//
// Each description names what that level admits, tracking the severity the core sorts on: whatever
// stops or blocks work (a crash, the restart limit, an agent waiting on the user) survives every
// level but None, while a terminal's own signals survive only All. The narrower level says what it
// drops as well as what it keeps, because a user who reads "Important only" as *more* alerts
// silently stops hearing about crashes.
export const NOTIFICATION_LEVEL_CHOICES: Choice<NotificationLevel>[] = [
  {
    value: "all",
    label: "All",
    description: "Crashes, agents that need you, terminal bells, and notifications a script sends.",
  },
  {
    value: "important",
    label: "Important only",
    description:
      "Crashes and agents that need you. Terminal bells and script notifications are dropped.",
  },
  {
    value: "none",
    label: "None",
    description: "Nothing at all, not even a crash.",
  },
];

// A command with no level of its own inherits its project's. A radio value cannot carry `null`, so
// that state travels as a sentinel string; the two mappings across that edge live here only.
const INHERIT = "inherit";

export function commandLevelValue(level: NotificationLevel | null): string {
  return level ?? INHERIT;
}

export function commandLevelFromValue(value: string): NotificationLevel | null {
  return value === INHERIT ? null : (value as NotificationLevel);
}

/** What a level is called on its own, for prose naming a level the core resolved. */
export function levelLabel(level: NotificationLevel): string {
  const choice = NOTIFICATION_LEVEL_CHOICES.find((candidate) => candidate.value === level);
  return choice ? choice.label : level;
}

// The command picker's choices: inheriting first, then the three levels. Inheriting is an option of
// its own rather than an empty state, so it can never be mistaken for None — they mean opposite
// things, and only one of them is silence. It names the project's current level, because the
// project setting lives on another tab and cannot be read from here.
export function commandLevelChoices(projectLevel: NotificationLevel): Choice<string>[] {
  return [
    {
      value: INHERIT,
      label: "Same as project",
      description: `Follows the project setting, currently ${levelLabel(projectLevel)}.`,
    },
    ...NOTIFICATION_LEVEL_CHOICES,
  ];
}
