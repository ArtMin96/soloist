//! Guards for the generated `solo.yml` JSON Schema. Two things must hold: the committed
//! `solo.schema.json` stays in step with the model (no drift), and the schema encodes exactly
//! the constraints the loader enforces — `command` required, no unknown fields — so an editor
//! accepts the same configs the loader does and rejects the same ones. The behavioural
//! accept/reject over fixtures lives in `load.rs` (the loader is the schema's contract); these
//! tests prove the schema *is* that contract.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::solo_schema_json;

/// The committed schema at the repository root — the file editors point at. Resolved from the
/// crate manifest dir so the path holds regardless of the test runner's working directory.
fn committed_schema_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../solo.schema.json")
}

#[test]
fn committed_schema_matches_the_model() {
    let generated = solo_schema_json().expect("schema generates");
    let path = committed_schema_path();
    // `BLESS_SOLO_SCHEMA=1 cargo test -p soloist-core --features schema` (or `just schema`)
    // rewrites the committed file from the model after an intentional schema change.
    if std::env::var_os("BLESS_SOLO_SCHEMA").is_some() {
        std::fs::write(&path, &generated).expect("write committed schema");
        return;
    }
    let committed = std::fs::read_to_string(&path).expect("read committed solo.schema.json");
    assert_eq!(
        committed, generated,
        "solo.schema.json is stale — regenerate it from the model with `just schema`"
    );
}

#[test]
fn schema_encodes_the_loader_contract() {
    let schema: Value = serde_json::from_str(&solo_schema_json().expect("schema generates"))
        .expect("schema parses");

    // 2020-12 so a `validator_for`-style editor selects the right dialect.
    assert_eq!(
        schema["$schema"], "https://json-schema.org/draft/2020-12/schema",
        "schema must declare the JSON Schema draft so editors pick the dialect"
    );

    // Top level rejects unknown keys, so an editor flags a mistyped top-level key (e.g. `process:`
    // for `processes:`).
    assert_eq!(schema["additionalProperties"], Value::Bool(false));
    // `processes` is a map keyed by name whose values are `ProcessSpec` — the shape an editor uses
    // to complete a new command's fields.
    let processes = &schema["properties"]["processes"];
    assert_eq!(processes["type"], "object");
    assert_eq!(
        processes["additionalProperties"]["$ref"], "#/$defs/ProcessSpec",
        "each process entry must validate against ProcessSpec"
    );

    let spec = &schema["$defs"]["ProcessSpec"];
    // The two rules a negative fixture trips: a command must name a `command`, and may carry no
    // unknown field (`deny_unknown_fields` ⇒ additionalProperties:false).
    assert_eq!(spec["required"], json!(["command"]));
    assert_eq!(spec["additionalProperties"], Value::Bool(false));
    // Field types match the model, so an editor flags a wrong-typed value.
    assert_eq!(spec["properties"]["command"]["type"], "string");
    assert_eq!(spec["properties"]["auto_start"]["type"], "boolean");
    assert_eq!(spec["properties"]["auto_restart"]["type"], "boolean");
    assert_eq!(spec["properties"]["restart_when_changed"]["type"], "array");
    assert_eq!(spec["properties"]["env"]["type"], "object");
}
