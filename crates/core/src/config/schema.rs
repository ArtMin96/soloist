//! The `solo.yml` JSON Schema, generated from the [`SoloYml`] model.
//!
//! Editors that speak the YAML language server validate and autocomplete `solo.yml` against
//! this schema. It is **generated from the model**, never hand-written, so it cannot drift from
//! what the loader accepts: the same serde contract (`deny_unknown_fields`, the required
//! `command`, the field types) drives both deserialization and the schema. The committed
//! `solo.schema.json` at the repository root is exactly [`solo_schema_json`]'s output; a test
//! guards against drift, and the `gen_solo_schema` example regenerates the file.

use schemars::schema_for;

use super::model::SoloYml;

/// The `solo.yml` JSON Schema as pretty-printed JSON with a trailing newline — the exact bytes
/// of the committed `solo.schema.json`. Derived from [`SoloYml`], so it always matches the
/// loader's contract. `Err` only if the schema fails to serialize, which a valid model cannot
/// cause; the caller (the generator example, the drift test) surfaces it rather than panicking.
pub fn solo_schema_json() -> Result<String, serde_json::Error> {
    let schema = schema_for!(SoloYml);
    let mut text = serde_json::to_string_pretty(&schema)?;
    text.push('\n');
    Ok(text)
}

#[cfg(test)]
#[path = "schema_tests.rs"]
mod tests;
