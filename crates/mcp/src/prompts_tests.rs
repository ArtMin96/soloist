use super::*;
use rmcp::model::{Content, ErrorCode, PromptMessageContent};
use soloist_core::{
    placeholders, McpToolGroups, RenderedPrompt, TemplateId, TemplateKind, TemplateScope,
};
use soloist_ipc::{IpcError, IpcResult};

use crate::testing::{all_feature_groups, handler_on, spawn_fake_app};

/// One template the fake app holds. The body is the source of truth for its placeholders, as it is
/// in the core, so a test cannot declare a placeholder the body does not have.
struct StoredTemplate {
    id: u64,
    name: &'static str,
    description: Option<&'static str>,
    body: &'static str,
    scope: TemplateScope,
}

impl StoredTemplate {
    fn summary(&self) -> TemplateSummary {
        TemplateSummary {
            id: TemplateId::from_raw(self.id),
            kind: TemplateKind::Prompt,
            name: self.name.to_owned(),
            description: self.description.map(str::to_owned),
            placeholders: placeholders(self.body),
            scope: self.scope,
            revision: 1,
        }
    }
}

/// A fake app holding `templates`, answering a list with their summaries and a render by honouring
/// the request's [`MissingPolicy`] — so a test of `prompts/get` fails for the real reason when the
/// policy on the wire is wrong, rather than because the fake was told what to return.
fn app_holding(templates: &'static [StoredTemplate]) -> impl Fn(IpcRequest) -> IpcResult {
    move |request| match request {
        IpcRequest::PromptTemplateList { .. } => Ok(IpcResponse::PromptTemplates(
            templates.iter().map(StoredTemplate::summary).collect(),
        )),
        IpcRequest::PromptTemplateRender {
            scope,
            name,
            values,
            policy,
        } => {
            let found = templates
                .iter()
                .find(|template| template.name == name && template.scope == scope)
                .ok_or(IpcError::UnknownTemplate)?;
            let declared = placeholders(found.body);
            let unfilled: Vec<String> = declared
                .iter()
                .filter(|name| !values.contains_key(*name))
                .cloned()
                .collect();
            if matches!(policy, MissingPolicy::Strict) && !unfilled.is_empty() {
                return Err(IpcError::MissingTemplateValues { names: unfilled });
            }
            let mut text = found.body.to_owned();
            for (key, value) in &values {
                text = text.replace(&format!("{{{{{key}}}}}"), value);
            }
            Ok(IpcResponse::PromptTemplateRendered(RenderedPrompt {
                text,
                unfilled,
                unknown: Vec::new(),
            }))
        }
        other => panic!("the prompts primitive asked for {other:?}"),
    }
}

/// A handler serving `templates`, with the given feature-group enablement.
fn handler_holding(
    socket: &std::path::Path,
    groups: McpToolGroups,
    templates: &'static [StoredTemplate],
) -> SoloistMcp {
    spawn_fake_app(socket.to_path_buf(), app_holding(templates));
    handler_on(socket.to_path_buf(), groups)
}

static REVIEW: &[StoredTemplate] = &[StoredTemplate {
    id: 7,
    name: "Review diff",
    description: Some("Review a diff"),
    body: "Review {{diff}} focusing on {{focus}}.",
    scope: TemplateScope::Global,
}];

/// The text of a `prompts/get` result's single message.
fn message_text(result: &GetPromptResult) -> &str {
    let [message] = result.messages.as_slice() else {
        panic!("expected exactly one message, got {:?}", result.messages);
    };
    assert_eq!(message.role, PromptMessageRole::User);
    match &message.content {
        PromptMessageContent::Text { text } => text,
        other => panic!("expected text content, got {other:?}"),
    }
}

/// Each stored template is offered as one prompt: a slugified, server-qualified name a client can
/// address unambiguously, its real name as the readable title, and one required argument per
/// placeholder — in the body's own order, which is the order a client binding positionally will
/// hand them back.
#[tokio::test]
async fn a_template_is_offered_with_one_required_argument_per_placeholder() {
    let dir = tempfile::tempdir().expect("tempdir");
    let handler = handler_holding(&dir.path().join("app.sock"), all_feature_groups(), REVIEW);

    let listed = handler
        .prompt_list()
        .await
        .expect("a list is never an error");

    let [prompt] = listed.prompts.as_slice() else {
        panic!("expected exactly one prompt, got {:?}", listed.prompts);
    };
    assert_eq!(prompt.name, "soloist-review-diff");
    assert_eq!(prompt.title.as_deref(), Some("Review diff"));
    assert_eq!(prompt.description.as_deref(), Some("Review a diff"));
    let arguments = prompt.arguments.as_ref().expect("declared arguments");
    assert_eq!(
        arguments
            .iter()
            .map(|argument| argument.name.as_str())
            .collect::<Vec<_>>(),
        vec!["diff", "focus"],
        "arguments keep the body's first-appearance order"
    );
    assert!(
        arguments
            .iter()
            .all(|argument| argument.required == Some(true)),
        "every placeholder must be supplied, since rendering is strict"
    );
}

/// The security gate: with the Prompt Templates group switched off, the prompts door reads out
/// nothing — even though the app holds templates and would happily list them. The tool router the
/// group is otherwise enforced by does not cover this method.
#[tokio::test]
async fn no_template_is_listed_while_the_feature_group_is_off() {
    let dir = tempfile::tempdir().expect("tempdir");
    let handler = handler_holding(
        &dir.path().join("app.sock"),
        McpToolGroups::default(),
        REVIEW,
    );

    let listed = handler
        .prompt_list()
        .await
        .expect("a list is never an error");

    assert!(
        listed.prompts.is_empty(),
        "a disabled group must expose no template, got {:?}",
        listed.prompts
    );
}

/// The same gate on the other method: a client may call a method it was never offered, so the
/// refusal cannot rest on the advertised capability alone.
#[tokio::test]
async fn getting_a_prompt_is_refused_while_the_feature_group_is_off() {
    let dir = tempfile::tempdir().expect("tempdir");
    let handler = handler_holding(
        &dir.path().join("app.sock"),
        McpToolGroups::default(),
        REVIEW,
    );

    let err = handler
        .prompt_get(GetPromptRequestParams::new("soloist-review-diff"))
        .await
        .expect_err("a disabled group must refuse");

    assert_eq!(err.code, ErrorCode::METHOD_NOT_FOUND);
}

/// A list is never an error, whatever the app says — a client that meets one there may drop the
/// server outright, taking its tools with it.
#[tokio::test]
async fn a_refused_list_is_reported_as_no_prompts_rather_than_an_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let socket = dir.path().join("app.sock");
    spawn_fake_app(socket.clone(), |_| Err(IpcError::NoProjectScope));
    let handler = handler_on(socket, all_feature_groups());

    let listed = handler
        .prompt_list()
        .await
        .expect("a list is never an error");

    assert!(listed.prompts.is_empty());
}

/// An unreachable app is the same: no prompts, no error.
#[tokio::test]
async fn an_unreachable_app_is_reported_as_no_prompts_rather_than_an_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let handler = handler_on(dir.path().join("absent.sock"), all_feature_groups());

    let listed = handler
        .prompt_list()
        .await
        .expect("a list is never an error");

    assert!(listed.prompts.is_empty());
}

/// The server does the templating: a `prompts/get` result carries the finished text, with every
/// marker replaced, because the protocol gives the client nothing to substitute with.
#[tokio::test]
async fn getting_a_prompt_returns_the_fully_substituted_text() {
    let dir = tempfile::tempdir().expect("tempdir");
    let handler = handler_holding(&dir.path().join("app.sock"), all_feature_groups(), REVIEW);

    let result = handler
        .prompt_get(
            GetPromptRequestParams::new("soloist-review-diff").with_arguments(
                [
                    ("diff".to_owned(), serde_json::json!("a/b.rs")),
                    ("focus".to_owned(), serde_json::json!("error handling")),
                ]
                .into_iter()
                .collect(),
            ),
        )
        .await
        .expect("a rendered prompt");

    assert_eq!(
        message_text(&result),
        "Review a/b.rs focusing on error handling."
    );
    assert_eq!(result.description.as_deref(), Some("Review a diff"));
}

/// Rendering here is strict: with no channel to report a gap on, a value left out is refused as a
/// bad parameter rather than handed back as a prompt with a marker still in it.
#[tokio::test]
async fn getting_a_prompt_without_a_required_argument_is_refused_as_a_bad_parameter() {
    let dir = tempfile::tempdir().expect("tempdir");
    let handler = handler_holding(&dir.path().join("app.sock"), all_feature_groups(), REVIEW);

    let err = handler
        .prompt_get(
            GetPromptRequestParams::new("soloist-review-diff").with_arguments(
                [("diff".to_owned(), serde_json::json!("a/b.rs"))]
                    .into_iter()
                    .collect(),
            ),
        )
        .await
        .expect_err("a missing value must be refused");

    assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
    assert!(
        err.message.contains("focus"),
        "the refusal must name what to supply, got {}",
        err.message
    );
}

/// A name no template is offered under is the caller's mistake, reported with the same code as a
/// bad argument.
#[tokio::test]
async fn getting_an_unknown_prompt_is_refused_as_a_bad_parameter() {
    let dir = tempfile::tempdir().expect("tempdir");
    let handler = handler_holding(&dir.path().join("app.sock"), all_feature_groups(), REVIEW);

    let err = handler
        .prompt_get(GetPromptRequestParams::new("soloist-absent"))
        .await
        .expect_err("an unknown name must be refused");

    assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
}

/// The protocol types an argument as a string, so anything else is a malformed call rather than
/// something to coerce.
#[tokio::test]
async fn a_non_string_argument_is_refused_as_a_bad_parameter() {
    let dir = tempfile::tempdir().expect("tempdir");
    let handler = handler_holding(&dir.path().join("app.sock"), all_feature_groups(), REVIEW);

    let err = handler
        .prompt_get(
            GetPromptRequestParams::new("soloist-review-diff").with_arguments(
                [("diff".to_owned(), serde_json::json!(7))]
                    .into_iter()
                    .collect(),
            ),
        )
        .await
        .expect_err("a non-string argument must be refused");

    assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
}

static ALIKE: &[StoredTemplate] = &[
    StoredTemplate {
        id: 1,
        name: "Code Review",
        description: None,
        body: "Review it.",
        scope: TemplateScope::Global,
    },
    StoredTemplate {
        id: 2,
        name: "code-review",
        description: None,
        body: "Review it differently.",
        scope: TemplateScope::Project,
    },
];

/// Slugifying is lossy, so two differently-named templates can reduce to the same slug. Neither may
/// disappear behind the other, and each must still resolve to its own body.
#[tokio::test]
async fn templates_whose_names_slugify_alike_stay_separately_addressable() {
    let dir = tempfile::tempdir().expect("tempdir");
    let handler = handler_holding(&dir.path().join("app.sock"), all_feature_groups(), ALIKE);

    let listed = handler
        .prompt_list()
        .await
        .expect("a list is never an error");
    let names: Vec<&str> = listed
        .prompts
        .iter()
        .map(|prompt| prompt.name.as_str())
        .collect();

    assert_eq!(names, vec!["soloist-code-review", "soloist-code-review-2"]);
    for (name, expected) in [
        ("soloist-code-review", "Review it."),
        ("soloist-code-review-2", "Review it differently."),
    ] {
        let result = handler
            .prompt_get(GetPromptRequestParams::new(name))
            .await
            .expect("a rendered prompt");
        assert_eq!(message_text(&result), expected, "{name} resolved wrongly");
    }
}

static UNREPRESENTABLE: &[StoredTemplate] = &[StoredTemplate {
    id: 42,
    name: "🚀",
    description: None,
    body: "Ship it.",
    scope: TemplateScope::Project,
}];

/// A name with nothing sluggable in it still gets an addressable prompt rather than an empty name
/// or a silent omission.
#[tokio::test]
async fn a_template_whose_name_has_no_sluggable_characters_is_still_addressable() {
    let dir = tempfile::tempdir().expect("tempdir");
    let handler = handler_holding(
        &dir.path().join("app.sock"),
        all_feature_groups(),
        UNREPRESENTABLE,
    );

    let listed = handler
        .prompt_list()
        .await
        .expect("a list is never an error");
    let [prompt] = listed.prompts.as_slice() else {
        panic!("expected exactly one prompt, got {:?}", listed.prompts);
    };
    assert_eq!(prompt.name, "soloist-template-42");
    assert_eq!(prompt.title.as_deref(), Some("🚀"));

    let result = handler
        .prompt_get(GetPromptRequestParams::new("soloist-template-42"))
        .await
        .expect("a rendered prompt");
    assert_eq!(message_text(&result), "Ship it.");
}

/// A template with no placeholders declares no arguments at all, rather than an empty list a client
/// would render as "takes arguments".
#[tokio::test]
async fn a_template_with_no_placeholders_declares_no_arguments() {
    let dir = tempfile::tempdir().expect("tempdir");
    let handler = handler_holding(
        &dir.path().join("app.sock"),
        all_feature_groups(),
        UNREPRESENTABLE,
    );

    let listed = handler
        .prompt_list()
        .await
        .expect("a list is never an error");

    assert_eq!(listed.prompts[0].arguments, None);
}

/// A successful write to a template leaves any prompt list a client is holding stale.
#[test]
fn a_successful_template_write_reports_the_prompt_list_changed() {
    for tool in ["prompt_template_create", "prompt_template_update"] {
        assert!(
            changed_prompt_list(tool, &CallToolResult::structured(serde_json::json!({}))),
            "{tool} changes which templates exist"
        );
    }
}

/// A delete that found nothing to remove changed no list, so there is nothing to tell a client.
#[test]
fn a_delete_that_removed_nothing_reports_no_change() {
    let removed = CallToolResult::structured(serde_json::json!({ "deleted": true }));
    let absent = CallToolResult::structured(serde_json::json!({ "deleted": false }));

    assert!(changed_prompt_list("prompt_template_delete", &removed));
    assert!(!changed_prompt_list("prompt_template_delete", &absent));
}

/// A refused write changed nothing, and a tool that does not touch templates never does.
#[test]
fn a_failed_or_unrelated_call_reports_no_prompt_list_change() {
    let failed = CallToolResult::error(vec![Content::text("refused")]);

    assert!(!changed_prompt_list("prompt_template_create", &failed));
    assert!(!changed_prompt_list(
        "prompt_template_read",
        &CallToolResult::structured(serde_json::json!({}))
    ));
}
