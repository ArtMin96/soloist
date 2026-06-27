//! Hotkeys settings (global Hotkeys tab): a remappable keymap of named actions.
//!
//! The action set and their **default** bindings are code-defined here — the single source of truth.
//! The durable document stores only the user's deviations: a remap ([`Binding`]) or a disable
//! (`None`). An action with no stored override uses its default, so "Reset all to defaults" is simply
//! clearing the overrides, and a future build that changes a default reaches every user who has not
//! overridden that action.
//!
//! Each action has a [`HotkeyScope`]; the same key may bind different actions in different scopes
//! (e.g. "previous project" in the sidebar and "previous process" in the terminal), so a conflict is
//! only ever within one scope. macOS `⌘`/`⌥` from Solo's reference are remapped to Ctrl/Alt for Linux.
//! Key tokens follow the web `KeyboardEvent.key` convention so the frontend matches a real key event
//! directly.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The context a hotkey is active in. Bindings only conflict within the same scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HotkeyScope {
    General,
    Sidebar,
    Terminal,
}

/// A named, remappable action. The closed set is the single source the settings document and the
/// frontend keyboard handler iterate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HotkeyAction {
    // General — app-wide actions, palettes, and system shortcuts.
    OpenCommandPalette,
    QuickActions,
    QuickJump,
    NewAgentOrTerminal,
    OpenSettings,
    OpenTerminalSearch,
    CloseAgentOrTerminal,
    // Sidebar — navigation.
    NextProjectGroup,
    PrevProjectGroup,
    NextSection,
    PrevSection,
    JumpToAgents,
    JumpToCommands,
    JumpToTerminals,
    CollapseOrSection,
    JumpToParentProject,
    ExpandProject,
    RestartSelection,
    // Terminal — active while the terminal is focused.
    PreviousProcess,
    NextProcess,
    IncreaseTerminalFontSize,
    DecreaseTerminalFontSize,
}

/// A key chord: the modifier flags plus the main key (a `KeyboardEvent.key` token, e.g. `K`,
/// `ArrowDown`, `=`). Stored as data, compared by value for conflict detection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    #[serde(rename = "super")]
    pub super_key: bool,
    pub key: String,
}

impl Binding {
    /// A `Ctrl`-modified chord (the Linux remap of a Solo `⌘` shortcut).
    fn ctrl(key: &str) -> Self {
        Self {
            ctrl: true,
            alt: false,
            shift: false,
            super_key: false,
            key: key.to_string(),
        }
    }

    /// An `Alt`-modified chord (the Linux remap of a Solo `⌥`/Option shortcut).
    fn alt(key: &str) -> Self {
        Self {
            ctrl: false,
            alt: true,
            shift: false,
            super_key: false,
            key: key.to_string(),
        }
    }

    /// An unmodified key (e.g. a single-letter sidebar shortcut).
    fn plain(key: &str) -> Self {
        Self {
            ctrl: false,
            alt: false,
            shift: false,
            super_key: false,
            key: key.to_string(),
        }
    }
}

impl HotkeyAction {
    /// Every action, in display order — the single list the document and UI iterate.
    pub const ALL: [HotkeyAction; 22] = [
        HotkeyAction::OpenCommandPalette,
        HotkeyAction::QuickActions,
        HotkeyAction::QuickJump,
        HotkeyAction::NewAgentOrTerminal,
        HotkeyAction::OpenSettings,
        HotkeyAction::OpenTerminalSearch,
        HotkeyAction::CloseAgentOrTerminal,
        HotkeyAction::NextProjectGroup,
        HotkeyAction::PrevProjectGroup,
        HotkeyAction::NextSection,
        HotkeyAction::PrevSection,
        HotkeyAction::JumpToAgents,
        HotkeyAction::JumpToCommands,
        HotkeyAction::JumpToTerminals,
        HotkeyAction::CollapseOrSection,
        HotkeyAction::JumpToParentProject,
        HotkeyAction::ExpandProject,
        HotkeyAction::RestartSelection,
        HotkeyAction::PreviousProcess,
        HotkeyAction::NextProcess,
        HotkeyAction::IncreaseTerminalFontSize,
        HotkeyAction::DecreaseTerminalFontSize,
    ];

    /// The scope this action is active in.
    pub fn scope(self) -> HotkeyScope {
        use HotkeyAction::*;
        match self {
            OpenCommandPalette | QuickActions | QuickJump | NewAgentOrTerminal | OpenSettings
            | OpenTerminalSearch | CloseAgentOrTerminal => HotkeyScope::General,
            NextProjectGroup | PrevProjectGroup | NextSection | PrevSection | JumpToAgents
            | JumpToCommands | JumpToTerminals | CollapseOrSection | JumpToParentProject
            | ExpandProject | RestartSelection => HotkeyScope::Sidebar,
            PreviousProcess | NextProcess | IncreaseTerminalFontSize | DecreaseTerminalFontSize => {
                HotkeyScope::Terminal
            }
        }
    }

    /// The code-defined default binding (Solo's `⌘`/`⌥` reference remapped to Ctrl/Alt for Linux).
    pub fn default_binding(self) -> Binding {
        use HotkeyAction::*;
        match self {
            OpenCommandPalette => Binding::ctrl("K"),
            QuickActions => Binding::ctrl("P"),
            QuickJump => Binding::ctrl("E"),
            NewAgentOrTerminal => Binding::ctrl("T"),
            OpenSettings => Binding::ctrl(","),
            OpenTerminalSearch => Binding::ctrl("F"),
            CloseAgentOrTerminal => Binding::ctrl("W"),
            NextProjectGroup => Binding::ctrl("ArrowDown"),
            PrevProjectGroup => Binding::ctrl("ArrowUp"),
            NextSection => Binding::alt("ArrowDown"),
            PrevSection => Binding::alt("ArrowUp"),
            JumpToAgents => Binding::alt("A"),
            JumpToCommands => Binding::alt("C"),
            JumpToTerminals => Binding::alt("T"),
            CollapseOrSection => Binding::plain("ArrowLeft"),
            JumpToParentProject => Binding::ctrl("ArrowLeft"),
            ExpandProject => Binding::plain("ArrowRight"),
            RestartSelection => Binding::plain("R"),
            PreviousProcess => Binding::ctrl("ArrowUp"),
            NextProcess => Binding::ctrl("ArrowDown"),
            IncreaseTerminalFontSize => Binding::ctrl("="),
            DecreaseTerminalFontSize => Binding::ctrl("-"),
        }
    }
}

/// One action's effective state in the read model the UI renders.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HotkeyBindingView {
    pub action: HotkeyAction,
    pub scope: HotkeyScope,
    /// The effective binding, or `None` when the action is disabled.
    pub binding: Option<Binding>,
    /// Whether the effective binding is the code default (no user override).
    pub is_default: bool,
    /// Whether this binding collides with another action in the same scope (`conflicts`) — the
    /// rows the UI flags. Carried in the view so the frontend never re-derives the rule.
    pub conflict: bool,
}

/// The Hotkeys tab document. Stores only deviations from the defaults: `Some(binding)` is a remap,
/// `None` is a disabled action; an action absent from the map uses its code default. So the persisted
/// record stays small and a default change reaches everyone who has not overridden the action.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Hotkeys {
    overrides: BTreeMap<HotkeyAction, Option<Binding>>,
}

impl Hotkeys {
    /// The effective binding for `action` — the override if one is stored (a remap, or `None` when
    /// disabled), otherwise the code default.
    pub fn binding(&self, action: HotkeyAction) -> Option<Binding> {
        match self.overrides.get(&action) {
            Some(over) => over.clone(),
            None => Some(action.default_binding()),
        }
    }

    /// Remaps `action` to `binding` (an override).
    pub fn remap(&mut self, action: HotkeyAction, binding: Binding) {
        self.overrides.insert(action, Some(binding));
    }

    /// Disables `action` (hover-and-press-x): it keeps no binding until reset.
    pub fn disable(&mut self, action: HotkeyAction) {
        self.overrides.insert(action, None);
    }

    /// Resets `action` to its code default by dropping any override.
    pub fn reset(&mut self, action: HotkeyAction) {
        self.overrides.remove(&action);
    }

    /// Resets every action to its code default ("Reset all to defaults").
    pub fn reset_all(&mut self) {
        self.overrides.clear();
    }

    /// The full keymap read model — every action with its scope, effective binding, and whether it
    /// is still the default.
    pub fn view(&self) -> Vec<HotkeyBindingView> {
        let conflicts: std::collections::BTreeSet<HotkeyAction> =
            self.conflicts().into_iter().collect();
        HotkeyAction::ALL
            .into_iter()
            .map(|action| HotkeyBindingView {
                action,
                scope: action.scope(),
                binding: self.binding(action),
                is_default: !self.overrides.contains_key(&action),
                conflict: conflicts.contains(&action),
            })
            .collect()
    }

    /// Actions whose effective binding collides with another action **in the same scope** — the
    /// conflicts the UI flags. A shared key across different scopes is never a conflict.
    pub fn conflicts(&self) -> Vec<HotkeyAction> {
        let mut clashing = Vec::new();
        let bound: Vec<_> = HotkeyAction::ALL
            .into_iter()
            .filter_map(|a| self.binding(a).map(|b| (a, b)))
            .collect();
        for (action, binding) in &bound {
            let collides = bound.iter().any(|(other, other_binding)| {
                other != action && other.scope() == action.scope() && other_binding == binding
            });
            if collides {
                clashing.push(*action);
            }
        }
        clashing
    }
}

#[cfg(test)]
#[path = "hotkeys_tests.rs"]
mod tests;
