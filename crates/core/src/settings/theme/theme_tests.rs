use crate::settings::Settings;
use crate::settings::{
    built_in_themes, default_theme_colors, Appearance, GlassOpacity, SoloistThemeRole,
    ThemeAppearance, ThemeColor, ThemeColorRole, ThemeConflictPolicy, ThemeError, ThemeFile,
    ThemeMutation, DEFAULT_THEME_ID,
};

const BUILT_INS: &[&str] = &[DEFAULT_THEME_ID, "poimandres-dark-theme"];

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
