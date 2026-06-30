// The Hotkeys tab's display data and the chord <-> KeyboardEvent conversion. The action set,
// scopes, and default bindings live in the core (the single source); this maps each closed enum
// to its human label and renders / parses a chord the way the core stores it (uppercase letters,
// `KeyboardEvent.key` tokens) so a captured chord round-trips and the live handler matches a real
// key event.

import type { Binding, HotkeyAction, HotkeyScope } from "@/domain";

export const HOTKEY_SCOPE_ORDER: HotkeyScope[] = ["general", "sidebar", "terminal"];

export const HOTKEY_SCOPE_LABELS: Record<HotkeyScope, string> = {
  general: "General",
  sidebar: "Sidebar",
  terminal: "Terminal",
};

export const HOTKEY_SCOPE_DESCRIPTIONS: Record<HotkeyScope, string> = {
  general: "App-wide actions, palettes, and system shortcuts.",
  sidebar: "Navigating the project tree.",
  terminal: "Active while the terminal is focused.",
};

// The human label for each action — one source the panel and search read.
export const HOTKEY_ACTION_LABELS: Record<HotkeyAction, string> = {
  open_command_palette: "Open command palette",
  quick_actions: "Quick actions",
  quick_jump: "Quick jump",
  new_agent_or_terminal: "New agent or terminal",
  open_settings: "Open settings",
  open_terminal_search: "Open terminal search",
  close_agent_or_terminal: "Close agent or terminal",
  next_project_group: "Next project group",
  prev_project_group: "Previous project group",
  next_section: "Next section",
  prev_section: "Previous section",
  jump_to_agents: "Jump to Agents",
  jump_to_commands: "Jump to Commands",
  jump_to_terminals: "Jump to Terminals",
  collapse_or_section: "Collapse / go to section",
  jump_to_parent_project: "Jump to parent project",
  expand_project: "Expand project",
  restart_selection: "Restart",
  previous_process: "Previous process",
  next_process: "Next process",
  increase_terminal_font_size: "Increase terminal font size",
  decrease_terminal_font_size: "Decrease terminal font size",
};

// The keys that are modifiers themselves — a chord is only complete once a non-modifier key is
// pressed, so a capture ignores these.
const MODIFIER_KEYS = new Set(["Control", "Alt", "Shift", "Meta"]);

// How a stored key token renders to the eye (arrows as glyphs, space named, letters as-is).
const KEY_GLYPHS: Record<string, string> = {
  ArrowUp: "↑",
  ArrowDown: "↓",
  ArrowLeft: "←",
  ArrowRight: "→",
  " ": "Space",
  Escape: "Esc",
};

// The ordered modifier tokens of a chord, then its key — e.g. ["Ctrl", "K"] or ["Alt", "↑"].
export function formatChord(binding: Binding): string[] {
  const tokens: string[] = [];
  if (binding.ctrl) tokens.push("Ctrl");
  if (binding.alt) tokens.push("Alt");
  if (binding.shift) tokens.push("Shift");
  if (binding.super) tokens.push("Super");
  tokens.push(KEY_GLYPHS[binding.key] ?? binding.key);
  return tokens;
}

// Build the chord a key event represents, the way the core stores it. Returns null while only a
// modifier is held (the capture waits for a real key). Single letters are uppercased so a press
// matches the core's "K"-style defaults without depending on the Shift state.
export function bindingFromEvent(event: KeyboardEvent): Binding | null {
  if (MODIFIER_KEYS.has(event.key)) return null;
  const key =
    event.key.length === 1 && /[a-z]/i.test(event.key) ? event.key.toUpperCase() : event.key;
  return {
    ctrl: event.ctrlKey,
    alt: event.altKey,
    shift: event.shiftKey,
    super: event.metaKey,
    key,
  };
}

// Whether a chord carries a command modifier (Ctrl/Alt/Super). Shift alone is still typing, so
// it does not count. App-wide shortcuts with a command modifier fire even while a text field is
// focused; bare-key shortcuts must yield to typing, so the live handler uses this to decide.
export function hasCommandModifier(binding: Binding): boolean {
  return binding.ctrl || binding.alt || binding.super;
}

export function bindingsEqual(a: Binding, b: Binding): boolean {
  return (
    a.ctrl === b.ctrl &&
    a.alt === b.alt &&
    a.shift === b.shift &&
    a.super === b.super &&
    a.key === b.key
  );
}

// True when the event originates in a text-editing surface (input, textarea, or
// contenteditable). A bare-key shortcut must yield to typing there; command-modifier
// shortcuts (Ctrl/Alt/Super) still fire everywhere, as in a native app.
export function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable;
}
