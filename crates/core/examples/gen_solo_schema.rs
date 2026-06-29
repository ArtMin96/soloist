//! Prints the `solo.yml` JSON Schema to stdout. Regenerate the committed schema with:
//!
//! ```sh
//! cargo run -q -p soloist-core --features schema --example gen_solo_schema > solo.schema.json
//! ```
//!
//! (Or `just schema`.) The drift test in `config::schema` guards that the committed file stays
//! in step with the model.

fn main() -> Result<(), serde_json::Error> {
    print!("{}", soloist_core::config::schema::solo_schema_json()?);
    Ok(())
}
