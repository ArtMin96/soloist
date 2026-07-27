//! Appearance settings (global Appearance tab): the application theme and the terminal typography.
//!
//! The theme and every terminal value restyle both the app design tokens and the xterm.js renderer
//! (I5), so they live in the durable settings document and are projected to the frontend, which maps
//! each closed enum to its concrete CSS / xterm value in one place. Discrete pickers are closed
//! enums (never bare strings or numbers) so the valid set is the single source of truth; the exact
//! step-sets are ours (the Solo demo confirms the controls and a few defaults, not the granularity).

use serde::{Deserialize, Serialize};

/// The application color scheme. `System` follows the OS light/dark preference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Light,
    Dark,
    #[default]
    System,
}

/// A discrete text-size step for the "A·A·A…" size pickers (interface and terminal). The demo shows
/// a stepped picker, not a free numeric field; the five steps are ours, mapped to a concrete size in
/// the frontend.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontScale {
    ExtraSmall,
    Small,
    #[default]
    Medium,
    Large,
    ExtraLarge,
}

/// A terminal font weight — the standard CSS 100–900 steps. The demo defaults regular text to 400
/// and bold text to 600.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontWeight {
    W100,
    W200,
    W300,
    W400,
    W500,
    W600,
    W700,
    W800,
    W900,
}

/// Terminal line height — the vertical spacing between rows. The demo's control ranges roughly
/// 1.0–1.8 and defaults near 1.1; we offer a discrete set mapped to a concrete value in the frontend.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineHeight {
    Compact,
    #[default]
    Default,
    Comfortable,
    Spacious,
}

/// Terminal letter spacing — the horizontal spacing between characters. The demo's control ranges
/// roughly 0.5–1.3 and defaults near 0.9; we offer a discrete set mapped to a concrete value in the
/// frontend.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LetterSpacing {
    Tight,
    #[default]
    Default,
    Wide,
    Wider,
}

/// The shape of the terminal cursor while the pane has focus.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorStyle {
    #[default]
    Block,
    Underline,
    Bar,
}

/// The shape of the terminal cursor while the pane does not have focus. `None` hides it entirely,
/// which is a legitimate choice but a poor default — an unfocused pane then looks like it has no
/// cursor position at all, so the default outlines the cell instead.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorInactiveStyle {
    #[default]
    Outline,
    Block,
    Bar,
    Underline,
    None,
}

/// Terminal typography — the xterm.js renderer is restyled from these.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TerminalAppearance {
    /// Focus the terminal on a single click instead of a double click.
    pub focus_on_click: bool,
    /// The monospace font family, or `None` to use the app default. The frontend offers the system's
    /// installed monospace fonts; the core only stores the chosen name.
    pub font_family: Option<String>,
    /// Weight for regular terminal text (demo default 400).
    pub font_weight: FontWeight,
    /// Weight for bold terminal text (demo default 600).
    pub bold_font_weight: FontWeight,
    /// The terminal font-size step.
    pub font_scale: FontScale,
    /// Spacing between terminal rows.
    pub line_height: LineHeight,
    /// Spacing between terminal characters.
    pub letter_spacing: LetterSpacing,
    /// The cursor shape while the pane has focus.
    pub cursor_style: CursorStyle,
    /// The cursor shape while the pane does not have focus.
    pub cursor_inactive_style: CursorInactiveStyle,
    /// Whether the cursor blinks.
    pub cursor_blink: bool,
}

impl Default for TerminalAppearance {
    fn default() -> Self {
        Self {
            focus_on_click: false,
            font_family: None,
            font_weight: FontWeight::W400,
            bold_font_weight: FontWeight::W600,
            font_scale: FontScale::default(),
            line_height: LineHeight::default(),
            letter_spacing: LetterSpacing::default(),
            cursor_style: CursorStyle::default(),
            cursor_inactive_style: CursorInactiveStyle::default(),
            // xterm's own default is `false`; the app has always run a blinking cursor, so `true`
            // keeps an upgrade from silently changing the terminal.
            cursor_blink: true,
        }
    }
}

/// The Appearance tab document: the app theme, the interface size step, and the terminal typography.
/// Every field carries a serde default so a record an older build wrote still reads.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub theme: Theme,
    pub interface_font_scale: FontScale,
    pub terminal: TerminalAppearance,
}
