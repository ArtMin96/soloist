use std::sync::OnceLock;

use serde::{de::Error as _, Deserialize, Deserializer, Serialize};

use super::{ThemeAppearance, ThemeColors, ThemeError, ThemeExtensions, DEFAULT_THEME_ID};

/// The supported T3 theme file format version.
pub const THEME_FILE_VERSION: u8 = 1;

const BUILT_IN_CATALOG: &str = include_str!("../../../../../themes/builtins/catalog.json");
const RESERVED_PREFERENCE_IDS: &[&str] = &["system", "light", "dark"];

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
/// Optional palette for the appearance opposite the theme's base palette.
pub struct ThemeVariants {
    /// Optional light palette.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub light: Option<ThemeColors>,
    /// Optional dark palette.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dark: Option<ThemeColors>,
}

impl ThemeVariants {
    /// Returns the palette for a concrete appearance when present.
    pub fn get(&self, appearance: ThemeAppearance) -> Option<&ThemeColors> {
        match appearance {
            ThemeAppearance::Light => self.light.as_ref(),
            ThemeAppearance::Dark => self.dark.as_ref(),
        }
    }

    fn take(&mut self, appearance: ThemeAppearance) -> Option<ThemeColors> {
        match appearance {
            ThemeAppearance::Light => self.light.take(),
            ThemeAppearance::Dark => self.dark.take(),
        }
    }

    fn set(&mut self, appearance: ThemeAppearance, colors: ThemeColors) {
        match appearance {
            ThemeAppearance::Light => self.light = Some(colors),
            ThemeAppearance::Dark => self.dark = Some(colors),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
/// A validated and normalized T3-v1 theme file plus optional Soloist extensions.
pub struct ThemeFile {
    /// Theme schema version.
    pub version: u8,
    /// Stable lowercase theme identifier.
    pub id: String,
    /// User-facing theme name.
    pub name: String,
    /// Appearance of the base palette.
    pub appearance: ThemeAppearance,
    /// Optional theme author.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Complete base palette.
    pub colors: ThemeColors,
    /// Optional opposite-appearance palette.
    #[serde(default, skip_serializing_if = "variants_are_empty")]
    pub variants: ThemeVariants,
    #[serde(default, skip_serializing_if = "ThemeExtensions::is_empty")]
    /// Optional application-specific extension colors.
    pub extensions: ThemeExtensions,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    /// Whether the theme enables compatible sidebar artwork.
    pub sidebar_artwork: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    /// Whether the theme is externally managed.
    pub managed: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawThemeFile {
    version: u8,
    #[serde(default)]
    id: Option<String>,
    name: String,
    appearance: ThemeAppearance,
    #[serde(default)]
    author: Option<String>,
    colors: ThemeColors,
    #[serde(default)]
    variants: ThemeVariants,
    #[serde(default)]
    extensions: ThemeExtensions,
    #[serde(default)]
    sidebar_artwork: bool,
    #[serde(default)]
    managed: bool,
}

#[derive(Clone, Copy)]
enum PaletteMode {
    CompleteSparse,
    RequireComplete,
}

impl ThemeFile {
    /// Parses, validates, normalizes, and completes sparse T3-v1 JSON.
    pub fn from_json(json: &str) -> Result<Self, ThemeError> {
        serde_json::from_str(json).map_err(|error| ThemeError::InvalidFile(error.to_string()))
    }

    /// Serializes normalized T3-v1 JSON with a trailing newline.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self).map(|mut json| {
            json.push('\n');
            json
        })
    }

    /// Returns the palette used for a concrete appearance.
    pub fn colors_for(&self, appearance: ThemeAppearance) -> Option<&ThemeColors> {
        if appearance == self.appearance {
            Some(&self.colors)
        } else {
            self.variants.get(appearance)
        }
    }

    /// Returns whether this theme supplies the requested appearance.
    pub fn supports(&self, appearance: ThemeAppearance) -> bool {
        self.colors_for(appearance).is_some()
    }

    pub(crate) fn validate(&self) -> Result<(), ThemeError> {
        if self.version != THEME_FILE_VERSION {
            return Err(ThemeError::InvalidFile(format!(
                "unsupported theme version {}; expected {THEME_FILE_VERSION}",
                self.version
            )));
        }
        validate_file_id(&self.id)?;
        validate_name(&self.name)?;
        if let Some(author) = &self.author {
            validate_author(author)?;
        }
        if !self.colors.is_complete() {
            return Err(ThemeError::InvalidFile(
                "stored theme colors must contain every supported role".into(),
            ));
        }
        if self.variants.get(self.appearance).is_some() {
            return Err(ThemeError::InvalidFile(format!(
                "theme variants must not repeat the base appearance {:?}",
                self.appearance
            )));
        }
        for appearance in [ThemeAppearance::Light, ThemeAppearance::Dark] {
            if self
                .variants
                .get(appearance)
                .is_some_and(|colors| !colors.is_complete())
            {
                return Err(ThemeError::InvalidFile(
                    "stored theme variants must contain every supported role".into(),
                ));
            }
        }
        Ok(())
    }

    fn from_raw(mut raw: RawThemeFile, mode: PaletteMode) -> Result<Self, ThemeError> {
        if raw.version != THEME_FILE_VERSION {
            return Err(ThemeError::InvalidFile(format!(
                "unsupported theme version {}; expected {THEME_FILE_VERSION}",
                raw.version
            )));
        }
        let name = raw.name.trim().to_owned();
        validate_name(&name)?;
        let id = raw
            .id
            .map(|id| id.trim().to_owned())
            .unwrap_or_else(|| theme_id_from_name(&name));
        validate_file_id(&id)?;
        let author = raw.author.map(|author| author.trim().to_owned());
        if let Some(author) = &author {
            validate_author(author)?;
        }
        if raw.colors.is_empty() {
            return Err(ThemeError::InvalidFile(
                "theme colors must contain at least one supported role".into(),
            ));
        }
        let colors = resolve_colors(raw.colors, raw.appearance, mode)?;

        if raw.variants.get(raw.appearance).is_some() {
            return Err(ThemeError::InvalidFile(format!(
                "theme variants must not repeat the base appearance {:?}",
                raw.appearance
            )));
        }
        let opposite = raw.appearance.opposite();
        if let Some(variant) = raw.variants.take(opposite) {
            if variant.is_empty() {
                return Err(ThemeError::InvalidFile(
                    "theme variants must contain at least one supported role".into(),
                ));
            }
            raw.variants
                .set(opposite, resolve_colors(variant, opposite, mode)?);
        }

        Ok(Self {
            version: THEME_FILE_VERSION,
            id,
            name,
            appearance: raw.appearance,
            author,
            colors,
            variants: raw.variants,
            extensions: raw.extensions,
            sidebar_artwork: raw.sidebar_artwork,
            managed: raw.managed,
        })
    }
}

impl<'de> Deserialize<'de> for ThemeFile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::from_raw(
            RawThemeFile::deserialize(deserializer)?,
            PaletteMode::CompleteSparse,
        )
        .map_err(D::Error::custom)
    }
}

/// Returns the Soloist Default palette used to complete sparse imports.
pub fn default_theme_colors(
    appearance: ThemeAppearance,
) -> Result<&'static ThemeColors, ThemeError> {
    let theme = soloist_default_theme()?;
    theme.colors_for(appearance).ok_or_else(|| {
        ThemeError::InvalidFile(format!(
            "the Soloist default asset is missing its {appearance:?} palette"
        ))
    })
}

/// Returns the paired Soloist Default built-in theme.
pub fn soloist_default_theme() -> Result<&'static ThemeFile, ThemeError> {
    built_in_themes()?
        .iter()
        .find(|theme| theme.id == DEFAULT_THEME_ID)
        .ok_or_else(|| {
            ThemeError::InvalidFile("the built-in catalog is missing Soloist Default".into())
        })
}

/// Parses and validates the shared built-in catalog once, preserving catalog order.
pub fn built_in_themes() -> Result<&'static [ThemeFile], ThemeError> {
    static THEMES: OnceLock<Result<Vec<ThemeFile>, ThemeError>> = OnceLock::new();
    match THEMES.get_or_init(|| {
        let raw: Vec<RawThemeFile> = serde_json::from_str(BUILT_IN_CATALOG)
            .map_err(|error| ThemeError::InvalidFile(error.to_string()))?;
        let themes = raw
            .into_iter()
            .map(|theme| ThemeFile::from_raw(theme, PaletteMode::RequireComplete))
            .collect::<Result<Vec<_>, _>>()?;
        let mut ids = std::collections::BTreeSet::new();
        if themes.iter().any(|theme| !ids.insert(theme.id.clone())) {
            return Err(ThemeError::InvalidFile(
                "built-in theme ids must be unique".into(),
            ));
        }
        let default = themes
            .iter()
            .find(|theme| theme.id == DEFAULT_THEME_ID)
            .ok_or_else(|| {
                ThemeError::InvalidFile("the built-in catalog is missing Soloist Default".into())
            })?;
        if !default.supports(ThemeAppearance::Light) || !default.supports(ThemeAppearance::Dark) {
            return Err(ThemeError::InvalidFile(
                "Soloist Default must contain both appearances".into(),
            ));
        }
        Ok(themes)
    }) {
        Ok(themes) => Ok(themes),
        Err(error) => Err(error.clone()),
    }
}

fn resolve_colors(
    colors: ThemeColors,
    appearance: ThemeAppearance,
    mode: PaletteMode,
) -> Result<ThemeColors, ThemeError> {
    match mode {
        PaletteMode::CompleteSparse => Ok(colors.completed_over(default_theme_colors(appearance)?)),
        PaletteMode::RequireComplete if colors.is_complete() => Ok(colors),
        PaletteMode::RequireComplete => Err(ThemeError::InvalidFile(format!(
            "the Soloist default {appearance:?} palette must contain every supported role"
        ))),
    }
}

fn validate_id(id: &str) -> Result<(), ThemeError> {
    let mut bytes = id.bytes();
    let first_is_valid = bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit());
    if id.len() > 48
        || !first_is_valid
        || !bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(ThemeError::InvalidFile(
            "theme ids must be 1-48 lowercase letters, numbers, or hyphens and start with a letter or number"
                .into(),
        ));
    }
    Ok(())
}

fn validate_file_id(id: &str) -> Result<(), ThemeError> {
    validate_id(id)?;
    if RESERVED_PREFERENCE_IDS.contains(&id) {
        return Err(ThemeError::InvalidFile(format!(
            "theme id {id:?} is reserved"
        )));
    }
    Ok(())
}

fn validate_name(name: &str) -> Result<(), ThemeError> {
    if name.is_empty() || name != name.trim() || name.chars().count() > 48 {
        return Err(ThemeError::InvalidFile(
            "theme names must contain 1-48 non-whitespace characters".into(),
        ));
    }
    Ok(())
}

fn validate_author(author: &str) -> Result<(), ThemeError> {
    if author.is_empty() || author != author.trim() || author.chars().count() > 48 {
        return Err(ThemeError::InvalidFile(
            "theme authors must contain 1-48 non-whitespace characters when provided".into(),
        ));
    }
    Ok(())
}

fn theme_id_from_name(name: &str) -> String {
    let mut id = String::new();
    let mut separator_pending = false;
    for character in name.trim().chars().flat_map(char::to_lowercase) {
        if character.is_ascii_lowercase() || character.is_ascii_digit() {
            if separator_pending && !id.is_empty() && id.len() < 48 {
                id.push('-');
            }
            separator_pending = false;
            if id.len() < 48 {
                id.push(character);
            }
        } else if !id.is_empty() {
            separator_pending = true;
        }
        if id.len() == 48 {
            break;
        }
    }
    while id.ends_with('-') {
        id.pop();
    }
    if id.is_empty() {
        "custom-theme".into()
    } else {
        id
    }
}

fn variants_are_empty(variants: &ThemeVariants) -> bool {
    variants.light.is_none() && variants.dark.is_none()
}
