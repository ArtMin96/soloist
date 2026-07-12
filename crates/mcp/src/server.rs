//! The rmcp server handler: a stateless front over the app holding one client connection.
//!
//! The tools themselves live in [`crate::tools`], one `#[tool_router(router = …)]` block per
//! logical category; [`SoloistMcp::new`] composes their routers into the one served router via
//! [`ToolRouter`]'s `Add`. The **core** groups are always served; the **feature** groups
//! (Scratchpads, Todos, Timers, Key-Value, Prompt Templates) are gated by the user's settings —
//! they are registered only when enabled, so a disabled group's tools are neither listed nor
//! callable (Key-Value and Prompt Templates default off). Tool *names* mirror Solo for
//! interop, but the parameter schemas are clean-room — derived from the argument structs in
//! [`crate::args`]. No domain logic lives in a tool: each
//! forwards to the app, which resolves identity, scope, and the trust gate in the core, and the
//! result is returned as structured content.

use std::collections::BTreeSet;
use std::sync::Arc;

use rmcp::handler::server::tool::{ToolCallContext, ToolRouter};
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, Implementation, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{tool_handler, ErrorData, RoleServer, ServerHandler};
use soloist_core::{onboarding_hint, McpFeatureGroup, McpToolGroups};

use crate::client::AppClient;
use crate::suggestions::Suggestions;

/// One tool group's identity: the display label the summary groups it under, the constructor for
/// its sub-router, and whether it is always served (a core group) or gated behind a feature
/// setting. Both [`SoloistMcp::new`] (which composes the served router) and
/// [`SoloistMcp::tools_summary`] (which labels the categories) read this one list, so the served
/// surface and its categorization are single-sourced and cannot drift apart.
struct ToolGroup {
    label: &'static str,
    router: fn() -> ToolRouter<SoloistMcp>,
    gate: GroupGate,
}

/// Whether a [`ToolGroup`] is always served or enabled by one of the user's feature settings.
enum GroupGate {
    Core,
    Feature(McpFeatureGroup),
}

/// The tools surfaced first in `tools/list`, in this order: identity and help, then a small
/// starter pack of the tools an agent reaches for most. rmcp lists tools alphabetically, which
/// buries `whoami` near the end; ordering the essentials first is a cheaper first-run nudge than a
/// heavier tool description. Every name here must be a served tool — a test guards against a typo
/// or a rename leaving a dangling entry.
const FEATURED_TOOLS: &[&str] = &[
    "whoami",
    "help",
    "mcp_tools_summary",
    "list_processes",
    "get_process_status",
    "get_process_output",
    "start_process",
    "restart_process",
    "send_input",
];

/// The Soloist MCP server: a stateless front over the app, holding one client connection.
#[derive(Clone)]
pub struct SoloistMcp {
    pub(crate) client: Arc<AppClient>,
    tool_router: ToolRouter<Self>,
    /// The per-session decaying next-tool suggestions. Shared across handler clones so the decay
    /// counts are one ledger for the connection.
    suggestions: Arc<Suggestions>,
}

impl SoloistMcp {
    /// Builds the handler over a client connection to the app. The core tool groups are always
    /// composed; each feature group is added only when `groups` enables it, so a disabled group is
    /// absent from `list_tools` and uncallable.
    pub fn new(client: Arc<AppClient>, groups: McpToolGroups) -> Self {
        // Compose the served router from the single group list: core groups always, each feature
        // group only when the user's settings enable it, so a disabled group is absent from
        // `list_tools` and uncallable.
        let mut tool_router = ToolRouter::new();
        for group in Self::tool_groups() {
            let served = match group.gate {
                GroupGate::Core => true,
                GroupGate::Feature(feature) => groups.enabled(feature),
            };
            if served {
                tool_router += (group.router)();
            }
        }
        Self {
            client,
            tool_router,
            suggestions: Arc::new(Suggestions::default()),
        }
    }

    /// How many tools this server currently serves — the size of its composed router, which
    /// already reflects the enabled feature groups (a disabled group contributes no routes).
    /// `whoami` reports it so an agent can tell whether its MCP client is showing the full
    /// surface or a stale, smaller one.
    pub(crate) fn served_tool_count(&self) -> usize {
        self.tool_router.list_all().len()
    }

    /// A compact, categorized map of the currently-enabled tools — each tool as its name and a
    /// one-line summary, grouped by category, with no input schemas. The categories come from the
    /// same [`tool_groups`](Self::tool_groups) list [`new`](Self::new) composes the served router
    /// from, filtered to the tools actually served, so a disabled feature group's tools drop out and
    /// the summary can never name a tool the server does not define. This is what an agent reads to
    /// see the whole surface without the weight of `tools/list`.
    pub(crate) fn tools_summary(&self) -> serde_json::Value {
        let served: BTreeSet<String> = self
            .tool_router
            .list_all()
            .into_iter()
            .map(|tool| tool.name.into_owned())
            .collect();

        let categories: Vec<serde_json::Value> = Self::tool_groups()
            .into_iter()
            .filter_map(|group| {
                let tools: Vec<serde_json::Value> = (group.router)()
                    .list_all()
                    .into_iter()
                    .filter(|tool| served.contains(tool.name.as_ref()))
                    .map(|tool| {
                        serde_json::json!({
                            "name": tool.name,
                            "summary": first_sentence(tool.description.as_deref()),
                        })
                    })
                    .collect();
                (!tools.is_empty())
                    .then(|| serde_json::json!({ "category": group.label, "tools": tools }))
            })
            .collect();

        serde_json::json!({ "tool_count": served.len(), "categories": categories })
    }

    /// Appends the contextual next-tool suggestion for `tool` to a successful `result`, until that
    /// suggestion decays for this session. An error result is returned untouched — a nudge only
    /// makes sense after the action succeeded. The suggestion is an extra text content block, so it
    /// never disturbs the tool's structured data.
    fn with_suggestion(&self, tool: &str, mut result: CallToolResult) -> CallToolResult {
        if result.is_error != Some(true) {
            if let Some(hint) = self.suggestions.take(tool) {
                result.content.push(Content::text(format!("Next: {hint}")));
            }
        }
        result
    }

    /// The served tools ordered for discovery: the featured starter pack first, in
    /// [`FEATURED_TOOLS`] order, then every remaining tool in its default (alphabetical) order. A
    /// featured name that is not currently served (a disabled group could hide one, though today
    /// all featured tools are core) is simply skipped.
    fn featured_tool_list(&self) -> Vec<Tool> {
        let mut rest = self.tool_router.list_all();
        let mut featured = Vec::with_capacity(rest.len());
        for name in FEATURED_TOOLS {
            if let Some(index) = rest.iter().position(|tool| tool.name == *name) {
                featured.push(rest.remove(index));
            }
        }
        featured.extend(rest);
        featured
    }

    /// Every tool group in display order — the single list [`new`](Self::new) composes the served
    /// router from and [`tools_summary`](Self::tools_summary) labels its categories with. A group
    /// added here is automatically both served (per its gate) and categorized in the summary, so the
    /// served surface and its categorization can never fall out of sync.
    fn tool_groups() -> [ToolGroup; 14] {
        use GroupGate::{Core, Feature};
        [
            ToolGroup {
                label: "Identity & session",
                router: Self::identity_router,
                gate: Core,
            },
            ToolGroup {
                label: "Projects",
                router: Self::project_router,
                gate: Core,
            },
            ToolGroup {
                label: "Processes",
                router: Self::process_router,
                gate: Core,
            },
            ToolGroup {
                label: "Agents",
                router: Self::agent_router,
                gate: Core,
            },
            ToolGroup {
                label: "Bulk commands",
                router: Self::bulk_router,
                gate: Core,
            },
            ToolGroup {
                label: "Output",
                router: Self::output_router,
                gate: Core,
            },
            ToolGroup {
                label: "Services",
                router: Self::services_router,
                gate: Core,
            },
            ToolGroup {
                label: "Locks (leases)",
                router: Self::lock_router,
                gate: Core,
            },
            ToolGroup {
                label: "Setup & support",
                router: Self::setup_router,
                gate: Core,
            },
            ToolGroup {
                label: "Scratchpads",
                router: Self::scratchpad_router,
                gate: Feature(McpFeatureGroup::Scratchpads),
            },
            ToolGroup {
                label: "Todos",
                router: Self::todo_router,
                gate: Feature(McpFeatureGroup::Todos),
            },
            ToolGroup {
                label: "Timers",
                router: Self::timer_router,
                gate: Feature(McpFeatureGroup::Timers),
            },
            ToolGroup {
                label: "Key-value",
                router: Self::kv_router,
                gate: Feature(McpFeatureGroup::KeyValue),
            },
            ToolGroup {
                label: "Prompt templates",
                router: Self::prompt_template_router,
                gate: Feature(McpFeatureGroup::PromptTemplates),
            },
        ]
    }
}

/// The first sentence of a tool description — the compact one-liner the tools summary shows in place
/// of the full description and input schema. A sentence ends at the first `". "` that begins a new
/// sentence, detected by the next character being uppercase; a period inside an abbreviation
/// (`e.g. `, `vs. `) or a dotted literal (`127.0.0.1. `, `v0.8.2. `) is followed by a lowercase
/// letter or a digit, so it is not mistaken for a boundary. Falls back to the whole trimmed text
/// when there is no such boundary (a single-sentence description).
fn first_sentence(description: Option<&str>) -> String {
    let text = description.unwrap_or("").trim();
    let boundary = text.match_indices(". ").find(|(index, sep)| {
        text[index + sep.len()..]
            .chars()
            .next()
            .is_some_and(char::is_uppercase)
    });
    match boundary {
        Some((index, _)) => text[..=index].to_string(),
        None => text.to_string(),
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for SoloistMcp {
    /// The initialization handshake. Advertises the tool capability and the server's own
    /// identity (the default reports the `rmcp` crate, not this binary), and carries the
    /// first-run path an agent should follow — `whoami`, then `help`, then `help` on a topic.
    /// The path is single-sourced from the core guide, so the handshake and the `help` tool
    /// teach the same start. The `#[tool_handler]` macro still supplies `list_tools`/`call_tool`.
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(onboarding_hint())
    }

    /// Serves `tools/list` with the featured tools first (see [`SoloistMcp::featured_tool_list`])
    /// rather than rmcp's default alphabetical order, so a client that preserves server order shows
    /// `whoami` and `help` up top. Providing this suppresses the `#[tool_handler]` macro's default
    /// `list_tools`. The full surface is unchanged — only its order — and there is no pagination.
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult {
            tools: self.featured_tool_list(),
            meta: None,
            next_cursor: None,
        })
    }

    /// Routes a tool call to the composed router (as the `#[tool_handler]` macro's default would),
    /// then appends a decaying next-tool suggestion when one applies (see
    /// [`SoloistMcp::with_suggestion`]). Providing this suppresses the macro's default `call_tool`.
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let tool = request.name.clone();
        let call = ToolCallContext::new(self, request, context);
        let result = self.tool_router.call(call).await?;
        Ok(self.with_suggestion(&tool, result))
    }
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
