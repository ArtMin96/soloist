use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::ThemeColor;

macro_rules! soloist_theme_roles {
    ($( $variant:ident => $wire:literal ),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        /// One of Soloist's optional application-specific palette roles.
        pub enum SoloistThemeRole {
            $(#[serde(rename = $wire)] $variant),+
        }

        impl SoloistThemeRole {
            /// Every supported Soloist extension role, in canonical order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
        }
    };
}

soloist_theme_roles! {
    StatusRunning => "statusRunning",
    StatusTransition => "statusTransition",
    StatusStopped => "statusStopped",
    StatusCrashed => "statusCrashed",
    StatusExhausted => "statusExhausted",
    StatusAttention => "statusAttention",
    GitModified => "gitModified",
    GitAdded => "gitAdded",
    GitDeleted => "gitDeleted",
    GitConflicted => "gitConflicted",
    GitIgnored => "gitIgnored",
    GitBranchSynced => "gitBranchSynced",
    GitBranchLocal => "gitBranchLocal",
    FileLanguageAmber => "fileLanguageAmber",
    FileLanguageAzure => "fileLanguageAzure",
    FileLanguageBlue => "fileLanguageBlue",
    FileLanguageCyan => "fileLanguageCyan",
    FileLanguageGreen => "fileLanguageGreen",
    FileLanguageOrange => "fileLanguageOrange",
    FileLanguagePink => "fileLanguagePink",
    FileLanguageRed => "fileLanguageRed",
    FileLanguageViolet => "fileLanguageViolet",
    OverlayScrim => "overlayScrim",
    ShadowInk => "shadowInk",
    TerminalSelectionInactive => "terminalSelectionInactive",
    TerminalScrollbarActive => "terminalScrollbarActive",
    TerminalOverviewRulerBorder => "terminalOverviewRulerBorder",
    TerminalAnsiBlack => "terminalAnsiBlack",
    TerminalAnsiRed => "terminalAnsiRed",
    TerminalAnsiGreen => "terminalAnsiGreen",
    TerminalAnsiYellow => "terminalAnsiYellow",
    TerminalAnsiBlue => "terminalAnsiBlue",
    TerminalAnsiMagenta => "terminalAnsiMagenta",
    TerminalAnsiCyan => "terminalAnsiCyan",
    TerminalAnsiWhite => "terminalAnsiWhite",
    TerminalAnsiBrightBlack => "terminalAnsiBrightBlack",
    TerminalAnsiBrightRed => "terminalAnsiBrightRed",
    TerminalAnsiBrightGreen => "terminalAnsiBrightGreen",
    TerminalAnsiBrightYellow => "terminalAnsiBrightYellow",
    TerminalAnsiBrightBlue => "terminalAnsiBrightBlue",
    TerminalAnsiBrightMagenta => "terminalAnsiBrightMagenta",
    TerminalAnsiBrightCyan => "terminalAnsiBrightCyan",
    TerminalAnsiBrightWhite => "terminalAnsiBrightWhite",
    TerminalSearchMatchBackground => "terminalSearchMatchBackground",
    TerminalSearchMatchBorder => "terminalSearchMatchBorder",
    TerminalSearchMatchOverviewRuler => "terminalSearchMatchOverviewRuler",
    TerminalSearchActiveMatchBackground => "terminalSearchActiveMatchBackground",
    TerminalSearchActiveMatchBorder => "terminalSearchActiveMatchBorder",
    TerminalSearchActiveMatchOverviewRuler => "terminalSearchActiveMatchOverviewRuler",
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
/// Optional explicit values for Soloist-specific roles omitted by the T3 schema.
pub struct SoloistThemeExtensions(BTreeMap<SoloistThemeRole, ThemeColor>);

impl SoloistThemeExtensions {
    /// Returns an explicitly supplied extension color.
    pub fn get(&self, role: SoloistThemeRole) -> Option<&str> {
        self.0.get(&role).map(ThemeColor::as_str)
    }

    /// Returns whether no Soloist extension values were supplied.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
/// Namespaced optional theme extensions preserved in T3-v1 JSON.
pub struct ThemeExtensions {
    /// Soloist-specific application and terminal palette overrides.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soloist: Option<SoloistThemeExtensions>,
}

impl ThemeExtensions {
    /// Returns whether every supported extension namespace is empty.
    pub fn is_empty(&self) -> bool {
        self.soloist
            .as_ref()
            .is_none_or(SoloistThemeExtensions::is_empty)
    }
}
