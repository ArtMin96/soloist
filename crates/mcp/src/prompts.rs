//! The MCP **prompts** primitive: the user's stored prompt templates offered as the native,
//! user-invoked slash commands an MCP client already knows how to show.
//!
//! Prompts are a second door onto the data the `prompt_template_*` tools reach, not a replacement:
//! the tools are the portable baseline (every MCP client serves tools; only some serve prompts), so
//! both route to the **same** core render command rather than either reimplementing substitution.
//!
//! Two protocol constraints shape everything here:
//!
//! - **The server renders.** `prompts/get` returns fully substituted messages; a client does no
//!   templating and never sees a `{{marker}}`. That makes the delimiter a private storage detail.
//! - **There is no warning channel.** A `prompts/get` result carries messages and nothing else, so a
//!   placeholder left unfilled would be indistinguishable from one deliberately blank. Rendering here
//!   is therefore [`MissingPolicy::Strict`] and a missing argument is refused as a bad parameter —
//!   where the render *tool*, whose result does carry the gap, leaves the marker in place.
//!
//! `prompts/list` never fails: an unreachable or refusing app yields an empty list, because a client
//! that meets an error on that call may drop the whole server — including its tools.

use std::collections::{BTreeMap, BTreeSet};

use rmcp::model::{
    CallToolResult, ErrorData, GetPromptRequestParams, GetPromptResult, JsonObject,
    ListPromptsResult, Prompt, PromptArgument, PromptMessage, PromptMessageRole,
};
use soloist_core::{MissingPolicy, TemplateSummary};
use soloist_ipc::{IpcRequest, IpcResponse};

use crate::client::ClientError;
use crate::server::SoloistMcp;
use crate::tools::reply::unexpected;

/// Prefixes every offered prompt name. Some clients expose a prompt under its bare name with no
/// server qualifier, so an unprefixed `code-review` would collide with any other connected server's
/// — and the user could not tell which one they invoked. Qualifying every name costs a little
/// verbosity on clients that already namespace, and removes the ambiguity on those that do not.
const NAME_PREFIX: &str = "soloist-";

/// Separates words in an offered name, and the character a run of anything unrepresentable
/// collapses to.
const NAME_SEPARATOR: char = '-';

/// Stands in for a template whose name has no representable characters at all (an emoji-only or
/// wholly non-ASCII name), which would otherwise slugify to nothing. Disambiguated by the
/// template's own id, so such a template is still addressable rather than silently unreachable.
const UNNAMED: &str = "template";

/// The tools whose success changes which prompt templates exist, so a client holding a
/// `prompts/list` result has a stale one until it re-reads. Every name here must be a served tool —
/// a test guards against a typo or a rename leaving a dangling entry.
pub(crate) const PROMPT_LIST_MUTATORS: &[&str] = &[
    "prompt_template_create",
    "prompt_template_update",
    "prompt_template_delete",
];

/// The key a delete reports on: it removes nothing when the name was already absent, and a list
/// that did not change is not worth telling a client about. Shared with the tool that emits it —
/// a literal on either side could be renamed alone, and the notification would just stop firing.
pub(crate) const DELETED: &str = "deleted";

/// Whether a completed tool call changed which prompt templates exist, leaving a client's
/// `prompts/list` result stale. A call that failed changed nothing, and neither did a delete that
/// found nothing to remove.
pub(crate) fn changed_prompt_list(tool: &str, result: &CallToolResult) -> bool {
    if result.is_error == Some(true) || !PROMPT_LIST_MUTATORS.contains(&tool) {
        return false;
    }
    match result
        .structured_content
        .as_ref()
        .and_then(|content| content.get(DELETED))
    {
        Some(deleted) => deleted == &serde_json::Value::Bool(true),
        None => true,
    }
}

/// The prompt this template is offered as, under the name it was given.
///
/// `title` carries the template's real name, so the readability the slug spends on being
/// unambiguous is handed back to the human reading the client's prompt picker. Every declared
/// placeholder becomes one required argument: rendering is strict, so there is no such thing as an
/// optional one.
fn prompt_of(summary: &TemplateSummary, name: String) -> Prompt {
    let arguments: Vec<PromptArgument> = summary
        .placeholders
        .iter()
        .map(|placeholder| PromptArgument::new(placeholder.clone()).with_required(true))
        .collect();
    Prompt::new(
        name,
        summary.description.clone(),
        (!arguments.is_empty()).then_some(arguments),
    )
    .with_title(summary.name.clone())
}

/// A template name reduced to lowercase words joined by [`NAME_SEPARATOR`]. Anything else — spaces,
/// punctuation, non-ASCII — collapses to a single separator, and leading and trailing separators are
/// dropped, so the result is addressable as a slash command whatever the user named the template.
fn slugify(name: &str) -> String {
    let mut slug = String::with_capacity(name.len());
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
        } else if !slug.is_empty() && !slug.ends_with(NAME_SEPARATOR) {
            slug.push(NAME_SEPARATOR);
        }
    }
    while slug.ends_with(NAME_SEPARATOR) {
        slug.pop();
    }
    slug
}

/// The name `summary` is offered under, kept distinct from the names already `taken`.
///
/// Slugifying is lossy — `Code Review` and `code-review` reduce to the same slug — so a clash is
/// broken with the template's id, which is stable across edits and unique by construction. Two
/// templates therefore never silently become one.
fn offered_name(summary: &TemplateSummary, taken: &BTreeSet<String>) -> String {
    let slug = slugify(&summary.name);
    let base = match slug.is_empty() {
        true => format!("{NAME_PREFIX}{UNNAMED}{NAME_SEPARATOR}{}", summary.id.get()),
        false => format!("{NAME_PREFIX}{slug}"),
    };
    match taken.contains(&base) {
        true => format!("{base}{NAME_SEPARATOR}{}", summary.id.get()),
        false => base,
    }
}

/// The caller's arguments as the values a render takes. The protocol types a prompt argument as a
/// string, so any other JSON type is a malformed call rather than something to coerce and guess at.
fn render_values(arguments: Option<JsonObject>) -> Result<BTreeMap<String, String>, ErrorData> {
    arguments
        .unwrap_or_default()
        .into_iter()
        .map(|(key, value)| match value {
            serde_json::Value::String(text) => Ok((key, text)),
            _ => Err(ErrorData::invalid_params(
                format!("the argument {key} must be a string"),
                None,
            )),
        })
        .collect()
}

/// Maps a failed render onto the one error channel `prompts/get` has.
///
/// A refusal the caller can act on — an argument left out, a template that has since gone — is an
/// invalid-parameter error, which is what a client surfaces as a fixable mistake. Anything else (the
/// app down, a store failure) stays an internal error, so a genuine fault is never dressed up as the
/// caller's typo.
fn get_error(err: &ClientError) -> ErrorData {
    match err {
        ClientError::App(app) if app.is_request_error() => {
            ErrorData::invalid_params(app.to_string(), None)
        }
        _ => ErrorData::internal_error(err.to_string(), None),
    }
}

impl SoloistMcp {
    /// Every prompt template the session can address, each paired with the unique name it is offered
    /// under — the single derivation both [`prompt_list`](Self::prompt_list) and
    /// [`prompt_get`](Self::prompt_get) read, so a name that is listed always resolves and the two
    /// cannot drift.
    ///
    /// Empty rather than failing when the feature group is off, the app is unreachable, or the app
    /// refuses: this feeds `prompts/list`, which must never return an error.
    async fn offered_prompts(&self) -> Vec<(String, TemplateSummary)> {
        if !self.prompts_enabled() {
            return Vec::new();
        }
        let Ok(IpcResponse::PromptTemplates(templates)) = self
            .client
            .request(IpcRequest::PromptTemplateList { scope: None })
            .await
        else {
            return Vec::new();
        };
        let mut taken = BTreeSet::new();
        templates
            .into_iter()
            .map(|summary| {
                let name = offered_name(&summary, &taken);
                taken.insert(name.clone());
                (name, summary)
            })
            .collect()
    }

    /// Serves `prompts/list`: the session's prompt templates, in the order the core lists them.
    ///
    /// Always `Ok`. The result type mirrors the trait method this answers, and the invariant is the
    /// point: a client that meets an error here may drop this server outright, tools included, so
    /// every reason there might be nothing to list — the group off, the app down, the app refusing —
    /// is an empty list instead. The signature keeps that a rule a test can hold us to rather than
    /// an accident of how the body happens to be written today.
    pub(crate) async fn prompt_list(&self) -> Result<ListPromptsResult, ErrorData> {
        Ok(ListPromptsResult::with_all_items(
            self.offered_prompts()
                .await
                .into_iter()
                .map(|(name, summary)| prompt_of(&summary, name))
                .collect(),
        ))
    }

    /// Serves `prompts/get`: the named template, rendered with the caller's arguments.
    ///
    /// Refused as method-not-found while the Prompt Templates group is off — the same answer a
    /// server that never implemented prompts gives, which is exactly what the user asked for by
    /// turning the group off. The refusal is enforced here and not left to the advertised
    /// capability, since a client may call a method it was never offered.
    pub(crate) async fn prompt_get(
        &self,
        request: GetPromptRequestParams,
    ) -> Result<GetPromptResult, ErrorData> {
        if !self.prompts_enabled() {
            return Err(ErrorData::method_not_found::<
                rmcp::model::GetPromptRequestMethod,
            >());
        }
        let values = render_values(request.arguments)?;
        let (_, summary) = self
            .offered_prompts()
            .await
            .into_iter()
            .find(|(name, _)| *name == request.name)
            .ok_or_else(|| {
                ErrorData::invalid_params(format!("no prompt named {}", request.name), None)
            })?;

        match self
            .client
            .request(IpcRequest::PromptTemplateRender {
                scope: summary.scope,
                name: summary.name.clone(),
                values,
                policy: MissingPolicy::Strict,
            })
            .await
        {
            Ok(IpcResponse::PromptTemplateRendered(rendered)) => {
                let result = GetPromptResult::new(vec![PromptMessage::new_text(
                    PromptMessageRole::User,
                    rendered.text,
                )]);
                Ok(match summary.description {
                    Some(description) => result.with_description(description),
                    None => result,
                })
            }
            Ok(_) => Err(unexpected()),
            Err(err) => Err(get_error(&err)),
        }
    }
}

#[cfg(test)]
#[path = "prompts_tests.rs"]
mod tests;
