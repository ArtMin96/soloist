// The editor / terminal launch names this build offers in the Tools tab. The core stores the
// chosen launch command (null = the system default); probing which apps are actually installed
// is a later port, so these are a curated set of common Linux editors and terminals. One place,
// no magic strings — the panel maps the null "system default" to/from a sentinel at its edge
// (mirroring the Appearance font-family pattern, since Radix Select forbids an empty value).

import type { Option } from "@/lib/appearance";
import type { ToolDefaults } from "@/domain";

export const EDITOR_OPTIONS: Option<string | null>[] = [
  { value: null, label: "System default" },
  { value: "code", label: "VS Code" },
  { value: "code-insiders", label: "VS Code Insiders" },
  { value: "zed", label: "Zed" },
  { value: "subl", label: "Sublime Text" },
  { value: "nvim", label: "Neovim" },
  { value: "vim", label: "Vim" },
  { value: "emacs", label: "Emacs" },
  { value: "gnome-text-editor", label: "GNOME Text Editor" },
  { value: "kate", label: "Kate" },
];

export const TERMINAL_OPTIONS: Option<string | null>[] = [
  { value: null, label: "System default" },
  { value: "gnome-terminal", label: "GNOME Terminal" },
  { value: "konsole", label: "Konsole" },
  { value: "alacritty", label: "Alacritty" },
  { value: "kitty", label: "kitty" },
  { value: "wezterm", label: "WezTerm" },
  { value: "ghostty", label: "Ghostty" },
  { value: "tilix", label: "Tilix" },
  { value: "xterm", label: "xterm" },
];

// The pre-load placeholder for the Tools document — the facade returns the stored value (with
// its own defaults applied) on load, which supersedes this.
export const DEFAULT_TOOL_DEFAULTS: ToolDefaults = {
  default_editor: null,
  default_terminal: null,
};
