import type { AttentionKind, NotificationLevel, Notifications } from "@/domain";
import type { Choice } from "@/lib/appearance";

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

// How an alert is dressed as an in-app toast, and how long it stays.
export interface ToastDisplay {
  /** A shape carrying what happened without color, from the same vocabulary as process status. */
  glyph: string;
  /** Tailwind text-color utility for the glyph. */
  toneClass: string;
  /** How long the toast stays before dismissing itself; `null` stays until it is acted on. */
  dismissAfterMs: number | null;
}

// Long enough to read a two-line alert at a glance without standing in front of the work behind it.
// Also what a toast that persisted until now gets once the user has acted on it.
export const TOAST_LIFETIME_MS = 6000;

// The single place a kind becomes a toast. The core decides whether an alert is raised at all and
// writes its words; what is left is presentation, and it lives here rather than in the component so
// a toast and anything else that renders a kind cannot drift apart.
//
// What stays until acted on: a crash and auto-restart giving up. Both leave a process down and
// waiting on a decision, so a toast that expires would take the only prompt with it. Everything
// else reports something that has already resolved itself, so it expires. Saturated color is spent
// on the two that report a process's state; a terminal's own signals stay muted, since they are
// something a program said rather than a state the supervisor is in.
//
// The exhaustive Record makes the compiler demand an entry for every kind the core can raise.
export const TOAST: Record<AttentionKind, ToastDisplay> = {
  crashed: { glyph: "✕", toneClass: "text-status-crashed", dismissAfterMs: null },
  restart_exhausted: { glyph: "⚠", toneClass: "text-status-exhausted", dismissAfterMs: null },
  agent_permission: {
    glyph: "◆",
    toneClass: "text-status-attention",
    dismissAfterMs: TOAST_LIFETIME_MS,
  },
  agent_error: {
    glyph: "✕",
    toneClass: "text-status-crashed",
    dismissAfterMs: TOAST_LIFETIME_MS,
  },
  terminal_bell: {
    glyph: "◇",
    toneClass: "text-muted-foreground",
    dismissAfterMs: TOAST_LIFETIME_MS,
  },
  terminal_notification: {
    glyph: "◇",
    toneClass: "text-muted-foreground",
    dismissAfterMs: TOAST_LIFETIME_MS,
  },
};

// How many toasts are on screen at once; the rest stack behind them, newest in front. A burst of
// alerts must never grow an unbounded column down the window.
export const VISIBLE_TOASTS = 3;

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
