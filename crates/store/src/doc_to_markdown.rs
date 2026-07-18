//! One-way conversion of the coordination stores' formerly structured documents into free-form
//! Markdown bodies, run once as a schema migration step.
//!
//! Before this, a scratchpad `doc` column held the JSON of a fixed seven-field structure and a todo
//! `doc` held five fields; now a scratchpad body is raw Markdown and a todo doc is `{title, body,
//! status}`. This module carries the pure converters ([`scratchpad_body`], [`todo_doc`]) and the row
//! walk that applies them ([`convert`]). The old canonical section layout is the converter, so an
//! upgraded document reads back with every field it carried, laid out as Markdown headings. The
//! walk is idempotent: a body already converted is left untouched, so re-running the step is a no-op.

use std::fmt::Write as _;

use rusqlite::Connection;
use serde_json::Value;
use soloist_core::StoreError;

use crate::sql_err;

/// Converts every stored scratchpad and todo document to its Markdown form in place, under the
/// migration's connection. Each row is read, converted by the pure functions below, and written
/// back; a row whose body is already Markdown (a re-run) is left unchanged.
pub(crate) fn convert(conn: &Connection) -> Result<(), StoreError> {
    for (id, doc) in read_docs(conn, "scratchpads")? {
        let body = scratchpad_body(&doc);
        if body != doc {
            conn.execute("UPDATE scratchpads SET doc = ?2 WHERE id = ?1", (id, body))
                .map_err(sql_err)?;
        }
    }
    for (id, doc) in read_docs(conn, "todos")? {
        let converted = todo_doc(&doc);
        if converted != doc {
            conn.execute("UPDATE todos SET doc = ?2 WHERE id = ?1", (id, converted))
                .map_err(sql_err)?;
        }
    }
    Ok(())
}

/// The `(id, doc)` of every row in `table`, materialized so the update loop does not hold the
/// prepared statement while it writes back.
fn read_docs(conn: &Connection, table: &str) -> Result<Vec<(i64, String)>, StoreError> {
    let mut stmt = conn
        .prepare(&format!("SELECT id, doc FROM {table}"))
        .map_err(sql_err)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(sql_err)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(sql_err)?);
    }
    Ok(out)
}

/// The Markdown body for a stored scratchpad `doc`. When `doc` is the old structured JSON object, it
/// is rendered to the canonical sections (the name is not embedded — it stays the row's handle);
/// when `doc` is already Markdown (a re-run) or any non-object value, it is returned unchanged. Every
/// `doc` written before this migration was the structured JSON, so an object here is always legacy.
fn scratchpad_body(doc: &str) -> String {
    match serde_json::from_str::<Value>(doc) {
        Ok(value) if value.is_object() => render_scratchpad(&value),
        _ => doc.to_owned(),
    }
}

/// Renders a legacy scratchpad document to the canonical section layout (objective, context, an
/// ordered plan, checkbox acceptance criteria, risks, status, and optional notes), omitting only an
/// empty notes section — the shape the app rendered before the body went free-form.
fn render_scratchpad(value: &Value) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "## Objective\n{}\n",
        string_field(value, "objective").trim()
    );
    let _ = writeln!(
        out,
        "## Context\n{}\n",
        string_field(value, "context").trim()
    );
    let _ = writeln!(out, "## Plan");
    for (index, step) in string_array(value, "plan").iter().enumerate() {
        let _ = writeln!(out, "{}. {}", index + 1, step.trim());
    }
    let _ = writeln!(out, "\n## Acceptance criteria");
    for criterion in string_array(value, "acceptance_criteria") {
        let _ = writeln!(out, "- [ ] {}", criterion.trim());
    }
    let _ = writeln!(out, "\n## Risks");
    for risk in string_array(value, "risks") {
        let _ = writeln!(out, "- {}", risk.trim());
    }
    let _ = writeln!(out, "\n## Status\n{}", string_field(value, "status").trim());
    let notes = string_field(value, "notes");
    let notes = notes.trim();
    if !notes.is_empty() {
        let _ = writeln!(out, "\n## Notes\n{notes}");
    }
    out
}

/// The new todo `doc` JSON for a stored todo `doc`. When `doc` is the old five-field JSON, the
/// description leads the Markdown body followed by acceptance-criteria and risks sections, keeping
/// the title and status columns verbatim; when `doc` already carries a `body` field (a re-run), it
/// is returned unchanged. A value that does not parse as JSON is preserved inside a minimal body.
fn todo_doc(doc: &str) -> String {
    let Ok(value) = serde_json::from_str::<Value>(doc) else {
        return new_todo(
            "",
            &todo_body(doc, &[], &[]),
            Value::String("open".to_owned()),
        );
    };
    if value.get("body").is_some() {
        return doc.to_owned();
    }
    let title = string_field(&value, "title");
    let description = string_field(&value, "description");
    let acceptance_criteria = string_array(&value, "acceptance_criteria");
    let risks = string_array(&value, "risks");
    let status = value
        .get("status")
        .cloned()
        .unwrap_or_else(|| Value::String("open".to_owned()));
    let body = todo_body(&description, &acceptance_criteria, &risks);
    new_todo(&title, &body, status)
}

/// The Markdown body for a todo: the description as leading prose, then an acceptance-criteria and a
/// risks section, each omitted when empty.
fn todo_body(description: &str, acceptance_criteria: &[String], risks: &[String]) -> String {
    let mut out = String::new();
    let description = description.trim();
    if !description.is_empty() {
        out.push_str(description);
    }
    push_section(
        &mut out,
        "## Acceptance criteria",
        acceptance_criteria,
        "- [ ] ",
    );
    push_section(&mut out, "## Risks", risks, "- ");
    out
}

/// Appends a Markdown section listing `items` under `heading` with `bullet`, skipping blank items
/// and the whole section when none remain. A blank line separates it from whatever precedes.
fn push_section(out: &mut String, heading: &str, items: &[String], bullet: &str) {
    let items: Vec<&str> = items
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .collect();
    if items.is_empty() {
        return;
    }
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(heading);
    for item in items {
        out.push('\n');
        out.push_str(bullet);
        out.push_str(item);
    }
}

/// Serializes the new todo document shape (`{title, body, status}`) to the JSON the `doc` column
/// stores, with `status` carried through verbatim from the legacy row.
fn new_todo(title: &str, body: &str, status: Value) -> String {
    serde_json::json!({ "title": title, "body": body, "status": status }).to_string()
}

/// The string at `key`, or the empty string when absent or not a string.
fn string_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned()
}

/// The array of strings at `key`, dropping any non-string entry; empty when absent or not an array.
fn string_array(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "doc_to_markdown_tests.rs"]
mod tests;
