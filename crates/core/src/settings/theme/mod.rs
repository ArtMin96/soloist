mod colors;
mod extensions;
mod file;

use serde::{de::Error as _, Deserialize, Deserializer, Serialize};
use thiserror::Error;

use super::Appearance;

pub use colors::{ThemeColor, ThemeColorRole, ThemeColors};
pub use extensions::{SoloistThemeExtensions, SoloistThemeRole, ThemeExtensions};
pub use file::{
    built_in_themes, default_theme_colors, soloist_default_theme, ThemeFile,
    ThemeVariantExtensions, ThemeVariants, THEME_FILE_VERSION,
};

/// The stable ID of the paired light/dark Soloist Default theme.
pub const DEFAULT_THEME_ID: &str = "soloist-default";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// A concrete light or dark palette appearance.
pub enum ThemeAppearance {
    /// A light palette.
    Light,
    /// A dark palette.
    Dark,
}

impl ThemeAppearance {
    pub(crate) fn opposite(self) -> Self {
        match self {
            Self::Light => Self::Dark,
            Self::Dark => Self::Light,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
/// Independently selected theme IDs for light and dark appearance.
pub struct SelectedThemes {
    /// Theme selected when the resolved appearance is light.
    pub light: String,
    /// Theme selected when the resolved appearance is dark.
    pub dark: String,
}

impl Default for SelectedThemes {
    fn default() -> Self {
        Self {
            light: DEFAULT_THEME_ID.into(),
            dark: DEFAULT_THEME_ID.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
/// Validated percentage opacity for Soloist's in-app glass surfaces.
pub struct GlassOpacity(u8);

impl GlassOpacity {
    /// Lowest allowed glass opacity percentage.
    pub const MIN: u8 = 40;
    /// Highest allowed glass opacity percentage.
    pub const MAX: u8 = 100;
    /// Allowed increment between opacity values.
    pub const STEP: u8 = 5;

    /// Validates and constructs a glass opacity percentage.
    pub fn new(value: u8) -> Result<Self, ThemeError> {
        if (Self::MIN..=Self::MAX).contains(&value) && value.is_multiple_of(Self::STEP) {
            Ok(Self(value))
        } else {
            Err(ThemeError::InvalidGlassOpacity(value))
        }
    }

    /// Returns the stored percentage.
    pub fn get(self) -> u8 {
        self.0
    }
}

impl Default for GlassOpacity {
    fn default() -> Self {
        Self(80)
    }
}

impl<'de> Deserialize<'de> for GlassOpacity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(u8::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Conflict behavior for importing a theme whose ID already exists.
pub enum ThemeConflictPolicy {
    /// Reject every conflict.
    Reject,
    /// Replace an existing custom theme, never a built-in.
    Replace,
    /// Install under a deterministic unique ID.
    KeepBoth,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "action")]
/// Outcome of installing or updating a custom theme.
pub enum ThemeMutation {
    /// A new theme was installed.
    Installed { id: String },
    /// An existing custom theme was replaced.
    Updated { id: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
/// A theme file or library mutation violated the public theme contract.
pub enum ThemeError {
    /// JSON or a normalized theme file is invalid.
    #[error("invalid theme file: {0}")]
    InvalidFile(String),
    /// The glass opacity is outside the supported stepped range.
    #[error("glass opacity must be 40-100 in steps of 5; got {0}")]
    InvalidGlassOpacity(u8),
    /// A theme ID conflicts with an existing library entry.
    #[error("a theme with id {0:?} already exists")]
    DuplicateId(String),
    /// A mutation attempted to change or remove a built-in theme.
    #[error("built-in theme {0:?} is immutable")]
    BuiltInThemeImmutable(String),
    /// The requested custom theme does not exist.
    #[error("custom theme {0:?} was not found")]
    ThemeNotFound(String),
    /// The selected theme has no palette for the requested appearance.
    #[error("theme {theme_id:?} does not provide a {appearance:?} palette")]
    UnsupportedAppearance {
        theme_id: String,
        appearance: ThemeAppearance,
    },
}

impl Appearance {
    /// Selects an installed theme for one appearance half.
    pub fn select_theme(
        &mut self,
        appearance: ThemeAppearance,
        theme_id: &str,
        built_in_themes: &[ThemeFile],
    ) -> Result<(), ThemeError> {
        let theme_id = theme_id.trim();
        let theme = self
            .custom_themes
            .iter()
            .chain(built_in_themes)
            .find(|theme| theme.id == theme_id)
            .ok_or_else(|| ThemeError::ThemeNotFound(theme_id.into()))?;
        if !theme.supports(appearance) {
            return Err(ThemeError::UnsupportedAppearance {
                theme_id: theme_id.into(),
                appearance,
            });
        }
        match appearance {
            ThemeAppearance::Light => self.selected_themes.light = theme_id.into(),
            ThemeAppearance::Dark => self.selected_themes.dark = theme_id.into(),
        }
        Ok(())
    }

    /// Installs a validated custom theme according to a conflict policy.
    pub fn install_custom_theme(
        &mut self,
        mut theme: ThemeFile,
        conflict: ThemeConflictPolicy,
        built_in_ids: &[&str],
    ) -> Result<ThemeMutation, ThemeError> {
        theme.validate()?;
        let built_in_conflict = is_built_in_id(&theme.id, built_in_ids);
        let custom_index = self
            .custom_themes
            .iter()
            .position(|existing| existing.id == theme.id);

        if built_in_conflict || custom_index.is_some() {
            match conflict {
                ThemeConflictPolicy::Reject if built_in_conflict => {
                    return Err(ThemeError::BuiltInThemeImmutable(theme.id));
                }
                ThemeConflictPolicy::Reject => return Err(ThemeError::DuplicateId(theme.id)),
                ThemeConflictPolicy::Replace if built_in_conflict => {
                    return Err(ThemeError::BuiltInThemeImmutable(theme.id));
                }
                ThemeConflictPolicy::Replace => {
                    let Some(index) = custom_index else {
                        return Err(ThemeError::DuplicateId(theme.id));
                    };
                    let id = theme.id.clone();
                    self.custom_themes[index] = theme;
                    return Ok(ThemeMutation::Updated { id });
                }
                ThemeConflictPolicy::KeepBoth => {
                    theme.id = unique_numbered_id(&theme.id, built_in_ids, &self.custom_themes);
                }
            }
        }

        let id = theme.id.clone();
        self.custom_themes.push(theme);
        Ok(ThemeMutation::Installed { id })
    }

    /// Replaces one existing custom theme.
    pub fn update_custom_theme(
        &mut self,
        theme: ThemeFile,
        built_in_ids: &[&str],
    ) -> Result<ThemeMutation, ThemeError> {
        theme.validate()?;
        if is_built_in_id(&theme.id, built_in_ids) {
            return Err(ThemeError::BuiltInThemeImmutable(theme.id));
        }
        let Some(index) = self
            .custom_themes
            .iter()
            .position(|existing| existing.id == theme.id)
        else {
            return Err(ThemeError::ThemeNotFound(theme.id));
        };
        let id = theme.id.clone();
        self.custom_themes[index] = theme;
        Ok(ThemeMutation::Updated { id })
    }

    /// Copies a built-in or custom theme under a deterministic unique ID.
    pub fn duplicate_theme(
        &mut self,
        theme_id: &str,
        built_in_themes: &[ThemeFile],
    ) -> Result<String, ThemeError> {
        let built_in_ids = built_in_themes
            .iter()
            .map(|theme| theme.id.as_str())
            .collect::<Vec<_>>();
        let Some(source) = self
            .custom_themes
            .iter()
            .chain(built_in_themes)
            .find(|theme| theme.id == theme_id)
            .cloned()
        else {
            return Err(ThemeError::ThemeNotFound(theme_id.into()));
        };

        let mut copy = source;
        let copy_base = id_with_suffix(&copy.id, "-copy");
        copy.id = if id_is_available(&copy_base, &built_in_ids, &self.custom_themes) {
            copy_base
        } else {
            unique_numbered_id(&copy_base, &built_in_ids, &self.custom_themes)
        };
        copy.name = copy_name(&copy.name);
        copy.validate()?;
        let id = copy.id.clone();
        self.custom_themes.push(copy);
        Ok(id)
    }

    /// Removes a custom theme and resets any affected selection to Soloist Default.
    pub fn remove_custom_theme(
        &mut self,
        theme_id: &str,
        built_in_ids: &[&str],
    ) -> Result<bool, ThemeError> {
        if is_built_in_id(theme_id, built_in_ids) {
            return Err(ThemeError::BuiltInThemeImmutable(theme_id.into()));
        }
        let Some(index) = self
            .custom_themes
            .iter()
            .position(|theme| theme.id == theme_id)
        else {
            return Ok(false);
        };
        self.custom_themes.remove(index);
        if self.selected_themes.light == theme_id {
            self.selected_themes.light = DEFAULT_THEME_ID.into();
        }
        if self.selected_themes.dark == theme_id {
            self.selected_themes.dark = DEFAULT_THEME_ID.into();
        }
        Ok(true)
    }
}

fn unique_numbered_id(base: &str, built_in_ids: &[&str], themes: &[ThemeFile]) -> String {
    for number in 2_u64.. {
        let candidate = id_with_suffix(base, &format!("-{number}"));
        if id_is_available(&candidate, built_in_ids, themes) {
            return candidate;
        }
    }
    unreachable!("the unbounded numeric suffix space cannot be exhausted")
}

fn id_is_available(candidate: &str, built_in_ids: &[&str], themes: &[ThemeFile]) -> bool {
    !is_built_in_id(candidate, built_in_ids) && !themes.iter().any(|theme| theme.id == candidate)
}

fn is_built_in_id(theme_id: &str, built_in_ids: &[&str]) -> bool {
    theme_id == DEFAULT_THEME_ID || built_in_ids.contains(&theme_id)
}

fn id_with_suffix(base: &str, suffix: &str) -> String {
    let available = 48_usize.saturating_sub(suffix.len());
    let base = base.get(..base.len().min(available)).unwrap_or(base);
    format!("{}{suffix}", base.trim_end_matches('-'))
}

fn copy_name(name: &str) -> String {
    const SUFFIX: &str = " copy";
    let mut copy = name
        .chars()
        .take(48 - SUFFIX.chars().count())
        .collect::<String>();
    copy.push_str(SUFFIX);
    copy
}

#[cfg(test)]
#[path = "theme_tests.rs"]
mod tests;
