//! Rendering a prompt template (context C6): the pipeline that turns a stored body plus a caller's
//! values into the text an agent is actually handed.
//!
//! Load, validate, substitute. The last two stages are pure functions over owned inputs, so the
//! substitution rules are unit-testable with no repo, clock, or fake; only the load touches a port.
//!
//! Substitution reads the **same** [`template_scan::scan`](super::template_scan::scan) stream that
//! [`placeholders`](super::template::placeholders) reports from, so a name a caller is asked to fill
//! is always a name that gets filled, and an escaped marker is invisible to both. Each token's
//! replacement is pushed straight onto the output and never back into the scan, so a value that
//! happens to contain `{{other}}` lands as literal text — a caller's data cannot inject a marker.
//!
//! Rendering is a query: it reads a template and returns text, publishes no
//! [`DomainEvent`](crate::events::DomainEvent), and changes nothing.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::template::{Templates, MAX_RENDERED_PROMPT};
use super::template_scan::{scan, Token};
use crate::ids::ProjectId;
use crate::ports::StoreError;
use crate::template::TemplateKind;

/// The one kind a template is rendered at. A prompt is applied to an agent with its fill-ins
/// resolved; the scratchpad and todo kinds seed a new document's body verbatim, so their markers
/// are content the author goes on to edit and are never substituted.
const RENDERABLE_KIND: TemplateKind = TemplateKind::Prompt;

/// What to do about a placeholder the caller supplied no value for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingPolicy {
    /// Leave the marker in the output and name it in [`RenderedPrompt::unfilled`]. The gap then
    /// travels with the artifact: a reader of `review {{file}}` can see what was never filled in,
    /// where `review ` would read as a complete instruction with a target to be guessed at.
    #[default]
    LeaveVerbatim,
    /// Refuse the render with [`RenderError::MissingValues`]. For a caller whose protocol has no
    /// channel to carry a warning back, so a partial render would be indistinguishable from a
    /// complete one.
    Strict,
}

/// What to render, and how.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RenderRequest {
    /// The template's name within the addressed scope.
    pub name: String,
    /// A value per placeholder name. Sorted, so `unknown` is reported in a stable order.
    pub values: BTreeMap<String, String>,
    pub policy: MissingPolicy,
}

/// A rendered prompt, and what did not line up while rendering it.
///
/// Both reports are advisory under [`MissingPolicy::LeaveVerbatim`]: the text is usable either way,
/// and the lists tell a surface what to warn about. `unknown` turns a mistyped value key — `dif`
/// for `{{diff}}` — from silence into feedback.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderedPrompt {
    pub text: String,
    /// Placeholders the body declares that no value was supplied for, in the body's order.
    pub unfilled: Vec<String>,
    /// Value names the body declares no placeholder for, in the values' order.
    pub unknown: Vec<String>,
}

/// Why a render was refused.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// No prompt template of that name exists in the addressed scope.
    #[error("no template under that name")]
    TemplateNotFound,
    /// [`MissingPolicy::Strict`] met placeholders with no value; the names are listed so the caller
    /// can supply them all in one retry.
    #[error("no value supplied for: {}", .0.join(", "))]
    MissingValues(Vec<String>),
    /// Substituting the values would produce more text than [`MAX_RENDERED_PROMPT`] allows, so the
    /// render was refused rather than allocating it.
    #[error("the rendered prompt would be {bytes} bytes, over the {cap} byte cap")]
    RenderedTooLarge { bytes: usize, cap: usize },
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl Templates {
    /// The prompt template `request.name` in the scope, rendered with `request.values`.
    ///
    /// The declared placeholders come from the read itself ([`TemplateView::placeholders`]), so the
    /// names reported here and the names substituted below are one derivation of one scan.
    ///
    /// [`TemplateView::placeholders`]: super::template::TemplateView::placeholders
    pub fn render(
        &self,
        project: Option<ProjectId>,
        request: &RenderRequest,
    ) -> Result<RenderedPrompt, RenderError> {
        let template = self
            .read(RENDERABLE_KIND, project, &request.name)?
            .ok_or(RenderError::TemplateNotFound)?;
        render_body(&template.body, &template.placeholders, request)
    }
}

/// Validates and substitutes an already-loaded body: everything after the port.
fn render_body(
    body: &str,
    declared: &[String],
    request: &RenderRequest,
) -> Result<RenderedPrompt, RenderError> {
    let (unfilled, unknown) = classify(declared, &request.values);
    if matches!(request.policy, MissingPolicy::Strict) && !unfilled.is_empty() {
        return Err(RenderError::MissingValues(unfilled));
    }
    Ok(RenderedPrompt {
        text: substitute(body, &request.values)?,
        unfilled,
        unknown,
    })
}

/// The declared names no value was supplied for, and the supplied names the body declares no
/// placeholder for — each keeping its source's order, so a report is stable across renders.
fn classify(declared: &[String], values: &BTreeMap<String, String>) -> (Vec<String>, Vec<String>) {
    let unfilled = declared
        .iter()
        .filter(|name| !values.contains_key(*name))
        .cloned()
        .collect();
    let unknown = values
        .keys()
        .filter(|supplied| !declared.iter().any(|name| name == *supplied))
        .cloned()
        .collect();
    (unfilled, unknown)
}

/// The body with every placeholder that has a value replaced by it.
///
/// Sized before it is built: the token stream is measured first and refused over the cap, so an
/// over-large render costs one pass rather than the allocation it was refused for.
fn substitute(body: &str, values: &BTreeMap<String, String>) -> Result<String, RenderError> {
    let bytes = scan(body)
        .map(|token| piece(token, values).len())
        .fold(0, usize::saturating_add);
    if bytes > MAX_RENDERED_PROMPT {
        return Err(RenderError::RenderedTooLarge {
            bytes,
            cap: MAX_RENDERED_PROMPT,
        });
    }
    let mut text = String::with_capacity(bytes);
    for token in scan(body) {
        text.push_str(piece(token, values));
    }
    Ok(text)
}

/// The text one token contributes: a placeholder's value when the caller supplied one, else the
/// token's own source text — so an unfilled marker survives into the output instead of collapsing
/// to nothing.
///
/// Values are emitted exactly as given. A prompt carries code, so escaping its angle brackets,
/// ampersands, or quotes would corrupt the payload rather than protect anything.
fn piece<'a>(token: Token<'a>, values: &'a BTreeMap<String, String>) -> &'a str {
    match token {
        Token::Placeholder { name, raw } => values.get(name).map_or(raw, String::as_str),
        other => other.verbatim(),
    }
}

#[cfg(test)]
#[path = "template_render_tests.rs"]
mod tests;
