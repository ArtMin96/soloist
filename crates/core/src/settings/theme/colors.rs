use std::collections::BTreeMap;
use std::fmt;

use serde::{de::Error as _, Deserialize, Deserializer, Serialize};

macro_rules! theme_color_roles {
    ($( $variant:ident => $wire:literal ),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        /// One of the exact T3-v1 color roles Soloist accepts.
        pub enum ThemeColorRole {
            $(#[serde(rename = $wire)] $variant),+
        }

        impl ThemeColorRole {
            /// Every supported T3-v1 role, in canonical order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
        }
    };
}

theme_color_roles! {
    Canvas => "canvas",
    Chrome => "chrome",
    Toolbar => "toolbar",
    ToolbarForeground => "toolbarForeground",
    ToolbarBorder => "toolbarBorder",
    ToolbarControl => "toolbarControl",
    ToolbarControlForeground => "toolbarControlForeground",
    ToolbarControlHover => "toolbarControlHover",
    Surface => "surface",
    SurfaceRaised => "surfaceRaised",
    SurfaceOverlay => "surfaceOverlay",
    Text => "text",
    TextMuted => "textMuted",
    Border => "border",
    Input => "input",
    Focus => "focus",
    Accent => "accent",
    AccentForeground => "accentForeground",
    Secondary => "secondary",
    SecondaryForeground => "secondaryForeground",
    Muted => "muted",
    MutedForeground => "mutedForeground",
    Placeholder => "placeholder",
    SecondaryLabel => "secondaryLabel",
    IconMuted => "iconMuted",
    Error => "error",
    ErrorForeground => "errorForeground",
    ErrorSurface => "errorSurface",
    Warning => "warning",
    WarningForeground => "warningForeground",
    WarningSurface => "warningSurface",
    Update => "update",
    UpdateForeground => "updateForeground",
    UpdateSurface => "updateSurface",
    AccentSurface => "accentSurface",
    AccentSurfaceForeground => "accentSurfaceForeground",
    MessageSurface => "messageSurface",
    MessageForeground => "messageForeground",
    MessageAction => "messageAction",
    MessageActionForeground => "messageActionForeground",
    MessageActionHover => "messageActionHover",
    CodeBackground => "codeBackground",
    CodeForeground => "codeForeground",
    Sidebar => "sidebar",
    SidebarForeground => "sidebarForeground",
    SidebarMutedForeground => "sidebarMutedForeground",
    SidebarControlSurface => "sidebarControlSurface",
    SidebarRowHover => "sidebarRowHover",
    SidebarRowActive => "sidebarRowActive",
    SidebarRowSelected => "sidebarRowSelected",
    SidebarBorder => "sidebarBorder",
    TerminalBackground => "terminalBackground",
    TerminalForeground => "terminalForeground",
    TerminalCursor => "terminalCursor",
    TerminalSelection => "terminalSelection",
    TerminalScrollbar => "terminalScrollbar",
    TerminalScrollbarHover => "terminalScrollbarHover",
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
/// A normalized hexadecimal theme color.
pub struct ThemeColor(String);

impl ThemeColor {
    /// Parses #RGB(A) or #RRGGBB(AA), normalizing it to lowercase long form.
    pub fn parse(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        let digits = value.strip_prefix('#').unwrap_or_default();
        if matches!(digits.len(), 3 | 4 | 6 | 8)
            && digits.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            let normalized = if matches!(digits.len(), 3 | 4) {
                digits
                    .chars()
                    .flat_map(|digit| [digit.to_ascii_lowercase(); 2])
                    .collect::<String>()
            } else {
                digits.to_ascii_lowercase()
            };
            Ok(Self(format!("#{normalized}")))
        } else {
            Err(format!(
                "theme colors must be hexadecimal (#RGB, #RGBA, #RRGGBB, or #RRGGBBAA); got {value:?}"
            ))
        }
    }

    /// Returns the normalized hexadecimal value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ThemeColor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ThemeColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::parse(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
/// A strict map of T3-v1 color roles to validated hexadecimal colors.
pub struct ThemeColors(BTreeMap<ThemeColorRole, ThemeColor>);

impl ThemeColors {
    /// Returns a role's normalized color when supplied by the file.
    pub fn get(&self, role: ThemeColorRole) -> Option<&str> {
        self.0.get(&role).map(ThemeColor::as_str)
    }

    /// Returns the number of supplied color roles.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns whether the palette supplies no roles.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn completed_over(self, fallback: &Self) -> Self {
        let mut colors = fallback.0.clone();
        colors.extend(self.0);
        Self(colors)
    }

    pub(crate) fn is_complete(&self) -> bool {
        ThemeColorRole::ALL
            .iter()
            .all(|role| self.0.contains_key(role))
    }
}
