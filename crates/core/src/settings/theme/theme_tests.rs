use super::file::MAX_DESCRIPTION_CHARS;
use crate::settings::Settings;
use crate::settings::{
    built_in_themes, default_theme_colors, soloist_default_theme, Appearance, GlassOpacity,
    SoloistThemeRole, ThemeAppearance, ThemeColor, ThemeColorRole, ThemeColors,
    ThemeConflictPolicy, ThemeError, ThemeFile, ThemeMutation, DEFAULT_THEME_ID,
};

const BUILT_INS: &[&str] = &[DEFAULT_THEME_ID, "poimandres-dark-theme"];

fn git_modified(theme: &ThemeFile, appearance: ThemeAppearance) -> Option<&str> {
    theme
        .extensions_for(appearance)
        .soloist
        .as_ref()?
        .get(SoloistThemeRole::GitModified)
}

fn theme(id: &str, name: &str, appearance: ThemeAppearance, accent: &str) -> ThemeFile {
    ThemeFile::from_json(&format!(
        r##"{{"version":1,"id":"{id}","name":"{name}","appearance":"{}","colors":{{"accent":"{accent}"}}}}"##,
        match appearance {
            ThemeAppearance::Light => "light",
            ThemeAppearance::Dark => "dark",
        }
    ))
    .expect("valid test theme")
}

#[test]
fn appearance_defaults_are_backward_compatible_and_choose_the_paired_default_theme() {
    let stored: Settings = serde_json::from_str(r#"{"appearance":{"theme":"dark"}}"#)
        .expect("an old durable settings document reads");
    let old = stored.appearance;

    assert_eq!(
        old,
        Appearance {
            theme: crate::settings::Theme::Dark,
            ..Appearance::default()
        }
    );
    assert_eq!(old.selected_themes.light, DEFAULT_THEME_ID);
    assert_eq!(old.selected_themes.dark, DEFAULT_THEME_ID);
    assert_eq!(old.glass_opacity.get(), 80);
    assert!(old.custom_themes.is_empty());

    let written = serde_json::to_value(old).expect("serialize appearance");
    assert_eq!(
        written["theme"], "dark",
        "the legacy selector stays on the wire"
    );
    assert_eq!(written["selected_themes"]["light"], DEFAULT_THEME_ID);
}

#[test]
fn glass_opacity_accepts_only_the_documented_steps() {
    for value in [40, 45, 80, 95, 100] {
        let opacity: GlassOpacity = serde_json::from_str(&value.to_string()).expect("valid step");
        assert_eq!(opacity.get(), value);
    }

    for value in [0, 39, 41, 99, 101, 255] {
        assert!(
            serde_json::from_str::<GlassOpacity>(&value.to_string()).is_err(),
            "{value} must be rejected"
        );
    }
}

#[test]
fn the_supplied_poimandres_file_is_a_real_complete_theme_fixture() {
    let parsed = built_in_themes()
        .expect("shared catalog parses")
        .iter()
        .find(|theme| theme.id == "poimandres-dark-theme")
        .expect("Poimandres is built in");

    assert_eq!(parsed.version, 1);
    assert_eq!(parsed.id, "poimandres-dark-theme");
    assert_eq!(parsed.name, "Poimandres dark theme");
    assert_eq!(parsed.appearance, ThemeAppearance::Dark);
    assert_eq!(parsed.author.as_deref(), Some("sbansal1999"));
    assert_eq!(parsed.colors.len(), ThemeColorRole::ALL.len());
    assert_eq!(parsed.colors.get(ThemeColorRole::Canvas), Some("#1b1e28"));
    assert_eq!(parsed.colors.get(ThemeColorRole::Error), Some("#d0679d"));
    assert_eq!(
        parsed.colors.get(ThemeColorRole::TerminalCursor),
        Some("#a6accd")
    );

    let copied = parsed.to_json().expect("theme exports");
    assert!(
        serde_json::from_str::<serde_json::Value>(&copied)
            .expect("exported JSON")
            .get("extensions")
            .is_none(),
        "a plain T3 theme exports without a Soloist-only key"
    );
    let reparsed = ThemeFile::from_json(&copied).expect("export imports");
    assert_eq!(&reparsed, parsed);
}

#[test]
fn explicit_ids_and_every_supported_hex_width_are_normalized() {
    let parsed = ThemeFile::from_json(
        r##"{
          "version": 1,
          "id": "  normalized-id  ",
          "name": "Normalized",
          "appearance": "dark",
          "colors": {
            "canvas": "#ABC",
            "accent": "#ABCD",
            "text": "#ABCDEF",
            "border": "#ABCDEF80"
          }
        }"##,
    )
    .expect("normalizable theme parses");

    assert_eq!(parsed.id, "normalized-id");
    assert_eq!(parsed.colors.get(ThemeColorRole::Canvas), Some("#aabbcc"));
    assert_eq!(parsed.colors.get(ThemeColorRole::Accent), Some("#aabbccdd"));
    assert_eq!(parsed.colors.get(ThemeColorRole::Text), Some("#abcdef"));
    assert_eq!(parsed.colors.get(ThemeColorRole::Border), Some("#abcdef80"));
    assert_eq!(ThemeColor::parse("#F0A").unwrap().as_str(), "#ff00aa");
}

#[test]
fn soloist_extensions_are_sparse_strict_and_normalized_without_changing_t3_base_roles() {
    let parsed = ThemeFile::from_json(
        r##"{
          "version": 1,
          "id": "extended",
          "name": "Extended",
          "appearance": "dark",
          "colors": { "canvas": "#000" },
          "extensions": {
            "soloist": {
              "statusRunning": "#0F0",
              "terminalAnsiBrightWhite": "#FFFFFFFF",
              "terminalSearchActiveMatchBorder": "#ABC8"
            }
          }
        }"##,
    )
    .expect("known extensions parse");

    let soloist = parsed
        .extensions
        .soloist
        .as_ref()
        .expect("Soloist extension");
    assert_eq!(SoloistThemeRole::ALL.len(), 49);
    assert_eq!(
        soloist.get(SoloistThemeRole::StatusRunning),
        Some("#00ff00")
    );
    assert_eq!(
        soloist.get(SoloistThemeRole::TerminalAnsiBrightWhite),
        Some("#ffffffff")
    );
    assert_eq!(
        soloist.get(SoloistThemeRole::TerminalSearchActiveMatchBorder),
        Some("#aabbcc88")
    );

    for invalid in [
        r##"{"version":1,"id":"bad","name":"Bad","appearance":"dark","colors":{"canvas":"#000"},"extensions":{"soloist":{"unknownRole":"#fff"}}}"##,
        r##"{"version":1,"id":"bad","name":"Bad","appearance":"dark","colors":{"canvas":"#000"},"extensions":{"soloist":{"statusRunning":"green"}}}"##,
        r##"{"version":1,"id":"bad","name":"Bad","appearance":"dark","colors":{"canvas":"#000"},"extensions":{"someApp":{}}}"##,
    ] {
        assert!(
            ThemeFile::from_json(invalid).is_err(),
            "must reject: {invalid}"
        );
    }
}

#[test]
fn an_optional_description_is_trimmed_kept_and_exported() {
    let parsed = ThemeFile::from_json(
        r##"{
          "version": 1,
          "id": "described",
          "name": "Described",
          "appearance": "light",
          "author": "saltjsx",
          "description": "  Apple inspired theme  ",
          "colors": { "canvas": "#fff" }
        }"##,
    )
    .expect("a described theme parses");

    assert_eq!(parsed.description.as_deref(), Some("Apple inspired theme"));

    let exported = parsed.to_json().expect("theme exports");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&exported).expect("exported JSON")["description"],
        "Apple inspired theme",
        "an exported theme keeps its description"
    );
    assert_eq!(
        ThemeFile::from_json(&exported).expect("export imports"),
        parsed
    );

    let prose = "A calm, low-contrast light palette inspired by frosted glass, tuned for long \
                 sessions in bright rooms without washing out terminal output.";
    assert_eq!(
        ThemeFile::from_json(&format!(
            r##"{{"version":1,"id":"prose","name":"Prose","appearance":"light","description":{},"colors":{{"canvas":"#fff"}}}}"##,
            serde_json::to_string(prose).expect("quote prose")
        ))
        .expect("a sentence-length description is not a naming label")
        .description
        .as_deref(),
        Some(prose)
    );

    for invalid in [
        r##"{"version":1,"id":"x","name":"X","appearance":"dark","colors":{"canvas":"#000"},"description":"   "}"##,
        &format!(
            r##"{{"version":1,"id":"x","name":"X","appearance":"dark","colors":{{"canvas":"#000"}},"description":"{}"}}"##,
            "d".repeat(MAX_DESCRIPTION_CHARS + 1)
        ),
    ] {
        assert!(
            ThemeFile::from_json(invalid).is_err(),
            "must reject: {invalid}"
        );
    }
}

/// A complete light-base T3 file carrying `author`, `description`, and a full opposite-appearance
/// palette — the shape a theme shared between T3-compatible apps actually has.
const DESCRIBED_COMPLETE_THEME: &str = r##"
    {
      "version": 1,
      "id": "apple-frost",
      "name": "Apple Frost",
      "appearance": "light",
      "author": "saltjsx",
      "description": "Apple inspired theme",
      "colors": {
        "canvas": "#f5f5f7",
        "chrome": "#ffffff",
        "toolbar": "#ffffff",
        "toolbarForeground": "#1d1d1f",
        "toolbarBorder": "#d2d2d7",
        "toolbarControl": "#e2e2e5",
        "toolbarControlForeground": "#1d1d1f",
        "toolbarControlHover": "#d7d7dc",
        "surface": "#ffffff",
        "surfaceRaised": "#f4f8fb",
        "surfaceOverlay": "#ffffff",
        "text": "#1d1d1f",
        "textMuted": "#6e6e73",
        "border": "#d2d2d7",
        "input": "#ffffff",
        "focus": "#0071e3",
        "accent": "#0071e3",
        "accentForeground": "#ffffff",
        "secondary": "#e2e2e5",
        "secondaryForeground": "#1d1d1f",
        "muted": "#ececef",
        "mutedForeground": "#5c5c61",
        "placeholder": "#858585",
        "secondaryLabel": "#707070",
        "iconMuted": "#707070",
        "error": "#d70015",
        "errorForeground": "#8c000e",
        "errorSurface": "#fdeced",
        "warning": "#b25000",
        "warningForeground": "#7a3600",
        "warningSurface": "#fdf1e5",
        "update": "#0066cc",
        "updateForeground": "#004f9e",
        "updateSurface": "#e8f1fb",
        "accentSurface": "#e8f2fd",
        "accentSurfaceForeground": "#0055a6",
        "messageSurface": "#ffffff",
        "messageForeground": "#1d1d1f",
        "messageAction": "#f0f0f3",
        "messageActionForeground": "#1d1d1f",
        "messageActionHover": "#e2e2e5",
        "codeBackground": "#1d1d1f",
        "codeForeground": "#f4f8fb",
        "sidebar": "#f0f0f3",
        "sidebarForeground": "#1d1d1f",
        "sidebarMutedForeground": "#6a6a6f",
        "sidebarControlSurface": "#ffffff",
        "sidebarRowHover": "#e6e6ea",
        "sidebarRowActive": "#dededf",
        "sidebarRowSelected": "#d9e8f8",
        "sidebarBorder": "#d2d2d7",
        "terminalBackground": "#1d1d1f",
        "terminalForeground": "#f4f8fb",
        "terminalCursor": "#2997ff",
        "terminalSelection": "#33507a",
        "terminalScrollbar": "#3a3a3c",
        "terminalScrollbarHover": "#4f4f52"
      },
      "variants": {
        "dark": {
          "canvas": "#000000",
          "chrome": "#0a0a0b",
          "toolbar": "#0a0a0b",
          "toolbarForeground": "#f5f5f7",
          "toolbarBorder": "#333333",
          "toolbarControl": "#2c2c2e",
          "toolbarControlForeground": "#f5f5f7",
          "toolbarControlHover": "#3a3a3c",
          "surface": "#1d1d1f",
          "surfaceRaised": "#252527",
          "surfaceOverlay": "#2c2c2e",
          "text": "#f5f5f7",
          "textMuted": "#8e8e93",
          "border": "#333333",
          "input": "#1d1d1f",
          "focus": "#0a84ff",
          "accent": "#0a84ff",
          "accentForeground": "#ffffff",
          "secondary": "#2c2c2e",
          "secondaryForeground": "#f5f5f7",
          "muted": "#252527",
          "mutedForeground": "#a1a1a6",
          "placeholder": "#6e6e73",
          "secondaryLabel": "#98989d",
          "iconMuted": "#8e8e93",
          "error": "#ff453a",
          "errorForeground": "#ffb4ae",
          "errorSurface": "#3a1614",
          "warning": "#ff9f0a",
          "warningForeground": "#ffcf87",
          "warningSurface": "#3a2708",
          "update": "#2997ff",
          "updateForeground": "#a8d4ff",
          "updateSurface": "#12263a",
          "accentSurface": "#12283d",
          "accentSurfaceForeground": "#9fd0ff",
          "messageSurface": "#1d1d1f",
          "messageForeground": "#f5f5f7",
          "messageAction": "#2c2c2e",
          "messageActionForeground": "#f5f5f7",
          "messageActionHover": "#3a3a3c",
          "codeBackground": "#000000",
          "codeForeground": "#f4f8fb",
          "sidebar": "#0a0a0b",
          "sidebarForeground": "#f5f5f7",
          "sidebarMutedForeground": "#98989d",
          "sidebarControlSurface": "#1d1d1f",
          "sidebarRowHover": "#1d1d1f",
          "sidebarRowActive": "#252527",
          "sidebarRowSelected": "#12283d",
          "sidebarBorder": "#2a2a2c",
          "terminalBackground": "#000000",
          "terminalForeground": "#f4f8fb",
          "terminalCursor": "#2997ff",
          "terminalSelection": "#1f3c5c",
          "terminalScrollbar": "#3a3a3c",
          "terminalScrollbarHover": "#4f4f52"
        }
      }
    }
"##;

#[test]
fn a_complete_described_t3_file_imports_with_its_supplied_id_and_survives_export() {
    let parsed = ThemeFile::from_json(DESCRIBED_COMPLETE_THEME)
        .expect("a complete described T3 file imports");

    assert_eq!(
        parsed.id, "apple-frost",
        "a supplied id is kept rather than regenerated from the name"
    );
    assert_eq!(parsed.name, "Apple Frost");
    assert_eq!(parsed.appearance, ThemeAppearance::Light);
    assert_eq!(parsed.author.as_deref(), Some("saltjsx"));
    assert_eq!(parsed.description.as_deref(), Some("Apple inspired theme"));
    assert_eq!(parsed.colors.get(ThemeColorRole::Accent), Some("#0071e3"));
    assert_eq!(parsed.colors.len(), ThemeColorRole::ALL.len());
    assert_eq!(
        parsed
            .colors_for(ThemeAppearance::Dark)
            .map(ThemeColors::len),
        Some(ThemeColorRole::ALL.len()),
        "the supplied dark variant keeps every role"
    );

    let exported = parsed.to_json().expect("theme exports");
    let written = serde_json::from_str::<serde_json::Value>(&exported).expect("exported JSON");
    assert_eq!(written["id"], "apple-frost", "an id survives export");
    assert_eq!(
        written["description"], "Apple inspired theme",
        "a description survives export"
    );
    assert_eq!(
        written["colors"].as_object().map(serde_json::Map::len),
        Some(ThemeColorRole::ALL.len())
    );
    assert_eq!(
        written["variants"]["dark"]
            .as_object()
            .map(serde_json::Map::len),
        Some(ThemeColorRole::ALL.len()),
        "the exported dark variant still carries every role"
    );

    let reparsed = ThemeFile::from_json(&exported).expect("export imports");
    assert_eq!(reparsed.id, "apple-frost");
    assert_eq!(
        reparsed.description.as_deref(),
        Some("Apple inspired theme")
    );
    assert_eq!(reparsed.colors.len(), ThemeColorRole::ALL.len());
    assert_eq!(
        reparsed
            .colors_for(ThemeAppearance::Dark)
            .map(ThemeColors::len),
        Some(ThemeColorRole::ALL.len())
    );
    assert_eq!(reparsed, parsed, "an export loses nothing it imported");
}

#[test]
fn every_shared_builtin_satisfies_the_authoritative_theme_contract() {
    let built_ins = built_in_themes().expect("shared catalog parses");
    assert_eq!(built_ins.len(), 6);
    assert!(built_ins.iter().any(|theme| {
        theme.id == "poimandres-dark-theme" && theme.name == "Poimandres dark theme"
    }));
    assert!(built_ins
        .iter()
        .all(|theme| theme.colors.len() == ThemeColorRole::ALL.len()));
}

#[test]
fn sparse_base_and_opposite_variant_palettes_complete_from_the_matching_defaults() {
    let parsed = ThemeFile::from_json(
        r##"{
          "version": 1,
          "id": "paired",
          "name": "Paired",
          "appearance": "dark",
          "colors": { "accent": "#abc" },
          "variants": { "light": { "canvas": "#fff" } }
        }"##,
    )
    .expect("sparse paired theme parses");

    assert_eq!(parsed.colors.get(ThemeColorRole::Accent), Some("#aabbcc"));
    assert_eq!(
        parsed.colors.get(ThemeColorRole::Canvas),
        default_theme_colors(ThemeAppearance::Dark)
            .expect("default dark palette")
            .get(ThemeColorRole::Canvas)
    );
    let light = parsed
        .colors_for(ThemeAppearance::Light)
        .expect("opposite variant exists");
    assert_eq!(light.get(ThemeColorRole::Canvas), Some("#ffffff"));
    assert_eq!(
        light.get(ThemeColorRole::Text),
        default_theme_colors(ThemeAppearance::Light)
            .expect("default light palette")
            .get(ThemeColorRole::Text)
    );
}

#[test]
fn each_appearance_resolves_the_extension_colors_authored_for_its_own_palette() {
    let parsed = ThemeFile::from_json(
        r##"{
          "version": 1,
          "id": "paired-extended",
          "name": "Paired Extended",
          "appearance": "light",
          "colors": { "canvas": "#fff" },
          "extensions": { "soloist": { "gitModified": "#b06c00" } },
          "variants": {
            "dark": { "canvas": "#101010" },
            "extensions": { "dark": { "soloist": { "gitModified": "#EBA941" } } }
          }
        }"##,
    )
    .expect("a paired theme with per-appearance extensions parses");

    assert_eq!(
        git_modified(&parsed, ThemeAppearance::Light),
        Some("#b06c00"),
        "the base appearance resolves the theme-level set"
    );
    assert_eq!(
        git_modified(&parsed, ThemeAppearance::Dark),
        Some("#eba941"),
        "the opposite appearance resolves its own set, normalized like any theme color"
    );

    let exported = parsed.to_json().expect("theme exports");
    let reparsed = ThemeFile::from_json(&exported).expect("export imports");
    assert_eq!(reparsed, parsed, "an export loses nothing it imported");
}

#[test]
fn a_theme_level_extension_set_alone_still_applies_to_both_appearances() {
    let parsed = ThemeFile::from_json(
        r##"{
          "version": 1,
          "id": "paired-shared",
          "name": "Paired Shared",
          "appearance": "light",
          "colors": { "canvas": "#fff" },
          "extensions": { "soloist": { "gitModified": "#b06c00" } },
          "variants": { "dark": { "canvas": "#101010" } }
        }"##,
    )
    .expect("a paired theme without per-appearance extensions parses");

    assert_eq!(
        git_modified(&parsed, ThemeAppearance::Light),
        Some("#b06c00")
    );
    assert_eq!(
        git_modified(&parsed, ThemeAppearance::Dark),
        Some("#b06c00"),
        "a theme written before appearances could carry their own extensions is unchanged"
    );
}

#[test]
fn a_supplied_but_empty_per_appearance_set_replaces_the_theme_level_extensions() {
    let parsed = ThemeFile::from_json(
        r##"{
          "version": 1,
          "id": "derived-dark",
          "name": "Derived Dark",
          "appearance": "light",
          "colors": { "canvas": "#fff" },
          "extensions": { "soloist": { "gitModified": "#b06c00" } },
          "variants": { "dark": { "canvas": "#101010" }, "extensions": { "dark": {} } }
        }"##,
    )
    .expect("an empty dark extension set parses");

    assert_eq!(
        git_modified(&parsed, ThemeAppearance::Light),
        Some("#b06c00")
    );
    assert!(
        parsed.extensions_for(ThemeAppearance::Dark).is_empty(),
        "an empty set asks for derivation instead of inheriting light-authored hex"
    );

    let reparsed =
        ThemeFile::from_json(&parsed.to_json().expect("theme exports")).expect("export imports");
    assert_eq!(
        reparsed, parsed,
        "an empty set survives export rather than collapsing into an absent one"
    );
}

#[test]
fn variant_extensions_survive_export_without_a_variant_palette_beside_them() {
    let parsed = ThemeFile::from_json(
        r##"{
          "version": 1,
          "id": "extensions-only",
          "name": "Extensions Only",
          "appearance": "light",
          "colors": { "canvas": "#fff" },
          "variants": { "extensions": { "dark": { "soloist": { "gitModified": "#eba941" } } } }
        }"##,
    )
    .expect("a variants block carrying only extensions parses");

    let reparsed =
        ThemeFile::from_json(&parsed.to_json().expect("theme exports")).expect("export imports");
    assert_eq!(reparsed, parsed, "an export loses nothing it imported");
}

#[test]
fn the_paired_default_authors_light_extensions_and_derives_its_dark_ones() {
    let default = soloist_default_theme().expect("the shared catalog parses");

    let light = default
        .extensions_for(ThemeAppearance::Light)
        .soloist
        .as_ref()
        .expect("the light appearance carries explicit extensions");
    assert!(
        SoloistThemeRole::ALL
            .iter()
            .all(|role| light.get(*role).is_some()),
        "every Soloist role is authored against the light palette"
    );
    assert!(
        default.extensions_for(ThemeAppearance::Dark).is_empty(),
        "the dark appearance declares its own empty set, so its roles derive from the dark palette \
         instead of repainting light-authored hex onto it"
    );
}

#[test]
fn a_rejected_theme_reports_the_invalid_file_prefix_exactly_once() {
    // Taken from the error itself rather than restated, so rewording the variant cannot leave this
    // measuring a phrase the code no longer emits.
    let prefix = ThemeError::InvalidFile(String::new()).to_string();
    let overlong_description = "x".repeat(MAX_DESCRIPTION_CHARS + 1);
    let rejected = [
        format!(
            r##"{{"version":1,"id":"x","name":"X","appearance":"dark","colors":{{"accent":"#abc"}},"description":"{overlong_description}"}}"##
        ),
        r##"{"version":2,"id":"x","name":"X","appearance":"dark","colors":{"accent":"#abc"}}"##
            .to_owned(),
        r##"{"version":1,"id":"Bad_ID","name":"X","appearance":"dark","colors":{"accent":"#abc"}}"##
            .to_owned(),
        r##"{"version":1,"id":"x","name":"X","appearance":"dark","colors":{"accent":"#abc"},"variants":{"dark":{"canvas":"#000"}}}"##
            .to_owned(),
        r##"{"version":1,"id":"x","name":"X","appearance":"dark","colors":{"accent":"not-a-color"}}"##
            .to_owned(),
        r##"{"version":1,"id":"x","name":"X","appearance":"dark","colors":{"accent":"#abc"},"surprise":true}"##
            .to_owned(),
    ];

    for json in rejected {
        let message = ThemeFile::from_json(&json)
            .expect_err("the theme is rejected")
            .to_string();
        assert_eq!(
            message.matches(prefix.as_str()).count(),
            1,
            "a rejection reads once, not twice: {message:?}"
        );
    }
}

#[test]
fn strict_theme_file_validation_rejects_invalid_and_unknown_input() {
    let invalid = [
        r##"{"version":2,"id":"x","name":"X","appearance":"dark","colors":{"accent":"#abc"}}"##,
        r##"{"version":1,"id":"Bad_ID","name":"X","appearance":"dark","colors":{"accent":"#abc"}}"##,
        r##"{"version":1,"id":"x","name":"   ","appearance":"dark","colors":{"accent":"#abc"}}"##,
        r##"{"version":1,"id":"x","name":"X","appearance":"dark","colors":{}}"##,
        r##"{"version":1,"id":"x","name":"X","appearance":"dark","colors":{"accent":"red"}}"##,
        r##"{"version":1,"id":"x","name":"X","appearance":"dark","colors":{"notAThemeRole":"#abc"}}"##,
        r##"{"version":1,"id":"x","name":"X","appearance":"dark","colors":{"accent":"#abc"},"variants":{"dark":{"canvas":"#000"}}}"##,
        r##"{"version":1,"id":"x","name":"X","appearance":"dark","colors":{"accent":"#abc"},"variants":{"dim":{"canvas":"#000"}}}"##,
        r##"{"version":1,"id":"x","name":"X","appearance":"dark","colors":{"accent":"#abc"},"variants":{"extensions":{"dark":{}}}}"##,
        r##"{"version":1,"id":"x","name":"X","appearance":"dark","colors":{"accent":"#abc"},"variants":{"extensions":{"dim":{}}}}"##,
        r##"{"version":1,"id":"x","name":"X","appearance":"dark","colors":{"accent":"#abc"},"variants":{"extensions":{"light":{"someApp":{}}}}}"##,
        r##"{"version":1,"id":"x","name":"X","appearance":"dark","colors":{"accent":"#abc"},"variants":{"extensions":{"light":{"soloist":{"gitModified":"green"}}}}}"##,
        r##"{"version":1,"id":"x","name":"X","appearance":"dark","colors":{"accent":"#abc"},"surprise":true}"##,
    ];

    for json in invalid {
        assert!(ThemeFile::from_json(json).is_err(), "must reject: {json}");
    }
}

#[test]
fn missing_id_is_derived_from_the_name_using_the_t3_v1_rule() {
    let parsed = ThemeFile::from_json(
        r##"{"version":1,"name":"  My New Theme!  ","appearance":"light","colors":{"canvas":"#fff"}}"##,
    )
    .expect("id is optional in an imported T3 theme file");

    assert_eq!(parsed.id, "my-new-theme");
    assert_eq!(parsed.name, "My New Theme!");
}

#[test]
fn install_replace_and_keep_both_have_deterministic_conflict_behavior() {
    let mut appearance = Appearance::default();
    let original = theme("night", "Night", ThemeAppearance::Dark, "#123");
    assert_eq!(
        appearance
            .install_custom_theme(original, ThemeConflictPolicy::Reject, BUILT_INS)
            .expect("first install"),
        ThemeMutation::Installed { id: "night".into() }
    );

    let replacement = theme("night", "Night revised", ThemeAppearance::Dark, "#456");
    assert!(matches!(
        appearance.install_custom_theme(
            replacement.clone(),
            ThemeConflictPolicy::Reject,
            BUILT_INS
        ),
        Err(ThemeError::DuplicateId(id)) if id == "night"
    ));
    assert_eq!(
        appearance
            .install_custom_theme(replacement.clone(), ThemeConflictPolicy::Replace, BUILT_INS)
            .expect("replace"),
        ThemeMutation::Updated { id: "night".into() }
    );
    assert_eq!(appearance.custom_themes[0], replacement);

    let kept = appearance
        .install_custom_theme(
            theme("night", "Night", ThemeAppearance::Dark, "#789"),
            ThemeConflictPolicy::KeepBoth,
            BUILT_INS,
        )
        .expect("keep both");
    assert_eq!(
        kept,
        ThemeMutation::Installed {
            id: "night-2".into()
        }
    );
    let kept_again = appearance
        .install_custom_theme(
            theme("night", "Night", ThemeAppearance::Dark, "#abc"),
            ThemeConflictPolicy::KeepBoth,
            BUILT_INS,
        )
        .expect("keep another");
    assert_eq!(
        kept_again,
        ThemeMutation::Installed {
            id: "night-3".into()
        }
    );
}

#[test]
fn built_ins_are_immutable_but_keep_both_can_install_a_renamed_copy() {
    let mut appearance = Appearance::default();
    let built_in_copy = theme(
        DEFAULT_THEME_ID,
        "Soloist Default",
        ThemeAppearance::Light,
        "#123",
    );

    assert!(matches!(
        appearance.install_custom_theme(
            built_in_copy.clone(),
            ThemeConflictPolicy::Replace,
            BUILT_INS
        ),
        Err(ThemeError::BuiltInThemeImmutable(id)) if id == DEFAULT_THEME_ID
    ));
    assert!(matches!(
        appearance.remove_custom_theme(DEFAULT_THEME_ID, &[]),
        Err(ThemeError::BuiltInThemeImmutable(id)) if id == DEFAULT_THEME_ID
    ));

    let result = appearance
        .install_custom_theme(built_in_copy, ThemeConflictPolicy::KeepBoth, BUILT_INS)
        .expect("copying a built-in gets a new id");
    assert_eq!(
        result,
        ThemeMutation::Installed {
            id: "soloist-default-2".into()
        }
    );
}

#[test]
fn duplicate_and_remove_use_stable_ids_and_reset_every_selected_half() {
    let mut appearance = Appearance::default();
    appearance
        .install_custom_theme(
            theme("paired", "Paired", ThemeAppearance::Light, "#123"),
            ThemeConflictPolicy::Reject,
            BUILT_INS,
        )
        .expect("install source");
    appearance.selected_themes.light = "paired".into();
    appearance.selected_themes.dark = "paired".into();

    assert_eq!(
        appearance
            .duplicate_theme("paired", &[])
            .expect("first duplicate"),
        "paired-copy"
    );
    assert_eq!(
        appearance
            .duplicate_theme("paired", &[])
            .expect("second duplicate"),
        "paired-copy-2"
    );

    assert!(appearance
        .remove_custom_theme("paired", BUILT_INS)
        .expect("remove custom theme"));
    assert_eq!(appearance.selected_themes.light, DEFAULT_THEME_ID);
    assert_eq!(appearance.selected_themes.dark, DEFAULT_THEME_ID);
    assert!(!appearance
        .remove_custom_theme("missing", BUILT_INS)
        .expect("missing removal is a stable no-op"));
}

#[test]
fn built_ins_can_be_duplicated_into_an_editable_custom_theme() {
    let mut appearance = Appearance::default();
    let built_ins = built_in_themes().expect("built-in catalog");

    let copy_id = appearance
        .duplicate_theme("poimandres-dark-theme", built_ins)
        .expect("built-ins are valid duplicate sources");

    assert_eq!(copy_id, "poimandres-dark-theme-copy");
    assert_eq!(appearance.custom_themes.len(), 1);
    assert_eq!(appearance.custom_themes[0].id, copy_id);
    assert_eq!(
        appearance.custom_themes[0].name,
        "Poimandres dark theme copy"
    );
}

#[test]
fn update_requires_an_existing_custom_theme_and_keeps_its_position() {
    let mut appearance = Appearance::default();
    appearance
        .install_custom_theme(
            theme("first", "First", ThemeAppearance::Light, "#123"),
            ThemeConflictPolicy::Reject,
            BUILT_INS,
        )
        .unwrap();
    appearance
        .install_custom_theme(
            theme("second", "Second", ThemeAppearance::Dark, "#456"),
            ThemeConflictPolicy::Reject,
            BUILT_INS,
        )
        .unwrap();

    let changed = theme("first", "First edited", ThemeAppearance::Light, "#789");
    appearance
        .update_custom_theme(changed.clone(), BUILT_INS)
        .expect("update installed theme");
    assert_eq!(appearance.custom_themes[0], changed);
    assert_eq!(appearance.custom_themes[1].id, "second");
    assert!(matches!(
        appearance.update_custom_theme(
            theme("missing", "Missing", ThemeAppearance::Light, "#abc"),
            BUILT_INS
        ),
        Err(ThemeError::ThemeNotFound(id)) if id == "missing"
    ));
}
